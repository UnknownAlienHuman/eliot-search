//! Real redb file tests with deterministic application checkpoints.
//! No synthetic database, sleeping deadline or public fault switch is used.

use super::*;
use super::super::{ControlInterruption, Unscoped};
use crate::{ControlKey, ControlMutation, ControlRecordClass, ControlValue, ControlWrite};
use search_contracts::{Blake3Digest32, DataRootId, InstallationIncarnationId, OpaqueRef, OwnerEpoch, RequestId};
use search_ports::{PackageOpaque, PortErrorKind, PortRetryability};
use std::cell::Cell;
use std::fs::{self, OpenOptions};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const LIMITS: JournalLimits = JournalLimits::BASELINE;
const OPERATION: MutationId = MutationId([7; 32]);
static NEXT: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Default)]
struct Cancellation(Arc<AtomicBool>);
impl PackageOpaque for Cancellation {
    fn owner_package(&self) -> &'static str { "search-control-redb" }
}
impl CancellationProbe for Cancellation {
    fn is_cancelled(&self) -> bool { self.0.load(Ordering::SeqCst) }
}

fn context(cancel: &Cancellation, milliseconds: u64) -> OperationContext<Cancellation> {
    OperationContext::new(
        RequestId::from_bytes([1; 16]), milliseconds, cancel.clone(),
        OpaqueRef::new("budget:journal-lifecycle").unwrap(),
    ).unwrap()
}

fn identity() -> JournalIdentity {
    JournalIdentity {
        installation_incarnation_id: InstallationIncarnationId::from_bytes([1; 16]),
        data_root_id: DataRootId::from_bytes([2; 16]),
        owner_epoch: OwnerEpoch::new(1).unwrap(),
        path_identity_digest: Blake3Digest32::from_bytes([3; 32]),
        schema_family_digest: Blake3Digest32::from_bytes([4; 32]),
        schema_version: 1,
    }
}

fn successor() -> JournalIdentity {
    JournalIdentity { owner_epoch: OwnerEpoch::new(2).unwrap(), ..identity() }
}

fn request() -> ControlMutation {
    ControlMutation::new(
        MutationId([5; 32]), Blake3Digest32::from_bytes([6; 32]), 0,
        vec![ControlWrite {
            key: ControlKey::new(b"lifecycle".to_vec(), LIMITS).unwrap(),
            value: ControlValue::new(ControlRecordClass::State, b"READY".to_vec(), LIMITS).unwrap(),
        }], vec![],
    )
}

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!(
            "eliot-redb-lifecycle-{}-{stamp}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&root).unwrap();
        Self(root)
    }
    fn path(&self) -> PathBuf { self.0.join("control.redb") }
    fn new_file(&self) -> File {
        OpenOptions::new().read(true).write(true).create_new(true).open(self.path()).unwrap()
    }
    fn file(&self) -> File {
        OpenOptions::new().read(true).write(true).open(self.path()).unwrap()
    }
    fn populated(&self) -> PersistentControlJournal {
        let mut journal = PersistentControlJournal::create(self.new_file(), identity(), LIMITS).unwrap();
        journal.transact(request()).unwrap();
        journal
    }
    fn reopen(&self, expected: JournalIdentity) -> PersistentControlJournal {
        PersistentControlJournal::open(self.file(), expected, LIMITS).unwrap()
    }
}
impl Drop for Scratch {
    fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); }
}

struct At<'a, F> {
    inner: &'a dyn Check,
    point: Point,
    nth: usize,
    hits: Cell<usize>,
    action: F,
}
impl<F: Fn()> Check for At<'_, F> {
    fn check(&self, point: Point) -> Result<(), ControlError> {
        if point == self.point {
            let hits = self.hits.get() + 1;
            self.hits.set(hits);
            if hits == self.nth { (self.action)(); }
        }
        self.inner.check(point)
    }
}

fn interrupted<T>(
    why: ControlInterruption,
    point: Point,
    nth: usize,
    operation: impl FnOnce(&dyn Check) -> Result<T, ControlError>,
) -> ControlCallError {
    let cancel = Cancellation::default();
    let ctx = context(&cancel, 10);
    let start = Instant::now();
    let now = Cell::new(start);
    let budget = Budget::with_clock(&ctx, || now.get());
    let gate = At {
        inner: &budget, point, nth, hits: Cell::new(0),
        action: || match why {
            ControlInterruption::Cancelled => cancel.0.store(true, Ordering::SeqCst),
            ControlInterruption::DeadlineElapsed => now.set(start + Duration::from_millis(10)),
        },
    };
    let raw = operation(&gate).err().expect("checkpoint must prevent returned journal");
    assert!(gate.hits.get() >= nth, "{point:?} was not exercised");
    let error = budget.failure(raw, Some(OPERATION));
    assert_eq!(error.operation_id(), Some(OPERATION));
    assert_eq!(error.interruption(), Some(why));
    error
}

fn interruptions() -> [ControlInterruption; 2] {
    [ControlInterruption::Cancelled, ControlInterruption::DeadlineElapsed]
}

#[test]
fn public_create_open_and_handoff_keep_exact_state_and_do_not_add_operation_rows() {
    let scratch = Scratch::new();
    let cancel = Cancellation::default();
    let ctx = context(&cancel, 60_000);
    let mut journal = PersistentControlJournal::create_with_context(
        scratch.new_file(), identity(), LIMITS, OPERATION, &ctx,
    ).unwrap();
    assert_eq!(journal.verify().unwrap().generation, 0);
    assert_eq!(journal.committed_writes(), 1);
    let receipt = journal.transact(request()).unwrap();
    let before = journal.verify().unwrap();
    drop(journal);
    let journal = PersistentControlJournal::open_with_context(
        scratch.file(), identity(), LIMITS, OPERATION, &ctx,
    ).unwrap();
    assert_eq!(journal.verify().unwrap(), before);
    assert_eq!(journal.committed_writes(), 0);
    let mut journal = journal.advance_owner_with_context(successor(), OPERATION, &ctx).unwrap();
    let after = journal.verify().unwrap();
    assert_eq!(after.records, before.records);
    assert_eq!(after.generation, before.generation);
    assert_eq!(after.identity, successor());
    let replay = journal.transact(request()).unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.operation_id, receipt.operation_id);
    assert_eq!(journal.committed_writes(), 1);
}

#[test]
fn public_pre_cancelled_calls_preserve_files_and_return_original_operation_identity() {
    let scratch = Scratch::new();
    let cancel = Cancellation::default();
    cancel.0.store(true, Ordering::SeqCst);
    let ctx = context(&cancel, 60_000);
    let error = PersistentControlJournal::create_with_context(
        scratch.new_file(), identity(), LIMITS, OPERATION, &ctx,
    ).unwrap_err();
    assert_eq!(error.operation_id(), Some(OPERATION));
    assert_eq!(error.kind(), PortErrorKind::CancelledBeforeSideEffect);
    assert_eq!(error.retryability(), PortRetryability::SameIdentity);
    assert_eq!(fs::metadata(scratch.path()).unwrap().len(), 0);
    drop(PersistentControlJournal::create(scratch.file(), identity(), LIMITS).unwrap());
    let before = fs::read(scratch.path()).unwrap();
    let error = PersistentControlJournal::open_with_context(
        scratch.file(), identity(), LIMITS, OPERATION, &ctx,
    ).unwrap_err();
    assert_eq!(error.kind(), PortErrorKind::CancelledBeforeSideEffect);
    assert_eq!(error.operation_id(), Some(OPERATION));
    assert_eq!(fs::read(scratch.path()).unwrap(), before);
    let journal = scratch.reopen(identity());
    let error = journal.advance_owner_with_context(successor(), OPERATION, &ctx).unwrap_err();
    assert_eq!(error.kind(), PortErrorKind::CancelledBeforeSideEffect);
    assert_eq!(error.operation_id(), Some(OPERATION));
    assert_eq!(scratch.reopen(identity()).identity(), identity());
}

#[test]
fn creation_interruption_before_native_dispatch_keeps_the_empty_file() {
    for why in interruptions() {
        for point in [Point::Start, Point::Validated, Point::BeforeOpen] {
            let scratch = Scratch::new();
            let error = interrupted(why, point, 1, |check| {
                PersistentControlJournal::create_checked(scratch.new_file(), identity(), LIMITS, check)
            });
            assert_ne!(error.kind(), PortErrorKind::OutcomeUnknown);
            assert_eq!(error.retryability(), PortRetryability::SameIdentity);
            assert_eq!(fs::metadata(scratch.path()).unwrap().len(), 0);
        }
    }
}

#[test]
fn creation_interruption_after_native_dispatch_never_adopts_partial_tables() {
    for why in interruptions() {
        for (point, nth) in [(Point::AfterOpen, 1), (Point::StageRecord, 2), (Point::BeforeCommit, 1)] {
            let scratch = Scratch::new();
            let error = interrupted(why, point, nth, |check| {
                PersistentControlJournal::create_checked(scratch.new_file(), identity(), LIMITS, check)
            });
            assert_eq!(error.kind(), PortErrorKind::OutcomeUnknown);
            assert_eq!(error.retryability(), PortRetryability::AfterReadback);
            assert!(fs::metadata(scratch.path()).unwrap().len() > 0);
            assert!(matches!(
                PersistentControlJournal::open(scratch.file(), identity(), LIMITS),
                Err(ControlError::SchemaMismatch)
            ));
            assert!(matches!(
                PersistentControlJournal::create(scratch.file(), identity(), LIMITS),
                Err(ControlError::StoreCorrupt)
            ));
        }
    }
}

#[test]
fn completed_initialization_with_lost_acknowledgement_reopens_without_recreation() {
    for why in interruptions() {
        for point in [Point::AfterCommit, Point::ReadHeader, Point::LifecycleComplete] {
            let scratch = Scratch::new();
            let error = interrupted(why, point, 1, |check| {
                PersistentControlJournal::create_checked(scratch.new_file(), identity(), LIMITS, check)
            });
            assert_eq!(error.kind(), PortErrorKind::OutcomeUnknown);
            let journal = scratch.reopen(identity());
            let snapshot = journal.verify().unwrap();
            assert!(snapshot.records.is_empty());
            assert_eq!(snapshot.generation, 0);
            assert_eq!(journal.committed_writes(), 0);
        }
    }
}

#[test]
fn create_never_replaces_an_existing_populated_journal() {
    let scratch = Scratch::new();
    let before = scratch.populated().verify().unwrap();
    let bytes = fs::read(scratch.path()).unwrap();
    assert!(matches!(
        PersistentControlJournal::create_checked(scratch.file(), identity(), LIMITS, &Unscoped),
        Err(ControlError::StoreCorrupt)
    ));
    assert_eq!(fs::read(scratch.path()).unwrap(), bytes);
    assert_eq!(scratch.reopen(identity()).verify().unwrap(), before);
}

#[test]
fn empty_existing_file_is_never_initialized_by_context_open() {
    let scratch = Scratch::new();
    let cancel = Cancellation::default();
    let error = PersistentControlJournal::open_with_context(
        scratch.new_file(), identity(), LIMITS, OPERATION, &context(&cancel, 60_000),
    ).unwrap_err();
    assert_eq!(error.control_error(), ControlError::StoreCorrupt);
    assert_eq!(fs::metadata(scratch.path()).unwrap().len(), 0);
}

#[test]
fn interrupted_open_keeps_all_records_and_requires_readback_after_dispatch() {
    for why in interruptions() {
        for point in [Point::BeforeOpen, Point::AfterOpen, Point::ReadRecord, Point::ReadOperation, Point::LifecycleComplete] {
            let scratch = Scratch::new();
            let before = scratch.populated().verify().unwrap();
            let error = interrupted(why, point, 1, |check| {
                PersistentControlJournal::open_checked(scratch.file(), identity(), LIMITS, check)
            });
            assert_eq!(error.kind() == PortErrorKind::OutcomeUnknown, point != Point::BeforeOpen);
            assert_eq!(scratch.reopen(identity()).verify().unwrap(), before);
        }
    }
}

#[test]
fn context_open_refuses_wrong_identity_and_corrupt_schema_without_rebinding() {
    let scratch = Scratch::new();
    let before = scratch.populated().verify().unwrap();
    let cancel = Cancellation::default();
    for expected in [
        successor(),
        JournalIdentity { data_root_id: DataRootId::from_bytes([8; 16]), ..identity() },
        JournalIdentity { schema_family_digest: Blake3Digest32::from_bytes([8; 32]), ..identity() },
    ] {
        let error = PersistentControlJournal::open_with_context(
            scratch.file(), expected, LIMITS, OPERATION, &context(&cancel, 60_000),
        ).unwrap_err();
        assert_eq!(error.kind(), PortErrorKind::Quarantined);
        assert!(matches!(error.control_error(), ControlError::IdentityMismatch | ControlError::SchemaMismatch));
        assert_eq!(error.interruption(), None);
        assert_eq!(scratch.reopen(identity()).verify().unwrap(), before);
    }
    let journal = scratch.reopen(identity());
    let write = journal.database.begin_write().unwrap();
    assert!(write.delete_table(OPERATIONS).unwrap());
    write.commit().unwrap();
    drop(journal);
    assert!(matches!(
        PersistentControlJournal::open(scratch.file(), identity(), LIMITS),
        Err(ControlError::SchemaMismatch)
    ));
}

#[test]
fn second_context_open_cannot_replace_the_live_guard() {
    let scratch = Scratch::new();
    let journal = scratch.populated();
    let cancel = Cancellation::default();
    let error = PersistentControlJournal::open_with_context(
        scratch.file(), identity(), LIMITS, OPERATION, &context(&cancel, 60_000),
    ).unwrap_err();
    assert_eq!(error.control_error(), ControlError::StoreUnavailable);
    assert_eq!(journal.verify().unwrap().generation, 1);
}

#[test]
fn handoff_cancelled_before_write_dispatch_preserves_prior_epoch() {
    for why in interruptions() {
        for point in [Point::Start, Point::ReadRecord, Point::Validated, Point::BeforeWrite] {
            let scratch = Scratch::new();
            let journal = scratch.populated();
            let before = journal.verify().unwrap();
            let error = interrupted(why, point, 1, |check| journal.advance_owner_checked(successor(), check));
            assert_ne!(error.kind(), PortErrorKind::OutcomeUnknown);
            assert_eq!(scratch.reopen(identity()).verify().unwrap(), before);
        }
    }
}

#[test]
fn dispatched_handoff_abort_is_unknown_until_prior_header_is_reopened() {
    for why in interruptions() {
        for point in [Point::StageRecord, Point::BeforeCommit] {
            let scratch = Scratch::new();
            let journal = scratch.populated();
            let before = journal.verify().unwrap();
            let error = interrupted(why, point, 1, |check| journal.advance_owner_checked(successor(), check));
            assert_eq!(error.kind(), PortErrorKind::OutcomeUnknown);
            assert_eq!(error.retryability(), PortRetryability::AfterReadback);
            assert!(matches!(
                PersistentControlJournal::open(scratch.file(), successor(), LIMITS),
                Err(ControlError::IdentityMismatch)
            ));
            assert_eq!(scratch.reopen(identity()).verify().unwrap(), before);
        }
    }
}

#[test]
fn committed_handoff_lost_ack_keeps_new_epoch_and_old_mutation_replay() {
    for why in interruptions() {
        for (point, nth) in [(Point::AfterCommit, 1), (Point::ReadRecord, 2), (Point::LifecycleComplete, 1)] {
            let scratch = Scratch::new();
            let journal = scratch.populated();
            let before = journal.verify().unwrap();
            let error = interrupted(why, point, nth, |check| journal.advance_owner_checked(successor(), check));
            assert_eq!(error.kind(), PortErrorKind::OutcomeUnknown);
            assert!(matches!(
                PersistentControlJournal::open(scratch.file(), identity(), LIMITS),
                Err(ControlError::IdentityMismatch)
            ));
            let mut journal = scratch.reopen(successor());
            let after = journal.verify().unwrap();
            assert_eq!(after.records, before.records);
            assert_eq!(after.generation, before.generation);
            assert!(journal.transact(request()).unwrap().replayed);
            assert_eq!(journal.committed_writes(), 0);
        }
    }
}

#[test]
fn same_epoch_handoff_is_verified_noop_and_never_uses_write_checkpoint() {
    let scratch = Scratch::new();
    let journal = scratch.populated();
    let commits = journal.committed_writes();
    let gate = At {
        inner: &Unscoped, point: Point::BeforeWrite, nth: 1, hits: Cell::new(0),
        action: || panic!("already-current owner must not dispatch a write"),
    };
    let journal = journal.advance_owner_checked(identity(), &gate).unwrap();
    assert_eq!(journal.committed_writes(), commits);
    assert_eq!(gate.hits.get(), 0);
    let error = interrupted(ControlInterruption::Cancelled, Point::LifecycleComplete, 1,
        |check| journal.advance_owner_checked(identity(), check));
    assert_ne!(error.kind(), PortErrorKind::OutcomeUnknown);
    assert_eq!(scratch.reopen(identity()).verify().unwrap().generation, 1);
}

#[test]
fn handoff_rejects_skipped_epoch_or_changed_immutable_identity() {
    for next in [
        JournalIdentity { owner_epoch: OwnerEpoch::new(3).unwrap(), ..identity() },
        JournalIdentity { data_root_id: DataRootId::from_bytes([8; 16]), ..successor() },
    ] {
        let scratch = Scratch::new();
        let journal = scratch.populated();
        assert!(matches!(journal.advance_owner_checked(next, &Unscoped), Err(ControlError::IdentityMismatch)));
        assert_eq!(scratch.reopen(identity()).identity(), identity());
    }
}

#[test]
fn deadline_is_not_reset_between_preflight_open_and_ready_verification() {
    let scratch = Scratch::new();
    let cancel = Cancellation::default();
    let ctx = context(&cancel, 10);
    let start = Instant::now();
    let now = Cell::new(start);
    let budget = Budget::with_clock(&ctx, || now.get());
    struct Advance<'a> { inner: &'a dyn Check, now: &'a Cell<Instant> }
    impl Check for Advance<'_> {
        fn check(&self, point: Point) -> Result<(), ControlError> {
            if matches!(point, Point::Validated | Point::BeforeOpen | Point::AfterOpen) {
                self.now.set(self.now.get() + Duration::from_millis(4));
            }
            self.inner.check(point)
        }
    }
    let gate = Advance { inner: &budget, now: &now };
    let raw = PersistentControlJournal::create_checked(scratch.new_file(), identity(), LIMITS, &gate).unwrap_err();
    let error = budget.failure(raw, Some(OPERATION));
    assert_eq!(now.get().duration_since(start), Duration::from_millis(12));
    assert_eq!(error.interruption(), Some(ControlInterruption::DeadlineElapsed));
    assert_eq!(error.kind(), PortErrorKind::OutcomeUnknown);
    assert!(fs::metadata(scratch.path()).unwrap().len() > 0);
}

#[test]
fn native_open_error_mapping_distinguishes_corruption_from_uncertain_io() {
    assert_eq!(open_error(DatabaseError::DatabaseAlreadyOpen), ControlError::StoreUnavailable);
    assert_eq!(open_error(DatabaseError::UpgradeRequired(0)), ControlError::MigrationUnverified);
    assert_eq!(open_error(DatabaseError::Storage(redb::StorageError::Corrupted("bad page".to_owned()))), ControlError::StoreCorrupt);
    assert_eq!(open_error(DatabaseError::Storage(redb::StorageError::Io(std::io::Error::other("private-path")))), ControlError::CommitOutcomeUnknown);
    assert_eq!(open_error(DatabaseError::Storage(redb::StorageError::PreviousIo)), ControlError::CommitOutcomeUnknown);
    assert_eq!(open_error(DatabaseError::RepairAborted), ControlError::CommitOutcomeUnknown);
}


#[test]
fn unresolved_mutation_cannot_be_cleared_by_owner_handoff() {
    let scratch = Scratch::new();
    let mut journal = PersistentControlJournal::create(scratch.new_file(), identity(), LIMITS).unwrap();
    assert_eq!(journal.transact_inner(request(), super::super::Boundary::LostAcknowledgement),
        Err(ControlError::CommitOutcomeUnknown));
    assert!(journal.requires_recovery());
    assert!(matches!(journal.advance_owner_checked(successor(), &Unscoped), Err(ControlError::StoreQuarantined)));
    let mut journal = scratch.reopen(identity());
    assert_eq!(journal.identity(), identity());
    assert!(matches!(journal.recover_transaction(&request()).unwrap(), crate::CommitRecoveryDecision::Committed(_)));
    assert_eq!(journal.advance_owner_checked(successor(), &Unscoped).unwrap().identity(), successor());
}

#[test]
fn unsupported_migration_is_a_quarantine_blocker_not_an_internal_error() {
    let cancel = Cancellation::default();
    let ctx = context(&cancel, 10_000);
    let budget = Budget::new(&ctx);
    let error = budget.failure(ControlError::MigrationUnverified, Some(OPERATION));
    assert_eq!(error.kind(), PortErrorKind::Quarantined);
    assert_eq!(error.retryability(), PortRetryability::AfterReadback);
    assert_eq!(error.interruption(), None);
}

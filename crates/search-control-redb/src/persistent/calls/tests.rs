//! Real-file journal tests with deterministic cancellation/fake-clock seams.
//! These tests exercise application checkpoints, not physical media faults.

use super::*;
use super::super::operation::{Budget, Check, Point};
use super::super::{ControlInterruption, RECORDS, map_storage_error, map_table_error};
use crate::{ControlKey, ControlRecordClass, ControlValue, ControlWrite, JournalIdentity, JournalLimits, MutationId};
use search_contracts::{Blake3Digest32, DataRootId, InstallationIncarnationId, OpaqueRef, OwnerEpoch, RequestId};
use search_ports::{PackageOpaque, PortErrorKind, PortRetryability};
use std::cell::Cell;
use std::fs::{self, OpenOptions};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const LIMITS: JournalLimits = JournalLimits::BASELINE;
static NEXT: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Default)]
struct Cancellation(Arc<AtomicBool>);
impl PackageOpaque for Cancellation {
    fn owner_package(&self) -> &'static str { "search-control-redb" }
}
impl CancellationProbe for Cancellation {
    fn is_cancelled(&self) -> bool { self.0.load(Ordering::SeqCst) }
}
impl Cancellation {
    fn set(&self, value: bool) { self.0.store(value, Ordering::SeqCst); }
}

fn context(cancel: &Cancellation, milliseconds: u64) -> OperationContext<Cancellation> {
    OperationContext::new(
        RequestId::from_bytes([1; 16]), milliseconds, cancel.clone(),
        OpaqueRef::new("budget:journal-test").unwrap(),
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

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!(
            "eliot-redb-context-{}-{stamp}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&root).unwrap();
        Self(root)
    }
    fn path(&self) -> PathBuf { self.0.join("control.redb") }
    fn create(&self) -> PersistentControlJournal {
        let file = OpenOptions::new().read(true).write(true).create_new(true).open(self.path()).unwrap();
        PersistentControlJournal::create(file, identity(), LIMITS).unwrap()
    }
    fn open(&self) -> PersistentControlJournal {
        let file = OpenOptions::new().read(true).write(true).open(self.path()).unwrap();
        PersistentControlJournal::open(file, identity(), LIMITS).unwrap()
    }
}
impl Drop for Scratch {
    fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); }
}

fn key(bytes: &[u8]) -> ControlKey { ControlKey::new(bytes.to_vec(), LIMITS).unwrap() }
fn change(name: &[u8], bytes: &[u8]) -> ControlWrite {
    ControlWrite { key: key(name), value: ControlValue::new(ControlRecordClass::State, bytes.to_vec(), LIMITS).unwrap() }
}
fn request(id: u8, generation: u64) -> ControlMutation {
    ControlMutation::new(
        MutationId([id; 32]), Blake3Digest32::from_bytes([9; 32]), generation,
        vec![change(b"a", b"READY"), change(b"b", b"DIRECT")], vec![key(b"gone")],
    )
}

// Supplies a deterministic side-effect-free probe at one real algorithm point.
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
            let next = self.hits.get() + 1;
            self.hits.set(next);
            if next == self.nth { (self.action)(); }
        }
        self.inner.check(point)
    }
}

fn trigger(why: ControlInterruption, cancel: &Cancellation, now: &Cell<Instant>, start: Instant) {
    match why {
        ControlInterruption::Cancelled => cancel.set(true),
        ControlInterruption::DeadlineElapsed => now.set(start + Duration::from_millis(10)),
    }
}

#[test]
fn pre_cancelled_public_calls_touch_no_disk_or_committed_state() {
    let scratch = Scratch::new();
    let mut journal = scratch.create();
    let before = fs::read(scratch.path()).unwrap();
    let cancel = Cancellation::default();
    cancel.set(true);
    let ctx = context(&cancel, 10_000);
    for error in [
        journal.read_snapshot_with_context(&ctx).unwrap_err(),
        journal.verify_with_context(&ctx).unwrap_err(),
    ] {
        assert_eq!(error.kind(), PortErrorKind::CancelledBeforeSideEffect);
        assert_eq!(error.retryability(), PortRetryability::SameRequest);
        assert_eq!(error.operation_id(), None);
    }
    let command = request(1, 0);
    let error = journal.transact_with_context(command.clone(), &ctx).unwrap_err();
    assert_eq!(error.control_error(), ControlError::ReadCancelled);
    assert_eq!(error.interruption(), Some(ControlInterruption::Cancelled));
    assert_eq!(error.operation_id(), Some(command.id()));
    assert_eq!(error.retryability(), PortRetryability::SameIdentity);
    assert!(!journal.requires_recovery());
    assert!(!journal.quarantined);
    assert_eq!(fs::read(scratch.path()).unwrap(), before);
    assert_eq!(journal.committed_writes(), 1);
}

#[test]
fn interruptions_preserve_dispatch_classification_and_require_exact_absence_readback() {
    for why in [ControlInterruption::Cancelled, ControlInterruption::DeadlineElapsed] {
        for (point, nth) in [
            (Point::Start, 1), (Point::Validated, 1), (Point::ReadHeader, 1),
            (Point::PlanRecord, 2), (Point::BeforeWrite, 1), (Point::BeforeWrite, 2),
            (Point::StageRecord, 3), (Point::BeforeCommit, 1),
        ] {
            let scratch = Scratch::new();
            let mut journal = scratch.create();
            journal.transact(ControlMutation::new(
                MutationId([1; 32]), Blake3Digest32::from_bytes([9; 32]), 0,
                vec![change(b"a", b"PRIOR"), change(b"gone", b"KEEP")], vec![],
            )).unwrap();
            let before = journal.verify().unwrap();
            let cancel = Cancellation::default();
            let ctx = context(&cancel, 10);
            let start = Instant::now();
            let now = Cell::new(start);
            let budget = Budget::with_clock(&ctx, || now.get());
            let gate = At {
                inner: &budget, point, nth, hits: Cell::new(0),
                action: || trigger(why, &cancel, &now, start),
            };
            let command = request(2, 1);
            let raw = journal.transact_checked(command.clone(), Boundary::Normal, &gate).unwrap_err();
            let error = budget.failure(raw, Some(command.id()));
            assert_eq!(error.interruption(), Some(why), "{point:?}");
            let dispatched = (point == Point::BeforeWrite && nth == 2)
                || matches!(point, Point::StageRecord | Point::BeforeCommit);
            if dispatched {
                assert_eq!(error.kind(), PortErrorKind::OutcomeUnknown);
                assert_eq!(error.retryability(), PortRetryability::AfterReadback);
                assert!(journal.requires_recovery(), "{point:?}");
                assert_eq!(journal.read_snapshot(), Err(ControlError::StoreQuarantined));
                cancel.set(false);
                assert!(matches!(
                    journal.recover_transaction_with_context(&command, &context(&cancel, 10_000)).unwrap(),
                    CommitRecoveryDecision::NotCommittedRetrySameOperation
                ));
            } else {
                assert_eq!(error.retryability(), PortRetryability::SameIdentity);
                assert_ne!(error.kind(), PortErrorKind::OutcomeUnknown);
            }
            assert!(!journal.requires_recovery(), "{point:?}");
            assert!(!journal.quarantined, "{point:?}");
            assert_eq!(journal.verify().unwrap(), before);
            drop(journal);
            let mut journal = scratch.open();
            assert_eq!(journal.verify().unwrap(), before);
            assert!(!journal.transact(command).unwrap().replayed);
            assert_eq!(journal.verify().unwrap().generation, 2);
        }
    }
}

#[test]
fn after_commit_interruptions_keep_the_exact_pending_request_until_recovery() {
    for why in [ControlInterruption::Cancelled, ControlInterruption::DeadlineElapsed] {
        for (point, nth) in [(Point::AfterCommit, 1), (Point::Readback, 2), (Point::MutationComplete, 1)] {
            let scratch = Scratch::new();
            let mut journal = scratch.create();
            let cancel = Cancellation::default();
            let ctx = context(&cancel, 10);
            let start = Instant::now();
            let now = Cell::new(start);
            let budget = Budget::with_clock(&ctx, || now.get());
            let gate = At {
                inner: &budget, point, nth, hits: Cell::new(0),
                action: || trigger(why, &cancel, &now, start),
            };
            let command = request(1, 0);
            let raw = journal.transact_checked(command.clone(), Boundary::Normal, &gate).unwrap_err();
            let error = budget.failure(raw, Some(command.id()));
            assert_eq!(error.control_error(), ControlError::CommitOutcomeUnknown);
            assert_eq!(error.kind(), PortErrorKind::OutcomeUnknown);
            assert_eq!(error.interruption(), Some(why));
            assert_eq!(error.retryability(), PortRetryability::AfterReadback);
            assert!(journal.requires_recovery());
            assert!(!journal.quarantined);
            assert_eq!(journal.read_snapshot(), Err(ControlError::StoreQuarantined));
            assert_eq!(journal.transact(request(2, 1)), Err(ControlError::StoreQuarantined));
            cancel.set(false);
            let fresh = context(&cancel, 10_000);
            assert!(matches!(
                journal.recover_transaction_with_context(&command, &fresh).unwrap(),
                CommitRecoveryDecision::Committed(_)
            ));
            assert!(!journal.requires_recovery());
            let snapshot = journal.verify().unwrap();
            assert_eq!(snapshot.generation, 1);
            assert_eq!(snapshot.records.len(), 2);
            assert!(journal.transact_with_context(command, &fresh).unwrap().replayed);
            assert_eq!(journal.verify().unwrap(), snapshot);
        }
    }
}

#[test]
fn public_recovery_cancellation_returns_unknown_and_never_quarantines() {
    let scratch = Scratch::new();
    let mut journal = scratch.create();
    let command = request(1, 0);
    assert_eq!(journal.transact_inner(command.clone(), Boundary::LostAcknowledgement), Err(ControlError::CommitOutcomeUnknown));
    let pending = journal.pending;
    let cancel = Cancellation::default();
    cancel.set(true);
    let error = journal.recover_transaction_with_context(&command, &context(&cancel, 10_000)).unwrap_err();
    assert_eq!(error.kind(), PortErrorKind::OutcomeUnknown);
    assert_eq!(error.retryability(), PortRetryability::AfterReadback);
    assert_eq!(error.interruption(), Some(ControlInterruption::Cancelled));
    assert_eq!(journal.pending, pending);
    assert!(!journal.quarantined);
    cancel.set(false);
    assert!(matches!(journal.recover_transaction(&command).unwrap(), CommitRecoveryDecision::Committed(_)));
}

#[test]
fn interrupted_recovery_scans_do_not_clear_pending_or_forge_corruption() {
    for why in [ControlInterruption::Cancelled, ControlInterruption::DeadlineElapsed] {
        for point in [Point::ReadRecord, Point::ReadOperation, Point::Readback, Point::RecoveryComplete] {
            let scratch = Scratch::new();
            let mut journal = scratch.create();
            let command = request(1, 0);
            assert_eq!(journal.transact_inner(command.clone(), Boundary::LostAcknowledgement), Err(ControlError::CommitOutcomeUnknown));
            let pending = journal.pending;
            let cancel = Cancellation::default();
            let ctx = context(&cancel, 10);
            let start = Instant::now();
            let now = Cell::new(start);
            let budget = Budget::with_clock(&ctx, || now.get());
            let gate = At { inner: &budget, point, nth: 1, hits: Cell::new(0), action: || trigger(why, &cancel, &now, start) };
            let raw = journal.recover_transaction_checked(&command, &gate).unwrap_err();
            assert!(matches!(raw, ControlError::ReadCancelled | ControlError::BudgetExceeded));
            assert_eq!(journal.pending, pending);
            assert!(!journal.quarantined);
            assert!(matches!(journal.recover_transaction(&command).unwrap(), CommitRecoveryDecision::Committed(_)));
        }
    }
}

#[test]
fn transient_recovery_failure_is_not_a_structural_corruption_verdict() {
    struct Transient;
    impl Check for Transient {
        fn check(&self, point: Point) -> Result<(), ControlError> {
            if point == Point::ReadOperation { Err(ControlError::StoreUnavailable) } else { Ok(()) }
        }
    }
    let scratch = Scratch::new();
    let mut journal = scratch.create();
    let command = request(1, 0);
    assert_eq!(journal.transact_inner(command.clone(), Boundary::LostAcknowledgement), Err(ControlError::CommitOutcomeUnknown));
    let pending = journal.pending;
    assert_eq!(journal.recover_transaction_checked(&command, &Transient), Err(ControlError::StoreUnavailable));
    assert_eq!(journal.pending, pending);
    assert!(!journal.quarantined);
    assert!(matches!(journal.recover_transaction(&command).unwrap(), CommitRecoveryDecision::Committed(_)));
}

#[test]
fn actual_corruption_during_context_recovery_still_quarantines() {
    let scratch = Scratch::new();
    let mut journal = scratch.create();
    let command = request(1, 0);
    assert_eq!(journal.transact_inner(command.clone(), Boundary::LostAcknowledgement), Err(ControlError::CommitOutcomeUnknown));
    let write = journal.database.begin_write().unwrap();
    {
        let mut table = write.open_table(RECORDS).unwrap();
        table.insert(b"a".as_slice(), b"\xffBROKE".as_slice()).unwrap();
    }
    write.commit().unwrap();
    let cancel = Cancellation::default();
    assert!(matches!(journal.recover_transaction_with_context(&command, &context(&cancel, 10_000)).unwrap(), CommitRecoveryDecision::PartialOrCorruptQuarantine));
    assert!(journal.quarantined);
    assert!(journal.requires_recovery());
}

#[test]
fn cancelled_snapshot_or_full_verification_returns_no_partial_success() {
    for (point, verify) in [(Point::ReadRecord, false), (Point::ReadOperation, true), (Point::ReadComplete, true)] {
        let scratch = Scratch::new();
        let mut journal = scratch.create();
        journal.transact(request(1, 0)).unwrap();
        let before = fs::read(scratch.path()).unwrap();
        let cancel = Cancellation::default();
        let ctx = context(&cancel, 10_000);
        let budget = Budget::new(&ctx);
        let gate = At { inner: &budget, point, nth: 1, hits: Cell::new(0), action: || cancel.set(true) };
        let result = if verify { journal.verify_checked(&gate) } else { journal.read_snapshot_checked(&gate) };
        assert_eq!(result, Err(ControlError::ReadCancelled));
        assert!(!journal.quarantined);
        assert!(!journal.requires_recovery());
        assert_eq!(fs::read(scratch.path()).unwrap(), before);
    }
}

#[test]
fn empty_scans_still_observe_deadlines_before_success() {
    for verify in [false, true] {
        let scratch = Scratch::new();
        let journal = scratch.create();
        let cancel = Cancellation::default();
        let ctx = context(&cancel, 10);
        let start = Instant::now();
        let now = Cell::new(start);
        let budget = Budget::with_clock(&ctx, || now.get());
        let gate = At { inner: &budget, point: Point::ReadComplete, nth: 1, hits: Cell::new(0), action: || now.set(start + Duration::from_millis(10)) };
        let result = if verify { journal.verify_checked(&gate) } else { journal.read_snapshot_checked(&gate) };
        assert_eq!(result, Err(ControlError::BudgetExceeded));
        assert_eq!(budget.failure(result.unwrap_err(), None).kind(), PortErrorKind::DeadlineDuringOperation);
    }
}

#[test]
fn historical_replay_cancellation_does_not_restore_old_values_or_create_pending_write() {
    let scratch = Scratch::new();
    let mut journal = scratch.create();
    let first = request(1, 0);
    journal.transact(first.clone()).unwrap();
    let next = ControlMutation::new(MutationId([2; 32]), Blake3Digest32::from_bytes([9; 32]), 1, vec![change(b"a", b"LATER")], vec![]);
    journal.transact(next).unwrap();
    let before = journal.verify().unwrap();
    let cancel = Cancellation::default();
    let ctx = context(&cancel, 10_000);
    let budget = Budget::new(&ctx);
    let gate = At { inner: &budget, point: Point::ReplayComplete, nth: 1, hits: Cell::new(0), action: || cancel.set(true) };
    assert_eq!(journal.transact_checked(first.clone(), Boundary::Normal, &gate), Err(ControlError::ReadCancelled));
    assert!(!journal.requires_recovery());
    assert_eq!(journal.verify().unwrap(), before);
    assert!(journal.transact(first).unwrap().replayed);
    assert_eq!(journal.verify().unwrap(), before);
}

#[test]
fn deadline_is_one_budget_across_planning_and_staging_not_per_phase() {
    let scratch = Scratch::new();
    let mut journal = scratch.create();
    let cancel = Cancellation::default();
    let ctx = context(&cancel, 5);
    let start = Instant::now();
    let now = Cell::new(start);
    let budget = Budget::with_clock(&ctx, || now.get());
    let stage = At { inner: &budget, point: Point::StageRecord, nth: 1, hits: Cell::new(0), action: || now.set(now.get() + Duration::from_millis(3)) };
    let plan = At { inner: &stage, point: Point::PlanRecord, nth: 1, hits: Cell::new(0), action: || now.set(now.get() + Duration::from_millis(3)) };
    let command = request(1, 0);
    assert_eq!(journal.transact_checked(command.clone(), Boundary::Normal, &plan), Err(ControlError::CommitOutcomeUnknown));
    assert!(stage.hits.get() > 0);
    assert!(plan.hits.get() > 0);
    assert!(journal.requires_recovery());
    assert!(matches!(journal.recover_transaction(&command).unwrap(), CommitRecoveryDecision::NotCommittedRetrySameOperation));
    assert_eq!(journal.verify().unwrap().generation, 0);
    assert!(!journal.requires_recovery());
}

#[test]
fn budget_stop_is_latched_and_exact_deadline_is_expired() {
    let cancel = Cancellation::default();
    let ctx = context(&cancel, 10);
    let start = Instant::now();
    let now = Cell::new(start);
    let budget = Budget::with_clock(&ctx, || now.get());
    now.set(start + Duration::from_millis(9));
    assert_eq!(budget.check(Point::ReadRecord), Ok(()));
    now.set(start + Duration::from_millis(10));
    assert_eq!(budget.check(Point::ReadRecord), Err(ControlError::BudgetExceeded));
    now.set(start);
    assert_eq!(budget.check(Point::ReadComplete), Err(ControlError::BudgetExceeded));
    let fresh = Budget::with_clock(&ctx, || now.get());
    assert_eq!(fresh.check(Point::Start), Ok(()));
}

#[test]
fn resource_limit_is_not_mislabeled_as_deadline_expiration() {
    let cancel = Cancellation::default();
    let ctx = context(&cancel, 10_000);
    let budget = Budget::new(&ctx);
    let error = budget.failure(ControlError::BudgetExceeded, Some(MutationId([1; 32])));
    assert_eq!(error.kind(), PortErrorKind::ResourceExhausted);
    assert_eq!(error.interruption(), None);
    assert_eq!(error.retryability(), PortRetryability::Never);
    assert_eq!(budget.failure(ControlError::StoreUnavailable, Some(MutationId([1; 32]))).for_recovery().retryability(), PortRetryability::AfterReadback);
}

#[test]
fn typed_vendor_errors_preserve_unavailable_versus_corrupt_without_leaking_text() {
    let error = map_table_error(redb::TableError::Storage(redb::StorageError::Io(std::io::Error::other("private-path-sentinel"))));
    assert_eq!(error, ControlError::StoreUnavailable);
    assert_eq!(map_storage_error(redb::StorageError::PreviousIo), ControlError::StoreUnavailable);
    assert_eq!(map_storage_error(redb::StorageError::Corrupted("private bytes".to_owned())), ControlError::StoreCorrupt);
    assert_eq!(map_table_error(redb::TableError::TableDoesNotExist("private table".to_owned())), ControlError::SchemaMismatch);
    let cancel = Cancellation::default();
    let ctx = context(&cancel, 10_000);
    let budget = Budget::new(&ctx);
    let failure = budget.failure(error, Some(MutationId([123; 32])));
    assert!(!format!("{failure:?}").contains("123"));
    assert!(!format!("{failure}").contains("private"));
}

#[test]
fn public_context_methods_agree_with_existing_exact_disk_semantics() {
    let scratch = Scratch::new();
    let mut journal = scratch.create();
    let cancel = Cancellation::default();
    let ctx = context(&cancel, 10_000);
    let command = request(1, 0);
    let committed = journal.transact_with_context(command.clone(), &ctx).unwrap();
    assert_eq!(committed.after_generation, 1);
    assert_eq!(journal.read_snapshot_with_context(&ctx).unwrap(), journal.verify_with_context(&ctx).unwrap());
    assert!(journal.transact_with_context(command.clone(), &ctx).unwrap().replayed);
    assert!(matches!(journal.recover_transaction_with_context(&command, &ctx).unwrap(), CommitRecoveryDecision::Committed(_)));
    drop(journal);
    assert_eq!(scratch.open().verify().unwrap().generation, 1);
}

#[test]
fn wrong_recovery_request_cannot_clear_an_existing_pending_fence() {
    let scratch = Scratch::new();
    let mut journal = scratch.create();
    let command = request(1, 0);
    assert_eq!(journal.transact_inner(command.clone(), Boundary::LostAcknowledgement), Err(ControlError::CommitOutcomeUnknown));
    let pending = journal.pending;
    let cancel = Cancellation::default();
    let ctx = context(&cancel, 10_000);
    assert!(matches!(journal.recover_transaction_with_context(&request(2, 0), &ctx).unwrap(), CommitRecoveryDecision::ConflictingInput));
    assert_eq!(journal.pending, pending);
    assert!(matches!(journal.recover_transaction_with_context(&command, &ctx).unwrap(), CommitRecoveryDecision::Committed(_)));
}

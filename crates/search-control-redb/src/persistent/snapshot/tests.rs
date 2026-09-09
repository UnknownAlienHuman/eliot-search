//! Real-file publication tests. Checkpoints use cancellation/fake clocks, not sleeps.

use super::*;
use super::super::{Boundary, CommitRecoveryDecision, ControlInterruption, RECORDS};
use crate::{ControlJournal, ControlKey, ControlRecordClass, ControlValue, ControlWrite,
    ControlMutation, JournalIdentity, JournalLimits};
use search_contracts::{Blake3Digest32, DataRootId, InstallationIncarnationId, OpaqueRef,
    OwnerEpoch, RequestId};
use search_ports::{PackageOpaque, PortErrorKind, PortRetryability};
use std::cell::{Cell, RefCell};
use std::fs::{self, OpenOptions, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const LIMITS: JournalLimits = JournalLimits::BASELINE;
static NEXT: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Default)]
struct Cancel(Arc<AtomicBool>);
impl PackageOpaque for Cancel {
    fn owner_package(&self) -> &'static str { "search-control-redb" }
}
impl CancellationProbe for Cancel {
    fn is_cancelled(&self) -> bool { self.0.load(Ordering::SeqCst) }
}
impl Cancel {
    fn set(&self, value: bool) { self.0.store(value, Ordering::SeqCst); }
}
fn context(cancel: &Cancel, ms: u64) -> OperationContext<Cancel> {
    OperationContext::new(RequestId::from_bytes([1; 16]), ms, cancel.clone(),
        OpaqueRef::new("budget:snapshot-test").unwrap()).unwrap()
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
fn command(id: u8, generation: u64) -> ControlMutation {
    ControlMutation::new(MutationId([id; 32]), Blake3Digest32::from_bytes([9; 32]), generation,
        vec![ControlWrite {
            key: ControlKey::new(b"state".to_vec(), LIMITS).unwrap(),
            value: ControlValue::new(ControlRecordClass::State, vec![id; 3], LIMITS).unwrap(),
        }], vec![])
}
struct Scratch {
    root: PathBuf,
    handle: RefCell<Option<File>>,
}
impl Scratch {
    fn keep(&self, file: &File) { *self.handle.borrow_mut() = Some(file.try_clone().unwrap()); }
    /// Exact database bytes, read through a handle duplicated from the one redb
    /// owns. redb holds an exclusive byte-range lock for the life of the database;
    /// on Windows an unrelated `fs::read` of the same path fails with
    /// `ERROR_LOCK_VIOLATION`. A duplicated handle shares that lock ownership, so
    /// this reads the same bytes without unlocking, dropping or reopening.
    fn bytes(&self) -> Vec<u8> {
        let mut guard = self.handle.borrow_mut();
        let file = guard.as_mut().expect("scratch file handle");
        file.seek(SeekFrom::Start(0)).unwrap();
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer).unwrap();
        buffer
    }
    fn new() -> Self {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("eliot-publish-{}-{stamp}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&path).unwrap();
        Self { root: path, handle: RefCell::new(None) }
    }
    fn path(&self) -> PathBuf { self.root.join("control.redb") }
    fn create(&self, identity: JournalIdentity) -> PersistentControlJournal {
        let file = OpenOptions::new().read(true).write(true).create_new(true).open(self.path()).unwrap();
        self.keep(&file);
        PersistentControlJournal::create(file, identity, LIMITS).unwrap()
    }
    fn open(&self) -> PersistentControlJournal {
        let file = OpenOptions::new().read(true).write(true).open(self.path()).unwrap();
        self.keep(&file);
        PersistentControlJournal::open(file, identity(), LIMITS).unwrap()
    }
    fn populated(&self) -> (PersistentControlJournal, ControlSnapshotPublisher) {
        let mut journal = self.create(identity());
        let receipt = journal.transact(&command(1, 0)).unwrap();
        let mut publisher = ControlSnapshotPublisher::new();
        journal.publish_committed_snapshot(&receipt, &mut publisher).unwrap();
        (journal, publisher)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) { let _ = fs::remove_dir_all(&self.root); }
}
struct At<'a, F> { inner: &'a dyn Check, point: Point, action: F }
impl<F: Fn()> Check for At<'_, F> {
    fn check(&self, point: Point) -> Result<(), ControlError> {
        if point == self.point { (self.action)(); }
        self.inner.check(point)
    }
}
fn blocked(publisher: &ControlSnapshotPublisher) {
    assert!(publisher.requires_recovery());
    assert!(publisher.current().is_none());
}

#[test]
fn public_snapshot_methods_agree_and_create_no_durable_writes() {
    let scratch = Scratch::new();
    let mut journal = scratch.create(identity());
    let receipt = journal.transact(&command(1, 0)).unwrap();
    let before = scratch.bytes();
    let commits = journal.committed_writes();
    let cancel = Cancel::default();
    let ctx = context(&cancel, 10_000);
    let state = journal.control_snapshot_with_context(&ctx).unwrap();
    assert_eq!(state, journal.control_snapshot().unwrap());
    let mut publisher = ControlSnapshotPublisher::new();
    let publish_receipt = journal.publish_committed_snapshot_with_context(&receipt, &mut publisher, &ctx).unwrap();
    assert_eq!(publish_receipt.operation_id, Some(receipt.operation_id));
    assert_eq!(publisher.current().unwrap().as_ref(), &state);
    let recovered = journal.recover_snapshot_publication_with_context(&mut publisher, &ctx).unwrap().unwrap();
    assert_eq!(recovered, publish_receipt);
    assert!(!publisher.requires_recovery());
    assert_eq!(journal.committed_writes(), commits);
    assert_eq!(scratch.bytes(), before);
}

#[test]
fn precancelled_publication_suspends_old_admission_but_does_not_replay_commit() {
    let scratch = Scratch::new();
    let (mut journal, mut publisher) = scratch.populated();
    let old = publisher.current().unwrap();
    let receipt = journal.transact(&command(2, 1)).unwrap();
    let before = scratch.bytes();
    let commits = journal.committed_writes();
    let cancel = Cancel::default();
    cancel.set(true);
    let error = journal.publish_committed_snapshot_with_context(&receipt, &mut publisher, &context(&cancel, 10_000)).unwrap_err();
    assert_eq!(error.kind(), PortErrorKind::CancelledBeforeSideEffect);
    assert_eq!(error.retryability(), PortRetryability::AfterReadback);
    assert_eq!(error.operation_id(), Some(receipt.operation_id));
    blocked(&publisher);
    assert_eq!(old.generation, 1); // Existing immutable views are not authority.
    assert!(!journal.requires_recovery());
    cancel.set(false);
    journal.recover_snapshot_publication_with_context(&mut publisher, &context(&cancel, 10_000)).unwrap();
    assert_eq!(publisher.current().unwrap().generation, 2);
    assert_eq!(journal.committed_writes(), commits);
    assert_eq!(scratch.bytes(), before);
}

#[test]
fn cancellation_and_deadline_at_each_publication_phase_keep_admission_closed() {
    for recovery in [false, true] {
        for why in [ControlInterruption::Cancelled, ControlInterruption::DeadlineElapsed] {
            for point in [Point::ReadHeader, Point::ReadRecord, Point::ReadOperation,
                Point::ReadComplete, Point::SnapshotRebuild, Point::SnapshotPrepared, Point::BeforePublish]
            {
                let scratch = Scratch::new();
                let (mut journal, mut publisher) = scratch.populated();
                let receipt = journal.transact(&command(2, 1)).unwrap();
                let before = scratch.bytes();
                let cancel = Cancel::default();
                let ctx = context(&cancel, 10);
                let start = Instant::now();
                let now = Cell::new(start);
                let budget = Budget::with_clock(&ctx, || now.get());
                let gate = At { inner: &budget, point, action: || match why {
                    ControlInterruption::Cancelled => cancel.set(true),
                    ControlInterruption::DeadlineElapsed => now.set(start + Duration::from_millis(10)),
                }};
                let raw = if recovery {
                    journal.recover_publication_checked(&mut publisher, &gate).unwrap_err()
                } else {
                    journal.publish_snapshot_checked(&receipt, &mut publisher, &gate).unwrap_err()
                };
                let failure = budget.failure(raw, Some(receipt.operation_id)).for_recovery();
                assert_eq!(failure.interruption(), Some(why), "{point:?}");
                assert_ne!(failure.kind(), PortErrorKind::OutcomeUnknown);
                assert_eq!(failure.retryability(), PortRetryability::AfterReadback);
                blocked(&publisher);
                assert!(!journal.requires_recovery());
                journal.recover_snapshot_publication(&mut publisher).unwrap();
                assert_eq!(publisher.current().unwrap().generation, 2);
                assert_eq!(scratch.bytes(), before);
            }
        }
    }
}

#[test]
fn cancelled_standalone_rebuild_never_returns_partial_state() {
    let scratch = Scratch::new();
    let (journal, _) = scratch.populated();
    for point in [Point::SnapshotRebuild, Point::SnapshotPrepared] {
        let cancel = Cancel::default();
        let ctx = context(&cancel, 10_000);
        let budget = Budget::new(&ctx);
        let gate = At { inner: &budget, point, action: || cancel.set(true) };
        assert_eq!(journal.control_snapshot_checked(&gate), Err(ControlError::ReadCancelled));
        assert!(!journal.requires_recovery());
    }
}

#[test]
fn empty_recovery_checks_final_deadline_without_fabricating_receipt() {
    let scratch = Scratch::new();
    let journal = scratch.create(identity());
    let before = scratch.bytes();
    let mut publisher = ControlSnapshotPublisher::new();
    let cancel = Cancel::default();
    let ctx = context(&cancel, 10);
    let start = Instant::now();
    let now = Cell::new(start);
    let budget = Budget::with_clock(&ctx, || now.get());
    let gate = At { inner: &budget, point: Point::BeforePublish,
        action: || now.set(start + Duration::from_millis(10)) };
    assert_eq!(journal.recover_publication_checked(&mut publisher, &gate), Err(ControlError::BudgetExceeded));
    blocked(&publisher);
    assert_eq!(journal.recover_snapshot_publication(&mut publisher).unwrap(), None);
    assert!(!publisher.requires_recovery());
    assert!(publisher.current().is_none());
    assert_eq!(scratch.bytes(), before);
}

#[test]
fn snapshot_recovery_cannot_resolve_a_pending_mutation() {
    let scratch = Scratch::new();
    let (mut journal, mut publisher) = scratch.populated();
    let pending = command(2, 1);
    assert_eq!(journal.transact_inner(&pending.clone(), Boundary::LostAcknowledgement), Err(ControlError::CommitOutcomeUnknown));
    assert_eq!(journal.recover_snapshot_publication(&mut publisher), Err(ControlError::StoreQuarantined));
    blocked(&publisher);
    assert!(journal.requires_recovery());
    assert!(matches!(journal.recover_transaction(&pending).unwrap(), CommitRecoveryDecision::Committed(_)));
    blocked(&publisher); // Transaction readback alone does not publish the new view.
    journal.recover_snapshot_publication(&mut publisher).unwrap();
    assert_eq!(publisher.current().unwrap().generation, 2);
}

#[test]
fn forged_receipts_cannot_set_a_fictional_generation_floor() {
    let scratch = Scratch::new();
    let (mut journal, mut publisher) = scratch.populated();
    let receipt = journal.transact(&command(2, 1)).unwrap();
    for variant in 0..4 {
        let mut forged = receipt.clone();
        match variant {
            0 => forged.after_generation = u64::MAX,
            1 => forged.operation_id = MutationId([99; 32]),
            2 => forged.command_digest = Blake3Digest32::from_bytes([99; 32]),
            _ => forged.changed_keys.clear(),
        }
        assert_eq!(journal.publish_committed_snapshot(&forged, &mut publisher), Err(ControlError::SnapshotPublicationFailed));
        blocked(&publisher);
        journal.recover_snapshot_publication(&mut publisher).unwrap();
        assert_eq!(publisher.current().unwrap().generation, 2);
    }
}

#[test]
fn reference_model_and_caller_snapshot_cannot_bypass_a_disk_binding() {
    let scratch = Scratch::new();
    let (journal, mut publisher) = scratch.populated();
    let mut model = ControlJournal::open_or_create(identity(), LIMITS).unwrap();
    let receipt = model.transact(&command(1, 0)).unwrap();
    let snapshot = journal.control_snapshot().unwrap();
    // Even a ready disk-bound publisher cannot silently switch to model truth.
    assert_eq!(publisher.publish_snapshot_after_commit(&receipt, snapshot.clone()), Err(ControlError::SnapshotPublicationFailed));
    let cancel = Cancel::default();
    cancel.set(true);
    assert!(journal.recover_snapshot_publication_with_context(&mut publisher, &context(&cancel, 10_000)).is_err());
    assert_eq!(publisher.publish_snapshot_after_commit(&receipt, snapshot), Err(ControlError::SnapshotPublicationFailed));
    assert_eq!(publisher.recover_snapshot_publication(&model), Err(ControlError::SnapshotPublicationFailed));
    blocked(&publisher);
    journal.recover_snapshot_publication(&mut publisher).unwrap();
    assert!(!publisher.requires_recovery());
}

#[test]
fn foreign_identity_cannot_poison_or_unblock_the_correct_publisher() {
    let first = Scratch::new();
    let second = Scratch::new();
    let (journal, mut publisher) = first.populated();
    let other = second.create(JournalIdentity { data_root_id: DataRootId::from_bytes([88; 16]), ..identity() });
    let before = publisher.current().unwrap();
    assert_eq!(other.recover_snapshot_publication(&mut publisher), Err(ControlError::IdentityMismatch));
    assert!(Arc::ptr_eq(&before, &publisher.current().unwrap()));
    let cancel = Cancel::default();
    cancel.set(true);
    assert!(journal.recover_snapshot_publication_with_context(&mut publisher, &context(&cancel, 10_000)).is_err());
    assert_eq!(other.recover_snapshot_publication(&mut publisher), Err(ControlError::IdentityMismatch));
    blocked(&publisher);
}

#[test]
fn observed_later_generation_cannot_be_reset_by_an_empty_historical_file() {
    let scratch = Scratch::new();
    let historical = Scratch::new();
    let old_database = historical.create(identity()); // Deliberately stale same-identity fixture.
    let (mut journal, mut publisher) = scratch.populated();
    let old_receipt = journal.transact(&command(2, 1)).unwrap();
    journal.transact(&command(3, 2)).unwrap();
    assert_eq!(journal.publish_committed_snapshot(&old_receipt, &mut publisher), Err(ControlError::SnapshotPublicationFailed));
    assert_eq!(old_database.recover_snapshot_publication(&mut publisher), Err(ControlError::SnapshotPublicationFailed));
    blocked(&publisher);
    journal.recover_snapshot_publication(&mut publisher).unwrap();
    assert_eq!(publisher.current().unwrap().generation, 3);
}

#[test]
fn same_generation_conflict_still_uses_the_hidden_previous_pointer() {
    let scratch = Scratch::new();
    let (journal, mut publisher) = scratch.populated();
    let write = journal.database.begin_write().unwrap();
    {
        let mut table = write.open_table(RECORDS).unwrap();
        table.insert(b"state".as_slice(), [3_u8, 7, 7, 7].as_slice()).unwrap();
    }
    write.commit().unwrap(); // Valid class/length, deliberately conflicting record bytes.
    assert_eq!(journal.recover_snapshot_publication(&mut publisher), Err(ControlError::SnapshotPublicationFailed));
    blocked(&publisher);
    assert_eq!(journal.recover_snapshot_publication(&mut publisher), Err(ControlError::SnapshotPublicationFailed));
    blocked(&publisher); // Hiding current() did not erase the comparison baseline.
}

#[test]
fn corrupted_records_and_receipts_never_release_admission() {
    for corrupt_receipt in [false, true] {
        let scratch = Scratch::new();
        let (journal, mut publisher) = scratch.populated();
        let write = journal.database.begin_write().unwrap();
        if corrupt_receipt {
            let mut table = write.open_table(OPERATIONS).unwrap();
            table.insert([1_u8; 32].as_slice(), b"broken".as_slice()).unwrap();
        } else {
            let mut table = write.open_table(RECORDS).unwrap();
            table.insert(b"state".as_slice(), [255_u8, 1, 1, 1].as_slice()).unwrap();
        }
        write.commit().unwrap();
        assert!(journal.recover_snapshot_publication(&mut publisher).is_err());
        blocked(&publisher);
    }
}

#[test]
fn transient_read_failure_requires_recovery_not_false_corruption() {
    struct Fail;
    impl Check for Fail {
        fn check(&self, point: Point) -> Result<(), ControlError> {
            if point == Point::ReadRecord { Err(ControlError::StoreUnavailable) } else { Ok(()) }
        }
    }
    let scratch = Scratch::new();
    let (journal, mut publisher) = scratch.populated();
    assert_eq!(journal.recover_publication_checked(&mut publisher, &Fail), Err(ControlError::StoreUnavailable));
    blocked(&publisher);
    assert!(!journal.quarantined);
    journal.recover_snapshot_publication(&mut publisher).unwrap();
    assert!(!publisher.requires_recovery());
}

#[test]
fn owner_handoff_and_restart_recover_without_mutation_replay() {
    let scratch = Scratch::new();
    let (mut journal, mut publisher) = scratch.populated();
    journal.transact(&command(2, 1)).unwrap();
    let cancel = Cancel::default();
    cancel.set(true);
    assert!(journal.recover_snapshot_publication_with_context(&mut publisher, &context(&cancel, 10_000)).is_err());
    drop(journal);
    let journal = scratch.open();
    assert_eq!(journal.committed_writes(), 0);
    journal.recover_snapshot_publication(&mut publisher).unwrap();
    let next = JournalIdentity { owner_epoch: OwnerEpoch::new(2).unwrap(), ..identity() };
    let journal = journal.advance_owner(next).unwrap();
    journal.recover_snapshot_publication(&mut publisher).unwrap();
    assert_eq!(publisher.current().unwrap().identity, next);
    assert_eq!(publisher.current().unwrap().generation, 2);
    assert_eq!(journal.committed_writes(), 1); // Only the explicit owner handoff.
    let historical = Scratch::new();
    let old_owner = historical.create(identity());
    assert_eq!(old_owner.recover_snapshot_publication(&mut publisher), Err(ControlError::SnapshotPublicationFailed));
    assert!(!publisher.requires_recovery());
}

#[test]
fn a_single_deadline_spans_read_and_rebuild_without_phase_reset() {
    let scratch = Scratch::new();
    let (journal, mut publisher) = scratch.populated();
    let cancel = Cancel::default();
    let ctx = context(&cancel, 10);
    let now = Cell::new(Instant::now());
    let budget = Budget::with_clock(&ctx, || now.get());
    let rebuild = At { inner: &budget, point: Point::SnapshotPrepared,
        action: || now.set(now.get() + Duration::from_millis(6)) };
    let read = At { inner: &rebuild, point: Point::ReadRecord,
        action: || now.set(now.get() + Duration::from_millis(6)) };
    assert_eq!(journal.recover_publication_checked(&mut publisher, &read), Err(ControlError::BudgetExceeded));
    blocked(&publisher);
}

#[test]
fn cancellation_after_the_final_checkpoint_does_not_relabel_a_published_result() {
    struct Late<'a> { budget: &'a dyn Check, cancel: &'a Cancel }
    impl Check for Late<'_> {
        fn check(&self, point: Point) -> Result<(), ControlError> {
            self.budget.check(point)?;
            if point == Point::BeforePublish { self.cancel.set(true); }
            Ok(())
        }
    }
    let scratch = Scratch::new();
    let (journal, mut publisher) = scratch.populated();
    let cancel = Cancel::default();
    let ctx = context(&cancel, 10_000);
    let budget = Budget::new(&ctx);
    let result = journal.recover_publication_checked(&mut publisher, &Late { budget: &budget, cancel: &cancel }).unwrap();
    assert_eq!(result.unwrap().generation, 1);
    assert!(cancel.is_cancelled());
    assert!(!publisher.requires_recovery());
    assert_eq!(publisher.current().unwrap().generation, 1);
}

#[test]
fn ten_thousand_snapshot_admissions_write_no_database_bytes() {
    let scratch = Scratch::new();
    let (journal, publisher) = scratch.populated();
    let before = scratch.bytes();
    let writes = journal.committed_writes();
    for _ in 0..10_000 { assert_eq!(publisher.current().unwrap().generation, 1); }
    assert_eq!(journal.committed_writes(), writes);
    assert_eq!(scratch.bytes(), before);
}

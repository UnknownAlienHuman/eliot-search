//! Real-file diagnostics regressions; none of these observations grants admission.

use super::*;
use crate::{CommitRecoveryDecision, ControlKey, ControlMutation, ControlRecordClass,
    ControlValue, ControlWrite, JournalLimits, MutationId};
use crate::persistent::{Boundary, META, OPERATIONS, RECORDS};
use redb::ReadableTable;
use search_contracts::{Blake3Digest32, DataRootId, InstallationIncarnationId,
    OpaqueRef, OwnerEpoch, RequestId};
use search_ports::{PackageOpaque, PortErrorKind};
use std::cell::Cell;
use std::fs::{self, File, OpenOptions};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const LIMITS: JournalLimits = JournalLimits::BASELINE;
static NEXT: AtomicU64 = AtomicU64::new(0);

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
        let root = std::env::temp_dir().join(format!("eliot-control-health-{}-{stamp}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&root).unwrap();
        Self(root)
    }
    fn path(&self) -> PathBuf { self.0.join("control.redb") }
    fn file(&self) -> File { OpenOptions::new().read(true).write(true).open(self.path()).unwrap() }
    fn create(&self) -> PersistentControlJournal { self.create_as(identity(), LIMITS) }
    fn create_as(&self, id: JournalIdentity, limits: JournalLimits) -> PersistentControlJournal {
        let file = OpenOptions::new().create_new(true).read(true).write(true).open(self.path()).unwrap();
        PersistentControlJournal::create(file, id, limits).unwrap()
    }
}
impl Drop for Scratch {
    fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); }
}

#[derive(Debug)]
struct Cancellation(bool);
impl PackageOpaque for Cancellation {
    fn owner_package(&self) -> &'static str { "search-control-redb" }
}
impl CancellationProbe for Cancellation {
    fn is_cancelled(&self) -> bool { self.0 }
}
fn context(cancelled: bool) -> OperationContext<Cancellation> {
    OperationContext::new(RequestId::from_bytes([7; 16]), 10_000, Cancellation(cancelled),
        OpaqueRef::new("budget:health-fixture").unwrap()).unwrap()
}
fn key(name: &[u8]) -> ControlKey { ControlKey::new(name.to_vec(), LIMITS).unwrap() }
fn value(bytes: &[u8]) -> ControlValue {
    ControlValue::new(ControlRecordClass::State, bytes.to_vec(), LIMITS).unwrap()
}
fn put(id: u8, generation: u64, name: &[u8], bytes: &[u8]) -> ControlMutation {
    ControlMutation::new(MutationId([id; 32]), Blake3Digest32::from_bytes([9; 32]), generation,
        vec![ControlWrite { key: key(name), value: value(bytes) }], vec![])
}
fn counters(journal: &PersistentControlJournal) -> JournalWriteCounters {
    journal.write_counters_with_context(&context(false)).unwrap()
}
fn health(journal: &PersistentControlJournal, publisher: Option<&ControlSnapshotPublisher>) -> ControlStoreHealth {
    journal.journal_health_with_context(publisher, &context(false)).unwrap()
}
fn hold_marker(journal: &PersistentControlJournal, bytes: &[u8]) {
    let write = journal.database.begin_write().unwrap();
    {
        let mut meta = write.open_table(META).unwrap();
        meta.insert("quarantine", bytes).unwrap();
    }
    write.commit().unwrap();
}

#[test]
fn initialized_counts_distinguish_data_commits_from_successful_lifecycle_calls() {
    let scratch = Scratch::new();
    let journal = scratch.create();
    assert_eq!(counters(&journal), JournalWriteCounters {
        data_generation: 0, live_records: 0, live_value_bytes: 0,
        operation_records: 0, operation_record_bytes: 0, acknowledged_mutating_calls: 1,
    });
    let observed = health(&journal, None);
    assert_eq!(observed.state, JournalHealthState::MetadataReadable);
    assert_eq!(observed.snapshot, SnapshotHealthState::NotObserved);
    assert_eq!(observed.reason, None);
    assert!(!observed.pending_mutation);
    assert!(!observed.local_quarantine);
}

#[test]
fn counters_follow_actual_commit_replace_delete_and_not_replay_or_reads() {
    let scratch = Scratch::new();
    let mut journal = scratch.create();
    let mutation = put(1, 0, b"state", b"READY");
    journal.transact(mutation.clone()).unwrap();
    let first = counters(&journal);
    assert_eq!((first.data_generation, first.live_records, first.live_value_bytes), (1, 1, 5));
    assert_eq!(first.operation_records, 1);
    assert!(first.operation_record_bytes > 0);
    assert_eq!(first.acknowledged_mutating_calls, 2);
    assert!(journal.transact(mutation).unwrap().replayed);
    journal.read_snapshot().unwrap();
    journal.verify().unwrap();
    assert_eq!(counters(&journal), first);
    journal.transact(put(2, 1, b"state", b"OK")).unwrap();
    let second = counters(&journal);
    assert_eq!((second.data_generation, second.live_records, second.live_value_bytes), (2, 1, 2));
    assert!(second.operation_record_bytes > first.operation_record_bytes);
    let delete = ControlMutation::new(MutationId([3; 32]), Blake3Digest32::from_bytes([9; 32]), 2,
        vec![], vec![key(b"state")]);
    journal.transact(delete).unwrap();
    let third = counters(&journal);
    assert_eq!((third.data_generation, third.live_records, third.live_value_bytes), (3, 0, 0));
    assert_eq!(third.operation_records, 3);
    assert_eq!(third.acknowledged_mutating_calls, 4);
}

#[test]
fn reopen_and_owner_handoff_do_not_reset_durable_data_counts() {
    let scratch = Scratch::new();
    let before = {
        let mut journal = scratch.create();
        journal.transact(put(1, 0, b"state", b"READY")).unwrap();
        counters(&journal)
    };
    let journal = PersistentControlJournal::open(scratch.file(), identity(), LIMITS).unwrap();
    assert_eq!(counters(&journal), JournalWriteCounters { acknowledged_mutating_calls: 0, ..before });
    let next = JournalIdentity { owner_epoch: OwnerEpoch::new(2).unwrap(), ..identity() };
    let journal = journal.advance_owner(next).unwrap();
    assert_eq!(counters(&journal), JournalWriteCounters { acknowledged_mutating_calls: 1, ..before });
    assert_eq!(health(&journal, None).expected_identity, next);
}

#[test]
fn lost_ack_health_observes_commit_but_never_resolves_the_pending_mutation() {
    let scratch = Scratch::new();
    let mut journal = scratch.create();
    let mutation = put(1, 0, b"state", b"READY");
    assert_eq!(journal.transact_inner(mutation.clone(), Boundary::LostAcknowledgement),
        Err(ControlError::CommitOutcomeUnknown));
    let before = fs::read(scratch.path()).unwrap();
    let observed = health(&journal, None);
    assert_eq!(observed.state, JournalHealthState::RecoveryRequired);
    assert!(observed.pending_mutation);
    assert_eq!(observed.snapshot, SnapshotHealthState::BlockedByJournal);
    let observed_counts = observed.counters.unwrap();
    assert_eq!(observed_counts.data_generation, 1);
    assert_eq!(observed_counts.acknowledged_mutating_calls, 1); // no false success count
    assert!(journal.requires_recovery());
    assert_eq!(journal.write_counters_with_context(&context(false)).unwrap_err().control_error(),
        ControlError::StoreQuarantined);
    assert_eq!(fs::read(scratch.path()).unwrap(), before);
    assert!(matches!(journal.recover_transaction(&mutation).unwrap(), CommitRecoveryDecision::Committed(_)));
    assert_eq!(counters(&journal), observed_counts); // recovery is not another write
}

#[test]
fn health_reports_publisher_alignment_without_publishing_or_clearing_its_fence() {
    let scratch = Scratch::new();
    let mut journal = scratch.create();
    let mut publisher = ControlSnapshotPublisher::new();
    assert_eq!(health(&journal, Some(&publisher)).snapshot, SnapshotHealthState::Unbound);
    assert_eq!(journal.recover_snapshot_publication(&mut publisher).unwrap(), None);
    assert_eq!(health(&journal, Some(&publisher)).snapshot, SnapshotHealthState::NotPublished);
    let first = journal.transact(put(1, 0, b"state", b"READY")).unwrap();
    journal.publish_committed_snapshot(&first, &mut publisher).unwrap();
    assert_eq!(health(&journal, Some(&publisher)).snapshot, SnapshotHealthState::GenerationAligned);
    let old_pointer = publisher.current().unwrap();
    journal.transact(put(2, 1, b"state", b"STOPPED")).unwrap();
    assert_eq!(health(&journal, Some(&publisher)).snapshot, SnapshotHealthState::BehindJournal);
    assert!(std::sync::Arc::ptr_eq(&old_pointer, &publisher.current().unwrap()));
    publisher.begin_disk_publication(identity()).unwrap();
    assert_eq!(health(&journal, Some(&publisher)).snapshot, SnapshotHealthState::RecoveryRequired);
    assert!(publisher.requires_recovery());
    assert!(publisher.current().is_none());
}

#[test]
fn foreign_or_previous_owner_publisher_is_observed_not_poisoned() {
    let scratch = Scratch::new();
    let mut journal = scratch.create();
    let commit = journal.transact(put(1, 0, b"state", b"READY")).unwrap();
    let mut publisher = ControlSnapshotPublisher::new();
    journal.publish_committed_snapshot(&commit, &mut publisher).unwrap();
    let old_pointer = publisher.current().unwrap();
    let other_root = Scratch::new();
    let foreign = JournalIdentity { data_root_id: DataRootId::from_bytes([8; 16]), ..identity() };
    let other = other_root.create_as(foreign, LIMITS);
    assert_eq!(health(&other, Some(&publisher)).snapshot, SnapshotHealthState::DifferentBinding);
    let next = JournalIdentity { owner_epoch: OwnerEpoch::new(2).unwrap(), ..identity() };
    let journal = journal.advance_owner(next).unwrap();
    assert_eq!(health(&journal, Some(&publisher)).snapshot, SnapshotHealthState::DifferentBinding);
    assert!(!publisher.requires_recovery());
    assert!(std::sync::Arc::ptr_eq(&old_pointer, &publisher.current().unwrap()));
}

#[test]
fn a_newer_snapshot_is_not_evidence_that_an_older_journal_was_recovered() {
    let scratch = Scratch::new();
    let mut journal = scratch.create();
    let commit = journal.transact(put(1, 0, b"state", b"READY")).unwrap();
    let mut publisher = ControlSnapshotPublisher::new();
    journal.publish_committed_snapshot(&commit, &mut publisher).unwrap();
    // Pure comparison fixture: no database rollback or false native owner claim.
    assert_eq!(snapshot_health(Some(&publisher), identity(), 0), SnapshotHealthState::AheadOfJournal);
    assert_eq!(publisher.current().unwrap().generation, 1);
}

#[test]
fn local_quarantine_and_pending_state_are_reported_without_being_erased() {
    let scratch = Scratch::new();
    let mut journal = scratch.create();
    journal.pending = Some((MutationId([8; 32]), [7; 32]));
    journal.quarantined = true;
    let observed = health(&journal, None);
    assert_eq!(observed.state, JournalHealthState::LocallyQuarantined);
    assert!(observed.local_quarantine && observed.pending_mutation);
    assert_eq!(observed.counters.unwrap().data_generation, 0);
    assert_eq!(observed.snapshot, SnapshotHealthState::BlockedByJournal);
    assert!(journal.quarantined && journal.requires_recovery());
    assert_eq!(journal.write_counters_with_context(&context(false)).unwrap_err().control_error(),
        ControlError::StoreQuarantined);
}

#[test]
fn any_durable_marker_presence_is_reported_as_a_hold_without_decoding_it_as_healthy() {
    for marker in [b"broken-marker".as_slice(), b"".as_slice()] {
        let scratch = Scratch::new();
        let journal = scratch.create();
        hold_marker(&journal, marker);
        let before = fs::read(scratch.path()).unwrap();
        let observed = health(&journal, None);
        assert_eq!(observed.state, JournalHealthState::DurablyQuarantined);
        assert_eq!(observed.counters, None);
        assert_eq!(observed.reason, Some(ControlError::StoreQuarantined));
        assert_eq!(observed.snapshot, SnapshotHealthState::BlockedByJournal);
        assert!(!journal.quarantined); // diagnostics did not invent/change local state
        assert_eq!(fs::read(scratch.path()).unwrap(), before);
        assert_eq!(journal.read_snapshot(), Err(ControlError::StoreQuarantined));
    }
}

#[test]
fn invalid_header_reports_quarantine_required_without_creating_a_hold() {
    let scratch = Scratch::new();
    let journal = scratch.create();
    {
        let write = journal.database.begin_write().unwrap();
        {
            let mut meta = write.open_table(META).unwrap();
            meta.insert("header", b"bad-header".as_slice()).unwrap();
        }
        write.commit().unwrap();
    }
    let before = fs::read(scratch.path()).unwrap();
    let observed = health(&journal, None);
    assert_eq!(observed.state, JournalHealthState::QuarantineRequired);
    assert_eq!(observed.counters, None);
    assert_eq!(observed.reason, Some(ControlError::SchemaUnsupported));
    assert_eq!(observed.snapshot, SnapshotHealthState::BlockedByJournal);
    let read = journal.database.begin_read().unwrap();
    assert!(read.open_table(META).unwrap().get("quarantine").unwrap().is_none());
    assert!(!journal.quarantined);
    assert_eq!(fs::read(scratch.path()).unwrap(), before);
}

#[test]
fn cardinality_mismatch_cannot_produce_apparently_valid_counters() {
    let scratch = Scratch::new();
    let mut journal = scratch.create();
    journal.transact(put(1, 0, b"state", b"READY")).unwrap();
    {
        let write = journal.database.begin_write().unwrap();
        {
            let mut records = write.open_table(RECORDS).unwrap();
            records.remove(b"state".as_slice()).unwrap();
        }
        write.commit().unwrap();
    }
    assert_eq!(journal.write_counters_with_context(&context(false)).unwrap_err().control_error(),
        ControlError::StoreCorrupt);
    let observed = health(&journal, None);
    assert_eq!(observed.state, JournalHealthState::QuarantineRequired);
    assert_eq!(observed.counters, None);
}

#[test]
fn metadata_readability_does_not_claim_record_or_receipt_body_integrity() {
    for corrupt_receipt in [false, true] {
        let scratch = Scratch::new();
        let mut journal = scratch.create();
        journal.transact(put(1, 0, b"state", b"READY")).unwrap();
        {
            let write = journal.database.begin_write().unwrap();
            if corrupt_receipt {
                let mut receipts = write.open_table(OPERATIONS).unwrap();
                receipts.insert([1_u8; 32].as_slice(), b"invalid-receipt".as_slice()).unwrap();
            } else {
                let mut records = write.open_table(RECORDS).unwrap();
                records.insert(b"state".as_slice(), b"\xffREADY".as_slice()).unwrap();
            }
            write.commit().unwrap();
        }
        let observed = health(&journal, None);
        // The small metadata operation does not walk bodies; its status says so.
        assert_eq!(observed.state, JournalHealthState::MetadataReadable);
        assert_eq!(observed.counters.unwrap().data_generation, 1);
        assert!(journal.verify().is_err());
    }
}

#[test]
fn pre_cancelled_diagnostics_return_errors_without_partial_observations() {
    let scratch = Scratch::new();
    let journal = scratch.create();
    let before = fs::read(scratch.path()).unwrap();
    let ctx = context(true);
    let health_error = journal.journal_health_with_context(None, &ctx).unwrap_err();
    let counter_error = journal.write_counters_with_context(&ctx).unwrap_err();
    for error in [health_error, counter_error] {
        assert_eq!(error.kind(), PortErrorKind::CancelledBeforeSideEffect);
        assert_eq!(error.operation_id(), None);
    }
    assert_eq!(fs::read(scratch.path()).unwrap(), before);
    assert!(!journal.requires_recovery());
}

struct StopAt(Point);
impl Check for StopAt {
    fn check(&self, point: Point) -> Result<(), ControlError> {
        if point == self.0 { Err(ControlError::ReadCancelled) } else { Ok(()) }
    }
}

#[test]
fn cancellation_after_observation_preserves_publisher_and_existing_journal_fences() {
    let scratch = Scratch::new();
    let mut journal = scratch.create();
    let commit = journal.transact(put(1, 0, b"state", b"READY")).unwrap();
    let mut publisher = ControlSnapshotPublisher::new();
    journal.publish_committed_snapshot(&commit, &mut publisher).unwrap();
    let old_pointer = publisher.current().unwrap();
    assert_eq!(journal.write_counters_checked(&StopAt(Point::ReadComplete)), Err(ControlError::ReadCancelled));
    journal.pending = Some((MutationId([8; 32]), [7; 32]));
    assert_eq!(journal.journal_health_checked(Some(&publisher), &StopAt(Point::ReadComplete)),
        Err(ControlError::ReadCancelled));
    assert!(journal.requires_recovery());
    assert!(!publisher.requires_recovery());
    assert!(std::sync::Arc::ptr_eq(&old_pointer, &publisher.current().unwrap()));
}

#[test]
fn one_relative_deadline_covers_header_read_and_final_observation() {
    let scratch = Scratch::new();
    let journal = scratch.create();
    let ctx = OperationContext::new(RequestId::from_bytes([8; 16]), 4, Cancellation(false),
        OpaqueRef::new("budget:health-deadline").unwrap()).unwrap();
    for read_counters in [false, true] {
        let start = Instant::now();
        let ticks = Cell::new(0_u64);
        let budget = Budget::with_clock(&ctx, || {
            let tick = ticks.get(); ticks.set(tick + 1);
            start + Duration::from_millis(tick)
        });
        let result = if read_counters {
            journal.write_counters_checked(&budget).map(|_| ())
        } else {
            journal.journal_health_checked(None, &budget).map(|_| ())
        };
        assert_eq!(result, Err(ControlError::BudgetExceeded));
    }
    assert_eq!(counters(&journal).acknowledged_mutating_calls, 1);
}

#[test]
fn exhausted_mutation_ledger_does_not_disable_read_only_diagnostics() {
    let scratch = Scratch::new();
    let limits = JournalLimits { max_operation_records: 1, ..LIMITS };
    let mut journal = scratch.create_as(identity(), limits);
    journal.transact(put(1, 0, b"state", b"READY")).unwrap();
    assert_eq!(journal.transact(put(2, 1, b"state", b"STOPPED")),
        Err(ControlError::IdempotencyCapacityExceeded));
    assert_eq!(counters(&journal).operation_records, 1);
    assert_eq!(health(&journal, None).state, JournalHealthState::MetadataReadable);
}

#[test]
fn ten_thousand_diagnostics_do_not_scan_values_publish_snapshots_or_write_bytes() {
    let scratch = Scratch::new();
    let mut journal = scratch.create();
    let writes = (0..128).map(|n| ControlWrite {
        key: key(format!("fixture-{n:03}").as_bytes()), value: value(b"private-value-sentinel"),
    }).collect();
    let mutation = ControlMutation::new(MutationId([1; 32]), Blake3Digest32::from_bytes([9; 32]), 0, writes, vec![]);
    let commit = journal.transact(mutation).unwrap();
    let mut publisher = ControlSnapshotPublisher::new();
    journal.publish_committed_snapshot(&commit, &mut publisher).unwrap();
    let pointer = publisher.current().unwrap();
    let expected = counters(&journal);
    let expected_health = health(&journal, Some(&publisher));
    let before_bytes = fs::read(scratch.path()).unwrap();
    let before_scans = journal.work.snapshot_reads.load(Ordering::Relaxed);
    let before_points = journal.work.point_reads.load(Ordering::Relaxed);
    let ctx = context(false);
    for _ in 0..10_000 {
        assert_eq!(journal.write_counters_with_context(&ctx).unwrap(), expected);
        assert_eq!(journal.journal_health_with_context(Some(&publisher), &ctx).unwrap(), expected_health);
    }
    assert_eq!(journal.work.snapshot_reads.load(Ordering::Relaxed), before_scans);
    assert_eq!(journal.work.point_reads.load(Ordering::Relaxed), before_points);
    assert_eq!(fs::read(scratch.path()).unwrap(), before_bytes);
    assert!(std::sync::Arc::ptr_eq(&pointer, &publisher.current().unwrap()));
    assert!(!publisher.requires_recovery());
}

#[test]
fn diagnostic_debug_contains_no_record_keys_values_or_paths() {
    let scratch = Scratch::new();
    let mut journal = scratch.create();
    journal.transact(put(1, 0, b"private-key-sentinel", b"private-value-sentinel")).unwrap();
    let debug = format!("{:?} {:?}", counters(&journal), health(&journal, None));
    assert!(!debug.contains("private-key-sentinel"));
    assert!(!debug.contains("private-value-sentinel"));
    assert!(!debug.contains(scratch.0.to_str().unwrap()));
}

#[test]
fn matching_but_unbound_snapshot_is_not_reported_as_disk_published() {
    let scratch = Scratch::new();
    let mut journal = scratch.create();
    let commit = journal.transact(put(1, 0, b"state", b"READY")).unwrap();
    let mut raw_publisher = ControlSnapshotPublisher::new();
    raw_publisher.publish_snapshot_after_commit(&commit, journal.control_snapshot().unwrap()).unwrap();
    assert_eq!(raw_publisher.current().unwrap().generation, counters(&journal).data_generation);
    assert_eq!(health(&journal, Some(&raw_publisher)).snapshot, SnapshotHealthState::Unbound);
}

#[test]
fn unavailable_inspection_is_an_error_not_a_fabricated_quarantine_diagnosis() {
    struct Unavailable;
    impl Check for Unavailable {
        fn check(&self, point: Point) -> Result<(), ControlError> {
            if point == Point::ReadHeader { Err(ControlError::StoreUnavailable) } else { Ok(()) }
        }
    }
    let scratch = Scratch::new();
    let journal = scratch.create();
    let before = fs::read(scratch.path()).unwrap();
    assert_eq!(journal.journal_health_checked(None, &Unavailable), Err(ControlError::StoreUnavailable));
    assert_eq!(journal.write_counters_checked(&Unavailable), Err(ControlError::StoreUnavailable));
    assert!(!journal.quarantined && !journal.requires_recovery());
    assert_eq!(fs::read(scratch.path()).unwrap(), before);
}

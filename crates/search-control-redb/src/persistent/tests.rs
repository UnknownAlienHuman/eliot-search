use super::*;
use crate::{ControlJournal, ControlRecordClass, ControlValue, ControlWrite};
use search_contracts::{Blake3Digest32, DataRootId, InstallationIncarnationId, OwnerEpoch};
use std::fs::{self, OpenOptions};
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static NEXT: AtomicU64 = AtomicU64::new(0);
const LIMITS: JournalLimits = JournalLimits::BASELINE;

fn identity() -> JournalIdentity {
    // Fixed identities are test fixtures, never runtime-generated authority.
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
        let path = std::env::temp_dir().join(format!("eliot-redb-{}-{stamp}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn path(&self) -> PathBuf { self.0.join("control.redb") }
    fn create(&self, limits: JournalLimits) -> PersistentControlJournal {
        let file = OpenOptions::new().read(true).write(true).create_new(true).open(self.path()).unwrap();
        PersistentControlJournal::create(file, identity(), limits).unwrap()
    }
    fn file(&self) -> File { OpenOptions::new().read(true).write(true).open(self.path()).unwrap() }
    fn open(&self, limits: JournalLimits) -> PersistentControlJournal {
        PersistentControlJournal::open(self.file(), identity(), limits).unwrap()
    }
}
impl Drop for Scratch {
    fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); }
}

fn key(value: &[u8]) -> ControlKey { ControlKey::new(value.to_vec(), LIMITS).unwrap() }
fn value(value: &[u8]) -> ControlValue {
    ControlValue::new(ControlRecordClass::State, value.to_vec(), LIMITS).unwrap()
}
fn mutation(id: u8, generation: u64, bytes: &[u8]) -> ControlMutation {
    ControlMutation::new(MutationId([id; 32]), Blake3Digest32::from_bytes([9; 32]), generation,
        vec![ControlWrite { key: key(b"lifecycle"), value: value(bytes) }], vec![])
}

#[test]
fn transaction_reopens_and_replays_without_writing_again() {
    let scratch = Scratch::new();
    let request = mutation(1, 0, b"READY");
    let receipt = {
        let mut journal = scratch.create(LIMITS);
        journal.transact(request.clone()).unwrap()
    };
    let mut journal = scratch.open(LIMITS);
    let replay = journal.transact(request).unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.after_generation, receipt.after_generation);
    assert_eq!(journal.read_snapshot().unwrap().records, vec![(key(b"lifecycle"), value(b"READY"))]);
    assert_eq!(journal.committed_writes(), 0);
}

#[test]
fn same_claimed_digest_cannot_hide_changed_request() {
    let scratch = Scratch::new();
    let mut journal = scratch.create(LIMITS);
    journal.transact(mutation(1, 0, b"READY")).unwrap();
    assert_eq!(journal.transact(mutation(1, 0, b"STOPPED")), Err(ControlError::OperationConflict));
    assert_eq!(journal.transact(mutation(1, 1, b"READY")), Err(ControlError::OperationConflict));
    let changed_class = ControlMutation::new(MutationId([1; 32]), Blake3Digest32::from_bytes([9; 32]), 0,
        vec![ControlWrite { key: key(b"lifecycle"), value: ControlValue::new(ControlRecordClass::Identity, b"READY".to_vec(), LIMITS).unwrap() }], vec![]);
    assert_eq!(journal.transact(changed_class), Err(ControlError::OperationConflict));
    assert_eq!(journal.read_snapshot().unwrap().generation, 1);
}

#[test]
fn canonical_write_order_replays_but_duplicate_keys_fail() {
    let scratch = Scratch::new();
    let mut journal = scratch.create(LIMITS);
    let writes = vec![ControlWrite { key: key(b"a"), value: value(b"1") }, ControlWrite { key: key(b"b"), value: value(b"2") }];
    let make = |writes| ControlMutation::new(MutationId([1; 32]), Blake3Digest32::from_bytes([9; 32]), 0, writes, vec![]);
    journal.transact(make(writes.clone())).unwrap();
    let mut reordered = writes.clone();
    reordered.reverse();
    assert!(journal.transact(make(reordered)).unwrap().replayed);
    assert_eq!(journal.transact(make(vec![writes[0].clone(), writes[0].clone()])), Err(ControlError::DuplicateMutationKey));
}

#[test]
fn stale_generation_and_capacity_failure_leave_no_partial_record_or_receipt() {
    let scratch = Scratch::new();
    let limits = JournalLimits { max_records: 1, ..LIMITS };
    let mut journal = scratch.create(limits);
    journal.transact(mutation(1, 0, b"READY")).unwrap();
    let before = journal.verify().unwrap();
    assert_eq!(journal.transact(mutation(2, 0, b"STOPPED")), Err(ControlError::TransactionConflict));
    let request = ControlMutation::new(MutationId([3; 32]), Blake3Digest32::from_bytes([7; 32]), 1,
        vec![ControlWrite { key: key(b"second"), value: value(b"new") }], vec![]);
    assert_eq!(journal.transact(request.clone()), Err(ControlError::BudgetExceeded));
    assert!(matches!(journal.recover_transaction(&request).unwrap(), CommitRecoveryDecision::NotCommittedRetrySameOperation));
    assert_eq!(journal.verify().unwrap(), before);
}

#[test]
fn delete_and_insert_are_atomic_and_match_the_reference_model() {
    let scratch = Scratch::new();
    let mut disk = scratch.create(LIMITS);
    let mut reference = ControlJournal::open_or_create(identity(), LIMITS).unwrap();
    let first = mutation(1, 0, b"READY");
    assert_eq!(disk.transact(first.clone()).unwrap(), reference.transact(first).unwrap());
    let second = ControlMutation::new(MutationId([2; 32]), Blake3Digest32::from_bytes([8; 32]), 1,
        vec![ControlWrite { key: key(b"route"), value: value(b"DIRECT") }], vec![key(b"lifecycle")]);
    assert_eq!(disk.transact(second.clone()).unwrap(), reference.transact(second).unwrap());
    assert_eq!(disk.verify().unwrap(), reference.read_snapshot().unwrap());
    let snapshot = disk.control_snapshot().unwrap();
    let expected = rebuild_control_snapshot(reference.read_snapshot().unwrap(), identity(), LIMITS).unwrap();
    assert_eq!(snapshot, expected);
}

#[test]
fn precommit_failure_rolls_back_and_same_operation_can_retry() {
    let scratch = Scratch::new();
    let mut journal = scratch.create(LIMITS);
    let request = mutation(1, 0, b"READY");
    assert_eq!(journal.transact_inner(request.clone(), Boundary::BeforeCommit), Err(ControlError::StoreUnavailable));
    assert!(!journal.requires_recovery());
    assert_eq!(journal.verify().unwrap().generation, 0);
    assert!(matches!(journal.recover_transaction(&request).unwrap(), CommitRecoveryDecision::NotCommittedRetrySameOperation));
    assert!(!journal.transact(request).unwrap().replayed);
}

#[test]
fn lost_acknowledgement_blocks_reads_until_exact_request_recovery() {
    let scratch = Scratch::new();
    let mut journal = scratch.create(LIMITS);
    let request = mutation(1, 0, b"READY");
    assert_eq!(journal.transact_inner(request.clone(), Boundary::LostAcknowledgement), Err(ControlError::CommitOutcomeUnknown));
    assert!(journal.requires_recovery());
    assert_eq!(journal.read_snapshot(), Err(ControlError::StoreQuarantined));
    assert!(matches!(journal.recover_transaction(&mutation(2, 0, b"READY")).unwrap(), CommitRecoveryDecision::ConflictingInput));
    assert!(journal.requires_recovery());
    assert!(matches!(journal.recover_transaction(&request).unwrap(), CommitRecoveryDecision::Committed(_)));
    assert!(!journal.requires_recovery());
    assert_eq!(journal.read_snapshot().unwrap().generation, 1);
    assert!(journal.transact(request).unwrap().replayed);
}

#[test]
fn historical_recovery_does_not_reapply_over_newer_values() {
    let scratch = Scratch::new();
    let mut journal = scratch.create(LIMITS);
    let first = mutation(1, 0, b"READY");
    journal.transact(first.clone()).unwrap();
    journal.transact(mutation(2, 1, b"STOPPED")).unwrap();
    assert!(matches!(journal.recover_transaction(&first).unwrap(), CommitRecoveryDecision::Committed(_)));
    assert_eq!(journal.read_snapshot().unwrap().records, vec![(key(b"lifecycle"), value(b"STOPPED"))]);
    assert_eq!(journal.read_snapshot().unwrap().generation, 2);
}

#[test]
fn ten_thousand_reads_change_neither_database_bytes_nor_receipt_count() {
    let scratch = Scratch::new();
    let mut journal = scratch.create(LIMITS);
    journal.transact(mutation(1, 0, b"READY")).unwrap();
    let before = fs::read(scratch.path()).unwrap();
    let writes = journal.committed_writes();
    for _ in 0..10_000 { assert_eq!(journal.read_snapshot().unwrap().generation, 1); }
    assert_eq!(fs::read(scratch.path()).unwrap(), before);
    assert_eq!(journal.committed_writes(), writes);
    assert_eq!(journal.verify().unwrap().generation, 1);
}

#[test]
fn reopening_checks_every_identity_field_and_never_rebinds_owner() {
    let scratch = Scratch::new();
    drop(scratch.create(LIMITS));
    let base = identity();
    let identities = [
        JournalIdentity { installation_incarnation_id: InstallationIncarnationId::from_bytes([5; 16]), ..base },
        JournalIdentity { data_root_id: DataRootId::from_bytes([5; 16]), ..base },
        JournalIdentity { owner_epoch: OwnerEpoch::new(2).unwrap(), ..base },
        JournalIdentity { path_identity_digest: Blake3Digest32::from_bytes([5; 32]), ..base },
        JournalIdentity { schema_family_digest: Blake3Digest32::from_bytes([5; 32]), ..base },
        JournalIdentity { schema_version: 2, ..base },
    ];
    for other in identities { assert!(PersistentControlJournal::open(scratch.file(), other, LIMITS).is_err()); }
    assert_eq!(scratch.open(LIMITS).identity(), base);
}

#[test]
fn second_database_handle_is_refused() {
    let scratch = Scratch::new();
    let journal = scratch.create(LIMITS);
    assert!(PersistentControlJournal::open(scratch.file(), identity(), LIMITS).is_err());
    assert_eq!(journal.verify().unwrap().generation, 0);
}

#[test]
fn empty_existing_file_is_not_initialized() {
    let scratch = Scratch::new();
    let file = OpenOptions::new().read(true).write(true).create_new(true).open(scratch.path()).unwrap();
    assert!(matches!(PersistentControlJournal::open(file, identity(), LIMITS), Err(ControlError::StoreCorrupt)));
    assert_eq!(fs::metadata(scratch.path()).unwrap().len(), 0);
}

#[test]
fn corrupt_receipt_and_missing_table_are_not_repaired_into_success() {
    for remove_table in [false, true] {
        let scratch = Scratch::new();
        let mut journal = scratch.create(LIMITS);
        journal.transact(mutation(1, 0, b"READY")).unwrap();
        let write = journal.database.begin_write().unwrap();
        if remove_table {
            assert!(write.delete_table(OPERATIONS).unwrap());
        } else {
            let mut table = write.open_table(OPERATIONS).unwrap();
            table.insert([1_u8; 32].as_slice(), b"truncated".as_slice()).unwrap();
        }
        write.commit().unwrap();
        drop(journal);
        assert!(PersistentControlJournal::open(scratch.file(), identity(), LIMITS).is_err());
    }
}

#[test]
fn strict_codecs_reject_trailing_bytes_and_unknown_record_classes() {
    let mut header = Header::empty(identity()).encode();
    header.push(0);
    assert_eq!(Header::decode(&header, identity(), LIMITS), Err(ControlError::StoreCorrupt));
    assert_eq!(decode_value(b"\xffsecret", LIMITS), Err(ControlError::ForbiddenControlPayload));
    assert_eq!(decode_value(b"\x03", LIMITS), Err(ControlError::StoreCorrupt));
    assert_eq!(decode_value(&[], LIMITS), Err(ControlError::StoreCorrupt));
}

#[test]
fn process_exit_before_and_after_commit_reopens_to_exact_atomic_state() {
    for (mode, expected_exit, generation) in [("before", 73, 0), ("after", 74, 1)] {
        let scratch = Scratch::new();
        drop(scratch.create(LIMITS));
        let status = Command::new(std::env::current_exe().unwrap())
            .args(["--ignored", "--exact", "persistent::tests::crash_child", "--nocapture"])
            .env("ELIOT_REDB_CRASH_PATH", scratch.path())
            .env("ELIOT_REDB_CRASH_MODE", mode)
            .status().unwrap();
        assert_eq!(status.code(), Some(expected_exit));
        let mut journal = scratch.open(LIMITS);
        let snapshot = journal.verify().unwrap();
        assert_eq!(snapshot.generation, generation);
        assert_eq!(snapshot.records.len(), usize::from(generation == 1));
        let replay = journal.transact(mutation(1, 0, b"READY")).unwrap();
        assert_eq!(replay.replayed, generation == 1);
        assert_eq!(journal.verify().unwrap().generation, 1);
    }
}

#[test]
#[ignore = "spawned only by process_exit_before_and_after_commit_reopens_to_exact_atomic_state"]
fn crash_child() {
    let Some(path) = std::env::var_os("ELIOT_REDB_CRASH_PATH") else { return; };
    let mode = std::env::var("ELIOT_REDB_CRASH_MODE").unwrap();
    let file = OpenOptions::new().read(true).write(true).open(path).unwrap();
    let mut journal = PersistentControlJournal::open(file, identity(), LIMITS).unwrap();
    let boundary = match mode.as_str() { "before" => Boundary::ExitBeforeCommit, "after" => Boundary::ExitAfterCommit, _ => panic!("invalid child mode") };
    let _ = journal.transact_inner(mutation(1, 0, b"READY"), boundary);
    panic!("fault boundary must terminate without destructors");
}

#[test]
fn receipt_capacity_never_discards_replay_protection() {
    let scratch = Scratch::new();
    let limits = JournalLimits { max_operation_records: 1, ..LIMITS };
    let mut journal = scratch.create(limits);
    let first = mutation(1, 0, b"READY");
    journal.transact(first.clone()).unwrap();
    assert_eq!(journal.transact(mutation(2, 1, b"STOPPED")), Err(ControlError::IdempotencyCapacityExceeded));
    assert!(journal.transact(first).unwrap().replayed);
    assert_eq!(journal.verify().unwrap().generation, 1);
}

#[test]
fn unknown_table_is_rejected_without_silent_schema_adoption() {
    let scratch = Scratch::new();
    let journal = scratch.create(LIMITS);
    let write = journal.database.begin_write().unwrap();
    let extra: TableDefinition<&str, u64> = TableDefinition::new("unapproved.table");
    drop(write.open_table(extra).unwrap());
    write.commit().unwrap();
    drop(journal);
    assert!(matches!(PersistentControlJournal::open(scratch.file(), identity(), LIMITS), Err(ControlError::SchemaMismatch)));
}

#[test]
fn snapshot_publication_consumes_real_current_receipt_and_recovers_after_restart() {
    let scratch = Scratch::new();
    let mut publisher = ControlSnapshotPublisher::new();
    let first = {
        let mut journal = scratch.create(LIMITS);
        assert_eq!(journal.recover_snapshot_publication(&mut publisher).unwrap(), None);
        assert!(publisher.current().is_none());
        let receipt = journal.transact(mutation(1, 0, b"READY")).unwrap();
        let mut forged = receipt.clone();
        forged.changed_keys.clear();
        assert_eq!(journal.publish_committed_snapshot(&forged, &mut publisher), Err(ControlError::SnapshotPublicationFailed));
        assert!(publisher.current().is_none());
        journal.publish_committed_snapshot(&receipt, &mut publisher).unwrap();
        assert_eq!(publisher.current().unwrap().generation, 1);
        journal.transact(mutation(2, 1, b"STOPPED")).unwrap();
        assert_eq!(journal.publish_committed_snapshot(&receipt, &mut publisher), Err(ControlError::SnapshotPublicationFailed));
        receipt
    };
    let journal = scratch.open(LIMITS);
    let recovered = journal.recover_snapshot_publication(&mut publisher).unwrap().unwrap();
    assert_eq!(recovered.generation, 2);
    assert_eq!(publisher.current().unwrap().records, vec![(key(b"lifecycle"), value(b"STOPPED"))]);
    assert_eq!(journal.committed_writes(), 0);
    assert_eq!(first.after_generation, 1);
}

#[test]
fn owner_handoff_preserves_old_operation_recovery_without_rewriting_data() {
    let scratch = Scratch::new();
    let mut journal = scratch.create(LIMITS);
    let request = mutation(1, 0, b"READY");
    journal.transact(request.clone()).unwrap();
    let next = JournalIdentity { owner_epoch: OwnerEpoch::new(2).unwrap(), ..identity() };
    let mut journal = journal.advance_owner(next).unwrap();
    assert_eq!(journal.identity(), next);
    assert_eq!(journal.verify().unwrap().generation, 1);
    assert!(journal.transact(request.clone()).unwrap().replayed);
    let commits = journal.committed_writes();
    let journal = journal.advance_owner(next).unwrap();
    assert_eq!(journal.committed_writes(), commits);
    drop(journal);
    assert!(matches!(PersistentControlJournal::open(scratch.file(), identity(), LIMITS), Err(ControlError::IdentityMismatch)));
    let mut journal = PersistentControlJournal::open(scratch.file(), next, LIMITS).unwrap();
    assert!(matches!(journal.recover_transaction(&request).unwrap(), CommitRecoveryDecision::Committed(_)));
    assert_eq!(journal.verify().unwrap().generation, 1);
}

#[test]
fn owner_handoff_rejects_skipped_epoch_and_changed_root_identity() {
    for next in [
        JournalIdentity { owner_epoch: OwnerEpoch::new(3).unwrap(), ..identity() },
        JournalIdentity { owner_epoch: OwnerEpoch::new(2).unwrap(), data_root_id: DataRootId::from_bytes([7; 16]), ..identity() },
    ] {
        let scratch = Scratch::new();
        let journal = scratch.create(LIMITS);
        assert!(matches!(journal.advance_owner(next), Err(ControlError::IdentityMismatch)));
        assert_eq!(scratch.open(LIMITS).identity(), identity());
    }
}

#[test]
fn canonical_request_sha256_matches_independent_known_answer() {
    let expected = [
        0x10, 0x13, 0x74, 0xff, 0x94, 0xcc, 0xbd, 0x0f, 0x57, 0x1a, 0x94, 0x50, 0xae, 0x90, 0x1f, 0xfd,
        0x28, 0xed, 0x8e, 0x91, 0x1b, 0xbc, 0xf7, 0x21, 0x3c, 0xb7, 0xd7, 0x35, 0x7b, 0x2d, 0xb8, 0xa1,
    ];
    assert_eq!(request_fingerprint(identity(), &mutation(1, 0, b"READY")).unwrap(), expected);
}

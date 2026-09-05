//! Real-file regressions for the touched-key path and exact replay/recovery.
//! Direct database edits below are corruption fixtures, not a product API.

use super::*;
use crate::{
    CommitRecoveryDecision, ControlJournal, ControlRecordClass, ControlValue, ControlWrite,
    JournalIdentity, JournalLimits, MutationId,
};
use search_contracts::{Blake3Digest32, DataRootId, InstallationIncarnationId, OwnerEpoch};
use std::fs::{self, OpenOptions};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static NEXT: AtomicU64 = AtomicU64::new(0);
const LIMITS: JournalLimits = JournalLimits::BASELINE;

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!(
            "eliot-redb-delta-{}-{stamp}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn create(&self, limits: JournalLimits) -> PersistentControlJournal {
        let file = OpenOptions::new().read(true).write(true).create_new(true)
            .open(self.0.join("control.redb")).unwrap();
        PersistentControlJournal::create(file, identity(), limits).unwrap()
    }

    fn reopen(&self, limits: JournalLimits) -> PersistentControlJournal {
        let file = OpenOptions::new().read(true).write(true)
            .open(self.0.join("control.redb")).unwrap();
        PersistentControlJournal::open(file, identity(), limits).unwrap()
    }
}

impl Drop for Scratch {
    fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); }
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

fn key(bytes: &[u8]) -> ControlKey { ControlKey::new(bytes.to_vec(), LIMITS).unwrap() }

fn change(key_bytes: &[u8], bytes: &[u8]) -> ControlWrite {
    ControlWrite {
        key: key(key_bytes),
        value: ControlValue::new(ControlRecordClass::State, bytes.to_vec(), LIMITS).unwrap(),
    }
}

fn request(id: u64, generation: u64, writes: Vec<ControlWrite>, deletes: Vec<ControlKey>) -> ControlMutation {
    let mut identifier = [0_u8; 32];
    identifier[..8].copy_from_slice(&id.to_be_bytes());
    ControlMutation::new(
        MutationId(identifier), Blake3Digest32::from_bytes([9; 32]), generation, writes, deletes,
    )
}

fn put(id: u64, generation: u64, key_bytes: &[u8], value: &[u8]) -> ControlMutation {
    request(id, generation, vec![change(key_bytes, value)], vec![])
}

fn reset_work(journal: &PersistentControlJournal) {
    journal.work.point_reads.store(0, Ordering::Relaxed);
    journal.work.snapshot_reads.store(0, Ordering::Relaxed);
}

fn assert_work(journal: &PersistentControlJournal, lookups: u64) {
    assert_eq!(journal.work.point_reads.load(Ordering::Relaxed), lookups);
    assert_eq!(journal.work.snapshot_reads.load(Ordering::Relaxed), 0);
}

#[test]
fn one_record_update_uses_two_point_reads_and_no_catalog_snapshot() {
    let scratch = Scratch::new();
    let mut journal = scratch.create(LIMITS);
    let writes = (0_u32..1024).map(|id| change(&id.to_be_bytes(), &[0xa5; 64])).collect();
    journal.transact(request(1, 0, writes, vec![])).unwrap();
    reset_work(&journal);
    journal.transact(put(2, 1, &512_u32.to_be_bytes(), b"new")).unwrap();
    assert_work(&journal, 2);
    let snapshot = journal.verify().unwrap();
    assert_eq!(snapshot.records.len(), 1024);
    for (stored_key, value) in &snapshot.records {
        if stored_key == &key(&512_u32.to_be_bytes()) {
            assert_eq!(value.as_bytes(), b"new");
        } else {
            assert_eq!(value.as_bytes(), &[0xa5; 64]);
        }
    }
    drop(journal);
    assert_eq!(scratch.reopen(LIMITS).verify().unwrap(), snapshot);
}

#[test]
fn replacement_at_both_capacity_limits_does_not_count_a_new_record() {
    let scratch = Scratch::new();
    let limits = JournalLimits { max_records: 1, max_total_value_bytes: 512, ..LIMITS };
    let mut journal = scratch.create(limits);
    journal.transact(put(1, 0, b"a", &[1; 512])).unwrap();
    journal.transact(put(2, 1, b"a", &[2; 512])).unwrap();
    let snapshot = journal.verify().unwrap();
    assert_eq!(snapshot.records.len(), 1);
    assert_eq!(snapshot.records[0].1.as_bytes(), &[2; 512]);
}

#[test]
fn shrinking_and_growing_in_one_batch_is_independent_of_write_order() {
    for reverse in [false, true] {
        let scratch = Scratch::new();
        let limits = JournalLimits { max_records: 2, max_total_value_bytes: 1024, ..LIMITS };
        let mut journal = scratch.create(limits);
        journal.transact(request(1, 0, vec![change(b"a", &[1; 512]), change(b"b", &[2; 512])], vec![])).unwrap();
        let mut writes = vec![change(b"a", &[3; 32]), change(b"b", &[4; 992])];
        if reverse { writes.reverse(); }
        reset_work(&journal);
        journal.transact(request(2, 1, writes, vec![])).unwrap();
        assert_work(&journal, 4);
        let snapshot = journal.verify().unwrap();
        assert_eq!(snapshot.records.iter().map(|(_, value)| value.len()).sum::<usize>(), 1024);
        assert_eq!(snapshot.records[0].1.as_bytes(), &[3; 32]);
        assert_eq!(snapshot.records[1].1.as_bytes(), &[4; 992]);
    }
}

#[test]
fn absent_delete_is_idempotent_without_underflowing_record_counts() {
    let scratch = Scratch::new();
    let mut journal = scratch.create(LIMITS);
    let command = request(1, 0, vec![], vec![key(b"absent")]);
    reset_work(&journal);
    journal.transact(command.clone()).unwrap();
    assert_work(&journal, 2);
    assert!(journal.transact(command).unwrap().replayed);
    let snapshot = journal.verify().unwrap();
    assert!(snapshot.records.is_empty());
    assert_eq!(snapshot.generation, 1);
}

#[test]
fn rejected_aggregate_value_growth_persists_neither_records_nor_receipt() {
    let scratch = Scratch::new();
    let limits = JournalLimits { max_total_value_bytes: 512, ..LIMITS };
    let mut journal = scratch.create(limits);
    journal.transact(put(1, 0, b"a", &[1; 128])).unwrap();
    let before = journal.verify().unwrap();
    let commits = journal.committed_writes();
    let command = request(2, 1, vec![change(b"a", &[2; 256]), change(b"b", &[3; 257])], vec![]);
    assert_eq!(journal.transact(command.clone()), Err(ControlError::BudgetExceeded));
    assert!(!journal.requires_recovery());
    assert_eq!(journal.committed_writes(), commits);
    assert_eq!(journal.verify().unwrap(), before);
    assert!(matches!(journal.recover_transaction(&command).unwrap(), CommitRecoveryDecision::NotCommittedRetrySameOperation));
}

#[test]
fn mixed_insert_replace_and_delete_matches_full_reference_state() {
    let scratch = Scratch::new();
    let mut journal = scratch.create(LIMITS);
    let mut reference = ControlJournal::open_or_create(identity(), LIMITS).unwrap();
    let initial = request(1, 0, vec![change(b"a", b"1"), change(b"b", b"22"), change(b"c", b"333")], vec![]);
    assert_eq!(journal.transact(initial.clone()).unwrap(), reference.transact(initial).unwrap());
    let command = request(2, 1, vec![change(b"b", b"new"), change(b"d", b"4444")], vec![key(b"a"), key(b"missing")]);
    reset_work(&journal);
    assert_eq!(journal.transact(command.clone()).unwrap(), reference.transact(command).unwrap());
    assert_work(&journal, 8);
    assert_eq!(journal.verify().unwrap(), reference.read_snapshot().unwrap());
}

#[test]
fn changed_record_class_with_equal_bytes_is_read_back_exactly() {
    let scratch = Scratch::new();
    let mut journal = scratch.create(LIMITS);
    journal.transact(put(1, 0, b"a", b"1")).unwrap();
    let mut update = change(b"a", b"1");
    update.value = ControlValue::new(ControlRecordClass::Revision, b"1".to_vec(), LIMITS).unwrap();
    journal.transact(request(2, 1, vec![update], vec![])).unwrap();
    assert_eq!(journal.verify().unwrap().records[0].1.class(), ControlRecordClass::Revision);
}

fn alter_value(journal: &PersistentControlJournal, key_bytes: &[u8], bytes: &[u8]) {
    let mut write = journal.database.begin_write().unwrap();
    write.set_durability(Durability::Immediate);
    {
        let mut table = write.open_table(RECORDS).unwrap();
        let value = encode_value(&change(key_bytes, bytes).value);
        table.insert(key_bytes, value.as_slice()).unwrap();
    }
    write.commit().unwrap();
}

#[test]
fn current_replay_rejects_equal_length_content_corruption_and_quarantines() {
    let scratch = Scratch::new();
    let mut journal = scratch.create(LIMITS);
    let command = put(1, 0, b"state", b"READY");
    journal.transact(command.clone()).unwrap();
    alter_value(&journal, b"state", b"WRONG");
    // Structural counters still match; only exact request readback detects this.
    assert!(journal.verify().is_ok());
    assert_eq!(journal.transact(command), Err(ControlError::StoreCorrupt));
    assert_eq!(journal.read_snapshot(), Err(ControlError::StoreQuarantined));
}

#[test]
fn lost_ack_recovery_checks_current_values_before_clearing_the_fence() {
    let scratch = Scratch::new();
    let mut journal = scratch.create(LIMITS);
    let command = put(1, 0, b"state", b"READY");
    assert_eq!(journal.transact_inner(command.clone(), Boundary::LostAcknowledgement), Err(ControlError::CommitOutcomeUnknown));
    alter_value(&journal, b"state", b"WRONG");
    assert!(matches!(journal.recover_transaction(&command).unwrap(), CommitRecoveryDecision::PartialOrCorruptQuarantine));
    assert!(journal.requires_recovery());
    assert_eq!(journal.read_snapshot(), Err(ControlError::StoreQuarantined));
}

#[test]
fn lost_ack_recovery_checks_deleted_keys_are_actually_absent() {
    let scratch = Scratch::new();
    let mut journal = scratch.create(LIMITS);
    journal.transact(request(1, 0, vec![change(b"a", b"AAAAA"), change(b"b", b"BBBBB")], vec![])).unwrap();
    let command = request(2, 1, vec![], vec![key(b"a")]);
    assert_eq!(journal.transact_inner(command.clone(), Boundary::LostAcknowledgement), Err(ControlError::CommitOutcomeUnknown));
    let write = journal.database.begin_write().unwrap();
    {
        let mut table = write.open_table(RECORDS).unwrap();
        table.remove(b"b".as_slice()).unwrap();
        let value = encode_value(&change(b"a", b"AAAAA").value);
        table.insert(b"a".as_slice(), value.as_slice()).unwrap();
    }
    write.commit().unwrap();
    // Both row count and total value length remain valid.
    assert!(matches!(journal.recover_transaction(&command).unwrap(), CommitRecoveryDecision::PartialOrCorruptQuarantine));
    assert_eq!(journal.read_snapshot(), Err(ControlError::StoreQuarantined));
}

#[test]
fn historical_replay_does_not_compare_or_restore_obsolete_values() {
    let scratch = Scratch::new();
    let mut journal = scratch.create(LIMITS);
    let first = put(1, 0, b"state", b"READY");
    journal.transact(first.clone()).unwrap();
    journal.transact(put(2, 1, b"state", b"STOPPED")).unwrap();
    reset_work(&journal);
    assert!(journal.transact(first.clone()).unwrap().replayed);
    assert_work(&journal, 0);
    assert!(matches!(journal.recover_transaction(&first).unwrap(), CommitRecoveryDecision::Committed(_)));
    assert_eq!(journal.verify().unwrap().records[0].1.as_bytes(), b"STOPPED");
}

#[test]
fn inconsistent_receipt_binding_cannot_return_replayed_success() {
    let scratch = Scratch::new();
    let mut journal = scratch.create(LIMITS);
    let command = put(1, 0, b"state", b"READY");
    journal.transact(command.clone()).unwrap();
    let read = journal.database.begin_read().unwrap();
    let header = journal.header_from(&read).unwrap();
    let mut stored = operation_from(&read, command.id(), &header, LIMITS).unwrap().unwrap();
    drop(read);
    stored.receipt.command_digest = Blake3Digest32::from_bytes([7; 32]);
    let bytes = stored.encode(LIMITS).unwrap();
    let write = journal.database.begin_write().unwrap();
    {
        let mut table = write.open_table(OPERATIONS).unwrap();
        table.insert(command.id().0.as_slice(), bytes.as_slice()).unwrap();
    }
    write.commit().unwrap();
    assert_eq!(journal.transact(command), Err(ControlError::StoreCorrupt));
    assert_eq!(journal.read_snapshot(), Err(ControlError::StoreQuarantined));
}

#[test]
fn invalid_old_record_cannot_be_overwritten_as_silent_repair() {
    let scratch = Scratch::new();
    let mut journal = scratch.create(LIMITS);
    journal.transact(put(1, 0, b"state", b"READY")).unwrap();
    let write = journal.database.begin_write().unwrap();
    {
        let mut table = write.open_table(RECORDS).unwrap();
        table.insert(b"state".as_slice(), b"\xffREADY".as_slice()).unwrap();
    }
    write.commit().unwrap();
    let commits = journal.committed_writes();
    assert_eq!(journal.transact(put(2, 1, b"state", b"FIXED")), Err(ControlError::ForbiddenControlPayload));
    assert_eq!(journal.committed_writes(), commits);
    assert_eq!(journal.read_snapshot(), Err(ControlError::StoreQuarantined));
}

#[test]
fn impossible_old_byte_counter_is_rejected_without_saturating_or_repairing() {
    let scratch = Scratch::new();
    let mut journal = scratch.create(LIMITS);
    journal.transact(put(1, 0, b"state", b"READY")).unwrap();
    let read = journal.database.begin_read().unwrap();
    let mut header = journal.header_from(&read).unwrap();
    drop(read);
    header.value_bytes = 0;
    let write = journal.database.begin_write().unwrap();
    {
        let mut table = write.open_table(META).unwrap();
        let encoded = header.encode();
        table.insert("header", encoded.as_slice()).unwrap();
    }
    write.commit().unwrap();
    let commits = journal.committed_writes();
    assert_eq!(journal.transact(put(2, 1, b"state", b"FIXED")), Err(ControlError::StoreCorrupt));
    assert_eq!(journal.committed_writes(), commits);
    assert_eq!(journal.read_snapshot(), Err(ControlError::StoreQuarantined));
}

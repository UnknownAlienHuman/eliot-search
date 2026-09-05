use super::*;
use crate::{ControlMutation, ControlRecordClass, ControlWrite, JournalLimits, PersistentControlJournal};
use search_contracts::{Blake3Digest32, DataRootId, InstallationIncarnationId, OwnerEpoch};
use std::fs::{self, OpenOptions};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const LIMITS: JournalLimits = JournalLimits::BASELINE;

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

fn mutation(id: u8, generation: u64, state: &[u8]) -> ControlMutation {
    ControlMutation::new(MutationId([id; 32]), Blake3Digest32::from_bytes([9; 32]), generation,
        vec![ControlWrite {
            key: ControlKey::new(b"state".to_vec(), LIMITS).unwrap(),
            value: ControlValue::new(ControlRecordClass::State, state.to_vec(), LIMITS).unwrap(),
        }], vec![])
}

fn model(identity: JournalIdentity) -> ControlJournal {
    ControlJournal::open_or_create(identity, LIMITS).unwrap()
}

fn snapshot(journal: &ControlJournal) -> ControlSnapshot {
    crate::rebuild_control_snapshot(journal.read_snapshot().unwrap(), journal.identity(), LIMITS).unwrap()
}

fn populated() -> (ControlJournal, ControlCommitReceipt, ControlSnapshotPublisher) {
    let mut journal = model(identity());
    let receipt = journal.transact(mutation(1, 0, b"READY")).unwrap();
    let mut publisher = ControlSnapshotPublisher::new();
    publisher.publish_snapshot_after_commit(&receipt, snapshot(&journal)).unwrap();
    (journal, receipt, publisher)
}

fn unchanged(publisher: &ControlSnapshotPublisher, previous: &Arc<ControlSnapshot>) {
    assert!(Arc::ptr_eq(previous, &publisher.current().unwrap()));
}

#[test]
fn delayed_publication_cannot_roll_back_a_newer_generation() {
    let (mut journal, first, mut publisher) = populated();
    let old = snapshot(&journal);
    let second = journal.transact(mutation(2, 1, b"STOPPED")).unwrap();
    publisher.publish_snapshot_after_commit(&second, snapshot(&journal)).unwrap();
    let current = publisher.current().unwrap();
    assert_eq!(publisher.publish_snapshot_after_commit(&first, old), Err(ControlError::SnapshotPublicationFailed));
    unchanged(&publisher, &current);
}

#[test]
fn equal_generation_requires_identical_content_and_operation() {
    let (journal, receipt, mut publisher) = populated();
    let current = publisher.current().unwrap();
    let mut contradictory = snapshot(&journal);
    contradictory.records[0].1 = ControlValue::new(ControlRecordClass::State, b"STOPPED".to_vec(), LIMITS).unwrap();
    assert_eq!(publisher.publish_snapshot_after_commit(&receipt, contradictory), Err(ControlError::SnapshotPublicationFailed));
    unchanged(&publisher, &current);
    let mut wrong_operation = receipt.clone();
    wrong_operation.operation_id = MutationId([77; 32]);
    assert_eq!(publisher.publish_snapshot_after_commit(&wrong_operation, snapshot(&journal)), Err(ControlError::SnapshotPublicationFailed));
    unchanged(&publisher, &current);
    // An exact replay remains permitted.
    let mut replay = receipt;
    replay.replayed = true;
    assert!(publisher.publish_snapshot_after_commit(&replay, snapshot(&journal)).is_ok());
}

#[test]
fn immutable_identity_fields_cannot_be_substituted() {
    let (journal, receipt, mut publisher) = populated();
    let base = identity();
    for foreign in [
        JournalIdentity { installation_incarnation_id: InstallationIncarnationId::from_bytes([7; 16]), ..base },
        JournalIdentity { data_root_id: DataRootId::from_bytes([7; 16]), ..base },
        JournalIdentity { path_identity_digest: Blake3Digest32::from_bytes([7; 32]), ..base },
        JournalIdentity { schema_family_digest: Blake3Digest32::from_bytes([7; 32]), ..base },
        JournalIdentity { schema_version: 2, ..base },
    ] {
        let previous = publisher.current().unwrap();
        let mut changed = snapshot(&journal);
        changed.identity = foreign;
        assert_eq!(publisher.publish_snapshot_after_commit(&receipt, changed), Err(ControlError::IdentityMismatch));
        unchanged(&publisher, &previous);
    }
}

#[test]
fn a_larger_generation_cannot_reinstate_an_older_owner() {
    let next = JournalIdentity { owner_epoch: OwnerEpoch::new(2).unwrap(), ..identity() };
    let mut journal = model(next);
    let first = journal.transact(mutation(1, 0, b"READY")).unwrap();
    let mut publisher = ControlSnapshotPublisher::new();
    publisher.publish_snapshot_after_commit(&first, snapshot(&journal)).unwrap();
    let second = journal.transact(mutation(2, 1, b"STOPPED")).unwrap();
    let mut stale = snapshot(&journal);
    stale.identity = identity();
    let before = publisher.current().unwrap();
    assert_eq!(publisher.publish_snapshot_after_commit(&second, stale), Err(ControlError::SnapshotPublicationFailed));
    unchanged(&publisher, &before);
}

#[test]
fn malformed_receipts_and_record_order_do_not_publish() {
    let (journal, receipt, _) = populated();
    let mut publisher = ControlSnapshotPublisher::new();
    let mut malformed = receipt.clone();
    malformed.before_generation = u64::MAX;
    assert!(publisher.publish_snapshot_after_commit(&malformed, snapshot(&journal)).is_err());
    malformed = receipt.clone();
    malformed.changed_keys.clear();
    assert!(publisher.publish_snapshot_after_commit(&malformed, snapshot(&journal)).is_err());
    malformed = receipt.clone();
    malformed.changed_keys.push(malformed.changed_keys[0].clone());
    assert!(publisher.publish_snapshot_after_commit(&malformed, snapshot(&journal)).is_err());
    let mut duplicate = snapshot(&journal);
    duplicate.records.push(duplicate.records[0].clone());
    assert!(publisher.publish_snapshot_after_commit(&receipt, duplicate).is_err());
    assert!(publisher.current().is_none());
}

#[test]
fn recovery_cannot_reset_a_populated_publisher_to_an_empty_old_journal() {
    let (journal, _, mut publisher) = populated();
    let current = publisher.current().unwrap();
    assert_eq!(publisher.recover_snapshot_publication(&model(identity())), Err(ControlError::SnapshotPublicationFailed));
    unchanged(&publisher, &current);
    assert!(publisher.recover_snapshot_publication(&journal).is_ok());
    assert_eq!(publisher.current().unwrap().generation, 1);
}

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("eliot-snapshot-fence-{}-{stamp}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn create(&self, identity: JournalIdentity) -> PersistentControlJournal {
        let file = OpenOptions::new().read(true).write(true).create_new(true)
            .open(self.0.join("control.redb")).unwrap();
        PersistentControlJournal::create(file, identity, LIMITS).unwrap()
    }
}
impl Drop for Scratch {
    fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); }
}

#[test]
fn actual_redb_publication_rejects_another_root_even_with_a_real_commit() {
    let first_root = Scratch::new();
    let second_root = Scratch::new();
    let mut first = first_root.create(identity());
    let second_identity = JournalIdentity { data_root_id: DataRootId::from_bytes([7; 16]), ..identity() };
    let mut second = second_root.create(second_identity);
    let first_receipt = first.transact(mutation(1, 0, b"READY")).unwrap();
    let second_receipt = second.transact(mutation(1, 0, b"READY")).unwrap();
    let mut publisher = ControlSnapshotPublisher::new();
    first.publish_committed_snapshot(&first_receipt, &mut publisher).unwrap();
    let current = publisher.current().unwrap();
    assert_eq!(second.publish_committed_snapshot(&second_receipt, &mut publisher), Err(ControlError::IdentityMismatch));
    assert_eq!(second.recover_snapshot_publication(&mut publisher), Err(ControlError::IdentityMismatch));
    unchanged(&publisher, &current);
}

#[test]
fn actual_redb_owner_handoff_keeps_data_generation_and_replay_valid() {
    let directory = Scratch::new();
    let mut journal = directory.create(identity());
    let receipt = journal.transact(mutation(1, 0, b"READY")).unwrap();
    let mut publisher = ControlSnapshotPublisher::new();
    journal.publish_committed_snapshot(&receipt, &mut publisher).unwrap();
    let next = JournalIdentity { owner_epoch: OwnerEpoch::new(2).unwrap(), ..identity() };
    let journal = journal.advance_owner(next).unwrap();
    journal.recover_snapshot_publication(&mut publisher).unwrap();
    assert_eq!(publisher.current().unwrap().identity, next);
    assert_eq!(publisher.current().unwrap().generation, 1);
    journal.publish_committed_snapshot(&receipt, &mut publisher).unwrap();
}

use super::*;
use search_contracts::{DataRootId, Epoch, InstallationIncarnationId, OpaqueRef,
    OwnerEpoch, PublicationGuards, PublicationIntentId, ReceiptRef, RequestId};
use search_ports::PackageOpaque;
use std::cell::RefCell;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

mod checkpoint;

const LIMITS: JournalLimits = JournalLimits::BASELINE;
static NEXT: AtomicU64 = AtomicU64::new(0);

fn identity() -> JournalIdentity {
    JournalIdentity { installation_incarnation_id: InstallationIncarnationId::from_bytes([1; 16]),
        data_root_id: DataRootId::from_bytes([2; 16]), owner_epoch: OwnerEpoch::new(1).unwrap(),
        path_identity_digest: Blake3Digest32::from_bytes([3; 32]),
        schema_family_digest: Blake3Digest32::from_bytes([4; 32]), schema_version: PUBLICATION_INTENT_SCHEMA_VERSION }
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
        let root = std::env::temp_dir().join(format!("eliot-intent-{}-{stamp}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&root).unwrap();
        Self { root, handle: RefCell::new(None) }
    }
    fn path(&self) -> PathBuf { self.root.join("control.redb") }
    fn file(&self) -> File { OpenOptions::new().read(true).write(true).open(self.path()).unwrap() }
    fn create(&self) -> PersistentControlJournal {
        let file = OpenOptions::new().read(true).write(true).create_new(true).open(self.path()).unwrap();
        self.keep(&file);
        PersistentControlJournal::create(file, identity(), LIMITS).unwrap()
    }
    fn reopen(&self) -> PersistentControlJournal {
        PersistentControlJournal::open(self.file(), identity(), LIMITS).unwrap()
    }
}
impl Drop for Scratch { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.root); } }

#[derive(Debug)]
struct Cancel(bool);
impl PackageOpaque for Cancel {
    fn owner_package(&self) -> &'static str { "search-control-redb" }
}
impl CancellationProbe for Cancel { fn is_cancelled(&self) -> bool { self.0 } }
fn context(cancel: bool) -> OperationContext<Cancel> {
    OperationContext::new(RequestId::from_bytes([1; 16]), 60_000, Cancel(cancel),
        OpaqueRef::new("budget:intent-test").unwrap()).unwrap()
}
fn prepared() -> PublicationIntent {
    PublicationIntent { publication_intent_id: PublicationIntentId::from_bytes([5; 16]),
        target_epoch: Epoch::new(1).unwrap(), prepared_manifest_ref: ReceiptRef::new("receipt:manifest-sentinel").unwrap(),
        owner_source_membership_access_guards: PublicationGuards {
            owner_epoch: OwnerEpoch::new(1).unwrap(), source_catalog_generation: 11,
            membership_generation: 12, access_generation: 13, shadow_generation: 14,
            purge_generation: 15, profile_digest: Blake3Digest32::from_bytes([6; 32]) },
        state: PublicationIntentState::Prepared }
}
fn begin() -> PublicationIntentUpdate {
    PublicationIntentUpdate::begin(MutationId([1; 32]), Blake3Digest32::from_bytes([9; 32]), 0, &prepared()).unwrap()
}
fn advance(id: u8, generation: u64, previous: &PublicationIntent, state: PublicationIntentState) -> PublicationIntentUpdate {
    PublicationIntentUpdate::advance(MutationId([id; 32]), Blake3Digest32::from_bytes([9; 32]),
        generation, previous.clone(), state).unwrap()
}
fn persist(journal: &mut PersistentControlJournal, update: &PublicationIntentUpdate) -> ControlCommitReceipt {
    journal.persist_publication_intent(update, &context(false)).unwrap()
}

#[test]
fn all_shared_fields_and_state_tags_round_trip_exactly() {
    for &state in PublicationIntentState::ALL {
        let mut intent = prepared();
        intent.state = state;
        let value = codec::encode(&intent, LIMITS).unwrap();
        assert_eq!(codec::decode(&value).unwrap(), intent);
        assert_eq!(codec::encode(&codec::decode(&value).unwrap(), LIMITS).unwrap(), value);
    }
    let intent = prepared();
    let value = codec::encode(&intent, LIMITS).unwrap();
    // Independent fixed offsets: magic(8), ID(16), epoch(8), state(1), six u64s,
    // digest(32), reference byte length(2), then the exact unnormalized reference.
    assert_eq!(&value.as_bytes()[..8], b"ELIPUB01");
    assert_eq!(&value.as_bytes()[24..32], &1_i64.to_be_bytes());
    for (offset, counter) in [1_u64, 11, 12, 13, 14, 15].into_iter().enumerate() {
        assert_eq!(&value.as_bytes()[33 + offset * 8..41 + offset * 8], &counter.to_be_bytes());
    }
    assert_eq!(&value.as_bytes()[81..113], &[6; 32]);
    assert_eq!(&value.as_bytes()[115..], intent.prepared_manifest_ref.as_str().as_bytes());
}

#[test]
fn truncation_trailing_bytes_bad_tags_epochs_owners_and_references_are_not_defaulted() {
    let bytes = codec::encode(&prepared(), LIMITS).unwrap().as_bytes().to_vec();
    let mut invalid = (1..bytes.len()).map(|length| bytes[..length].to_vec()).collect::<Vec<_>>();
    let mut trailing = bytes.clone(); trailing.push(0); invalid.push(trailing);
    for (offset, value) in [(0, 0), (7, b'2'), (32, 0), (32, 255), (113, 255), (115, 255)] {
        let mut changed = bytes.clone(); changed[offset] = value; invalid.push(changed);
    }
    for epoch in [-1_i64, 0, i64::MAX] {
        let mut changed = bytes.clone(); changed[24..32].copy_from_slice(&epoch.to_be_bytes()); invalid.push(changed);
    }
    let mut zero_owner = bytes.clone(); zero_owner[33..41].fill(0); invalid.push(zero_owner);
    for bytes in invalid {
        let value = ControlValue::new(ControlRecordClass::Operation, bytes, LIMITS).unwrap();
        assert_eq!(codec::decode(&value), Err(ControlError::StoreCorrupt));
    }
    let wrong_class = ControlValue::new(ControlRecordClass::State, bytes, LIMITS).unwrap();
    assert_eq!(codec::decode(&wrong_class), Err(ControlError::StoreCorrupt));
}

#[test]
fn first_intent_reopens_with_exact_guards_and_does_not_change_visibility() {
    let scratch = Scratch::new();
    let update = begin();
    {
        let mut journal = scratch.create();
        assert!(journal.load_unresolved_publication(&context(false)).unwrap().is_none());
        persist(&mut journal, &update);
        assert_eq!(journal.verify().unwrap().records.len(), 1);
        assert_eq!(journal.control_snapshot(), Err(ControlError::SnapshotRebuildFailed));
    }
    let journal = scratch.reopen();
    let head = journal.read_publication_intent(&context(false)).unwrap();
    assert_eq!(head.identity, identity());
    assert_eq!(head.generation, 1);
    assert_eq!(head.intent.as_ref(), Some(update.intent()));
    assert_eq!(journal.load_unresolved_publication(&context(false)).unwrap().as_ref(), Some(update.intent()));
}

#[test]
fn exact_replay_has_no_write_and_second_active_intent_cannot_replace_the_first() {
    let scratch = Scratch::new();
    let mut journal = scratch.create();
    let update = begin();
    persist(&mut journal, &update);
    let writes = journal.committed_writes();
    assert!(persist(&mut journal, &update).replayed);
    assert_eq!(journal.committed_writes(), writes);
    let mut next = prepared(); next.publication_intent_id = PublicationIntentId::from_bytes([7; 16]);
    next.target_epoch = Epoch::new(2).unwrap();
    let competing = PublicationIntentUpdate::begin(MutationId([2; 32]), Blake3Digest32::from_bytes([9; 32]), 1, &next).unwrap();
    assert_eq!(journal.persist_publication_intent(&competing, &context(false)).unwrap_err().control_error(),
        ControlError::GenerationMismatch);
    assert_eq!(journal.read_publication_intent(&context(false)).unwrap().intent.as_ref(), Some(update.intent()));
}

#[test]
fn ordinary_state_updates_cannot_commit_visibility_or_skip_required_readback() {
    let current = begin();
    for state in [PublicationIntentState::ReadbackVerified, PublicationIntentState::ControlCommitted,
        PublicationIntentState::Reclaimable, PublicationIntentState::InvalidationOnlyCommitted] {
        assert!(PublicationIntentUpdate::advance(MutationId([2; 32]), Blake3Digest32::from_bytes([9; 32]),
            1, current.intent().clone(), state).is_err());
    }
    let scratch = Scratch::new();
    let mut journal = scratch.create();
    persist(&mut journal, &current);
    let mut intent = current.intent().clone();
    for (n, state) in [PublicationIntentState::NewPointsAcknowledged,
        PublicationIntentState::OldPointsClosedAcknowledged, PublicationIntentState::ReadbackVerified].into_iter().enumerate() {
        let generation = u64::try_from(n + 1).unwrap();
        let update = advance(u8::try_from(n + 2).unwrap(), generation, &intent, state);
        persist(&mut journal, &update);
        intent = update.intent().clone();
    }
    assert_eq!(intent.owner_source_membership_access_guards, prepared().owner_source_membership_access_guards);
    assert_eq!(journal.control_snapshot(), Err(ControlError::SnapshotRebuildFailed));
    assert!(PublicationIntentUpdate::advance(MutationId([5; 32]), Blake3Digest32::from_bytes([9; 32]),
        4, intent, PublicationIntentState::ControlCommitted).is_err());
}

#[test]
fn changed_prepared_fields_cannot_pass_the_exact_previous_intent_guard() {
    let scratch = Scratch::new();
    let mut journal = scratch.create();
    let current = begin(); persist(&mut journal, &current);
    for axis in 0..8 {
        let mut expected = current.intent().clone();
        let guards = &mut expected.owner_source_membership_access_guards;
        match axis {
            0 => guards.source_catalog_generation += 1,
            1 => guards.membership_generation += 1,
            2 => guards.access_generation += 1,
            3 => guards.shadow_generation += 1,
            4 => guards.purge_generation += 1,
            5 => guards.profile_digest = Blake3Digest32::from_bytes([8; 32]),
            6 => expected.prepared_manifest_ref = ReceiptRef::new("receipt:another").unwrap(),
            _ => expected.target_epoch = Epoch::new(2).unwrap(),
        }
        let update = advance(2, 1, &expected, PublicationIntentState::NewPointsAcknowledged);
        assert_eq!(journal.persist_publication_intent(&update, &context(false)).unwrap_err().control_error(),
            ControlError::GenerationMismatch);
        assert_eq!(journal.read_publication_intent(&context(false)).unwrap().generation, 1);
    }
}

#[test]
fn lost_acknowledgement_recovers_exactly_after_reopen_without_repeating_the_write() {
    let scratch = Scratch::new();
    let update = begin();
    {
        let mut journal = scratch.create();
        assert_eq!(journal.persist_intent_checked(&update, Boundary::LostAcknowledgement, &super::super::Unscoped),
            Err(ControlError::CommitOutcomeUnknown));
        assert!(journal.requires_recovery());
        assert!(journal.load_unresolved_publication(&context(false)).is_err());
    }
    let mut journal = scratch.reopen();
    let mut other = prepared(); other.prepared_manifest_ref = ReceiptRef::new("receipt:wrong").unwrap();
    let wrong = PublicationIntentUpdate::begin(MutationId([1; 32]), Blake3Digest32::from_bytes([9; 32]), 0, &other).unwrap();
    assert!(matches!(journal.recover_publication_intent(&wrong, &context(false)).unwrap(), CommitRecoveryDecision::ConflictingInput));
    assert!(matches!(journal.recover_publication_intent(&update, &context(false)).unwrap(), CommitRecoveryDecision::Committed(_)));
    assert_eq!(journal.committed_writes(), 0);
    assert!(persist(&mut journal, &update).replayed);
}

struct StopAt(Point);
impl Check for StopAt {
    fn check(&self, point: Point) -> Result<(), ControlError> {
        if point == self.0 { Err(ControlError::ReadCancelled) } else { Ok(()) }
    }
}

#[test]
fn cancellation_before_dispatch_is_no_write_and_after_staging_needs_exact_recovery() {
    let scratch = Scratch::new();
    let mut journal = scratch.create();
    let update = begin();
    assert!(journal.persist_publication_intent(&update, &context(true)).is_err());
    assert_eq!(journal.verify().unwrap().generation, 0);
    assert_eq!(journal.persist_intent_checked(&update, Boundary::Normal, &StopAt(Point::StageRecord)),
        Err(ControlError::CommitOutcomeUnknown));
    assert!(journal.requires_recovery());
    assert!(journal.recover_publication_intent(&update, &context(true)).is_err());
    assert!(journal.requires_recovery());
    assert!(matches!(journal.recover_publication_intent(&update, &context(false)).unwrap(),
        CommitRecoveryDecision::NotCommittedRetrySameOperation));
    persist(&mut journal, &update);
}

#[test]
fn successor_owner_can_replay_or_compensate_but_not_resume_stale_forward_progress() {
    let scratch = Scratch::new();
    let mut journal = scratch.create();
    let update = begin(); persist(&mut journal, &update);
    let mut journal = journal.advance_owner(JournalIdentity { owner_epoch: OwnerEpoch::new(2).unwrap(), ..identity() }).unwrap();
    assert!(persist(&mut journal, &update).replayed);
    let forward = advance(2, 1, update.intent(), PublicationIntentState::NewPointsAcknowledged);
    assert_eq!(journal.persist_publication_intent(&forward, &context(false)).unwrap_err().control_error(),
        ControlError::GenerationMismatch);
    assert!(!journal.quarantined);
    let compensate = advance(2, 1, update.intent(), PublicationIntentState::Compensating);
    persist(&mut journal, &compensate);
    assert_eq!(journal.load_unresolved_publication(&context(false)).unwrap().unwrap().owner_source_membership_access_guards.owner_epoch,
        OwnerEpoch::new(1).unwrap());
}

#[test]
fn bare_aborted_record_retains_recovery_fence_and_cannot_be_replaced_as_an_empty_slot() {
    let scratch = Scratch::new();
    let mut journal = scratch.create();
    let update = begin(); persist(&mut journal, &update);
    let abort = advance(2, 1, update.intent(), PublicationIntentState::Aborted);
    persist(&mut journal, &abort);
    assert_eq!(journal.load_unresolved_publication(&context(false)).unwrap().as_ref(), Some(abort.intent()));
    assert_eq!(journal.read_publication_intent(&context(false)).unwrap().intent.as_ref(), Some(abort.intent()));
    assert_eq!(journal.control_snapshot(), Err(ControlError::SnapshotRebuildFailed));
    let next = PublicationIntentUpdate::begin(MutationId([3; 32]), Blake3Digest32::from_bytes([9; 32]), 2, &prepared()).unwrap();
    assert!(journal.persist_publication_intent(&next, &context(false)).is_err());
}

#[test]
fn lost_intent_is_corruption_not_no_unresolved_work_or_permission_to_reinitialize() {
    let scratch = Scratch::new();
    let mut journal = scratch.create();
    persist(&mut journal, &begin());
    // Simulate a bad lower-level deletion while retaining coherent table counts.
    journal.transact(&ControlMutation::new(MutationId([2; 32]), Blake3Digest32::from_bytes([9; 32]), 1,
        vec![], vec![ControlKey::new(KEY.to_vec(), LIMITS).unwrap()])).unwrap();
    assert_eq!(journal.load_unresolved_publication(&context(false)).unwrap_err().control_error(), ControlError::StoreCorrupt);
    assert_eq!(journal.read_snapshot(), Err(ControlError::StoreCorrupt));
    let next = PublicationIntentUpdate::begin(MutationId([3; 32]), Blake3Digest32::from_bytes([9; 32]), 2, &prepared()).unwrap();
    assert_eq!(journal.persist_publication_intent(&next, &context(false)).unwrap_err().control_error(), ControlError::StoreCorrupt);
    assert!(journal.quarantined);
}

#[test]
fn damaged_typed_record_blocks_open_and_snapshot_reconstruction() {
    let scratch = Scratch::new();
    let mut journal = scratch.create(); persist(&mut journal, &begin());
    let write = journal.database.begin_write().unwrap();
    {
        let mut table = write.open_table(RECORDS).unwrap();
        let mut bytes = codec::encode(begin().intent(), LIMITS).unwrap().as_bytes().to_vec();
        bytes[32] = 255;
        let value = ControlValue::new(ControlRecordClass::Operation, bytes, LIMITS).unwrap();
        let raw = super::super::encode_value(&value);
        table.insert(KEY, raw.as_slice()).unwrap();
    }
    write.commit().unwrap();
    assert_eq!(journal.verify(), Err(ControlError::StoreCorrupt));
    assert!(journal.load_unresolved_publication(&context(false)).is_err());
    drop(journal);
    assert!(PersistentControlJournal::open(scratch.file(), identity(), LIMITS).is_err());
}

#[test]
fn unresolved_intent_keeps_snapshot_admission_closed_without_replaying_mutation() {
    let scratch = Scratch::new();
    let mut journal = scratch.create();
    let receipt = persist(&mut journal, &begin());
    let mut publisher = crate::ControlSnapshotPublisher::new();
    assert_eq!(journal.publish_committed_snapshot(&receipt, &mut publisher), Err(ControlError::SnapshotRebuildFailed));
    assert!(publisher.current().is_none());
    assert!(publisher.requires_recovery());
    assert_eq!(journal.recover_snapshot_publication(&mut publisher), Err(ControlError::SnapshotRebuildFailed));
    assert_eq!(journal.verify().unwrap().generation, 1);
}

#[test]
fn repeated_typed_reads_are_nonmutating_and_diagnostics_redact_manifest_refs() {
    let scratch = Scratch::new();
    let mut journal = scratch.create(); let update = begin(); persist(&mut journal, &update);
    let disk = scratch.bytes(); let writes = journal.committed_writes();
    let head = journal.read_publication_intent(&context(false)).unwrap();
    for _ in 0..10_000 { assert_eq!(journal.read_publication_intent(&context(false)).unwrap(), head); }
    assert_eq!(scratch.bytes(), disk);
    assert_eq!(journal.committed_writes(), writes);
    assert!(!format!("{update:?} {head:?}").contains("manifest-sentinel"));
    assert!(journal.load_unresolved_publication(&context(true)).is_err());
}

#[test]
fn typed_schema_is_explicit_and_version_one_is_neither_upgraded_nor_quarantined() {
    let scratch = Scratch::new();
    let file = OpenOptions::new().read(true).write(true).create_new(true).open(scratch.path()).unwrap();
    scratch.keep(&file);
    let legacy = JournalIdentity { schema_version: 1, ..identity() };
    let mut journal = PersistentControlJournal::create(file, legacy, LIMITS).unwrap();
    let before = scratch.bytes();
    assert_eq!(journal.persist_publication_intent(&begin(), &context(false)).unwrap_err().control_error(),
        ControlError::SchemaUnsupported);
    assert!(journal.read_publication_intent(&context(false)).is_err());
    assert!(!journal.quarantined);
    assert_eq!(journal.verify().unwrap().generation, 0);
    assert_eq!(scratch.bytes(), before);
    drop(journal);
    assert!(PersistentControlJournal::open(scratch.file(), identity(), LIMITS).is_err());
    assert!(PersistentControlJournal::open(scratch.file(), legacy, LIMITS).is_ok());
}

#[test]
fn schema_two_cannot_be_downgraded_and_unknown_versions_remain_rejected() {
    let scratch = Scratch::new();
    drop(scratch.create());
    assert!(PersistentControlJournal::open(scratch.file(), JournalIdentity { schema_version: 1, ..identity() }, LIMITS).is_err());
    assert!(PersistentControlJournal::open(scratch.file(), JournalIdentity { schema_version: 3, ..identity() }, LIMITS).is_err());
    assert!(scratch.reopen().verify().is_ok());
}

#[test]
fn intent_process_exit_before_and_after_commit_recovers_exactly() {
    use std::process::Command;
    use std::time::{Duration, Instant};
    for (mode, code, committed) in [("before", 73, false), ("after", 74, true)] {
        let scratch = Scratch::new(); drop(scratch.create());
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args(["--ignored", "--exact", "persistent::publication::tests::crash_child", "--nocapture"])
            .env("ELIOT_TYPED_INTENT_CRASH_PATH", scratch.path())
            .env("ELIOT_TYPED_INTENT_CRASH_MODE", mode).spawn().unwrap();
        let deadline = Instant::now() + Duration::from_secs(30);
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() { break status; }
            if Instant::now() >= deadline {
                let _ = child.kill(); let _ = child.wait(); panic!("intent crash fixture timed out");
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        assert_eq!(status.code(), Some(code));
        let mut journal = scratch.reopen();
        assert_eq!(journal.read_publication_intent(&context(false)).unwrap().intent.is_some(), committed);
        let update = begin();
        assert_eq!(persist(&mut journal, &update).replayed, committed);
        assert_eq!(journal.read_publication_intent(&context(false)).unwrap().generation, 1);
    }
}

#[test]
#[ignore = "launched by the bounded intent process-exit fixture"]
fn crash_child() {
    let Some(path) = std::env::var_os("ELIOT_TYPED_INTENT_CRASH_PATH") else { return; };
    let file = OpenOptions::new().read(true).write(true).open(path).unwrap();
    let mut journal = PersistentControlJournal::open(file, identity(), LIMITS).unwrap();
    let boundary = match std::env::var("ELIOT_TYPED_INTENT_CRASH_MODE").unwrap().as_str() {
        "before" => Boundary::ExitBeforeCommit, "after" => Boundary::ExitAfterCommit,
        _ => panic!("invalid intent crash mode"),
    };
    let _ = journal.persist_intent_checked(&begin(), boundary, &super::super::Unscoped);
    panic!("crash boundary was not reached");
}

#[test]
fn one_deadline_covers_typed_validation_and_the_existing_transaction_engine() {
    use std::cell::Cell;
    use std::time::{Duration, Instant};
    let scratch = Scratch::new(); let mut journal = scratch.create();
    let ctx = OperationContext::new(RequestId::from_bytes([1; 16]), 4, Cancel(false),
        OpaqueRef::new("budget:typed-intent-deadline").unwrap()).unwrap();
    let start = Instant::now(); let calls = Cell::new(0_u64);
    let budget = Budget::with_clock(&ctx, || {
        let n = calls.get(); calls.set(n + 1); start + Duration::from_millis(n)
    });
    assert_eq!(journal.persist_intent_checked(&begin(), Boundary::Normal, &budget), Err(ControlError::BudgetExceeded));
    assert!(!journal.requires_recovery());
    assert_eq!(journal.verify().unwrap().generation, 0);
    assert_eq!(journal.read_intent_checked(&StopAt(Point::ReadComplete)), Err(ControlError::ReadCancelled));
}

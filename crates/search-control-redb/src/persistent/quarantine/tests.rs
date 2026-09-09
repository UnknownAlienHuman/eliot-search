use super::*;
use super::record::{MARKER_BYTES, Marker};
use crate::{ControlMutation, ControlRecordClass, ControlValue, ControlWrite};
use super::super::RECORDS;
use super::super::operation::Unscoped;
use search_contracts::{Blake3Digest32, DataRootId, InstallationIncarnationId, OpaqueRef, OwnerEpoch, RequestId};
use search_ports::{PackageOpaque, PortErrorKind};
use std::cell::RefCell;
use std::fs::{self, OpenOptions, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const LIMITS: JournalLimits = JournalLimits::BASELINE;
static NEXT: AtomicU64 = AtomicU64::new(0);
fn identity() -> JournalIdentity {
    JournalIdentity {
        installation_incarnation_id: InstallationIncarnationId::from_bytes([1; 16]),
        data_root_id: DataRootId::from_bytes([2; 16]), owner_epoch: OwnerEpoch::new(1).unwrap(),
        path_identity_digest: Blake3Digest32::from_bytes([3; 32]),
        schema_family_digest: Blake3Digest32::from_bytes([4; 32]), schema_version: 1,
    }
}
fn request(generation: u64) -> ControlQuarantineRequest {
    ControlQuarantineRequest::new(MutationId([42; 32]), generation,
        ControlQuarantineReason::IntegrityContradiction, [5; 32])
}
fn mutation() -> ControlMutation {
    ControlMutation::new(MutationId([7; 32]), Blake3Digest32::from_bytes([8; 32]), 0,
        vec![ControlWrite {
            key: crate::ControlKey::new(b"state".to_vec(), LIMITS).unwrap(),
            value: ControlValue::new(ControlRecordClass::State, b"READY".to_vec(), LIMITS).unwrap(),
        }], vec![])
}
#[derive(Debug)]
struct Cancel(bool);
impl PackageOpaque for Cancel {
    fn owner_package(&self) -> &'static str { "search-control-redb" }
}
impl CancellationProbe for Cancel { fn is_cancelled(&self) -> bool { self.0 } }
fn context(cancelled: bool) -> OperationContext<Cancel> {
    OperationContext::new(RequestId::from_bytes([9; 16]), 10_000, Cancel(cancelled),
        OpaqueRef::new("budget:quarantine-test").unwrap()).unwrap()
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
    /// ERROR_LOCK_VIOLATION. A duplicated handle shares that lock ownership, so
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
        let path = std::env::temp_dir().join(format!("eliot-quarantine-{}-{stamp}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&path).unwrap();
        Self { root: path, handle: RefCell::new(None) }
    }
    fn path(&self) -> PathBuf { self.root.join("control.redb") }
    fn file(&self) -> File { OpenOptions::new().read(true).write(true).open(self.path()).unwrap() }
    fn create(&self) -> PersistentControlJournal {
        let file = OpenOptions::new().create_new(true).read(true).write(true).open(self.path()).unwrap();
        self.keep(&file);
        PersistentControlJournal::create(file, identity(), LIMITS).unwrap()
    }
    fn open(&self) -> Result<PersistentControlJournal, ControlError> {
        PersistentControlJournal::open(self.file(), identity(), LIMITS)
    }
    fn recover(&self, request: &ControlQuarantineRequest) -> Option<ControlQuarantineReceipt> {
        PersistentControlJournal::recover_quarantine_with_context(self.file(), identity(), LIMITS,
            request, &context(false)).unwrap()
    }
}
impl Drop for Scratch { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.root); } }

// Application bytes, not redb's internal free-page/repair bookkeeping.
fn application_bytes(journal: &PersistentControlJournal) -> Vec<Vec<u8>> {
    let read = journal.database.begin_read().unwrap();
    let meta = read.open_table(META).unwrap();
    let mut result = vec![meta.get("header").unwrap().unwrap().value().to_vec()];
    for definition in [RECORDS, OPERATIONS] {
        let table = read.open_table(definition).unwrap();
        for row in table.iter().unwrap() {
            let (key, value) = row.unwrap();
            result.push(key.value().to_vec()); result.push(value.value().to_vec());
        }
    }
    result
}

#[test]
fn durable_quarantine_blocks_all_normal_paths_and_preserves_application_state() {
    let scratch = Scratch::new();
    let mut journal = scratch.create();
    let commit = journal.transact(mutation()).unwrap();
    let mut publisher = ControlSnapshotPublisher::new();
    journal.publish_committed_snapshot(&commit, &mut publisher).unwrap();
    assert!(publisher.current().is_some());
    let before = application_bytes(&journal);
    let writes = journal.committed_writes();
    let receipt = journal.quarantine_with_context(&request(1), &mut publisher, &context(false)).unwrap();
    assert_eq!(receipt.identity(), identity()); assert_eq!(receipt.request(), request(1));
    assert_eq!(journal.committed_writes(), writes + 1);
    assert!(publisher.current().is_none()); assert!(publisher.requires_recovery());
    assert_eq!(journal.read_snapshot(), Err(ControlError::StoreQuarantined));
    assert_eq!(journal.transact(mutation()), Err(ControlError::StoreQuarantined));
    assert_eq!(journal.recover_snapshot_publication(&mut publisher), Err(ControlError::StoreQuarantined));
    assert_eq!(application_bytes(&journal), before);
    drop(journal);
    assert!(matches!(scratch.open(), Err(ControlError::StoreQuarantined)));
    assert_eq!(scratch.recover(&request(1)), Some(receipt));
    // Recovery is diagnostic, never an unquarantine operation.
    assert!(matches!(scratch.open(), Err(ControlError::StoreQuarantined)));
}

#[test]
fn exact_repeat_is_read_only_and_conflicting_request_cannot_replace_the_marker() {
    let scratch = Scratch::new(); let mut journal = scratch.create();
    let mut publisher = ControlSnapshotPublisher::new();
    let original = request(0);
    let receipt = journal.quarantine_with_context(&original, &mut publisher, &context(false)).unwrap();
    let disk = scratch.bytes(); let writes = journal.committed_writes();
    assert_eq!(journal.quarantine_with_context(&original, &mut publisher, &context(false)).unwrap(), receipt);
    for other in [
        request(1),
        ControlQuarantineRequest::new(MutationId([99; 32]), 0, original.reason(), original.observation_sha256()),
        ControlQuarantineRequest::new(original.operation_id(), 0, ControlQuarantineReason::AdministrativeHold, original.observation_sha256()),
        ControlQuarantineRequest::new(original.operation_id(), 0, original.reason(), [6; 32]),
    ] {
        assert_eq!(journal.quarantine_with_context(&other, &mut publisher, &context(false)).unwrap_err().control_error(), ControlError::OperationConflict);
    }
    assert_eq!(journal.committed_writes(), writes); assert_eq!(scratch.bytes(), disk);
}

#[test]
fn pre_cancelled_request_suspends_admission_but_records_no_durable_hold() {
    let scratch = Scratch::new(); let mut journal = scratch.create();
    let mut publisher = ControlSnapshotPublisher::new(); let disk = scratch.bytes();
    let error = journal.quarantine_with_context(&request(0), &mut publisher, &context(true)).unwrap_err();
    assert_eq!(error.kind(), PortErrorKind::CancelledBeforeSideEffect);
    assert!(publisher.requires_recovery()); assert!(journal.quarantined);
    assert_eq!(scratch.bytes(), disk);
    drop(journal); assert_eq!(scratch.recover(&request(0)), None);
}

#[test]
fn stale_generation_never_records_a_hold_against_another_generation() {
    let scratch = Scratch::new(); let mut journal = scratch.create(); journal.transact(mutation()).unwrap();
    let mut publisher = ControlSnapshotPublisher::new();
    let error = journal.quarantine_with_context(&request(0), &mut publisher, &context(false)).unwrap_err();
    assert_eq!(error.control_error(), ControlError::GenerationMismatch);
    assert!(journal.quarantined); assert!(publisher.current().is_none());
    drop(journal); assert_eq!(scratch.recover(&request(0)), None);
}

#[test]
fn quarantine_does_not_overwrite_an_unknown_data_mutation_identity() {
    let scratch = Scratch::new(); let mut journal = scratch.create();
    assert_eq!(journal.transact_inner(mutation(), Boundary::LostAcknowledgement), Err(ControlError::CommitOutcomeUnknown));
    let pending = journal.pending; assert!(pending.is_some());
    let mut publisher = ControlSnapshotPublisher::new();
    journal.quarantine_with_context(&request(1), &mut publisher, &context(false)).unwrap();
    assert_eq!(journal.pending, pending);
    assert!(journal.requires_recovery());
    // A quarantine receipt is not a committed/not-committed data-operation receipt.
    assert!(matches!(journal.recover_transaction(&mutation()).unwrap(), crate::CommitRecoveryDecision::PartialOrCorruptQuarantine));
}

struct StopAt(Point);
impl Check for StopAt {
    fn check(&self, point: Point) -> Result<(), ControlError> {
        if point == self.0 { Err(ControlError::ReadCancelled) } else { Ok(()) }
    }
}
#[test]
fn staging_interruption_and_commit_readback_interruption_require_exact_recovery() {
    for (point, committed) in [(Point::StageRecord, false), (Point::BeforeCommit, false), (Point::AfterCommit, true), (Point::MutationComplete, true)] {
        let scratch = Scratch::new(); let mut journal = scratch.create();
        let mut publisher = ControlSnapshotPublisher::new();
        assert_eq!(journal.quarantine_checked(&request(0), &mut publisher, Boundary::Normal, &StopAt(point)), Err(ControlError::CommitOutcomeUnknown));
        assert!(journal.quarantined); assert!(publisher.requires_recovery());
        drop(journal); assert_eq!(scratch.recover(&request(0)).is_some(), committed);
    }
}

#[test]
fn absent_marker_is_not_a_claim_that_application_records_are_sound() {
    let scratch = Scratch::new(); let journal = scratch.create();
    let write = journal.database.begin_write().unwrap();
    assert!(write.delete_table(RECORDS).unwrap()); write.commit().unwrap();
    drop(journal);
    assert_eq!(scratch.recover(&request(0)), None);
    assert!(scratch.open().is_err());
}

#[test]
fn damaged_records_can_be_quarantined_without_repairing_or_deleting_them() {
    let scratch = Scratch::new(); let mut journal = scratch.create(); journal.transact(mutation()).unwrap();
    let write = journal.database.begin_write().unwrap();
    {
        let mut table = write.open_table(RECORDS).unwrap();
        table.insert(b"state".as_slice(), b"\xffWRONG".as_slice()).unwrap();
    }
    write.commit().unwrap();
    let before = application_bytes(&journal); let mut publisher = ControlSnapshotPublisher::new();
    journal.quarantine_with_context(&request(1), &mut publisher, &context(false)).unwrap();
    assert_eq!(application_bytes(&journal), before);
    drop(journal); assert!(scratch.recover(&request(1)).is_some());
}

#[test]
fn malformed_marker_is_always_denied_and_never_automatically_replaced() {
    for bytes in [vec![], vec![0; MARKER_BYTES], b"truncated".to_vec()] {
        let scratch = Scratch::new(); let mut journal = scratch.create();
        let write = journal.database.begin_write().unwrap();
        { let mut meta = write.open_table(META).unwrap(); meta.insert(MARKER_KEY, bytes.as_slice()).unwrap(); }
        write.commit().unwrap();
        let mut publisher = ControlSnapshotPublisher::new();
        assert_eq!(journal.quarantine_with_context(&request(0), &mut publisher, &context(false)).unwrap_err().control_error(), ControlError::StoreCorrupt);
        drop(journal);
        assert!(matches!(scratch.open(), Err(ControlError::StoreQuarantined)));
        assert_eq!(PersistentControlJournal::recover_quarantine_with_context(scratch.file(), identity(), LIMITS, &request(0), &context(false)).unwrap_err().control_error(), ControlError::StoreCorrupt);
    }
}

#[test]
fn corrupt_header_prevents_durable_receipt_instead_of_guessing_identity() {
    let scratch = Scratch::new(); let mut journal = scratch.create();
    let write = journal.database.begin_write().unwrap();
    { let mut meta = write.open_table(META).unwrap(); meta.insert("header", b"broken".as_slice()).unwrap(); }
    write.commit().unwrap(); let mut publisher = ControlSnapshotPublisher::new();
    assert!(journal.quarantine_with_context(&request(0), &mut publisher, &context(false)).is_err());
    assert!(journal.quarantined); assert!(publisher.requires_recovery());
    let read = journal.database.begin_read().unwrap(); let meta = read.open_table(META).unwrap();
    assert!(meta.get(MARKER_KEY).unwrap().is_none());
}

#[test]
fn owner_handoff_cannot_clear_a_durable_hold() {
    let scratch = Scratch::new(); let mut journal = scratch.create();
    journal.quarantine_with_context(&request(0), &mut ControlSnapshotPublisher::new(), &context(false)).unwrap();
    let next = JournalIdentity { owner_epoch: OwnerEpoch::new(2).unwrap(), ..identity() };
    assert!(matches!(journal.advance_owner(next), Err(ControlError::StoreQuarantined)));
    assert!(scratch.recover(&request(0)).is_some());
    assert_eq!(PersistentControlJournal::recover_quarantine_with_context(scratch.file(), next, LIMITS, &request(0), &context(false)).unwrap_err().control_error(), ControlError::IdentityMismatch);
}

#[test]
fn fixed_marker_does_not_depend_on_live_record_or_receipt_capacity() {
    let scratch = Scratch::new();
    let file = OpenOptions::new().create_new(true).read(true).write(true).open(scratch.path()).unwrap();
    scratch.keep(&file);
    let limits = JournalLimits { max_records: 1, max_operation_records: 1, ..LIMITS };
    let mut journal = PersistentControlJournal::create(file, identity(), limits).unwrap(); journal.transact(mutation()).unwrap();
    journal.quarantine_with_context(&request(1), &mut ControlSnapshotPublisher::new(), &context(false)).unwrap();
    let read = journal.database.begin_read().unwrap();
    assert_eq!(read.open_table(RECORDS).unwrap().len().unwrap(), 1);
    assert_eq!(read.open_table(OPERATIONS).unwrap().len().unwrap(), 1);
    assert_eq!(read.open_table(META).unwrap().len().unwrap(), 2);
}

#[test]
fn recovery_refuses_wrong_request_and_never_creates_an_empty_existing_database() {
    let scratch = Scratch::new(); let mut journal = scratch.create();
    journal.quarantine_with_context(&request(0), &mut ControlSnapshotPublisher::new(), &context(false)).unwrap(); drop(journal);
    assert_eq!(PersistentControlJournal::recover_quarantine_with_context(scratch.file(), identity(), LIMITS, &request(1), &context(false)).unwrap_err().control_error(), ControlError::OperationConflict);
    let empty = Scratch::new(); File::create(empty.path()).unwrap();
    assert_eq!(PersistentControlJournal::recover_quarantine_with_context(empty.file(), identity(), LIMITS, &request(0), &context(false)).unwrap_err().control_error(), ControlError::StoreCorrupt);
    assert_eq!(fs::metadata(empty.path()).unwrap().len(), 0);
}

#[test]
fn recovery_interruption_after_native_open_is_not_side_effect_free_inspection() {
    let scratch = Scratch::new(); let mut journal = scratch.create();
    journal.quarantine_with_context(&request(0), &mut ControlSnapshotPublisher::new(), &context(false)).unwrap(); drop(journal);
    assert_eq!(recover_checked(scratch.file(), identity(), LIMITS, &request(0), &StopAt(Point::AfterOpen)), Err(ControlError::CommitOutcomeUnknown));
    assert!(scratch.recover(&request(0)).is_some());
}

#[test]
fn marker_codec_is_strict_fixed_size_and_bound_to_the_exact_header() {
    let header = Header::empty(identity()).encode(); let marker = Marker::new(request(0), &header); let encoded = marker.encode();
    assert_eq!(encoded.len(), 145); assert!(Marker::decode(&encoded, &header, 0).is_ok());
    for index in 0..encoded.len() {
        let mut bad = encoded; bad[index] ^= 1; assert!(Marker::decode(&bad, &header, 0).is_err());
    }
    let mut trailing = encoded.to_vec(); trailing.push(0);
    assert!(Marker::decode(&trailing, &header, 0).is_err());
    assert!(Marker::decode(&encoded[..144], &header, 0).is_err());
    assert!(Marker::decode(&encoded, b"other-header", 0).is_err());
    assert!(Marker::decode(&encoded, &header, 1).is_err());
    assert!(!format!("{:?}", request(0)).contains("42, 42"));
}

#[test]
fn real_process_exit_before_and_after_quarantine_commit_reopens_fail_closed() {
    for (mode, expected_code, committed) in [("before", 73, false), ("after", 74, true)] {
        let scratch = Scratch::new(); drop(scratch.create());
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args(["--ignored", "--exact", "persistent::quarantine::tests::crash_child"])
            .env("ELIOT_CONTROL_QUARANTINE_TEST_FILE", scratch.path())
            .env("ELIOT_CONTROL_QUARANTINE_TEST_MODE", mode)
            .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::inherit()).spawn().unwrap();
        let deadline = Instant::now() + Duration::from_secs(30);
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() { break status; }
            if Instant::now() >= deadline { let _ = child.kill(); let _ = child.wait(); panic!("quarantine test child exceeded deadline"); }
            std::thread::sleep(Duration::from_millis(10));
        };
        assert_eq!(status.code(), Some(expected_code)); assert_eq!(scratch.recover(&request(0)).is_some(), committed);
        if committed { assert!(matches!(scratch.open(), Err(ControlError::StoreQuarantined))); }
        else { assert_eq!(scratch.open().unwrap().verify().unwrap().generation, 0); }
    }
}

#[test]
#[ignore = "subprocess helper invoked only by real_process_exit_before_and_after_quarantine_commit_reopens_fail_closed"]
fn crash_child() {
    let Some(path) = std::env::var_os("ELIOT_CONTROL_QUARANTINE_TEST_FILE") else { return; };
    let file = OpenOptions::new().read(true).write(true).open(path).unwrap();
    let mut journal = PersistentControlJournal::open(file, identity(), LIMITS).unwrap();
    let boundary = match std::env::var("ELIOT_CONTROL_QUARANTINE_TEST_MODE").unwrap().as_str() {
        "before" => Boundary::ExitBeforeCommit, "after" => Boundary::ExitAfterCommit, _ => panic!("invalid child mode"),
    };
    let result = journal.quarantine_checked(&request(0), &mut ControlSnapshotPublisher::new(), boundary, &Unscoped);
    panic!("expected process exit at a quarantine boundary: {result:?}");
}

#[test]
fn foreign_publisher_cannot_be_poisoned_by_another_journal() {
    let first = Scratch::new(); let mut journal = first.create();
    let second = Scratch::new();
    let file = OpenOptions::new().create_new(true).read(true).write(true).open(second.path()).unwrap();
    let other_identity = JournalIdentity { data_root_id: DataRootId::from_bytes([99; 16]), ..identity() };
    let mut other = PersistentControlJournal::create(file, other_identity, LIMITS).unwrap();
    let commit = other.transact(mutation()).unwrap();
    let mut publisher = ControlSnapshotPublisher::new(); other.publish_committed_snapshot(&commit, &mut publisher).unwrap();
    assert_eq!(journal.quarantine_with_context(&request(0), &mut publisher, &context(false)).unwrap_err().control_error(), ControlError::IdentityMismatch);
    assert!(!publisher.requires_recovery()); assert!(publisher.current().is_some());
    assert!(!journal.quarantined); assert_eq!(journal.verify().unwrap().generation, 0);
}

#[test]
fn marker_wire_bytes_match_an_independent_sha256_known_answer() {
    use sha2::{Digest, Sha256};
    let encoded = Marker::new(request(0), b"header").encode();
    let digest: [u8; 32] = Sha256::digest(encoded).into();
    assert_eq!(digest, [
        0x85, 0xa2, 0x68, 0x24, 0xa8, 0xf4, 0x7b, 0xde,
        0x5b, 0xd6, 0xb7, 0xc1, 0xad, 0x4a, 0xe1, 0x9d,
        0xf3, 0x00, 0xe6, 0xc5, 0x39, 0xe3, 0x4e, 0x5e,
        0x19, 0x8f, 0x22, 0x56, 0x82, 0xf6, 0xc1, 0xbe,
    ]);
}

#[test]
fn data_operation_id_cannot_be_reused_for_an_administrative_hold() {
    let scratch = Scratch::new(); let mut journal = scratch.create();
    let data_command = mutation(); journal.transact(data_command.clone()).unwrap();
    let conflicting = ControlQuarantineRequest::new(data_command.id(), 1,
        ControlQuarantineReason::AdministrativeHold, [5; 32]);
    let error = journal.quarantine_with_context(&conflicting, &mut ControlSnapshotPublisher::new(), &context(false)).unwrap_err();
    assert_eq!(error.control_error(), ControlError::OperationConflict);
    drop(journal); assert_eq!(scratch.recover(&conflicting), None);
}

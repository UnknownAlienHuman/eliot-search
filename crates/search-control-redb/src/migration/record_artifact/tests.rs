use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use super::*;
use crate::migration::{SourceImportOutputArtifactPlatform, SourceImportOutputLockPlatform};

#[derive(Clone, Debug)]
struct TestPlatform {
    identity: Arc<AtomicU64>,
    files: Arc<Mutex<BTreeMap<usize, u64>>>,
    paths: Arc<Mutex<BTreeMap<std::path::PathBuf, u64>>>,
    sync_fail: Arc<AtomicBool>,
    final_fail: Arc<AtomicBool>,
    syncs: Arc<AtomicU64>,
    replace_on_sync: Arc<Mutex<Option<std::path::PathBuf>>>,
}

impl TestPlatform {
    fn new() -> Self {
        Self {
            identity: Arc::new(AtomicU64::new(1)),
            files: Arc::default(),
            paths: Arc::default(),
            sync_fail: Arc::default(),
            final_fail: Arc::default(),
            syncs: Arc::default(),
            replace_on_sync: Arc::default(),
        }
    }

    fn replace_identity(&self) {
        self.identity.store(2, Ordering::SeqCst);
    }

    fn set_identity(&self, path: &Path, identity: u64) {
        self.paths
            .lock()
            .expect("path IDs")
            .insert(path.to_owned(), identity);
    }
}

// These are injected IDs attached to opened fixture handles, not native identity
// qualification. Handles are keys only; timestamps/file-index APIs are not used.
fn handle_key(file: &fs::File) -> usize {
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;
        file.as_raw_handle() as usize
    }
    #[cfg(unix)]
    {
        use std::os::fd::AsRawFd;
        usize::try_from(file.as_raw_fd()).expect("fixture fd")
    }
    #[cfg(not(any(windows, unix)))]
    {
        let _ = file;
        panic!("unsupported fixture platform")
    }
}

impl SourceImportOutputLockPlatform for TestPlatform {
    type Error = io::Error;

    fn validate_directory(&self, path: &Path) -> Result<(), Self::Error> {
        let metadata = fs::symlink_metadata(path)?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(io::Error::other("invalid directory"));
        }
        Ok(())
    }

    fn verify_locator(&self, expected: &fs::File, path: &Path) -> Result<(), Self::Error> {
        let metadata = fs::symlink_metadata(path)?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(io::Error::other("invalid locator"));
        }
        if self.final_fail.load(Ordering::SeqCst)
            && path.file_name() == Some(OsStr::new("artifact.source-map.v1"))
        {
            return Err(io::Error::other("injected final readback refusal"));
        }
        let identity = self
            .paths
            .lock()
            .expect("path IDs")
            .get(path)
            .copied()
            .unwrap_or_else(|| self.identity.load(Ordering::SeqCst));
        // Explicit fault observations; the operation's captured ID never changes.
        // Rebinding also avoids stale synthetic entries when an OS handle is reused.
        self.files
            .lock()
            .expect("file IDs")
            .insert(handle_key(expected), identity);
        Ok(())
    }

    fn sync_directory(&self, _path: &Path) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl SourceImportOutputArtifactPlatform for TestPlatform {
    type Identity = u64;

    fn identity(&self, file: &fs::File) -> Result<Self::Identity, Self::Error> {
        self.files
            .lock()
            .expect("file IDs")
            .get(&handle_key(file))
            .copied()
            .ok_or_else(|| io::Error::other("unbound fixture handle"))
    }
}

impl SourceImportRecordArtifactPlatform for TestPlatform {
    fn sync_record_artifact_directory(&self, _path: &Path) -> Result<(), Self::Error> {
        self.syncs.fetch_add(1, Ordering::SeqCst);
        if let Some(path) = self
            .replace_on_sync
            .lock()
            .expect("sync replacement")
            .take()
        {
            self.set_identity(&path, 2);
        }
        if self.sync_fail.load(Ordering::SeqCst) {
            return Err(io::Error::other("injected directory sync refusal"));
        }
        Ok(())
    }
}

struct Fixture(std::path::PathBuf);

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "eliot-source-import-record-artifact-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&root).expect("create fixture");
        Self(root)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn deadline() -> Instant {
    Instant::now() + Duration::from_secs(5)
}

#[test]
fn buffered_copy_polls_context_and_actual_flush_keeps_post_write_refusal() {
    let fixture = Fixture::new();
    let path = fixture.0.join(".source-map.buffered-copy.tmp");
    let mut artifact = SourceImportRecordArtifact::create(
        &fixture.0, ".source-map.buffered-copy.tmp", TestPlatform::new(), deadline(),
    ).unwrap();
    let polls = std::cell::Cell::new(0);
    let writes = std::cell::Cell::new(0);
    let work = |work| match work {
        Work::Process(0) => { polls.set(polls.get() + 1); Ok(()) }
        Work::Write(_) => { writes.set(writes.get() + 1); Ok(()) }
        Work::Checkpoint => Err(ControlError::ReadCancelled),
        _ => Ok(()),
    };
    artifact.push_with_context(b"alpha\n", deadline(), &work).unwrap();
    assert_eq!(polls.get(), 1);
    assert_eq!(writes.get(), 0);
    assert_eq!(fs::metadata(&path).unwrap().len(), 0);

    // A real flush still refuses after its actual write; it cannot report
    // success or retry the buffered row through Drop after that refusal.
    let failure = artifact.writer.as_mut().unwrap().flush::<io::Error>(deadline(), &work);
    assert!(matches!(failure,
        Err(SourceImportRecordArtifactError::Context(ControlError::ReadCancelled))));
    assert!(writes.get() > 0);
    drop(artifact);
    assert_eq!(fs::read(&path).unwrap(), b"alpha\n");

    let mut refused = SourceImportRecordArtifact::create(
        &fixture.0, ".source-map.refused-copy.tmp", TestPlatform::new(), deadline(),
    ).unwrap();
    let refusal = refused.push_with_context(b"beta\n", deadline(), &|work| {
        if matches!(work, Work::Process(0)) { Err(ControlError::ReadCancelled) } else { Ok(()) }
    });
    assert!(matches!(refusal,
        Err(SourceImportRecordArtifactError::Context(ControlError::ReadCancelled))));
    drop(refused);
    assert_eq!(fs::metadata(fixture.0.join(".source-map.refused-copy.tmp")).unwrap().len(), 0);
}

#[test]
fn contextual_artifact_refused_flush_never_runs_in_drop() {
    let fixture = Fixture::new();
    let mut writer = SourceImportRecordArtifact::create_with_context(
        &fixture.0,
        ".source-map.context.tmp",
        TestPlatform::new(),
        deadline(),
        &INERT,
    )
    .unwrap();
    writer
        .push_with_context(b"alpha\n", deadline(), &INERT)
        .unwrap();
    let failure = writer
        .freeze_with_context(deadline(), &|work| {
            if matches!(work, Work::Write(_)) {
                Err(ControlError::ReadCancelled)
            } else {
                Ok(())
            }
        })
        .err()
        .unwrap();
    assert!(matches!(
        failure,
        SourceImportRecordArtifactError::Context(ControlError::ReadCancelled)
    ));
    assert_eq!(
        fs::metadata(fixture.0.join(".source-map.context.tmp"))
            .unwrap()
            .len(),
        0
    );
}

#[test]
fn contextual_artifact_scan_refusal_stops_before_publication() {
    let fixture = Fixture::new();
    let platform = TestPlatform::new();
    let mut writer = SourceImportRecordArtifact::create(
        &fixture.0,
        ".source-map.context.tmp",
        platform,
        deadline(),
    )
    .unwrap();
    writer.push(b"alpha\n", deadline()).unwrap();
    let mut reader = writer
        .freeze(deadline())
        .unwrap()
        .begin_readback(deadline())
        .unwrap();
    reader.compare(b"alpha\n", deadline()).unwrap();
    let verified = reader.finish(deadline()).unwrap();
    let failure = verified
        .prepare_publication_with_context("artifact.source-map.v1", deadline(), &|work| {
            if matches!(work, Work::Read(bytes) if bytes > 0) {
                Err(ControlError::BudgetExceeded)
            } else {
                Ok(())
            }
        })
        .err()
        .unwrap();
    assert!(matches!(
        failure,
        SourceImportRecordArtifactError::Context(ControlError::BudgetExceeded)
    ));
    assert!(!fixture.0.join("artifact.source-map.v1").exists());
    assert!(fixture.0.join(".source-map.context.tmp").exists());
}

#[test]
fn contextual_artifact_last_prelink_refusal_cannot_dispatch_or_retry() {
    let fixture = Fixture::new();
    let mut operation = prepared(&fixture, TestPlatform::new());
    let failure = operation.publish_once_with_context(deadline(), &|work| {
        if matches!(work, Work::Write(_)) {
            Err(ControlError::ReadCancelled)
        } else {
            Ok(())
        }
    });
    assert_unknown(failure);
    assert!(!fixture.0.join("artifact.source-map.v1").exists());
    assert_eq!(
        operation.disposition(),
        SourceImportRecordArtifactPublicationDisposition::RetainedUnknown
    );
    assert_unknown(operation.publish_once(deadline()));
    assert!(!fixture.0.join("artifact.source-map.v1").exists());
}

#[test]
fn contextual_artifact_postlink_refusal_retains_both_locators() {
    let fixture = Fixture::new();
    let mut operation = prepared(&fixture, TestPlatform::new());
    let staging = operation.staging_name().to_owned();
    assert_unknown(operation.publish_once_with_context(deadline(), &|_| {
        if fixture.0.join("artifact.source-map.v1").exists() {
            Err(ControlError::ReadCancelled)
        } else {
            Ok(())
        }
    }));
    assert!(fixture.0.join(staging).exists());
    assert!(fixture.0.join("artifact.source-map.v1").exists());
    assert_eq!(
        operation.disposition(),
        SourceImportRecordArtifactPublicationDisposition::RetainedUnknown
    );
    assert_unknown(operation.publish_once(deadline()));
}

#[test]
fn contextual_artifact_short_rows_pay_for_buffer_fills_not_each_remaining_window() {
    use std::cell::Cell;
    let fixture = Fixture::new();
    let mut writer = SourceImportRecordArtifact::create(
        &fixture.0,
        ".source-map.short-rows.tmp",
        TestPlatform::new(),
        deadline(),
    )
    .unwrap();
    for _ in 0..1024 {
        writer.push(b"a\n", deadline()).unwrap();
    }
    let frozen = writer.freeze(deadline()).unwrap();
    let read = Cell::new(0_usize);
    let process = Cell::new(0_usize);
    let work = |unit| {
        match unit {
            Work::Read(bytes) => read.set(read.get() + bytes),
            Work::Process(bytes) => process.set(process.get() + bytes),
            _ => {}
        }
        Ok(())
    };
    let mut reader = frozen
        .begin_readback_with_context(deadline(), &work)
        .unwrap();
    for _ in 0..1024 {
        reader
            .compare_with_context(b"a\n", deadline(), &work)
            .unwrap();
    }
    reader.finish_with_context(deadline(), &work).unwrap();
    assert!(
        read.get() <= 3 * BUFFER_BYTES,
        "read reservation {}",
        read.get()
    );
    assert!(
        process.get() <= 8 * BUFFER_BYTES,
        "processing reservation {}",
        process.get()
    );
}

#[test]
fn contextual_artifact_reuse_outcome_survives_postlink_refusal() {
    use std::cell::Cell;
    let fixture = Fixture::new();
    let mut operation = prepared(&fixture, TestPlatform::new());
    fs::hard_link(
        fixture.0.join(operation.staging_name()),
        fixture.0.join(operation.final_name()),
    )
    .unwrap();
    let admitted = Cell::new(false);
    assert_unknown(operation.publish_once_with_context(deadline(), &|work| {
        if matches!(work, Work::Write(0)) {
            admitted.set(true);
            Ok(())
        } else if admitted.get() {
            Err(ControlError::ReadCancelled)
        } else {
            Ok(())
        }
    }));
    let resolved = operation.resolve_same_process(deadline()).unwrap();
    assert!(
        resolved.reused(),
        "an AlreadyExists outcome must not become a new publication"
    );
}

#[cfg(windows)]
#[test]
fn metadata_visible_open_refusal_is_unavailable_not_replaced() {
    use std::os::windows::fs::OpenOptionsExt;

    let fixture = Fixture::new();
    let name = ".source-map.unavailable.tmp";
    let path = fixture.0.join(name);
    let mut held = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .share_mode(0)
        .open(&path)
        .expect("exclusive fixture handle");
    held.write_all(b"alpha\n").expect("row");
    held.sync_all().expect("row sync");
    assert!(fs::symlink_metadata(&path).expect("metadata").is_file());
    assert!(fs::File::open(&path).is_err());

    let mut chain = SourceImportRecordChain::new();
    chain.push(b"alpha\n").expect("record");
    let expected = chain.finish();
    let observed = classify_source_import_record_artifact(
        &TestPlatform::new(),
        &fixture.0,
        name,
        "unavailable.source-map.v1",
        (&expected, 1, 6),
        deadline(),
    );
    assert_eq!(
        observed.state(),
        SourceImportRecordArtifactCurrentState::Unavailable
    );
    assert_eq!(
        observed.resolution(),
        SourceImportRecordArtifactResolution::Unresolved
    );
}

#[test]
fn exact_second_pass_publication_and_final_inspection_are_package_owned() {
    let fixture = Fixture::new();
    let platform = TestPlatform::new();
    let deadline = deadline();
    let mut staging = SourceImportRecordArtifact::create(
        &fixture.0,
        ".source-map.test.tmp",
        platform.clone(),
        deadline,
    )
    .expect("staging");
    for row in [b"alpha\n".as_slice(), b"beta\n".as_slice()] {
        staging.push(row, deadline).expect("write row");
    }
    let frozen = staging.freeze(deadline).expect("freeze");
    let expected_chain = *frozen.chain();
    let expected_bytes = frozen.encoded_bytes();
    let mut readback = frozen.begin_readback(deadline).expect("readback");
    for row in [b"alpha\n".as_slice(), b"beta\n".as_slice()] {
        readback.compare(row, deadline).expect("compare row");
    }
    let verified = readback.finish(deadline).expect("verified");
    let published = verified
        .publish("artifact.source-map.v1", deadline)
        .expect("published");

    assert!(!published.reused());
    assert_eq!(published.chain(), &expected_chain);
    assert_eq!(published.encoded_bytes(), expected_bytes);
    assert_eq!(published.name(), "artifact.source-map.v1");
    assert!(fixture.0.join(".source-map.test.tmp").exists());
    assert_eq!(
        published.cleanup_disposition(),
        SourceImportRecordArtifactCleanupDisposition::DeferredTo347
    );

    let observed = inspect_source_import_record_artifact(
        &platform,
        &fixture.0.join("artifact.source-map.v1"),
        expected_bytes,
        deadline,
    )
    .expect("inspect final");
    assert_eq!(observed.chain(), &expected_chain);
    assert_eq!(observed.encoded_bytes(), expected_bytes);
}

#[test]
fn changed_second_pass_row_fails_before_publication() {
    let fixture = Fixture::new();
    let platform = TestPlatform::new();
    let deadline = deadline();
    let mut staging = SourceImportRecordArtifact::create(
        &fixture.0,
        ".source-map.mismatch.tmp",
        platform,
        deadline,
    )
    .expect("staging");
    staging.push(b"alpha\n", deadline).expect("write");
    let frozen = staging.freeze(deadline).expect("freeze");
    let mut readback = frozen.begin_readback(deadline).expect("readback");
    assert!(matches!(
        readback.compare(b"other\n", deadline),
        Err(SourceImportRecordArtifactError::ReadbackMismatch)
    ));
    assert!(!fixture.0.join("artifact.source-map.v1").exists());
}

#[test]
fn existing_different_final_is_never_overwritten() {
    let fixture = Fixture::new();
    let platform = TestPlatform::new();
    let deadline = deadline();
    fs::write(fixture.0.join("artifact.source-map.v1"), b"other\n").expect("precreate final");
    let mut staging = SourceImportRecordArtifact::create(
        &fixture.0,
        ".source-map.conflict.tmp",
        platform,
        deadline,
    )
    .expect("staging");
    staging.push(b"alpha\n", deadline).expect("write");
    let frozen = staging.freeze(deadline).expect("freeze");
    let mut readback = frozen.begin_readback(deadline).expect("readback");
    readback.compare(b"alpha\n", deadline).expect("compare");
    let verified = readback.finish(deadline).expect("verified");
    assert!(matches!(
        verified.publish("artifact.source-map.v1", deadline),
        Err(SourceImportRecordArtifactError::PublishOutcomeUnknown)
    ));
    assert_eq!(
        fs::read(fixture.0.join("artifact.source-map.v1")).expect("read final"),
        b"other\n"
    );
    assert!(fixture.0.join(".source-map.conflict.tmp").exists());
}

#[test]
fn replaced_temporary_identity_fails_closed() {
    let fixture = Fixture::new();
    let platform = TestPlatform::new();
    let control = platform.clone();
    let deadline = deadline();
    let mut staging = SourceImportRecordArtifact::create(
        &fixture.0,
        ".source-map.identity.tmp",
        platform,
        deadline,
    )
    .expect("staging");
    staging.push(b"alpha\n", deadline).expect("write");
    let frozen = staging.freeze(deadline).expect("freeze");
    control.replace_identity();
    assert!(matches!(
        frozen.begin_readback(deadline),
        Err(SourceImportRecordArtifactError::IdentityChanged)
    ));
}

#[test]
fn nonlocal_temporary_and_final_names_are_rejected() {
    let fixture = Fixture::new();
    let platform = TestPlatform::new();
    let deadline = deadline();
    assert!(matches!(
        SourceImportRecordArtifact::create(&fixture.0, "../escape.tmp", platform, deadline,),
        Err(SourceImportRecordArtifactError::TemporaryNameInvalid)
    ));
}

fn prepared(
    fixture: &Fixture,
    platform: TestPlatform,
) -> SourceImportRecordArtifactPublication<TestPlatform> {
    let mut artifact = SourceImportRecordArtifact::create(
        &fixture.0,
        ".source-map.retained.tmp",
        platform,
        deadline(),
    )
    .expect("create");
    artifact.push(b"alpha\n", deadline()).expect("row");
    let mut replay = artifact
        .freeze(deadline())
        .expect("freeze")
        .begin_readback(deadline())
        .expect("readback");
    replay.compare(b"alpha\n", deadline()).expect("compare");
    replay
        .finish(deadline())
        .expect("verified")
        .prepare_publication("artifact.source-map.v1", deadline())
        .expect("prepare")
}

#[test]
fn verified_rows_replay_original_handle_twice_and_refuse_unknown() {
    let fixture = Fixture::new();
    let mut publication = prepared(&fixture, TestPlatform::new());
    {
        let mut rows = publication.begin_rows_with_context(deadline(), &INERT).expect("verified prepared cursor");
        assert!(rows.next_with_context(deadline(), &INERT).expect("row").is_some());
        assert!(rows.next_with_context(deadline(), &INERT).expect("EOF").is_none());
    }
    publication.publish_once(deadline()).expect("publish");
    for _ in 0..2 {
        let mut rows = publication.begin_rows_with_context(deadline(), &INERT).expect("cursor");
        assert_eq!(rows.next_with_context(deadline(), &INERT).expect("row"), Some(b"alpha\n".as_slice()));
        assert_eq!(rows.next_with_context(deadline(), &INERT).expect("verified EOF"), None);
        assert!(matches!(rows.next_with_context(deadline(), &INERT), Err(SourceImportRecordArtifactError::Closed)));
    }
    publication.disposition = SourceImportRecordArtifactPublicationDisposition::RetainedUnknown;
    assert!(matches!(publication.begin_rows_with_context(deadline(), &INERT), Err(SourceImportRecordArtifactError::Closed)));
}

#[test]
fn verified_rows_context_refusal_is_sticky_with_same_permissive_callback() {
    let fixture = Fixture::new();
    let mut publication = prepared(&fixture, TestPlatform::new());
    publication.publish_once(deadline()).expect("publish");
    let mut rows = publication.begin_rows_with_context(deadline(), &INERT).expect("cursor");
    assert!(matches!(
        rows.next_with_context(deadline(), &|_| Err(ControlError::ReadCancelled)),
        Err(SourceImportRecordArtifactError::Context(ControlError::ReadCancelled))
    ));
    assert!(matches!(rows.next_with_context(deadline(), &INERT), Err(SourceImportRecordArtifactError::Closed)));
}

#[test]
fn verified_rows_mutation_truncation_surplus_and_identity_drift_never_complete() {
    for changed in [b"bravo\n".as_slice(), b"alph".as_slice(), b"alpha\nbeta\n".as_slice()] {
        let fixture = Fixture::new();
        let mut publication = prepared(&fixture, TestPlatform::new());
        publication.publish_once(deadline()).expect("publish");
        let mut rows = publication.begin_rows_with_context(deadline(), &INERT).expect("cursor");
        fs::write(fixture.0.join(".source-map.retained.tmp"), changed).expect("mutate original");
        let mut refused = false;
        for _ in 0..3 {
            match rows.next_with_context(deadline(), &INERT) {
                Ok(Some(_)) => {}
                Ok(None) => panic!("changed original completed"),
                Err(_) => { refused = true; break; }
            }
        }
        assert!(refused);
        assert!(matches!(rows.next_with_context(deadline(), &INERT), Err(SourceImportRecordArtifactError::Closed)));
    }
    let fixture = Fixture::new();
    let platform = TestPlatform::new();
    let mut publication = prepared(&fixture, platform.clone());
    publication.publish_once(deadline()).expect("publish");
    let mut rows = publication.begin_rows_with_context(deadline(), &INERT).expect("cursor");
    assert!(rows.next_with_context(deadline(), &INERT).expect("row").is_some());
    platform.replace_identity();
    assert!(matches!(rows.next_with_context(deadline(), &INERT), Err(SourceImportRecordArtifactError::IdentityChanged)));
}

fn assert_unknown(
    result: Result<SourceImportPublishedRecordArtifact, SourceImportRecordArtifactError<io::Error>>,
) {
    assert!(matches!(
        result,
        Err(SourceImportRecordArtifactError::PublishOutcomeUnknown)
    ));
}

#[test]
fn unclassified_link_error_retains_original_and_never_dispatches_twice() {
    let fixture = Fixture::new();
    let mut operation = prepared(&fixture, TestPlatform::new());
    let dispatches = AtomicU64::new(0);
    assert_unknown(operation.publish_once_using(deadline(), |_, _| {
        dispatches.fetch_add(1, Ordering::SeqCst);
        Err(io::Error::other("unknown link effect"))
    }));
    assert_eq!(
        operation.disposition(),
        SourceImportRecordArtifactPublicationDisposition::RetainedUnknown
    );
    assert_eq!(operation.rows(), 1);
    assert_eq!(operation.encoded_bytes(), 6);
    assert_eq!(
        operation.staging_name(),
        OsStr::new(".source-map.retained.tmp")
    );
    assert_unknown(operation.publish_once_using(deadline(), |_, _| {
        dispatches.fetch_add(1, Ordering::SeqCst);
        panic!("second dispatch")
    }));
    assert_unknown(operation.resolve_same_process(deadline()));
    assert_eq!(dispatches.load(Ordering::SeqCst), 1);
    drop(operation);
    assert_eq!(
        fs::read(fixture.0.join(".source-map.retained.tmp")).expect("retained"),
        b"alpha\n"
    );
    assert!(!fixture.0.join("artifact.source-map.v1").exists());
}

#[test]
fn possible_link_effect_resolves_only_with_original_handle_sync_and_readback() {
    let fixture = Fixture::new();
    let platform = TestPlatform::new();
    let mut operation = prepared(&fixture, platform.clone());
    let original = &operation
        .frozen
        .owned
        .as_ref()
        .expect("owner")
        .original_file;
    assert_eq!(original.metadata().expect("held original").len(), 6);
    assert_unknown(
        operation.publish_once_using(deadline(), |stage, final_path| {
            fs::hard_link(stage, final_path)?;
            Err(io::Error::other("effect completed, outcome unknown"))
        }),
    );
    let receipt = operation
        .resolve_same_process(deadline())
        .expect("resolve with original");
    assert_eq!(
        receipt.cleanup_disposition(),
        SourceImportRecordArtifactCleanupDisposition::DeferredTo347
    );
    assert_eq!(platform.syncs.load(Ordering::SeqCst), 1);
    assert_eq!(
        operation.disposition(),
        SourceImportRecordArtifactPublicationDisposition::VerifiedCleanupDeferred
    );
    assert!(matches!(
        operation.resolve_same_process(deadline()),
        Err(SourceImportRecordArtifactError::Closed)
    ));
    assert!(matches!(
        operation.publish_once(deadline()),
        Err(SourceImportRecordArtifactError::Closed)
    ));
    drop(operation);
    assert!(fixture.0.join(".source-map.retained.tmp").exists());
    assert!(fixture.0.join("artifact.source-map.v1").exists());
}

#[test]
fn link_success_sync_refusal_is_unknown_and_retains_both_objects() {
    let fixture = Fixture::new();
    let platform = TestPlatform::new();
    let mut operation = prepared(&fixture, platform.clone());
    platform.sync_fail.store(true, Ordering::SeqCst);
    assert_unknown(operation.publish_once(deadline()));
    assert_unknown(operation.resolve_same_process(deadline()));
    assert_eq!(
        operation.disposition(),
        SourceImportRecordArtifactPublicationDisposition::RetainedUnknown
    );
    platform.sync_fail.store(false, Ordering::SeqCst);
    operation
        .resolve_same_process(deadline())
        .expect("sync then readback resolves");
    assert_eq!(platform.syncs.load(Ordering::SeqCst), 3);
    drop(operation);
    assert_eq!(
        fs::read(fixture.0.join(".source-map.retained.tmp")).expect("stage"),
        b"alpha\n"
    );
    assert_eq!(
        fs::read(fixture.0.join("artifact.source-map.v1")).expect("final"),
        b"alpha\n"
    );
}

#[test]
fn post_dispatch_readback_and_identity_refusals_retain_unknown() {
    for identity_refusal in [false, true] {
        let fixture = Fixture::new();
        let platform = TestPlatform::new();
        let mut operation = prepared(&fixture, platform.clone());
        if identity_refusal {
            *platform.replace_on_sync.lock().expect("fault") =
                Some(fixture.0.join("artifact.source-map.v1"));
        } else {
            platform.final_fail.store(true, Ordering::SeqCst);
        }
        assert_unknown(operation.publish_once(deadline()));
        drop(operation);
        assert!(fixture.0.join(".source-map.retained.tmp").exists());
        assert!(fixture.0.join("artifact.source-map.v1").exists());
    }
}

#[test]
fn late_deadline_after_dispatch_is_retained_unknown() {
    let fixture = Fixture::new();
    let mut operation = prepared(&fixture, TestPlatform::new());
    let expired_after_dispatch = Instant::now() + Duration::from_millis(20);
    assert_unknown(
        operation.publish_once_using(expired_after_dispatch, |stage, final_path| {
            fs::hard_link(stage, final_path)?;
            std::thread::sleep(expired_after_dispatch.saturating_duration_since(Instant::now()));
            Ok(())
        }),
    );
    drop(operation);
    assert!(fixture.0.join(".source-map.retained.tmp").exists());
    assert!(fixture.0.join("artifact.source-map.v1").exists());
}

#[test]
fn equal_byte_replacements_cannot_complete_original_operation() {
    for replace_final in [false, true] {
        let fixture = Fixture::new();
        let platform = TestPlatform::new();
        let mut operation = prepared(&fixture, platform.clone());
        assert_unknown(
            operation.publish_once_using(deadline(), |stage, final_path| {
                fs::hard_link(stage, final_path)?;
                Err(io::Error::other("unknown"))
            }),
        );
        platform.set_identity(&fixture.0.join(".source-map.retained.tmp"), 2);
        if replace_final {
            platform.set_identity(&fixture.0.join("artifact.source-map.v1"), 2);
        }
        assert_eq!(
            operation.classify_current(deadline()).state(),
            SourceImportRecordArtifactCurrentState::Replaced
        );
        assert_unknown(operation.resolve_same_process(deadline()));
        drop(operation);
        assert_eq!(
            fs::read(fixture.0.join(".source-map.retained.tmp")).expect("stage"),
            b"alpha\n"
        );
        assert_eq!(
            fs::read(fixture.0.join("artifact.source-map.v1")).expect("final"),
            b"alpha\n"
        );
    }
}

#[test]
fn eight_read_only_states_are_always_unresolved_without_original_owner_join() {
    use SourceImportRecordArtifactCurrentState as State;
    let fixture = Fixture::new();
    let platform = TestPlatform::new();
    let mut chain = SourceImportRecordChain::new();
    chain.push(b"alpha\n").expect("row");
    let digest = chain.finish();
    let stage = fixture.0.join(".source-map.observed.tmp");
    let final_path = fixture.0.join("artifact.source-map.v1");
    let classify = || {
        let result = classify_source_import_record_artifact(
            &platform,
            &fixture.0,
            ".source-map.observed.tmp",
            "artifact.source-map.v1",
            (&digest, 1, 6),
            deadline(),
        );
        assert_eq!(
            result.resolution(),
            SourceImportRecordArtifactResolution::Unresolved
        );
        result.state()
    };
    assert_eq!(classify(), State::Missing);
    fs::write(&stage, b"alpha\n").expect("stage");
    assert_eq!(classify(), State::StageExact);
    fs::hard_link(&stage, &final_path).expect("final");
    assert_eq!(classify(), State::BothExactSameObject);
    platform.set_identity(&final_path, 2);
    assert_eq!(classify(), State::BothExactDistinctObjects);
    fs::remove_file(&stage).expect("fixture removes stage");
    assert_eq!(classify(), State::FinalExact);
    fs::write(&stage, b"other\n").expect("conflict");
    assert_eq!(classify(), State::Conflict);
    fs::remove_file(&stage).expect("fixture removes conflict");
    fs::create_dir(&stage).expect("invalid shape");
    assert_eq!(classify(), State::Replaced);
    let result = classify_source_import_record_artifact(
        &platform,
        &fixture.0,
        ".source-map.observed.tmp",
        "artifact.source-map.v1",
        (&digest, 1, 6),
        Instant::now(),
    );
    assert_eq!(result.state(), State::Unavailable);
    assert_eq!(
        result.resolution(),
        SourceImportRecordArtifactResolution::Unresolved
    );
    assert_eq!(
        platform.syncs.load(Ordering::SeqCst),
        0,
        "classifier never syncs/mutates"
    );
    assert_eq!(fs::read(final_path).expect("final preserved"), b"alpha\n");
}

#[test]
fn lost_proof_never_mints_restart_success_or_cleanup() {
    let fixture = Fixture::new();
    let platform = TestPlatform::new();
    let mut operation = prepared(&fixture, platform.clone());
    let chain = *operation.chain();
    assert_unknown(
        operation.publish_once_using(deadline(), |stage, final_path| {
            fs::hard_link(stage, final_path)?;
            Err(io::Error::other("unknown"))
        }),
    );
    drop(operation);
    let current = classify_source_import_record_artifact(
        &platform,
        &fixture.0,
        ".source-map.retained.tmp",
        "artifact.source-map.v1",
        (&chain, 1, 6),
        deadline(),
    );
    assert_eq!(
        current.state(),
        SourceImportRecordArtifactCurrentState::BothExactSameObject
    );
    assert_eq!(
        current.resolution(),
        SourceImportRecordArtifactResolution::Unresolved
    );
    assert_eq!(platform.syncs.load(Ordering::SeqCst), 0);
    assert_eq!(
        fs::read_dir(&fixture.0).expect("directory").count(),
        2,
        "no sidecar"
    );
}

#[test]
fn consuming_publish_sync_failure_drops_only_handles() {
    let fixture = Fixture::new();
    let platform = TestPlatform::new();
    let mut artifact = SourceImportRecordArtifact::create(
        &fixture.0,
        ".source-map.adapter.tmp",
        platform.clone(),
        deadline(),
    )
    .expect("create");
    artifact.push(b"alpha\n", deadline()).expect("row");
    let mut replay = artifact
        .freeze(deadline())
        .expect("freeze")
        .begin_readback(deadline())
        .expect("readback");
    replay.compare(b"alpha\n", deadline()).expect("compare");
    platform.sync_fail.store(true, Ordering::SeqCst);
    assert_unknown(
        replay
            .finish(deadline())
            .expect("verified")
            .publish("artifact.source-map.v1", deadline()),
    );
    assert!(fixture.0.join(".source-map.adapter.tmp").exists());
    assert!(fixture.0.join("artifact.source-map.v1").exists());
}

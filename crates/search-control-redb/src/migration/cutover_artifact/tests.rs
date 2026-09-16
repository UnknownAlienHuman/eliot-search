use std::fs;
use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use search_contracts::{DataRootId, InstallationIncarnationId, SourceNamespaceId};

use super::*;

#[derive(Clone, Copy, Debug)]
struct TestPlatform;

impl SourceImportOutputLockPlatform for TestPlatform {
    type Error = io::Error;

    fn validate_directory(&self, path: &Path) -> Result<(), Self::Error> {
        let metadata = fs::symlink_metadata(path)?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(io::Error::other("invalid directory"));
        }
        Ok(())
    }

    fn verify_locator(
        &self,
        _expected: &fs::File,
        path: &Path,
    ) -> Result<(), Self::Error> {
        let metadata = fs::symlink_metadata(path)?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(io::Error::other("invalid locator"));
        }
        Ok(())
    }

    fn sync_directory(&self, _path: &Path) -> Result<(), Self::Error> {
        Ok(())
    }
}

struct Fixture(std::path::PathBuf);

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "eliot-control-cutover-marker-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&root).expect("create root");
        fs::create_dir(root.join("control")).expect("create control");
        Self(root)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn marker(byte: u8) -> ControlCutoverMarker {
    ControlCutoverMarker {
        target: SourceNamespaceId::from_bytes([byte; 16]),
        incarnation: InstallationIncarnationId::from_bytes([0x11; 16]),
        root: DataRootId::from_bytes([0x22; 16]),
        epoch: 3,
        catalog_snapshot: [0x33; 32],
        plan_chain: [0x44; 32],
        content_chain: [0x55; 32],
        database_file_name: format!("{}.source-map.v2.redb", "66".repeat(32)),
        database_schema: CONTROL_CUTOVER_STAGED_DATABASE_SCHEMA.to_owned(),
    }
}

#[test]
fn exact_publication_and_replay_are_idempotent() {
    let fixture = Fixture::new();
    let bytes = marker(1).encode();
    assert_eq!(
        resolve_control_cutover_marker(&TestPlatform, &fixture.0),
        ControlCutoverMarkerFileState::Absent
    );
    assert_eq!(
        publish_control_cutover_marker(&TestPlatform, &fixture.0, &bytes),
        Ok(ControlCutoverMarkerPublishOutcome::Committed)
    );
    assert_eq!(
        publish_control_cutover_marker(&TestPlatform, &fixture.0, &bytes),
        Ok(ControlCutoverMarkerPublishOutcome::ReplayIdentical)
    );
    match resolve_control_cutover_marker(&TestPlatform, &fixture.0) {
        ControlCutoverMarkerFileState::Valid(committed) => {
            assert_eq!(committed.marker, marker(1));
            assert_eq!(committed.bytes, bytes);
        }
        other => panic!("unexpected marker state: {other:?}"),
    }
    assert!(!fixture
        .0
        .join("control")
        .join(CONTROL_CUTOVER_MARKER_TEMP_FILE)
        .exists());
}

#[test]
fn different_valid_marker_is_never_overwritten() {
    let fixture = Fixture::new();
    let first = marker(1).encode();
    let second = marker(2).encode();
    publish_control_cutover_marker(&TestPlatform, &fixture.0, &first)
        .expect("first marker");
    assert_eq!(
        publish_control_cutover_marker(&TestPlatform, &fixture.0, &second),
        Err(ControlCutoverMarkerArtifactError::AlreadyCommitted)
    );
    assert_eq!(
        fs::read(
            fixture
                .0
                .join("control")
                .join(CONTROL_CUTOVER_MARKER_FILE),
        )
        .expect("read marker"),
        first
    );
}

#[test]
fn corrupt_existing_marker_and_invalid_proposal_fail_closed() {
    let fixture = Fixture::new();
    fs::write(
        fixture
            .0
            .join("control")
            .join(CONTROL_CUTOVER_MARKER_FILE),
        b"torn",
    )
    .expect("write corrupt marker");
    assert_eq!(
        resolve_control_cutover_marker(&TestPlatform, &fixture.0),
        ControlCutoverMarkerFileState::Corrupt
    );
    assert_eq!(
        publish_control_cutover_marker(
            &TestPlatform,
            &fixture.0,
            &marker(1).encode(),
        ),
        Err(ControlCutoverMarkerArtifactError::Corrupt)
    );

    let clean = Fixture::new();
    assert_eq!(
        publish_control_cutover_marker(&TestPlatform, &clean.0, b"invalid"),
        Err(ControlCutoverMarkerArtifactError::CreateFailed)
    );
}

#[test]
fn missing_control_directory_is_absent_for_read_and_invalid_for_write() {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let root = std::env::temp_dir().join(format!(
        "eliot-control-cutover-no-control-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed),
    ));
    fs::create_dir(&root).expect("create root");
    assert_eq!(
        resolve_control_cutover_marker(&TestPlatform, &root),
        ControlCutoverMarkerFileState::Absent
    );
    assert_eq!(
        publish_control_cutover_marker(&TestPlatform, &root, &marker(1).encode()),
        Err(ControlCutoverMarkerArtifactError::CreateFailed)
    );
    fs::remove_dir_all(root).expect("remove root");
}

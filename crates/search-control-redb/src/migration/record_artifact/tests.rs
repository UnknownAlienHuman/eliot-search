use std::fs;
use std::io;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use super::*;
use crate::migration::{
    SourceImportOutputArtifactPlatform, SourceImportOutputLockPlatform,
};

#[derive(Clone, Debug)]
struct TestPlatform {
    identity: Arc<AtomicU64>,
}

impl TestPlatform {
    fn new() -> Self {
        Self {
            identity: Arc::new(AtomicU64::new(1)),
        }
    }

    fn replace_identity(&self) {
        self.identity.store(2, Ordering::SeqCst);
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

impl SourceImportOutputArtifactPlatform for TestPlatform {
    type Identity = u64;

    fn identity(
        &self,
        _file: &fs::File,
    ) -> Result<Self::Identity, Self::Error> {
        Ok(self.identity.load(Ordering::SeqCst))
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
    assert!(!fixture.0.join(".source-map.test.tmp").exists());

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
    fs::write(fixture.0.join("artifact.source-map.v1"), b"other\n")
        .expect("precreate final");
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
        Err(SourceImportRecordArtifactError::ImmutableConflict)
    ));
    assert_eq!(
        fs::read(fixture.0.join("artifact.source-map.v1")).expect("read final"),
        b"other\n"
    );
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
        SourceImportRecordArtifact::create(
            &fixture.0,
            "../escape.tmp",
            platform,
            deadline,
        ),
        Err(SourceImportRecordArtifactError::TemporaryNameInvalid)
    ));
}

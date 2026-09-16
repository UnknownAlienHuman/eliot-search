use std::fs;
use std::io;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use super::*;

#[derive(Clone, Debug)]
struct TestPlatform {
    identity: Arc<AtomicU64>,
    flap: bool,
}

impl TestPlatform {
    fn stable() -> Self {
        Self {
            identity: Arc::new(AtomicU64::new(1)),
            flap: false,
        }
    }

    fn flapping() -> Self {
        Self {
            identity: Arc::new(AtomicU64::new(1)),
            flap: true,
        }
    }
}

impl LegacyPreparationArtifactPlatform for TestPlatform {
    type Identity = u64;
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
        _expected: &File,
        path: &Path,
    ) -> Result<(), Self::Error> {
        let metadata = fs::symlink_metadata(path)?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(io::Error::other("invalid locator"));
        }
        Ok(())
    }

    fn identity(&self, _file: &File) -> Result<Self::Identity, Self::Error> {
        if self.flap {
            Ok(self.identity.fetch_add(1, Ordering::SeqCst))
        } else {
            Ok(self.identity.load(Ordering::SeqCst))
        }
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
            "eliot-preparation-artifact-{}-{}",
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

#[test]
fn object_and_reference_artifacts_publish_read_and_replay_exactly() {
    let fixture = Fixture::new();
    let objects = fixture.0.join("objects");
    let platform = TestPlatform::stable();
    let first = publish_legacy_preparation_artifact(
        &platform,
        &objects,
        "artifact.prep",
        ".artifact.1.tmp",
        b"opaque preparation bytes",
        1024,
    )
    .expect("publish");
    assert!(!first.reused());
    assert_eq!(first.encoded_bytes(), 24);
    assert!(!objects.join(".artifact.1.tmp").exists());

    let observed = read_legacy_preparation_artifact(
        &platform,
        &objects.join("artifact.prep"),
        1024,
    )
    .expect("read");
    assert_eq!(observed.bytes(), b"opaque preparation bytes");

    let replay = publish_legacy_preparation_artifact(
        &platform,
        &objects,
        "artifact.prep",
        ".artifact.2.tmp",
        b"opaque preparation bytes",
        1024,
    )
    .expect("replay");
    assert!(replay.reused());
}

#[test]
fn conflict_preserves_existing_final_and_removes_exact_temp() {
    let fixture = Fixture::new();
    let refs = fixture.0.join("refs");
    fs::create_dir(&refs).expect("create refs");
    fs::write(refs.join("key.ref"), b"existing").expect("precreate ref");
    assert!(matches!(
        publish_legacy_preparation_artifact(
            &TestPlatform::stable(),
            &refs,
            "key.ref",
            ".key.tmp",
            b"different",
            64,
        ),
        Err(LegacyPreparationArtifactError::ImmutableConflict)
    ));
    assert_eq!(fs::read(refs.join("key.ref")).expect("read ref"), b"existing");
    assert!(!refs.join(".key.tmp").exists());
}

#[test]
fn changed_identity_and_oversize_fail_closed() {
    let fixture = Fixture::new();
    let path = fixture.0.join("object");
    fs::write(&path, b"bytes").expect("write object");
    assert!(matches!(
        read_legacy_preparation_artifact(&TestPlatform::flapping(), &path, 16),
        Err(LegacyPreparationArtifactError::IdentityChanged)
    ));
    assert!(matches!(
        read_legacy_preparation_artifact(&TestPlatform::stable(), &path, 4),
        Err(LegacyPreparationArtifactError::SizeInvalid)
    ));
}

#[test]
fn invalid_names_and_size_are_rejected_before_directory_creation() {
    let fixture = Fixture::new();
    let directory = fixture.0.join("objects");
    assert!(matches!(
        publish_legacy_preparation_artifact(
            &TestPlatform::stable(),
            &directory,
            "../escape",
            ".escape.tmp",
            b"x",
            16,
        ),
        Err(LegacyPreparationArtifactError::LocalNameInvalid)
    ));
    assert!(matches!(
        publish_legacy_preparation_artifact(
            &TestPlatform::stable(),
            &directory,
            "object",
            ".object.tmp",
            b"too large",
            2,
        ),
        Err(LegacyPreparationArtifactError::SizeInvalid)
    ));
    assert!(!directory.exists());
}

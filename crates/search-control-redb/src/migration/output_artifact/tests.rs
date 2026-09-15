use std::fs;
use std::io;
use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use super::{
    SourceImportOutputArtifact, SourceImportOutputArtifactError,
    SourceImportOutputArtifactPlatform,
};
use crate::migration::SourceImportOutputLockPlatform;

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
            "eliot-source-import-artifact-{}-{}",
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
fn pending_publication_and_verified_alias_cleanup_are_package_owned() {
    let fixture = Fixture::new();
    let platform = TestPlatform::new();
    let deadline = Instant::now() + Duration::from_secs(5);
    let artifact = SourceImportOutputArtifact::acquire(
        &fixture.0,
        "artifact.redb",
        platform,
        deadline,
    )
    .expect("owner");

    assert!(artifact.open_final(deadline).expect("final state").is_none());
    let pending = artifact
        .open_or_create_pending(deadline)
        .expect("pending");
    assert!(pending.created());
    let pending_identity = *pending.identity();
    let mut file = pending.into_file();
    file.write_all(b"redb").expect("write pending");
    file.sync_all().expect("sync pending");
    drop(file);
    artifact
        .sync_pending_creation(deadline)
        .expect("sync directory");

    let verified = artifact
        .open_pending_matching(&pending_identity, deadline)
        .expect("pending identity");
    drop(verified.into_file());
    let published = artifact
        .publish_pending(&pending_identity, deadline)
        .expect("publish");
    assert!(!published.reused());
    let final_identity = *published.identity();
    drop(published.into_file());

    let replay = artifact
        .publish_pending(&pending_identity, deadline)
        .expect("no-clobber replay");
    assert!(replay.reused());
    assert_eq!(*replay.identity(), final_identity);
    drop(replay.into_file());

    assert!(artifact
        .cleanup_verified_alias(&final_identity, deadline)
        .expect("cleanup"));
    assert!(fixture.0.join("artifact.redb").is_file());
    assert!(!fixture.0.join(".artifact.redb.pending").exists());
}

#[test]
fn changed_identity_fails_before_publication() {
    let fixture = Fixture::new();
    let platform = TestPlatform::new();
    let control = platform.clone();
    let deadline = Instant::now() + Duration::from_secs(5);
    let artifact = SourceImportOutputArtifact::acquire(
        &fixture.0,
        "artifact.redb",
        platform,
        deadline,
    )
    .expect("owner");

    let pending = artifact
        .open_or_create_pending(deadline)
        .expect("pending");
    let expected = *pending.identity();
    let mut file = pending.into_file();
    file.write_all(b"redb").expect("write pending");
    file.sync_all().expect("sync pending");
    drop(file);
    control.replace_identity();

    assert!(matches!(
        artifact.open_pending_matching(&expected, deadline),
        Err(SourceImportOutputArtifactError::IdentityChanged)
    ));
    assert!(!fixture.0.join("artifact.redb").exists());
}

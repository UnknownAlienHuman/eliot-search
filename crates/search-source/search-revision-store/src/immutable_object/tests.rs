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

impl LegacyRevisionObjectPlatform for TestPlatform {
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
            "eliot-revision-object-{}-{}",
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
fn publish_read_and_exact_reuse_are_package_owned() {
    let fixture = Fixture::new();
    let shard = fixture.0.join("aa");
    let platform = TestPlatform::stable();
    let first = publish_legacy_revision_object(
        &platform,
        &shard,
        "aabb.object",
        ".aabb.1.tmp",
        b"retained bytes",
        1024,
    )
    .expect("publish");
    assert!(!first.reused());
    assert_eq!(first.encoded_bytes(), 14);
    assert!(!shard.join(".aabb.1.tmp").exists());

    let observed = read_legacy_revision_object(
        &platform,
        &shard.join("aabb.object"),
        1024,
    )
    .expect("read");
    assert_eq!(observed.bytes(), b"retained bytes");
    assert_eq!(observed.identity(), first.identity());

    let replay = publish_legacy_revision_object(
        &platform,
        &shard,
        "aabb.object",
        ".aabb.2.tmp",
        b"retained bytes",
        1024,
    )
    .expect("replay");
    assert!(replay.reused());
    assert_eq!(replay.encoded_bytes(), 14);
}

#[test]
fn empty_revision_is_valid_and_conflicting_final_is_never_overwritten() {
    let fixture = Fixture::new();
    let shard = fixture.0.join("bb");
    let platform = TestPlatform::stable();
    publish_legacy_revision_object(
        &platform,
        &shard,
        "empty.object",
        ".empty.1.tmp",
        b"",
        16,
    )
    .expect("empty publish");
    assert_eq!(
        read_legacy_revision_object(&platform, &shard.join("empty.object"), 16)
            .expect("empty read")
            .bytes(),
        b""
    );

    fs::write(shard.join("conflict.object"), b"existing").expect("precreate final");
    assert!(matches!(
        publish_legacy_revision_object(
            &platform,
            &shard,
            "conflict.object",
            ".conflict.1.tmp",
            b"different",
            16,
        ),
        Err(LegacyRevisionObjectError::ImmutableConflict)
    ));
    assert_eq!(
        fs::read(shard.join("conflict.object")).expect("read conflict"),
        b"existing"
    );
}

#[test]
fn changed_native_identity_fails_closed_during_read() {
    let fixture = Fixture::new();
    let path = fixture.0.join("object");
    fs::write(&path, b"bytes").expect("write object");
    assert!(matches!(
        read_legacy_revision_object(&TestPlatform::flapping(), &path, 16),
        Err(LegacyRevisionObjectError::IdentityChanged)
    ));
}

#[test]
fn invalid_names_and_size_are_rejected_before_mutation() {
    let fixture = Fixture::new();
    let platform = TestPlatform::stable();
    assert!(matches!(
        publish_legacy_revision_object(
            &platform,
            &fixture.0.join("cc"),
            "../escape",
            ".escape.tmp",
            b"x",
            16,
        ),
        Err(LegacyRevisionObjectError::LocalNameInvalid)
    ));
    assert!(matches!(
        publish_legacy_revision_object(
            &platform,
            &fixture.0.join("cc"),
            "object",
            ".object.tmp",
            b"too large",
            2,
        ),
        Err(LegacyRevisionObjectError::SizeInvalid)
    ));
    assert!(!fixture.0.join("cc").exists());
}

#[test]
fn oversize_existing_object_is_rejected_before_allocation() {
    let fixture = Fixture::new();
    let path = fixture.0.join("large");
    fs::write(&path, b"12345").expect("write object");
    assert!(matches!(
        read_legacy_revision_object(&TestPlatform::stable(), &path, 4),
        Err(LegacyRevisionObjectError::SizeInvalid)
    ));
}

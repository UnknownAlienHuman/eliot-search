use std::fs;
use std::io;
use std::path::Path;
use std::sync::{Arc, Barrier};
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
    assert!(!shard.join(".conflict.1.tmp").exists());
}

#[test]
fn racing_publications_never_clobber_the_winner_or_leave_temporary_files() {
    let fixture = Fixture::new();
    let shard = fixture.0.join("cc");
    fs::create_dir(&shard).expect("create shared shard");
    let platform = TestPlatform::stable();
    let barrier = Arc::new(Barrier::new(2));
    let mut jobs = Vec::new();
    for (temporary, bytes) in [
        (".race.1.tmp", b"candidate-a".as_slice()),
        (".race.2.tmp", b"candidate-b".as_slice()),
    ] {
        let start = Arc::clone(&barrier);
        let platform = platform.clone();
        let shard = shard.clone();
        jobs.push(std::thread::spawn(move || {
            start.wait();
            (
                bytes,
                publish_legacy_revision_object(
                    &platform,
                    &shard,
                    "race.object",
                    temporary,
                    bytes,
                    64,
                ),
            )
        }));
    }
    let outcomes = jobs
        .into_iter()
        .map(|job| job.join().expect("join publication"))
        .collect::<Vec<_>>();
    let winners = outcomes
        .iter()
        .filter(|(_, outcome)| outcome.is_ok())
        .collect::<Vec<_>>();
    assert_eq!(winners.len(), 1, "{outcomes:?}");
    assert_eq!(
        fs::read(shard.join("race.object")).expect("read winner"),
        winners[0].0,
    );
    assert_eq!(
        fs::read_dir(&shard).expect("list shard").count(),
        1,
        "only the final object may remain"
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
            &fixture.0.join("dd"),
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
            &fixture.0.join("dd"),
            "object",
            ".object.tmp",
            b"too large",
            2,
        ),
        Err(LegacyRevisionObjectError::SizeInvalid)
    ));
    assert!(!fixture.0.join("dd").exists());
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

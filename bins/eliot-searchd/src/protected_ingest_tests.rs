//! Native Windows regressions against the primary revision-protected store.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::development::DataRootGuard;
use crate::direct_store::DirectStore;
use crate::revision_protection::PROTECTED_OBJECT_EXTENSION;
use crate::sha256;

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let base = std::env::temp_dir().join(format!(
            "eliot-protected-ingest-{}-{stamp}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(base.join("data")).unwrap();
        fs::create_dir(base.join("sources")).unwrap();
        Self(base)
    }

    fn owner(&self) -> DataRootGuard {
        DataRootGuard::acquire(&self.0.join("data")).unwrap()
    }

    fn source(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.0.join("sources").join(name);
        fs::write(&path, bytes).unwrap();
        path
    }

    fn log(&self) -> Vec<u8> {
        fs::read(self.0.join("data/control/source-events.log")).unwrap()
    }

    fn object(&self, revision: &str, extension: &str) -> PathBuf {
        self.0.join("data/revisions").join(&revision[..2])
            .join(format!("{revision}.{extension}"))
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn revision_id(source_id: &str, bytes: &[u8]) -> String {
    sha256::hex(&sha256::digest_parts(
        b"eliot-search/direct-revision-id/v1",
        &[
            source_id.as_bytes(),
            &sha256::digest(bytes),
            &u64::try_from(bytes.len()).unwrap().to_be_bytes(),
        ],
    ))
}

fn block_object(path: &Path) {
    fs::create_dir_all(path).unwrap();
}

#[test]
fn protection_failure_does_not_publish_or_spill_a_new_plaintext_revision() {
    let fixture = Fixture::new();
    let owner = fixture.owner();
    let mut store = DirectStore::open(owner.canonical_root()).unwrap();
    let path = fixture.source("source", b"old retained bytes");
    let first = store.index_file(&path).unwrap();
    assert!(!fixture.object(&first.revision_id, "bin").exists());
    assert!(fixture.object(&first.revision_id, PROTECTED_OBJECT_EXTENSION).is_file());

    let next = b"next retained bytes";
    let next_revision = revision_id(&first.source_id, next);
    let blocked = fixture.object(&next_revision, PROTECTED_OBJECT_EXTENSION);
    block_object(&blocked);
    let before = fixture.log();
    fs::write(&path, next).unwrap();

    assert!(store.index_file(&path).is_err());
    assert_eq!(fixture.log(), before);
    assert!(!fixture.object(&next_revision, "bin").exists());
    assert_eq!(store.list_sources()[0].revision_id, first.revision_id);
    assert_eq!(store.search("old", false).unwrap().matches.len(), 1);
    assert!(store.search("next", false).unwrap().matches.is_empty());

    fs::remove_dir(&blocked).unwrap();
    let retried = store.index_file(&path).unwrap();
    assert_eq!(retried.revision_id, next_revision);
    assert!(retried.changed);
    assert!(!fixture.object(&next_revision, "bin").exists());
    assert_eq!(store.verify().unwrap().referenced_revisions, 2);
    assert_eq!(store.search("next", false).unwrap().matches.len(), 1);
}

#[test]
fn directory_protection_failure_preserves_the_complete_previous_catalog() {
    let fixture = Fixture::new();
    let owner = fixture.owner();
    let mut store = DirectStore::open(owner.canonical_root()).unwrap();
    let first_path = fixture.source("a", b"first old");
    let second_path = fixture.source("b", b"second old");
    let first = store.index_file(&first_path).unwrap();
    let second = store.index_file(&second_path).unwrap();
    let next_first = b"first new";
    let next_second = b"second new";
    let first_revision = revision_id(&first.source_id, next_first);
    let second_revision = revision_id(&second.source_id, next_second);
    let blocked = fixture.object(&second_revision, PROTECTED_OBJECT_EXTENSION);
    block_object(&blocked);
    fs::write(&first_path, next_first).unwrap();
    fs::write(&second_path, next_second).unwrap();
    let before = fixture.log();
    let previous_sources = store.list_sources();

    assert!(store.index_directory(&fixture.0.join("sources")).is_err());
    assert_eq!(fixture.log(), before);
    assert_eq!(store.list_sources(), previous_sources);
    assert!(!fixture.object(&first_revision, "bin").exists());
    assert!(!fixture.object(&second_revision, "bin").exists());

    fs::remove_dir(&blocked).unwrap();
    let retried = store.index_directory(&fixture.0.join("sources")).unwrap();
    assert_eq!(retried.len(), 2);
    assert!(retried.iter().all(|source| source.changed));
    assert_eq!(store.verify().unwrap().referenced_revisions, 4);
    assert_eq!(store.search("new", false).unwrap().matches.len(), 2);
}

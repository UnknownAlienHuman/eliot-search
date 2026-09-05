//! Native Windows crash and migration regressions using real DPAPI protection.

use super::*;
use crate::development::DataRootGuard;
use crate::direct_store::DirectStore;
use crate::plaintext_direct_store;
use crate::revision_protection::PROTECTED_OBJECT_EXTENSION;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

static NEXT: AtomicU64 = AtomicU64::new(0);
const SENTINEL: &[u8] = b"ELIOT-PLAIN-TEXT-LEAK-SENTINEL-v1";

struct Fixture {
    base: PathBuf,
    data: PathBuf,
    source: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let base = std::env::temp_dir().join(format!(
            "eliot-cipher-boundary-{}-{stamp}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        let data = base.join("data");
        fs::create_dir_all(&data).unwrap();
        let source = base.join("source.txt");
        fs::write(&source, SENTINEL).unwrap();
        Self { base, data, source }
    }
    fn log(&self) -> Vec<u8> { fs::read(self.data.join("control/source-events.log")).unwrap() }
}
impl Drop for Fixture {
    fn drop(&mut self) { let _ = fs::remove_dir_all(&self.base); }
}

fn files(root: &Path) -> Vec<PathBuf> {
    let mut output = Vec::new();
    for entry in fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() { output.extend(files(&path)); } else { output.push(path); }
    }
    output
}

fn assert_no_plaintext(root: &Path) {
    for path in files(root) {
        assert!(!path.extension().is_some_and(|extension| extension == "bin"), "{path:?}");
        let bytes = fs::read(&path).unwrap();
        assert!(!bytes.windows(SENTINEL.len()).any(|window| window == SENTINEL), "{path:?}");
    }
}

#[test]
fn orphan_plaintext_is_not_silently_adopted_beside_new_ciphertext() {
    let fixture = Fixture::new();
    let guard = DataRootGuard::acquire(&fixture.data).unwrap();
    let mut store = DirectStore::open(guard.canonical_root()).unwrap();
    let before = fixture.log();
    let protector = &store.protector;
    let root = guard.canonical_root();
    let mut orphan = None;
    let mut writer = |_: &plaintext_direct_store::DirectStore, source: &IndexedSource, bytes: &[u8]| {
        // Test fixture for residue from a previous plaintext-first implementation.
        let path = legacy_path(root, &source.revision_id)?;
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, bytes).unwrap();
        orphan = Some(path);
        let result = persist_before_publication(root, protector, source, bytes);
        assert!(!protected_path(root, &source.revision_id).unwrap().exists());
        result
    };
    assert_eq!(
        store.inner.index_file_with_writer(&fixture.source, &mut writer),
        Err("DIRECT_NEW_REVISION_PLAINTEXT_PRESENT".to_owned()),
    );
    drop(writer);
    assert_eq!(fs::read(orphan.unwrap()).unwrap(), SENTINEL);
    assert_eq!(fixture.log(), before);
    assert!(store.list_sources().is_empty());
}

#[test]
fn corrupt_protected_object_is_not_replaced_by_plaintext_reindexing() {
    let fixture = Fixture::new();
    let guard = DataRootGuard::acquire(&fixture.data).unwrap();
    let mut store = DirectStore::open(guard.canonical_root()).unwrap();
    let indexed = store.index_file(&fixture.source).unwrap();
    let path = protected_path(guard.canonical_root(), &indexed.revision_id).unwrap();
    fs::write(&path, b"invalid-protected-object").unwrap();
    let before = fixture.log();
    assert!(store.index_file(&fixture.source).is_err());
    assert_eq!(fixture.log(), before);
    assert_eq!(fs::read(path).unwrap(), b"invalid-protected-object");
    assert_no_plaintext(&fixture.data);
}

#[test]
fn referenced_plaintext_migration_keeps_the_original_revision_identity() {
    let fixture = Fixture::new();
    let guard = DataRootGuard::acquire(&fixture.data).unwrap();
    let indexed = {
        let mut legacy = plaintext_direct_store::DirectStore::open(guard.canonical_root()).unwrap();
        legacy.index_file(&fixture.source).unwrap()
    };
    assert!(legacy_path(guard.canonical_root(), &indexed.revision_id).unwrap().is_file());
    let before = fixture.log();
    let mut store = DirectStore::open(guard.canonical_root()).unwrap();
    assert_eq!(fixture.log(), before);
    assert_no_plaintext(&fixture.data);
    let repeat = store.index_file(&fixture.source).unwrap();
    assert_eq!(repeat.revision_id, indexed.revision_id);
    assert!(!repeat.changed);
    assert_eq!(store.verify().unwrap().source_events, 1);
    let slice = store.read_revision_range(&indexed.revision_id, 0, SENTINEL.len() as u64).unwrap();
    assert_eq!(slice.bytes, SENTINEL);
}

#[test]
fn process_exit_after_ciphertext_before_catalog_is_recoverable_without_plaintext() {
    let fixture = Fixture::new();
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--ignored", "--exact", "direct_store::revision_writer::tests::crash_child", "--nocapture"])
        .env("ELIOT_PROTECTED_INGEST_TEST_ROOT", &fixture.base)
        .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::inherit())
        .spawn().unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() { break status; }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("protected-ingest child exceeded its deadline");
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    assert_eq!(status.code(), Some(77));
    assert_no_plaintext(&fixture.data);
    assert_eq!(files(&fixture.data).iter().filter(|path| {
        path.extension().is_some_and(|extension| extension == PROTECTED_OBJECT_EXTENSION)
    }).count(), 1);
    let guard = DataRootGuard::acquire(&fixture.data).unwrap();
    let mut store = DirectStore::open(guard.canonical_root()).unwrap();
    assert!(store.list_sources().is_empty());
    assert_eq!(store.verify().unwrap().source_events, 0);
    // Retry decrypts and compares the existing orphan ciphertext. It does not
    // compare randomized ciphertext from two separate encryption calls.
    let indexed = store.index_file(&fixture.source).unwrap();
    assert_eq!(store.verify().unwrap().source_events, 1);
    assert_no_plaintext(&fixture.data);
    assert_eq!(files(&fixture.data).iter().filter(|path| {
        path.extension().is_some_and(|extension| extension == PROTECTED_OBJECT_EXTENSION)
    }).count(), 1);
    assert_eq!(store.read_revision_range(&indexed.revision_id, 0, SENTINEL.len() as u64).unwrap().bytes, SENTINEL);
}

#[test]
#[ignore = "invoked by the native process-exit regression test"]
fn crash_child() {
    let Some(base) = std::env::var_os("ELIOT_PROTECTED_INGEST_TEST_ROOT") else { return; };
    let base = PathBuf::from(base);
    let guard = DataRootGuard::acquire(&base.join("data")).unwrap();
    let mut store = DirectStore::open(guard.canonical_root()).unwrap();
    let protector = &store.protector;
    let root = guard.canonical_root();
    let mut writer = |_: &plaintext_direct_store::DirectStore, source: &IndexedSource, bytes: &[u8]| -> Result<(), String> {
        persist_before_publication(root, protector, source, bytes)?;
        assert_no_plaintext(root);
        // Exit without destructors after real protected readback, before the
        // catalog has appended any source event. This is not a power-loss test.
        std::process::exit(77);
    };
    let result = store.inner.index_file_with_writer(&base.join("source.txt"), &mut writer);
    panic!("expected process exit at the protected-object boundary: {result:?}");
}

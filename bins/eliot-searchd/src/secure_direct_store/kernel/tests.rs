use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::development::DataRootGuard;
use crate::revision_protection::{
    TestCredentialGuard, lock_unit_vault_for_test,
};

use super::DirectStore;

static NEXT: AtomicU64 = AtomicU64::new(0);

fn corpus_snapshot(root: &Path) -> Vec<(PathBuf, u64)> {
    let mut output = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(directory) = stack.pop() {
        for entry in fs::read_dir(&directory).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if entry.file_type().unwrap().is_dir() {
                stack.push(path);
            } else if entry.file_type().unwrap().is_file()
                && path.strip_prefix(root).is_ok_and(|relative| {
                    relative.starts_with("control")
                        || relative.starts_with("revisions")
                        || relative.starts_with("preparation")
                })
            {
                output.push((path, entry.metadata().unwrap().len()));
            }
        }
    }
    output.sort();
    output
}

#[test]
fn ten_thousand_bounded_queries_cause_no_durable_corpus_writes() {
    let _vault = lock_unit_vault_for_test();
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let base = std::env::temp_dir().join(format!(
        "eliot-spine-gate-{}-{stamp}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let data = base.join("data");
    fs::create_dir_all(&data).unwrap();
    let source = base.join("source.txt");
    fs::write(&source, b"spine needle stable").unwrap();
    let credential_guard = TestCredentialGuard::for_data_root(&data);
    let guard = DataRootGuard::acquire(&data).unwrap();
    let mut store = DirectStore::open(guard.canonical_root()).unwrap();
    let indexed = store.index_file(&source).unwrap();
    assert!(indexed.changed);
    let first = store.search("needle", false).unwrap();
    assert_eq!(first.matches.len(), 1);
    assert!(first.complete);
    assert!(first.gaps.is_empty());
    let before = corpus_snapshot(guard.canonical_root());
    assert!(!before.is_empty());
    for _ in 0..10_000 {
        let result = store.search("needle", false).unwrap();
        assert_eq!(result.matches.len(), 1);
        assert!(result.complete);
        assert!(!result.match_limit_reached);
        assert!(result.gaps.is_empty());
        assert_eq!(result.searched_sources, 1);
    }
    assert_eq!(corpus_snapshot(guard.canonical_root()), before);
    credential_guard.cleanup();
    drop(store);
    drop(guard);
    let _ = fs::remove_dir_all(&base);
}

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use super::super::super::{DirectStore, IndexedSource};
use crate::source_composition as canonical;

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "eliot-ingest-{}-{stamp}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("data")).unwrap();
        Self(root)
    }

    fn store(&self) -> DirectStore {
        DirectStore::open(&self.0.join("data")).unwrap()
    }

    fn source(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.0.join(name);
        fs::write(&path, bytes).unwrap();
        path
    }

    fn log(&self) -> Vec<u8> {
        fs::read(self.0.join("data/control/source-events.log")).unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn plaintext_writer(
    store: &DirectStore,
    source: &IndexedSource,
    bytes: &[u8],
) -> Result<(), String> {
    store.persist_revision(&source.revision_id, &source.content_digest, bytes)
}

#[test]
fn failed_writer_cannot_publish_a_revision() {
    let fixture = Fixture::new();
    let mut store = fixture.store();
    let source = fixture.source("source", b"private bytes");
    let before = fixture.log();
    let result = store.index_file_with_writer(&source, &mut |_, _, _| {
        Err("PROTECTION_READBACK_FAILED".to_owned())
    });
    assert_eq!(result, Err("PROTECTION_READBACK_FAILED".to_owned()));
    assert_eq!(fixture.log(), before);
    assert!(store.list_sources().is_empty());
    assert!(fixture.store().list_sources().is_empty());
}

#[test]
fn later_writer_failure_leaves_all_batch_metadata_unpublished() {
    let fixture = Fixture::new();
    let mut store = fixture.store();
    let paths = vec![
        fixture.source("a", b"one"),
        fixture.source("b", b"two"),
    ];
    let before = fixture.log();
    let mut writes = 0;
    let result = store.index_paths_bounded(
        &paths,
        6,
        &mut |store, source, bytes| {
            writes += 1;
            assert!(store.list_sources().is_empty());
            assert_eq!(fixture.log(), before);
            if writes == 2 {
                return Err("SECOND_OBJECT_FAILED".to_owned());
            }
            plaintext_writer(store, source, bytes)
        },
    );
    assert_eq!(result, Err("SECOND_OBJECT_FAILED".to_owned()));
    assert_eq!(writes, 2);
    assert_eq!(fixture.log(), before);
    assert!(fixture.store().list_sources().is_empty());
}

#[test]
fn aggregate_byte_limit_fails_before_any_writer_call() {
    let fixture = Fixture::new();
    let mut store = fixture.store();
    let paths = vec![
        fixture.source("a", b"one"),
        fixture.source("b", b"two"),
    ];
    let mut calls = 0;
    let result = store.index_paths_bounded(&paths, 5, &mut |_, _, _| {
        calls += 1;
        Ok(())
    });
    assert_eq!(result, Err("DIRECT_BATCH_BYTES_EXCEEDED".to_owned()));
    assert_eq!(calls, 0);
    assert!(store.list_sources().is_empty());
}

#[test]
fn returning_to_old_content_is_a_new_transition_not_a_conflicting_replay() {
    let fixture = Fixture::new();
    let mut store = fixture.store();
    let path = fixture.source("source", b"old");
    let first = store.index_file(&path).unwrap();
    fs::write(&path, b"new").unwrap();
    let second = store.index_file(&path).unwrap();
    fs::write(&path, b"old").unwrap();
    let third = store.index_file(&path).unwrap();
    assert_eq!(first.source_id, third.source_id);
    assert_eq!(first.revision_id, third.revision_id);
    assert_ne!(second.revision_id, third.revision_id);
    assert!(third.changed);
    assert!(!store.index_file(&path).unwrap().changed);
    let verified = fixture.store().verify().unwrap();
    assert_eq!(verified.source_events, 3);
    assert_eq!(verified.referenced_revisions, 2);
}

#[test]
fn retired_source_can_be_reactivated_with_unchanged_bytes() {
    let fixture = Fixture::new();
    let mut store = fixture.store();
    let path = fixture.source("source", b"retained");
    let first = store.index_file(&path).unwrap();
    store.retire_source(&first.source_id).unwrap();
    assert!(store.index_file(&path).unwrap().changed);
    let verified = fixture.store().verify().unwrap();
    assert_eq!(verified.active_sources, 1);
    assert_eq!(verified.source_events, 3);
}

#[test]
fn stale_catalog_refuses_new_writes_before_calling_storage() {
    let fixture = Fixture::new();
    let mut first = fixture.store();
    let mut stale = fixture.store();
    let path = fixture.source("source", b"new");
    first.index_file(&path).unwrap();
    let result = stale.index_file_with_writer(&path, &mut |_, _, _| {
        panic!("stale catalog must not invoke storage");
    });
    assert_eq!(result, Err("DIRECT_CONTROL_READBACK_MISMATCH".to_owned()));
}

#[test]
fn byte_budget_counts_only_admitted_retained_bytes() {
    let fixture = Fixture::new();
    let mut store = fixture.store();
    let paths = vec![fixture.source("a", b"x"), fixture.source("b", b"y")];
    let mut calls = 0;
    let result = store.index_paths_bounded(&paths, 1, &mut |_, _, _| {
        calls += 1;
        Ok(())
    });
    assert_eq!(result, Err("DIRECT_BATCH_BYTES_EXCEEDED".to_owned()));
    assert_eq!(calls, 0);
    let admitted = store
        .index_paths_bounded(&paths, 2, &mut plaintext_writer)
        .unwrap();
    assert_eq!(admitted.len(), 2);
    assert_eq!(store.verify().unwrap().total_revision_bytes, 2);
}

#[test]
fn empty_source_is_denied_before_any_writer_call() {
    let fixture = Fixture::new();
    let mut store = fixture.store();
    let empty = fixture.source("empty", b"");
    let before = fixture.log();
    let mut calls = 0;
    let result = store.index_file_with_writer(&empty, &mut |_, _, _| {
        calls += 1;
        Ok(())
    });
    assert_eq!(result, Err(canonical::SOURCE_ADMISSION_DENIED.to_owned()));
    assert_eq!(calls, 0);
    assert_eq!(fixture.log(), before);
    assert!(store.list_sources().is_empty());
}

#[test]
fn denied_sources_never_reach_cas() {
    let fixture = Fixture::new();
    let mut store = fixture.store();
    for name in [
        "id_rsa",
        "tls.pem",
        "secret_notes.txt",
        "app.generated.js",
    ] {
        let path = fixture.source(name, b"candidate bytes");
        let before = fixture.log();
        let mut calls = 0;
        let result = store.index_file_with_writer(&path, &mut |_, _, _| {
            calls += 1;
            Ok(())
        });
        assert_eq!(
            result,
            Err(canonical::SOURCE_ADMISSION_DENIED.to_owned()),
            "name={name}"
        );
        assert_eq!(calls, 0, "denied source must not invoke storage");
        assert_eq!(fixture.log(), before);
    }
    assert!(store.list_sources().is_empty());
    let allowed = fixture.source("notes.txt", b"allowed bytes");
    let indexed = store.index_file(&allowed).unwrap();
    assert!(indexed.changed);
    assert_eq!(store.list_sources().len(), 1);
}

#[test]
fn equal_content_distinct_files_keep_distinct_stable_identities() {
    let fixture = Fixture::new();
    let mut store = fixture.store();
    let first = fixture.source("first.txt", b"same bytes");
    let second = fixture.source("second.txt", b"same bytes");
    let one = store.index_file(&first).unwrap();
    let two = store.index_file(&second).unwrap();
    assert_ne!(one.source_id, two.source_id);
    assert_ne!(one.revision_id, two.revision_id);
    assert_eq!(one.content_digest, two.content_digest);
    assert_eq!(store.list_sources().len(), 2);
    assert_eq!(fixture.store().verify().unwrap().referenced_revisions, 2);
}

#[test]
fn rename_preserves_stable_identity_with_new_locator_binding() {
    let fixture = Fixture::new();
    let mut store = fixture.store();
    let before = fixture.source("before.txt", b"renamed bytes");
    let first = store.index_file(&before).unwrap();
    let after = fixture.0.join("after.txt");
    fs::rename(&before, &after).unwrap();
    let second = store.index_file(&after).unwrap();
    assert_eq!(second.source_id, first.source_id);
    assert_eq!(second.revision_id, first.revision_id);
    assert!(second.changed);
    assert!(!store.index_file(&after).unwrap().changed);
    assert!(store.index_file(&before).is_err());
}

#[test]
fn replacement_at_same_path_with_new_identity_does_not_reuse_closed_source() {
    let fixture = Fixture::new();
    let mut store = fixture.store();
    let path = fixture.source("victim.txt", b"original bytes");
    let first = store.index_file(&path).unwrap();
    fs::remove_file(&path).unwrap();
    fs::write(&path, b"substituted bytes").unwrap();
    let second = store.index_file(&path).unwrap();
    assert_ne!(second.content_digest, first.content_digest);
    assert_ne!(second.revision_id, first.revision_id);
    assert!(second.changed);
    let verified = fixture.store().verify().unwrap();
    assert!(verified.source_events >= 2);
}

#[test]
fn restart_preserves_membership_revision_occurrences_and_lineage() {
    let fixture = Fixture::new();
    let first_id;
    let first_revision;
    {
        let mut store = fixture.store();
        let path = fixture.source("notes.txt", b"lineage bytes");
        let indexed = store.index_file(&path).unwrap();
        first_id = indexed.source_id;
        first_revision = indexed.revision_id;
        assert_eq!(store.list_sources().len(), 1);
    }
    let reopened = fixture.store();
    let sources = reopened.list_sources();
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0].source_id, first_id);
    assert_eq!(sources[0].revision_id, first_revision);
    let verified = reopened.verify().unwrap();
    assert_eq!(verified.source_events, 1);
    assert_eq!(verified.referenced_revisions, 1);
    let mut store = fixture.store();
    let path = fixture.0.join("notes.txt");
    assert!(!store.index_file(&path).unwrap().changed);
    assert_eq!(fixture.store().verify().unwrap().source_events, 1);
}

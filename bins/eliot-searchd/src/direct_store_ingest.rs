//! Bounded ingestion with an explicit revision-storage publication barrier.

use std::collections::BTreeSet;

use super::{
    CONTROL_DIRECTORY, DirectStore, IndexedSource, MAX_DIRECTORY_FILES, RecordDraft,
    SOURCE_LOG_FILE, SourceState, ZERO_DIGEST, collect_regular_files, ensure_directory,
    fs, load_registry, path_identity_bytes, read_file_snapshot, sha256,
};
use std::path::{Path, PathBuf};

const MAX_BATCH_INPUT_BYTES: usize = 512 * 1024 * 1024;

impl DirectStore {
    /// Publishes metadata only after the supplied writer verifies immutable bytes.
    /// The writer is a composition-owned adapter, never a client-provided callback.
    pub(crate) fn index_file_with_writer(
        &mut self,
        path: &Path,
        writer: &mut impl FnMut(&Self, &IndexedSource, &[u8]) -> Result<(), String>,
    ) -> Result<IndexedSource, String> {
        self.index_paths_bounded(vec![path.to_path_buf()], MAX_BATCH_INPUT_BYTES, writer)?
            .pop()
            .ok_or_else(|| "DIRECT_INDEX_EMPTY_RESULT".to_owned())
    }

    /// Uses the same prepublication barrier for every member of a directory batch.
    pub(crate) fn index_directory_with_writer(
        &mut self,
        directory: &Path,
        writer: &mut impl FnMut(&Self, &IndexedSource, &[u8]) -> Result<(), String>,
    ) -> Result<Vec<IndexedSource>, String> {
        ensure_directory(directory)?;
        let canonical = fs::canonicalize(directory)
            .map_err(|error| format!("DIRECT_DIRECTORY_CANONICALIZE_ERROR:{error}"))?;
        if canonical == self.root {
            return Err("DIRECT_SOURCE_DIRECTORY_IS_DATA_ROOT".to_owned());
        }
        ensure_directory(&canonical)?;
        let mut paths = Vec::new();
        collect_regular_files(&canonical, &self.root, 0, &mut paths)?;
        paths.sort_by_key(|path| path_identity_bytes(path));
        self.index_paths_bounded(paths, MAX_BATCH_INPUT_BYTES, writer)
    }

    fn index_paths_bounded(
        &mut self,
        paths: Vec<PathBuf>,
        max_batch_bytes: usize,
        writer: &mut impl FnMut(&Self, &IndexedSource, &[u8]) -> Result<(), String>,
    ) -> Result<Vec<IndexedSource>, String> {
        if max_batch_bytes == 0 || max_batch_bytes > MAX_BATCH_INPUT_BYTES {
            return Err("DIRECT_BATCH_LIMIT_INVALID".to_owned());
        }
        if paths.len() > MAX_DIRECTORY_FILES {
            return Err("DIRECT_DIRECTORY_FILE_LIMIT_EXCEEDED".to_owned());
        }
        let current = load_registry(&self.root.join(CONTROL_DIRECTORY).join(SOURCE_LOG_FILE))?;
        if current.last_sequence != self.registry.last_sequence
            || current.last_digest != self.registry.last_digest
            || current.latest != self.registry.latest
        {
            return Err("DIRECT_CONTROL_READBACK_MISMATCH".to_owned());
        }

        // Bound the complete retained input, not merely each individual file.
        // Every read receives the remaining budget before allocating its buffer.
        let mut snapshots = Vec::with_capacity(paths.len());
        let mut retained_bytes = 0_usize;
        for path in &paths {
            let remaining = max_batch_bytes
                .checked_sub(retained_bytes)
                .ok_or_else(|| "DIRECT_BATCH_BYTES_EXCEEDED".to_owned())?;
            let snapshot = read_file_snapshot(path, &self.root, remaining)?;
            retained_bytes = retained_bytes
                .checked_add(snapshot.bytes.len())
                .filter(|length| *length <= max_batch_bytes)
                .ok_or_else(|| "DIRECT_BATCH_BYTES_EXCEEDED".to_owned())?;
            snapshots.push(snapshot);
        }
        snapshots.sort_by(|left, right| left.path_digest.cmp(&right.path_digest));

        // Validate the complete batch before invoking any storage adapter.
        let mut seen = BTreeSet::new();
        let mut planned = Vec::with_capacity(snapshots.len());
        for snapshot in snapshots {
            let file_identity = sha256::decode_digest(&snapshot.file_identity_digest)
                .ok_or_else(|| "DIRECT_FILE_IDENTITY_INVALID".to_owned())?;
            let source_id = sha256::hex(&sha256::digest_parts(
                b"eliot-search/direct-source-id/v1",
                &[&self.namespace_id, &file_identity],
            ));
            if !seen.insert(source_id.clone()) {
                return Err("DIRECT_DUPLICATE_SOURCE_IN_BATCH".to_owned());
            }
            let digest = sha256::decode_digest(&snapshot.content_digest)
                .ok_or_else(|| "DIRECT_CONTENT_DIGEST_INVALID".to_owned())?;
            let byte_length = u64::try_from(snapshot.bytes.len())
                .map_err(|_| "DIRECT_SOURCE_TOO_LARGE".to_owned())?;
            let revision_id = sha256::hex(&sha256::digest_parts(
                b"eliot-search/direct-revision-id/v1",
                &[source_id.as_bytes(), &digest, &byte_length.to_be_bytes()],
            ));
            let previous = self.registry.latest.get(&source_id);
            if previous.is_some_and(|record| {
                record.file_identity_digest != snapshot.file_identity_digest
            }) {
                return Err("DIRECT_SOURCE_ID_COLLISION".to_owned());
            }
            let changed = !previous.is_some_and(|record| {
                record.state == SourceState::Active
                    && record.revision_id == revision_id
                    && record.path_digest == snapshot.path_digest
            });
            let source = IndexedSource {
                source_id,
                revision_id,
                content_digest: snapshot.content_digest.clone(),
                path_digest: snapshot.path_digest.clone(),
                byte_length,
                identity_strength: snapshot.identity_strength.tag(),
                changed,
            };
            let draft = if changed {
                // Returning to an old revision is a new transition, not a replay
                // of the first transition to those bytes. Bind its predecessor.
                let predecessor = previous.map_or(ZERO_DIGEST, |record| {
                    record.record_digest.as_str()
                });
                let operation_id = sha256::hex(&sha256::digest_parts(
                    b"eliot-search/direct-index-operation/v2",
                    &[
                        source.source_id.as_bytes(),
                        source.revision_id.as_bytes(),
                        source.path_digest.as_bytes(),
                        predecessor.as_bytes(),
                    ],
                ));
                Some(RecordDraft {
                    operation_id,
                    state: SourceState::Active,
                    source_id: source.source_id.clone(),
                    revision_id: source.revision_id.clone(),
                    content_digest: source.content_digest.clone(),
                    byte_length,
                    file_identity_digest: snapshot.file_identity_digest.clone(),
                    path_digest: source.path_digest.clone(),
                    identity_strength: snapshot.identity_strength,
                })
            } else {
                None
            };
            planned.push((snapshot, source, draft));
        }

        let mut results = Vec::with_capacity(planned.len());
        let mut drafts = Vec::new();
        for (snapshot, source, draft) in planned {
            // Even an unchanged revision requires exact storage readback.
            // A failed adapter can leave orphan objects, but no new catalog event.
            writer(self, &source, &snapshot.bytes)?;
            if let Some(draft) = draft {
                drafts.push(draft);
            }
            results.push(source);
        }
        if !drafts.is_empty() {
            self.append_drafts(drafts)?;
        }
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static NEXT: AtomicU64 = AtomicU64::new(0);

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
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
        let paths = vec![fixture.source("a", b"one"), fixture.source("b", b"two")];
        let before = fixture.log();
        let mut writes = 0;
        let result = store.index_paths_bounded(paths, 6, &mut |store, source, bytes| {
            writes += 1;
            assert!(store.list_sources().is_empty());
            assert_eq!(fixture.log(), before);
            if writes == 2 {
                return Err("SECOND_OBJECT_FAILED".to_owned());
            }
            plaintext_writer(store, source, bytes)
        });
        assert_eq!(result, Err("SECOND_OBJECT_FAILED".to_owned()));
        assert_eq!(writes, 2);
        assert_eq!(fixture.log(), before);
        assert!(fixture.store().list_sources().is_empty());
    }

    #[test]
    fn aggregate_byte_limit_fails_before_any_writer_call() {
        let fixture = Fixture::new();
        let mut store = fixture.store();
        let paths = vec![fixture.source("a", b"one"), fixture.source("b", b"two")];
        let mut calls = 0;
        let result = store.index_paths_bounded(paths, 5, &mut |_, _, _| {
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
    fn empty_files_do_not_consume_another_files_byte_budget() {
        let fixture = Fixture::new();
        let mut store = fixture.store();
        let paths = vec![fixture.source("a", b"x"), fixture.source("empty", b"")];
        let result = store.index_paths_bounded(paths, 1, &mut plaintext_writer).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(store.verify().unwrap().total_revision_bytes, 1);
    }
}

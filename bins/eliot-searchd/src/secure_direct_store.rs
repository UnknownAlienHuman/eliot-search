//! Revision-protected facade over the append-only DIRECT catalog.
//!
//! New Windows revisions are protected and read back before source metadata is
//! published. Existing plaintext revisions are migrated on opening. Other
//! platforms retain the explicit plaintext-development storage profile.

#![allow(
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::similar_names,
    clippy::too_many_lines
)]

use core::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use zeroize::Zeroizing;

use crate::development::ScanResult;
use crate::direct_preparation::{
    CANONICAL_CORPUS_BUDGET, SPINE_GAP_BUDGET_EXHAUSTED, SPINE_GAP_MATCH_LIMIT,
    SPINE_GAP_VALIDATION_FAILED, scan_prepared, validate_query, validate_source_backed_match,
};
use crate::plaintext_direct_store as plaintext;
use plaintext::{RevisionMetadata, verify_revision_identity};
use crate::revision_protection::RevisionProtector;
use crate::sha256;

#[path = "secure_direct_store_storage_io.rs"]
mod storage_io;
#[path = "secure_revision_writer.rs"]
mod revision_writer;
#[path = "preparation_store.rs"]
mod preparation_store;
#[path = "control_migration_objects.rs"]
mod migration_objects;
pub use preparation_store::{PreparationBatch, PreparationCursor};

use storage_io::{
    legacy_path, protected_path,
    read_plaintext_path, read_regular_file, remove_plaintext_after_readback,
};

pub use plaintext::{
    IndexedSource, RevisionSlice, SourceSummary, StoreGap, StoreSearchResult,
    StoreVerification, StoredMatch,
};

const REVISION_DIRECTORY: &str = "revisions";
const MAX_REVISION_OBJECT_BYTES: usize = 65 * 1024 * 1024;
const MAX_SEARCH_GAPS: usize = 100_000;
const MAX_READ_RANGE_BYTES: usize = 8 * 1024 * 1024;

/// DIRECT catalog with a platform-specific prepublication revision writer.
pub struct DirectStore {
    root: PathBuf,
    inner: plaintext::DirectStore,
    protector: RevisionProtector,
}

impl fmt::Debug for DirectStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DirectStore")
            .field("root", &self.root)
            .field("namespace_id", &self.inner.namespace_id())
            .field("protector", &self.protector)
            .field("revision_count", &self.inner.retained_revisions().len())
            .finish()
    }
}

impl DirectStore {
    /// Opens the source catalog and recovers every referenced protected object.
    pub(crate) fn open(root: &Path) -> Result<Self, String> {
        // Refuse lost catalog state before the legacy initializer can write.
        crate::catalog_presence::check_before_open(root)?;
        let canonical_root = fs::canonicalize(root)
            .map_err(|error| format!("DIRECT_ROOT_CANONICALIZE_ERROR:{error}"))?;
        let inner = plaintext::DirectStore::open(&canonical_root)?;
        let namespace_id = sha256::decode_digest(&inner.namespace_id())
            .ok_or_else(|| "DIRECT_NAMESPACE_INVALID".to_owned())?;
        let revision_root = canonical_root.join(REVISION_DIRECTORY);
        let protector = RevisionProtector::open(namespace_id, &revision_root)?;
        let store = Self {
            root: canonical_root,
            inner,
            protector,
        };
        if store.protector.encrypts_new_objects() {
            store.migrate_referenced_plaintext()?;
        }
        Ok(store)
    }

    /// Stable namespace identity retained with the data root.
    pub(crate) fn namespace_id(&self) -> String {
        self.inner.namespace_id()
    }

    /// Revision bytes and saved preparation both precede source publication.
    pub(crate) fn index_file(&mut self, path: &Path) -> Result<IndexedSource, String> {
        let namespace = self.inner.namespace_id();
        let root = &self.root;
        let protector = &self.protector;
        let indexed = self.inner.index_file_with_writer(path, &mut |_, source, bytes| {
            preparation_store::persist_source(root, protector, &namespace, source, bytes)
        })?;
        Ok(indexed)
    }

    /// Every batch member crosses the same revision/preparation barrier.
    pub(crate) fn index_directory(
        &mut self,
        directory: &Path,
    ) -> Result<Vec<IndexedSource>, String> {
        let namespace = self.inner.namespace_id();
        let root = &self.root;
        let protector = &self.protector;
        let indexed = self.inner.index_directory_with_writer(directory, &mut |_, source, bytes| {
            preparation_store::persist_source(root, protector, &namespace, source, bytes)
        })?;
        Ok(indexed)
    }

    /// Explicitly prepares a retained revision without rereading a current path.
    /// Missing objects may be reconstructed; conflicting immutable objects fail.
    pub(crate) fn prepare_revision(&self, revision_id: &str) -> Result<Option<&'static str>, String> {
        crate::catalog_presence::require_existing(&self.root)?;
        self.inner.verify_control()?;
        let metadata = self.inner.retained_revision(revision_id)
            .ok_or_else(|| "DIRECT_REVISION_NOT_FOUND".to_owned())?;
        let bytes = Zeroizing::new(self.read_revision_detailed(&metadata)?);
        preparation_store::persist(&self.root, &self.protector, &self.inner.namespace_id(), &metadata, &bytes)
    }

    /// Retires one source without deleting retained revision objects.
    pub(crate) fn retire_source(&mut self, source_id: &str) -> Result<SourceSummary, String> {
        let summary = self.inner.retire_source(source_id)?;
        Ok(summary)
    }

    /// Returns deterministic source summaries.
    pub(crate) fn list_sources(&self) -> Vec<SourceSummary> {
        self.inner.list_sources()
    }

    /// Searches verified revisions using saved profile-bound preparation.
    /// Missing or invalid preparation is an explicit gap, never a write-on-query.
    ///
    /// Canonical durable DIRECT spine gate (T17): the admitted registry
    /// (`list_sources`) freezes the denominator; every item reopens its exact
    /// verified retained revision, loads its stored representation/unit
    /// manifest, executes the bounded literal and revalidates each emitted
    /// range against the verified bytes. Entire-query corpus budget
    /// ([`CANONICAL_CORPUS_BUDGET`]) bounds sources, bytes, matches and gaps;
    /// every unattempted remainder is an explicit typed gap so an indexed
    /// top-k style narrowing can never become a complete claim (invariant 6).
    /// Partial/degraded outcomes stay typed with `complete = false`, never
    /// success (invariant 15). The query path performs no durable writes.
    pub(crate) fn search(
        &self,
        query: &str,
        ascii_insensitive: bool,
    ) -> Result<StoreSearchResult, String> {
        validate_query(query).map_err(str::to_owned)?;
        let namespace = self.inner.namespace_id();
        let active = self
            .inner
            .list_sources()
            .into_iter()
            .filter(|source| source.active)
            .collect::<Vec<_>>();
        let mut matches = Vec::new();
        let mut gaps = Vec::new();
        let mut searched_sources = 0_usize;
        let mut scanned_bytes = 0_u64;
        let mut complete = true;
        let mut match_limit_reached = false;

        for (index, source) in active.iter().enumerate() {
            if matches.len() >= CANONICAL_CORPUS_BUDGET.max_matches {
                complete = false;
                match_limit_reached = true;
                Self::push_remaining_gaps(
                    &active,
                    index,
                    SPINE_GAP_MATCH_LIMIT,
                    &mut gaps,
                    &mut complete,
                );
                break;
            }
            if index >= CANONICAL_CORPUS_BUDGET.max_sources {
                Self::push_remaining_gaps(
                    &active,
                    index,
                    SPINE_GAP_BUDGET_EXHAUSTED,
                    &mut gaps,
                    &mut complete,
                );
                break;
            }
            if scanned_bytes
                .checked_add(source.byte_length)
                .is_none_or(|total| total > CANONICAL_CORPUS_BUDGET.max_source_bytes)
            {
                Self::push_remaining_gaps(
                    &active,
                    index,
                    SPINE_GAP_BUDGET_EXHAUSTED,
                    &mut gaps,
                    &mut complete,
                );
                break;
            }
            scanned_bytes = scanned_bytes.saturating_add(source.byte_length);
            let metadata = RevisionMetadata {
                source_id: source.source_id.clone(),
                revision_id: source.revision_id.clone(),
                content_digest: source.content_digest.clone(),
                byte_length: source.byte_length,
            };
            let bytes = match self.read_verified_revision(&metadata) {
                Ok(bytes) => bytes,
                Err(reason) => {
                    if gaps.len() >= MAX_SEARCH_GAPS {
                        complete = false;
                        break;
                    }
                    gaps.push(StoreGap {
                        source_id: source.source_id.clone(),
                        revision_id: source.revision_id.clone(),
                        reason,
                    });
                    complete = false;
                    continue;
                }
            };
            let Ok(text) = String::from_utf8(bytes) else {
                if gaps.len() >= MAX_SEARCH_GAPS {
                    complete = false;
                    break;
                }
                gaps.push(StoreGap {
                    source_id: source.source_id.clone(),
                    revision_id: source.revision_id.clone(),
                    reason: "DIRECT_REVISION_NOT_UTF8",
                });
                complete = false;
                continue;
            };
            let ScanResult {
                matches: source_matches,
                coverage,
            } = match preparation_store::load(&self.root, &self.protector, &namespace, &metadata)
                .and_then(|saved| scan_prepared(&text, &saved, query, ascii_insensitive))
            {
                Ok(result) => result,
                Err(reason) => {
                    complete = false;
                    if gaps.len() >= MAX_SEARCH_GAPS {
                        break;
                    }
                    gaps.push(StoreGap {
                        source_id: source.source_id.clone(),
                        revision_id: source.revision_id.clone(),
                        reason,
                    });
                    continue;
                }
            };
            // Source-backed validation: every emitted range is rechecked against
            // the verified retained bytes before it can enter evidence fields.
            // A mismatch becomes an explicit per-source gap, never an unproven
            // match and never a silent substitution of current-path bytes.
            let mut validation_failed = false;
            for item in &source_matches {
                if validate_source_backed_match(
                    &text,
                    query,
                    ascii_insensitive,
                    item.byte_start,
                    item.byte_end,
                )
                .is_err()
                {
                    validation_failed = true;
                    break;
                }
            }
            if validation_failed {
                complete = false;
                if gaps.len() >= CANONICAL_CORPUS_BUDGET.max_gaps {
                    break;
                }
                gaps.push(StoreGap {
                    source_id: source.source_id.clone(),
                    revision_id: source.revision_id.clone(),
                    reason: SPINE_GAP_VALIDATION_FAILED,
                });
                continue;
            }
            searched_sources = searched_sources.saturating_add(1);
            if !coverage.complete {
                complete = false;
                match_limit_reached = coverage.match_limit_reached;
            }
            for item in source_matches {
                if matches.len() >= CANONICAL_CORPUS_BUDGET.max_matches {
                    complete = false;
                    match_limit_reached = true;
                    break;
                }
                let start = u64::try_from(item.byte_start)
                    .map_err(|_| "DIRECT_MATCH_OFFSET_OVERFLOW".to_owned())?;
                let end = u64::try_from(item.byte_end)
                    .map_err(|_| "DIRECT_MATCH_OFFSET_OVERFLOW".to_owned())?;
                let evidence_id = sha256::hex(&sha256::digest_parts(
                    b"eliot-search/direct-evidence/v1",
                    &[
                        source.source_id.as_bytes(),
                        source.revision_id.as_bytes(),
                        source.content_digest.as_bytes(),
                        &start.to_be_bytes(),
                        &end.to_be_bytes(),
                    ],
                ));
                matches.push(StoredMatch {
                    source_id: source.source_id.clone(),
                    revision_id: source.revision_id.clone(),
                    content_digest: source.content_digest.clone(),
                    path_digest: source.path_digest.clone(),
                    evidence_id,
                    byte_start: item.byte_start,
                    byte_end: item.byte_end,
                    line: item.line,
                    column_bytes: item.column_bytes,
                });
            }
            if match_limit_reached {
                Self::push_remaining_gaps(
                    &active,
                    index.saturating_add(1),
                    SPINE_GAP_MATCH_LIMIT,
                    &mut gaps,
                    &mut complete,
                );
                break;
            }
        }

        Ok(StoreSearchResult {
            matches,
            gaps,
            registered_sources: self.inner.list_sources().len(),
            active_sources: active.len(),
            searched_sources,
            complete,
            match_limit_reached,
        })
    }

    /// Records every unattempted admitted source as an explicit typed gap.
    ///
    /// The denominator is frozen from the admitted registry; skipping the
    /// remainder without gaps would narrow it like an indexed top-k view
    /// (invariant 6). The gap ceiling keeps the record bounded; hitting it
    /// still leaves `complete = false` so the outcome stays degraded typed
    /// data, never success (invariant 15). No durable write occurs here.
    fn push_remaining_gaps(
        active: &[SourceSummary],
        from: usize,
        reason: &'static str,
        gaps: &mut Vec<StoreGap>,
        complete: &mut bool,
    ) {
        *complete = false;
        for source in active.iter().skip(from) {
            if gaps.len() >= CANONICAL_CORPUS_BUDGET.max_gaps.min(MAX_SEARCH_GAPS) {
                return;
            }
            gaps.push(StoreGap {
                source_id: source.source_id.clone(),
                revision_id: source.revision_id.clone(),
                reason,
            });
        }
    }

    /// Reopens the event log and verifies every referenced revision object.
    pub(crate) fn verify(&self) -> Result<StoreVerification, String> {
        // Verification must not repair missing files by initializing them.
        crate::catalog_presence::require_existing(&self.root)?;
        self.inner.verify_control()?;
        let mut total_revision_bytes = 0_u64;
        let mut verified_revisions = 0_usize;
        for metadata in self.inner.retained_revisions() {
            let bytes = self
                .read_revision_detailed(&metadata)
                .map_err(|error| format!("DIRECT_REVISION_VERIFY_FAILED:{error}"))?;
            total_revision_bytes = total_revision_bytes
                .checked_add(
                    u64::try_from(bytes.len())
                        .map_err(|_| "DIRECT_TOTAL_BYTES_OVERFLOW".to_owned())?,
                )
                .ok_or_else(|| "DIRECT_TOTAL_BYTES_OVERFLOW".to_owned())?;
            verified_revisions = verified_revisions.saturating_add(1);
        }
        let sources = self.inner.list_sources();
        Ok(StoreVerification {
            source_events: self.inner.source_event_count(),
            registered_sources: sources.len(),
            active_sources: sources.iter().filter(|source| source.active).count(),
            referenced_revisions: self.inner.retained_revisions().len(),
            verified_revisions,
            total_revision_bytes,
        })
    }

    /// Reads one bounded exact range from a verified immutable revision.
    pub(crate) fn read_revision_range(
        &self,
        revision_id: &str,
        byte_start: u64,
        byte_end: u64,
    ) -> Result<RevisionSlice, String> {
        let metadata = self.inner.retained_revision(revision_id)
            .ok_or_else(|| "DIRECT_REVISION_NOT_FOUND".to_owned())?;
        if byte_start >= byte_end || byte_end > metadata.byte_length {
            return Err("DIRECT_REVISION_RANGE_INVALID".to_owned());
        }
        if byte_end.saturating_sub(byte_start)
            > u64::try_from(MAX_READ_RANGE_BYTES).unwrap_or(u64::MAX)
        {
            return Err("DIRECT_REVISION_RANGE_TOO_LARGE".to_owned());
        }
        let bytes = self.read_revision_detailed(&metadata)?;
        let start = usize::try_from(byte_start)
            .map_err(|_| "DIRECT_REVISION_RANGE_INVALID".to_owned())?;
        let end = usize::try_from(byte_end)
            .map_err(|_| "DIRECT_REVISION_RANGE_INVALID".to_owned())?;
        Ok(RevisionSlice {
            revision_id: metadata.revision_id,
            content_digest: metadata.content_digest,
            byte_start,
            byte_end,
            bytes: bytes[start..end].to_vec(),
        })
    }

    fn migrate_referenced_plaintext(&self) -> Result<(), String> {
        for metadata in self.inner.retained_revisions() {
            self.seal_revision(&metadata)?;
        }
        Ok(())
    }

    fn seal_revision(&self, metadata: &RevisionMetadata) -> Result<(), String> {
        verify_revision_identity(metadata)?;
        let path = legacy_path(&self.root, &metadata.revision_id)?;
        if path.exists() {
            let plaintext = Zeroizing::new(read_plaintext_path(&path, metadata)?);
            revision_writer::persist_verified(
                &self.root,
                &self.protector,
                metadata,
                &plaintext,
            )?;
            remove_plaintext_after_readback(&path)
        } else {
            // Opening an existing protected revision never needs current-path bytes.
            let _verified = Zeroizing::new(self.read_revision_detailed(metadata)?);
            Ok(())
        }
    }

    fn read_verified_revision(
        &self,
        metadata: &RevisionMetadata,
    ) -> Result<Vec<u8>, &'static str> {
        self.read_revision_detailed(metadata)
            .map_err(|error| classify_revision_error(&error))
    }


}

fn classify_revision_error(error: &str) -> &'static str {
    if error.contains("KEY_BINDING") || error.contains("KEY_MISSING") {
        "DIRECT_REVISION_KEY_UNAVAILABLE"
    } else if error.contains("DPAPI") || error.contains("ENCRYPTION") {
        "DIRECT_REVISION_DECRYPT_FAILED"
    } else if error.contains("CONTENT") {
        "DIRECT_REVISION_CONTENT_MISMATCH"
    } else if error.contains("LENGTH") || error.contains("TOO_LARGE") {
        "DIRECT_REVISION_LENGTH_MISMATCH"
    } else if error.contains("MISSING") {
        "DIRECT_REVISION_MISSING"
    } else if error.contains("OBJECT") || error.contains("FILE") {
        "DIRECT_REVISION_OBJECT_INVALID"
    } else {
        "DIRECT_REVISION_READ_FAILED"
    }
}

fn verify_plaintext(metadata: &RevisionMetadata, bytes: &[u8]) -> Result<(), String> {
    let length = u64::try_from(bytes.len())
        .map_err(|_| "DIRECT_REVISION_LENGTH_MISMATCH".to_owned())?;
    if length != metadata.byte_length {
        return Err("DIRECT_REVISION_LENGTH_MISMATCH".to_owned());
    }
    if sha256::hex(&sha256::digest(bytes)) != metadata.content_digest {
        return Err("DIRECT_REVISION_CONTENT_MISMATCH".to_owned());
    }
    verify_revision_identity(metadata)
}

#[cfg(test)]
mod spine_gate_tests {
    use super::*;
    use crate::development::DataRootGuard;
    use crate::revision_protection::{TestCredentialGuard, lock_unit_vault_for_test};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

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
        // The query path is read-only: revision/preparation objects are loaded
        // and verified, never created, repaired or re-published. Qdrant is not
        // consulted on this path, so its absence cannot block explicit DIRECT.
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
}

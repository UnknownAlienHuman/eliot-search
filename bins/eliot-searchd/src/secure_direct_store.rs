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
use crate::direct_preparation::{scan_prepared, validate_query};
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
pub(crate) use preparation_store::{PreparationBatch, PreparationCursor};

use storage_io::{
    legacy_path, protected_path,
    read_plaintext_path, read_regular_file, remove_plaintext_after_readback,
};

pub(crate) use plaintext::{
    IndexedSource, RevisionSlice, SourceSummary, StoreGap, StoreSearchResult,
    StoreVerification, StoredMatch,
};

const REVISION_DIRECTORY: &str = "revisions";
const MAX_REVISION_OBJECT_BYTES: usize = 65 * 1024 * 1024;
const MAX_SEARCH_GAPS: usize = 100_000;
const MAX_READ_RANGE_BYTES: usize = 8 * 1024 * 1024;

/// DIRECT catalog with a platform-specific prepublication revision writer.
pub(crate) struct DirectStore {
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
        let mut store = Self {
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
    pub(crate) fn prepare_revision(&mut self, revision_id: &str) -> Result<Option<&'static str>, String> {
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
        let mut complete = true;
        let mut match_limit_reached = false;

        for source in &active {
            if matches.len() >= crate::development::MAX_SCAN_MATCHES {
                complete = false;
                match_limit_reached = true;
                break;
            }
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
            let text = match String::from_utf8(bytes) {
                Ok(text) => text,
                Err(_) => {
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
                }
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
            searched_sources = searched_sources.saturating_add(1);
            if !coverage.complete {
                complete = false;
                match_limit_reached = coverage.match_limit_reached;
            }
            for item in source_matches {
                if matches.len() >= crate::development::MAX_SCAN_MATCHES {
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
            revision_id: metadata.revision_id.clone(),
            content_digest: metadata.content_digest.clone(),
            byte_start,
            byte_end,
            bytes: bytes[start..end].to_vec(),
        })
    }

    fn migrate_referenced_plaintext(&mut self) -> Result<(), String> {
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

    fn read_revision_detailed(
        &self,
        metadata: &RevisionMetadata,
    ) -> Result<Vec<u8>, String> {
        verify_revision_identity(metadata)?;
        let protected = protected_path(&self.root, &metadata.revision_id)?;
        let plaintext = legacy_path(&self.root, &metadata.revision_id)?;
        let bytes = if protected.exists() {
            let object = read_regular_file(
                &protected,
                MAX_REVISION_OBJECT_BYTES,
                "DIRECT_REVISION_PROTECTED_READ_ERROR",
            )?;
            self.protector.unprotect(
                &object,
                &metadata.revision_id,
                &metadata.content_digest,
                metadata.byte_length,
            )?
        } else if plaintext.exists() {
            #[cfg(windows)]
            {
                return Err("DIRECT_REVISION_PROTECTION_INCOMPLETE".to_owned());
            }
            #[cfg(not(windows))]
            {
                read_plaintext_path(&plaintext, metadata)?
            }
        } else {
            return Err("DIRECT_REVISION_MISSING".to_owned());
        };
        verify_plaintext(metadata, &bytes)?;
        Ok(bytes)
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

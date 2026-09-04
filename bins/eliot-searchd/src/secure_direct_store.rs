//! Revision-protected facade over the append-only development DIRECT catalog.
//!
//! The existing catalog remains the authority for source identity, lifecycle,
//! and the SHA-256 chained event log. On Windows this facade migrates every
//! referenced plaintext revision into a DPAPI object, removes the plaintext
//! only after exact protected readback, and serves search/ranges from protected
//! objects. Other platforms retain the explicit plaintext development profile.

#![allow(
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::similar_names,
    clippy::too_many_lines
)]

use core::fmt;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::development::{ScanResult, scan_text};
use crate::plaintext_direct_store as plaintext;
use crate::revision_protection::RevisionProtector;
use crate::sha256;

#[path = "secure_direct_store_storage_io.rs"]
mod storage_io;

use storage_io::{
    legacy_path, load_event_count, load_inventory, persist_immutable_object,
    protected_path, read_plaintext_path, read_regular_file,
    remove_plaintext_after_readback,
};

pub(crate) use plaintext::{
    IndexedSource, RevisionSlice, SourceSummary, StoreGap, StoreSearchResult,
    StoreVerification, StoredMatch,
};

const REVISION_DIRECTORY: &str = "revisions";
const MAX_REVISION_OBJECT_BYTES: usize = 65 * 1024 * 1024;
const MAX_SEARCH_GAPS: usize = 100_000;
const MAX_READ_RANGE_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
struct RevisionMetadata {
    source_id: String,
    revision_id: String,
    content_digest: String,
    byte_length: u64,
}

/// DIRECT store that adds platform revision-object protection to the existing
/// append-only source catalog.
pub(crate) struct DirectStore {
    root: PathBuf,
    inner: plaintext::DirectStore,
    protector: RevisionProtector,
    inventory: BTreeMap<String, RevisionMetadata>,
}

impl fmt::Debug for DirectStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DirectStore")
            .field("root", &self.root)
            .field("namespace_id", &self.inner.namespace_id())
            .field("protector", &self.protector)
            .field("revision_count", &self.inventory.len())
            .finish()
    }
}

impl DirectStore {
    /// Opens the source catalog and recovers every referenced protected object.
    pub(crate) fn open(root: &Path) -> Result<Self, String> {
        let canonical_root = fs::canonicalize(root)
            .map_err(|error| format!("DIRECT_ROOT_CANONICALIZE_ERROR:{error}"))?;
        let inner = plaintext::DirectStore::open(&canonical_root)?;
        let namespace_id = sha256::decode_digest(&inner.namespace_id())
            .ok_or_else(|| "DIRECT_NAMESPACE_INVALID".to_owned())?;
        let revision_root = canonical_root.join(REVISION_DIRECTORY);
        let protector = RevisionProtector::open(namespace_id, &revision_root)?;
        let inventory = load_inventory(&canonical_root)?;
        let mut store = Self {
            root: canonical_root,
            inner,
            protector,
            inventory,
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

    /// Indexes one exact same-handle file snapshot and seals its revision.
    pub(crate) fn index_file(&mut self, path: &Path) -> Result<IndexedSource, String> {
        let indexed = self.inner.index_file(path)?;
        self.seal_indexed(&indexed)?;
        self.refresh_inventory()?;
        Ok(indexed)
    }

    /// Deterministically indexes and seals every regular non-link file below a
    /// directory.
    pub(crate) fn index_directory(
        &mut self,
        directory: &Path,
    ) -> Result<Vec<IndexedSource>, String> {
        let indexed = self.inner.index_directory(directory)?;
        for source in &indexed {
            self.seal_indexed(source)?;
        }
        self.refresh_inventory()?;
        Ok(indexed)
    }

    /// Retires one source without deleting retained revision objects.
    pub(crate) fn retire_source(&mut self, source_id: &str) -> Result<SourceSummary, String> {
        let summary = self.inner.retire_source(source_id)?;
        self.refresh_inventory()?;
        Ok(summary)
    }

    /// Returns deterministic source summaries.
    pub(crate) fn list_sources(&self) -> Vec<SourceSummary> {
        self.inner.list_sources()
    }

    /// Searches active protected revisions after exact decrypt/readback.
    pub(crate) fn search(
        &self,
        query: &str,
        ascii_insensitive: bool,
    ) -> Result<StoreSearchResult, String> {
        if query.is_empty() {
            return Err("DIRECT_QUERY_EMPTY".to_owned());
        }
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
            } = scan_text(&text, query, ascii_insensitive)?;
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
        let reopened = plaintext::DirectStore::open(&self.root)?;
        if reopened.namespace_id() != self.inner.namespace_id()
            || reopened.list_sources() != self.inner.list_sources()
        {
            return Err("DIRECT_CONTROL_READBACK_MISMATCH".to_owned());
        }
        let inventory = load_inventory(&self.root)?;
        if inventory != self.inventory {
            return Err("DIRECT_CONTROL_READBACK_MISMATCH".to_owned());
        }
        let mut total_revision_bytes = 0_u64;
        let mut verified_revisions = 0_usize;
        for metadata in inventory.values() {
            let bytes = self
                .read_revision_detailed(metadata)
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
            source_events: load_event_count(&self.root)?,
            registered_sources: sources.len(),
            active_sources: sources.iter().filter(|source| source.active).count(),
            referenced_revisions: inventory.len(),
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
        let metadata = self
            .inventory
            .get(revision_id)
            .ok_or_else(|| "DIRECT_REVISION_NOT_FOUND".to_owned())?;
        if byte_start >= byte_end || byte_end > metadata.byte_length {
            return Err("DIRECT_REVISION_RANGE_INVALID".to_owned());
        }
        if byte_end.saturating_sub(byte_start)
            > u64::try_from(MAX_READ_RANGE_BYTES).unwrap_or(u64::MAX)
        {
            return Err("DIRECT_REVISION_RANGE_TOO_LARGE".to_owned());
        }
        let bytes = self.read_revision_detailed(metadata)?;
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

    fn refresh_inventory(&mut self) -> Result<(), String> {
        self.inventory = load_inventory(&self.root)?;
        Ok(())
    }

    fn migrate_referenced_plaintext(&mut self) -> Result<(), String> {
        let revisions = self.inventory.values().cloned().collect::<Vec<_>>();
        for metadata in revisions {
            self.seal_revision(&metadata)?;
        }
        Ok(())
    }

    fn seal_indexed(&self, source: &IndexedSource) -> Result<(), String> {
        if !self.protector.encrypts_new_objects() {
            return Ok(());
        }
        self.seal_revision(&RevisionMetadata {
            source_id: source.source_id.clone(),
            revision_id: source.revision_id.clone(),
            content_digest: source.content_digest.clone(),
            byte_length: source.byte_length,
        })
    }

    fn seal_revision(&self, metadata: &RevisionMetadata) -> Result<(), String> {
        verify_revision_identity(metadata)?;
        let plaintext_path = legacy_path(&self.root, &metadata.revision_id)?;
        let protected_path = protected_path(&self.root, &metadata.revision_id)?;

        if protected_path.exists() {
            let protected = read_regular_file(
                &protected_path,
                MAX_REVISION_OBJECT_BYTES,
                "DIRECT_REVISION_PROTECTED_READ_ERROR",
            )?;
            let readback = self.protector.unprotect(
                &protected,
                &metadata.revision_id,
                &metadata.content_digest,
                metadata.byte_length,
            )?;
            verify_plaintext(metadata, &readback)?;
            if plaintext_path.exists() {
                let plaintext = read_plaintext_path(&plaintext_path, metadata)?;
                if plaintext != readback {
                    return Err("DIRECT_REVISION_MIGRATION_CONFLICT".to_owned());
                }
                remove_plaintext_after_readback(&plaintext_path)?;
            }
            return Ok(());
        }

        let plaintext = read_plaintext_path(&plaintext_path, metadata)?;
        let protected = self.protector.protect(
            &metadata.revision_id,
            &metadata.content_digest,
            &plaintext,
        )?;
        persist_immutable_object(&protected_path, &protected)?;
        let observed = read_regular_file(
            &protected_path,
            MAX_REVISION_OBJECT_BYTES,
            "DIRECT_REVISION_PROTECTED_READ_ERROR",
        )?;
        let readback = self.protector.unprotect(
            &observed,
            &metadata.revision_id,
            &metadata.content_digest,
            metadata.byte_length,
        )?;
        if readback != plaintext {
            return Err("DIRECT_REVISION_PROTECTED_READBACK_MISMATCH".to_owned());
        }
        remove_plaintext_after_readback(&plaintext_path)
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

fn verify_revision_identity(metadata: &RevisionMetadata) -> Result<(), String> {
    let content_digest = sha256::decode_digest(&metadata.content_digest)
        .ok_or_else(|| "DIRECT_REVISION_CONTENT_MISMATCH".to_owned())?;
    let expected = sha256::hex(&sha256::digest_parts(
        b"eliot-search/direct-revision-id/v1",
        &[
            metadata.source_id.as_bytes(),
            &content_digest,
            &metadata.byte_length.to_be_bytes(),
        ],
    ));
    if expected == metadata.revision_id {
        Ok(())
    } else {
        Err("DIRECT_REVISION_ID_MISMATCH".to_owned())
    }
}

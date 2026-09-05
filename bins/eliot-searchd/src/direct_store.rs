//! Concrete development DIRECT corpus.
//!
//! The store uses an OS-locked data root, immutable revision objects, a
//! SHA-256-chained append-only source log, exact readback verification, stable
//! native file identity where the platform exposes one, and bounded literal
//! search over retained revisions. Its default writer is plaintext; primary
//! composition supplies a verified protected writer before catalog publication.

use std::collections::BTreeMap;
use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::development::{MAX_SCAN_INPUT_BYTES, ScanResult, scan_text};
use crate::sha256;

#[path = "direct_store_ingest.rs"]
mod ingest;

const CONTROL_DIRECTORY: &str = "control";
const REVISION_DIRECTORY: &str = "revisions";
const NAMESPACE_FILE: &str = "namespace.id";
const SOURCE_LOG_FILE: &str = "source-events.log";
const SOURCE_LOG_HEADER: &str = "ELIOT_SEARCH_SOURCE_EVENTS_V1";
const ZERO_DIGEST: &str = "0000000000000000000000000000000000000000000000000000000000000000";
const MAX_LOG_BYTES: u64 = 512 * 1024 * 1024;
const MAX_LOG_LINE_BYTES: usize = 256 * 1024;
const MAX_SOURCE_EVENTS: usize = 2_000_000;
const MAX_DIRECTORY_FILES: usize = 100_000;
const MAX_DIRECTORY_DEPTH: usize = 128;
const MAX_SEARCH_GAPS: usize = 100_000;
const MAX_READ_RANGE_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SourceState {
    Active,
    Retired,
}

impl SourceState {
    fn tag(self) -> &'static str {
        match self {
            Self::Active => "A",
            Self::Retired => "R",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "A" => Some(Self::Active),
            "R" => Some(Self::Retired),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum IdentityStrength {
    Native,
    PathBound,
}

impl IdentityStrength {
    fn tag(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::PathBound => "path-bound",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "native" => Some(Self::Native),
            "path-bound" => Some(Self::PathBound),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SourceRecord {
    sequence: u64,
    previous_digest: String,
    operation_id: String,
    state: SourceState,
    source_id: String,
    revision_id: String,
    content_digest: String,
    byte_length: u64,
    file_identity_digest: String,
    path_digest: String,
    identity_strength: IdentityStrength,
    record_digest: String,
}

impl SourceRecord {
    fn canonical_without_digest(&self) -> String {
        [
            "V1".to_owned(),
            self.sequence.to_string(),
            self.previous_digest.clone(),
            self.operation_id.clone(),
            self.state.tag().to_owned(),
            self.source_id.clone(),
            self.revision_id.clone(),
            self.content_digest.clone(),
            self.byte_length.to_string(),
            self.file_identity_digest.clone(),
            self.path_digest.clone(),
            self.identity_strength.tag().to_owned(),
        ]
        .join("\t")
    }

    fn line(&self) -> String {
        format!("{}\t{}\n", self.canonical_without_digest(), self.record_digest)
    }
}

#[derive(Clone, Debug)]
struct RecordDraft {
    operation_id: String,
    state: SourceState,
    source_id: String,
    revision_id: String,
    content_digest: String,
    byte_length: u64,
    file_identity_digest: String,
    path_digest: String,
    identity_strength: IdentityStrength,
}

#[derive(Clone, Debug, Default)]
struct RegistryState {
    last_sequence: u64,
    last_digest: String,
    latest: BTreeMap<String, SourceRecord>,
    operations: BTreeMap<String, String>,
    revisions: BTreeMap<String, SourceRecord>,
    event_count: usize,
}

/// Result of indexing one exact final-handle file snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct IndexedSource {
    pub(crate) source_id: String,
    pub(crate) revision_id: String,
    pub(crate) content_digest: String,
    pub(crate) path_digest: String,
    pub(crate) byte_length: u64,
    pub(crate) identity_strength: &'static str,
    pub(crate) changed: bool,
}

/// Exact source summary without persisted path text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SourceSummary {
    pub(crate) source_id: String,
    pub(crate) revision_id: String,
    pub(crate) content_digest: String,
    pub(crate) path_digest: String,
    pub(crate) byte_length: u64,
    pub(crate) identity_strength: &'static str,
    pub(crate) active: bool,
    pub(crate) sequence: u64,
}

/// One source-backed exact match over an immutable verified revision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct StoredMatch {
    pub(crate) source_id: String,
    pub(crate) revision_id: String,
    pub(crate) content_digest: String,
    pub(crate) path_digest: String,
    pub(crate) evidence_id: String,
    pub(crate) byte_start: usize,
    pub(crate) byte_end: usize,
    pub(crate) line: usize,
    pub(crate) column_bytes: usize,
}

/// Explicit source-level gap during corpus search or verification.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct StoreGap {
    pub(crate) source_id: String,
    pub(crate) revision_id: String,
    pub(crate) reason: &'static str,
}

/// Truthful corpus-search result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct StoreSearchResult {
    pub(crate) matches: Vec<StoredMatch>,
    pub(crate) gaps: Vec<StoreGap>,
    pub(crate) registered_sources: usize,
    pub(crate) active_sources: usize,
    pub(crate) searched_sources: usize,
    pub(crate) complete: bool,
    pub(crate) match_limit_reached: bool,
}

/// Exact readback-verification result over every referenced immutable revision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct StoreVerification {
    pub(crate) source_events: usize,
    pub(crate) registered_sources: usize,
    pub(crate) active_sources: usize,
    pub(crate) referenced_revisions: usize,
    pub(crate) verified_revisions: usize,
    pub(crate) total_revision_bytes: u64,
}

/// Exact bounded revision slice.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RevisionSlice {
    pub(crate) revision_id: String,
    pub(crate) content_digest: String,
    pub(crate) byte_start: u64,
    pub(crate) byte_end: u64,
    pub(crate) bytes: Vec<u8>,
}

#[derive(Clone, Debug)]
struct FileSnapshot {
    canonical_path: PathBuf,
    path_digest: String,
    file_identity_digest: String,
    identity_strength: IdentityStrength,
    content_digest: String,
    bytes: Vec<u8>,
}

/// Development retained-revision corpus under one already locked data root.
#[derive(Clone, Debug)]
pub(crate) struct DirectStore {
    root: PathBuf,
    namespace_id: [u8; 32],
    registry: RegistryState,
}

impl DirectStore {
    /// Opens or initializes the content-minimized control layout.
    pub(crate) fn open(root: &Path) -> Result<Self, String> {
        let canonical_root = fs::canonicalize(root)
            .map_err(|error| format!("DIRECT_ROOT_CANONICALIZE_ERROR:{error}"))?;
        ensure_directory(&canonical_root)?;
        let control = canonical_root.join(CONTROL_DIRECTORY);
        let revisions = canonical_root.join(REVISION_DIRECTORY);
        ensure_child_directory(&control)?;
        ensure_child_directory(&revisions)?;
        let namespace_id = load_or_create_namespace(&canonical_root, &control)?;
        let log_path = control.join(SOURCE_LOG_FILE);
        initialize_log(&log_path)?;
        let registry = load_registry(&log_path)?;
        Ok(Self {
            root: canonical_root,
            namespace_id,
            registry,
        })
    }

    /// Stable namespace identity retained with the data root.
    pub(crate) fn namespace_id(&self) -> String {
        sha256::hex(&self.namespace_id)
    }

    /// Indexes one exact same-handle snapshot using the development writer.
    pub(crate) fn index_file(&mut self, path: &Path) -> Result<IndexedSource, String> {
        self.index_file_with_writer(path, &mut |store, source, bytes| {
            store.persist_revision(&source.revision_id, &source.content_digest, bytes)
        })
    }

    /// Indexes a bounded directory batch using the development writer.
    pub(crate) fn index_directory(
        &mut self,
        directory: &Path,
    ) -> Result<Vec<IndexedSource>, String> {
        self.index_directory_with_writer(directory, &mut |store, source, bytes| {
            store.persist_revision(&source.revision_id, &source.content_digest, bytes)
        })
    }

    /// Retires one source from future corpus search without deleting revisions.
    pub(crate) fn retire_source(&mut self, source_id: &str) -> Result<SourceSummary, String> {
        validate_digest_text(source_id, "DIRECT_SOURCE_ID_INVALID")?;
        let existing = self
            .registry
            .latest
            .get(source_id)
            .cloned()
            .ok_or_else(|| "DIRECT_SOURCE_NOT_FOUND".to_owned())?;
        if existing.state == SourceState::Retired {
            return Ok(summary(&existing));
        }
        let operation_id = sha256::hex(&sha256::digest_parts(
            b"eliot-search/direct-retire-operation/v1",
            &[
                source_id.as_bytes(),
                existing.revision_id.as_bytes(),
                existing.record_digest.as_bytes(),
            ],
        ));
        let draft = RecordDraft {
            operation_id,
            state: SourceState::Retired,
            source_id: existing.source_id,
            revision_id: existing.revision_id,
            content_digest: existing.content_digest,
            byte_length: existing.byte_length,
            file_identity_digest: existing.file_identity_digest,
            path_digest: existing.path_digest,
            identity_strength: existing.identity_strength,
        };
        let record = self.append_drafts(vec![draft])?
            .pop()
            .ok_or_else(|| "DIRECT_RETIRE_EMPTY_RESULT".to_owned())?;
        Ok(summary(&record))
    }

    /// Returns deterministic source summaries.
    pub(crate) fn list_sources(&self) -> Vec<SourceSummary> {
        self.registry.latest.values().map(summary).collect()
    }

    /// Searches every active immutable revision with exact readback verification.
    pub(crate) fn search(
        &self,
        query: &str,
        ascii_insensitive: bool,
    ) -> Result<StoreSearchResult, String> {
        if query.is_empty() {
            return Err("DIRECT_QUERY_EMPTY".to_owned());
        }
        let active = self
            .registry
            .latest
            .values()
            .filter(|record| record.state == SourceState::Active)
            .cloned()
            .collect::<Vec<_>>();
        let mut matches = Vec::new();
        let mut gaps = Vec::new();
        let mut searched_sources = 0_usize;
        let mut complete = true;
        let mut match_limit_reached = false;

        for record in &active {
            if matches.len() >= crate::development::MAX_SCAN_MATCHES {
                complete = false;
                match_limit_reached = true;
                break;
            }
            let bytes = match self.read_verified_revision(record) {
                Ok(bytes) => bytes,
                Err(reason) => {
                    if gaps.len() >= MAX_SEARCH_GAPS {
                        complete = false;
                        break;
                    }
                    gaps.push(StoreGap {
                        source_id: record.source_id.clone(),
                        revision_id: record.revision_id.clone(),
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
                        source_id: record.source_id.clone(),
                        revision_id: record.revision_id.clone(),
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
                        record.source_id.as_bytes(),
                        record.revision_id.as_bytes(),
                        record.content_digest.as_bytes(),
                        &start.to_be_bytes(),
                        &end.to_be_bytes(),
                    ],
                ));
                matches.push(StoredMatch {
                    source_id: record.source_id.clone(),
                    revision_id: record.revision_id.clone(),
                    content_digest: record.content_digest.clone(),
                    path_digest: record.path_digest.clone(),
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
            registered_sources: self.registry.latest.len(),
            active_sources: active.len(),
            searched_sources,
            complete,
            match_limit_reached,
        })
    }

    /// Verifies the log chain and every unique referenced immutable revision.
    pub(crate) fn verify(&self) -> Result<StoreVerification, String> {
        let log_path = self.root.join(CONTROL_DIRECTORY).join(SOURCE_LOG_FILE);
        let reloaded = load_registry(&log_path)?;
        if reloaded.last_sequence != self.registry.last_sequence
            || reloaded.last_digest != self.registry.last_digest
            || reloaded.latest != self.registry.latest
        {
            return Err("DIRECT_CONTROL_READBACK_MISMATCH".to_owned());
        }
        let mut verified_revisions = 0_usize;
        let mut total_revision_bytes = 0_u64;
        for record in reloaded.revisions.values() {
            let bytes = self
                .read_verified_revision(record)
                .map_err(str::to_owned)?;
            total_revision_bytes = total_revision_bytes
                .checked_add(
                    u64::try_from(bytes.len())
                        .map_err(|_| "DIRECT_TOTAL_BYTES_OVERFLOW".to_owned())?,
                )
                .ok_or_else(|| "DIRECT_TOTAL_BYTES_OVERFLOW".to_owned())?;
            verified_revisions = verified_revisions.saturating_add(1);
        }
        Ok(StoreVerification {
            source_events: reloaded.event_count,
            registered_sources: reloaded.latest.len(),
            active_sources: reloaded
                .latest
                .values()
                .filter(|record| record.state == SourceState::Active)
                .count(),
            referenced_revisions: reloaded.revisions.len(),
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
        validate_digest_text(revision_id, "DIRECT_REVISION_ID_INVALID")?;
        let record = self
            .registry
            .revisions
            .get(revision_id)
            .ok_or_else(|| "DIRECT_REVISION_NOT_FOUND".to_owned())?;
        if byte_start >= byte_end || byte_end > record.byte_length {
            return Err("DIRECT_REVISION_RANGE_INVALID".to_owned());
        }
        let length = byte_end - byte_start;
        if length > u64::try_from(MAX_READ_RANGE_BYTES).unwrap_or(u64::MAX) {
            return Err("DIRECT_REVISION_RANGE_TOO_LARGE".to_owned());
        }
        let bytes = self
            .read_verified_revision(record)
            .map_err(str::to_owned)?;
        let start = usize::try_from(byte_start)
            .map_err(|_| "DIRECT_REVISION_RANGE_INVALID".to_owned())?;
        let end = usize::try_from(byte_end)
            .map_err(|_| "DIRECT_REVISION_RANGE_INVALID".to_owned())?;
        Ok(RevisionSlice {
            revision_id: record.revision_id.clone(),
            content_digest: record.content_digest.clone(),
            byte_start,
            byte_end,
            bytes: bytes[start..end].to_vec(),
        })
    }

    fn persist_revision(
        &self,
        revision_id: &str,
        expected_content_digest: &str,
        bytes: &[u8],
    ) -> Result<(), String> {
        validate_digest_text(revision_id, "DIRECT_REVISION_ID_INVALID")?;
        validate_digest_text(
            expected_content_digest,
            "DIRECT_CONTENT_DIGEST_INVALID",
        )?;
        let shard = self
            .root
            .join(REVISION_DIRECTORY)
            .join(&revision_id[..2]);
        ensure_child_directory(&shard)?;
        let path = shard.join(format!("{revision_id}.bin"));
        if path.exists() {
            verify_revision_path(&path, expected_content_digest, bytes.len())?;
            return Ok(());
        }

        let temporary = shard.join(format!(
            ".{revision_id}.{}.tmp",
            std::process::id()
        ));
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)
            .map_err(|error| format!("DIRECT_REVISION_CREATE_ERROR:{error}"))?;
        let write_result = file
            .write_all(bytes)
            .and_then(|()| file.sync_all())
            .map_err(|error| format!("DIRECT_REVISION_WRITE_ERROR:{error}"));
        if let Err(error) = write_result {
            let _ = fs::remove_file(&temporary);
            return Err(error);
        }
        drop(file);
        if let Err(error) = fs::rename(&temporary, &path) {
            let _ = fs::remove_file(&temporary);
            if path.exists() {
                verify_revision_path(&path, expected_content_digest, bytes.len())?;
            } else {
                return Err(format!("DIRECT_REVISION_RENAME_ERROR:{error}"));
            }
        }
        sync_directory(&shard)?;
        verify_revision_path(&path, expected_content_digest, bytes.len())
    }

    fn append_drafts(&mut self, drafts: Vec<RecordDraft>) -> Result<Vec<SourceRecord>, String> {
        if drafts.is_empty() {
            return Ok(Vec::new());
        }
        if self
            .registry
            .event_count
            .saturating_add(drafts.len())
            > MAX_SOURCE_EVENTS
        {
            return Err("DIRECT_SOURCE_EVENT_LIMIT_EXCEEDED".to_owned());
        }
        let mut sequence = self.registry.last_sequence;
        let mut previous_digest = if self.registry.last_digest.is_empty() {
            ZERO_DIGEST.to_owned()
        } else {
            self.registry.last_digest.clone()
        };
        let mut records = Vec::new();
        let mut encoded = String::new();

        for draft in drafts {
            if let Some(existing_digest) = self.registry.operations.get(&draft.operation_id) {
                let existing = self
                    .registry
                    .latest
                    .get(&draft.source_id)
                    .ok_or_else(|| "DIRECT_OPERATION_READBACK_MISSING".to_owned())?;
                if existing_digest == &existing.record_digest
                    && existing.state == draft.state
                    && existing.revision_id == draft.revision_id
                    && existing.path_digest == draft.path_digest
                {
                    records.push(existing.clone());
                    continue;
                }
                return Err("DIRECT_OPERATION_CONFLICT".to_owned());
            }
            sequence = sequence
                .checked_add(1)
                .ok_or_else(|| "DIRECT_SOURCE_SEQUENCE_EXHAUSTED".to_owned())?;
            let mut record = SourceRecord {
                sequence,
                previous_digest: previous_digest.clone(),
                operation_id: draft.operation_id,
                state: draft.state,
                source_id: draft.source_id,
                revision_id: draft.revision_id,
                content_digest: draft.content_digest,
                byte_length: draft.byte_length,
                file_identity_digest: draft.file_identity_digest,
                path_digest: draft.path_digest,
                identity_strength: draft.identity_strength,
                record_digest: String::new(),
            };
            record.record_digest = sha256::hex(&sha256::digest(
                record.canonical_without_digest().as_bytes(),
            ));
            let line = record.line();
            if line.len() > MAX_LOG_LINE_BYTES {
                return Err("DIRECT_SOURCE_EVENT_TOO_LARGE".to_owned());
            }
            encoded.push_str(&line);
            previous_digest = record.record_digest.clone();
            records.push(record);
        }

        if encoded.is_empty() {
            return Ok(records);
        }
        let log_path = self.root.join(CONTROL_DIRECTORY).join(SOURCE_LOG_FILE);
        ensure_regular_file(&log_path)?;
        let mut file = OpenOptions::new()
            .append(true)
            .open(&log_path)
            .map_err(|error| format!("DIRECT_CONTROL_LOG_OPEN_ERROR:{error}"))?;
        file.write_all(encoded.as_bytes())
            .and_then(|()| file.sync_all())
            .map_err(|error| format!("DIRECT_CONTROL_LOG_WRITE_ERROR:{error}"))?;
        drop(file);
        sync_directory(&self.root.join(CONTROL_DIRECTORY))?;

        let reloaded = load_registry(&log_path)?;
        for record in &records {
            let observed = reloaded
                .operations
                .get(&record.operation_id)
                .ok_or_else(|| "DIRECT_CONTROL_READBACK_MISSING".to_owned())?;
            if observed != &record.record_digest {
                return Err("DIRECT_CONTROL_READBACK_MISMATCH".to_owned());
            }
        }
        self.registry = reloaded;
        Ok(records)
    }

    fn read_verified_revision(&self, record: &SourceRecord) -> Result<Vec<u8>, &'static str> {
        let path = revision_path(&self.root, &record.revision_id)
            .map_err(|_| "DIRECT_REVISION_ID_INVALID")?;
        let metadata = fs::symlink_metadata(&path)
            .map_err(|_| "DIRECT_REVISION_MISSING")?;
        if metadata.file_type().is_symlink() || is_reparse(&metadata) || !metadata.is_file() {
            return Err("DIRECT_REVISION_OBJECT_INVALID");
        }
        if metadata.len() != record.byte_length
            || metadata.len() > u64::try_from(MAX_SCAN_INPUT_BYTES).unwrap_or(u64::MAX)
        {
            return Err("DIRECT_REVISION_LENGTH_MISMATCH");
        }
        let mut file = File::open(&path).map_err(|_| "DIRECT_REVISION_OPEN_FAILED")?;
        let mut bytes = Vec::with_capacity(
            usize::try_from(record.byte_length)
                .map_err(|_| "DIRECT_REVISION_LENGTH_MISMATCH")?,
        );
        (&mut file)
            .take(u64::try_from(MAX_SCAN_INPUT_BYTES + 1).unwrap_or(u64::MAX))
            .read_to_end(&mut bytes)
            .map_err(|_| "DIRECT_REVISION_READ_FAILED")?;
        if bytes.len() != usize::try_from(record.byte_length).unwrap_or(usize::MAX) {
            return Err("DIRECT_REVISION_LENGTH_MISMATCH");
        }
        let content_digest = sha256::hex(&sha256::digest(&bytes));
        if content_digest != record.content_digest {
            return Err("DIRECT_REVISION_CONTENT_MISMATCH");
        }
        let expected_revision = sha256::hex(&sha256::digest_parts(
            b"eliot-search/direct-revision-id/v1",
            &[
                record.source_id.as_bytes(),
                &sha256::decode_digest(&record.content_digest)
                    .ok_or("DIRECT_REVISION_CONTENT_MISMATCH")?,
                &record.byte_length.to_be_bytes(),
            ],
        ));
        if expected_revision != record.revision_id {
            return Err("DIRECT_REVISION_ID_MISMATCH");
        }
        Ok(bytes)
    }
}

fn summary(record: &SourceRecord) -> SourceSummary {
    SourceSummary {
        source_id: record.source_id.clone(),
        revision_id: record.revision_id.clone(),
        content_digest: record.content_digest.clone(),
        path_digest: record.path_digest.clone(),
        byte_length: record.byte_length,
        identity_strength: record.identity_strength.tag(),
        active: record.state == SourceState::Active,
        sequence: record.sequence,
    }
}

fn load_or_create_namespace(root: &Path, control: &Path) -> Result<[u8; 32], String> {
    let path = control.join(NAMESPACE_FILE);
    if path.exists() {
        ensure_regular_file(&path)?;
        let mut value = String::new();
        File::open(&path)
            .and_then(|mut file| file.take(256).read_to_string(&mut value))
            .map_err(|error| format!("DIRECT_NAMESPACE_READ_ERROR:{error}"))?;
        return sha256::decode_digest(value.trim())
            .ok_or_else(|| "DIRECT_NAMESPACE_INVALID".to_owned());
    }

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "DIRECT_NAMESPACE_CLOCK_INVALID".to_owned())?
        .as_nanos();
    let root_bytes = path_identity_bytes(root);
    let namespace = sha256::digest_parts(
        b"eliot-search/direct-namespace/v1",
        &[
            &root_bytes,
            &u64::from(std::process::id()).to_be_bytes(),
            &timestamp.to_be_bytes(),
        ],
    );
    let encoded = format!("{}\n", sha256::hex(&namespace));
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&path)
        .map_err(|error| format!("DIRECT_NAMESPACE_CREATE_ERROR:{error}"))?;
    file.write_all(encoded.as_bytes())
        .and_then(|()| file.sync_all())
        .map_err(|error| format!("DIRECT_NAMESPACE_WRITE_ERROR:{error}"))?;
    drop(file);
    sync_directory(control)?;
    ensure_regular_file(&path)?;
    Ok(namespace)
}

fn initialize_log(path: &Path) -> Result<(), String> {
    if path.exists() {
        ensure_regular_file(path)?;
        return Ok(());
    }
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .map_err(|error| format!("DIRECT_CONTROL_LOG_CREATE_ERROR:{error}"))?;
    file.write_all(SOURCE_LOG_HEADER.as_bytes())
        .and_then(|()| file.write_all(b"\n"))
        .and_then(|()| file.sync_all())
        .map_err(|error| format!("DIRECT_CONTROL_LOG_WRITE_ERROR:{error}"))?;
    Ok(())
}

fn load_registry(path: &Path) -> Result<RegistryState, String> {
    ensure_regular_file(path)?;
    let metadata = fs::metadata(path)
        .map_err(|error| format!("DIRECT_CONTROL_LOG_METADATA_ERROR:{error}"))?;
    if metadata.len() > MAX_LOG_BYTES {
        return Err("DIRECT_CONTROL_LOG_TOO_LARGE".to_owned());
    }
    let file = File::open(path)
        .map_err(|error| format!("DIRECT_CONTROL_LOG_OPEN_ERROR:{error}"))?;
    let mut reader = BufReader::new(file);
    let mut header = String::new();
    reader
        .read_line(&mut header)
        .map_err(|error| format!("DIRECT_CONTROL_LOG_READ_ERROR:{error}"))?;
    if header.trim_end_matches(['\r', '\n']) != SOURCE_LOG_HEADER {
        return Err("DIRECT_CONTROL_LOG_HEADER_INVALID".to_owned());
    }

    let mut state = RegistryState {
        last_digest: ZERO_DIGEST.to_owned(),
        ..RegistryState::default()
    };
    let mut line = String::new();
    loop {
        line.clear();
        let read = reader
            .read_line(&mut line)
            .map_err(|error| format!("DIRECT_CONTROL_LOG_READ_ERROR:{error}"))?;
        if read == 0 {
            break;
        }
        if read > MAX_LOG_LINE_BYTES {
            return Err("DIRECT_CONTROL_LOG_LINE_TOO_LARGE".to_owned());
        }
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            return Err("DIRECT_CONTROL_LOG_EMPTY_EVENT".to_owned());
        }
        let fields = trimmed.split('\t').collect::<Vec<_>>();
        if fields.len() != 13 || fields[0] != "V1" {
            return Err("DIRECT_CONTROL_LOG_EVENT_INVALID".to_owned());
        }
        let sequence = fields[1]
            .parse::<u64>()
            .map_err(|_| "DIRECT_CONTROL_LOG_SEQUENCE_INVALID".to_owned())?;
        let expected_sequence = state
            .last_sequence
            .checked_add(1)
            .ok_or_else(|| "DIRECT_SOURCE_SEQUENCE_EXHAUSTED".to_owned())?;
        if sequence != expected_sequence || fields[2] != state.last_digest {
            return Err("DIRECT_CONTROL_LOG_CHAIN_INVALID".to_owned());
        }
        for index in [2_usize, 3, 5, 6, 7, 9, 10, 12] {
            validate_digest_text(fields[index], "DIRECT_CONTROL_LOG_DIGEST_INVALID")?;
        }
        let state_value = SourceState::parse(fields[4])
            .ok_or_else(|| "DIRECT_CONTROL_LOG_STATE_INVALID".to_owned())?;
        let byte_length = fields[8]
            .parse::<u64>()
            .map_err(|_| "DIRECT_CONTROL_LOG_LENGTH_INVALID".to_owned())?;
        let identity_strength = IdentityStrength::parse(fields[11])
            .ok_or_else(|| "DIRECT_CONTROL_LOG_IDENTITY_INVALID".to_owned())?;
        let canonical = fields[..12].join("\t");
        let record_digest = sha256::hex(&sha256::digest(canonical.as_bytes()));
        if record_digest != fields[12] {
            return Err("DIRECT_CONTROL_LOG_RECORD_DIGEST_INVALID".to_owned());
        }
        if state.operations.contains_key(fields[3]) {
            return Err("DIRECT_CONTROL_LOG_OPERATION_DUPLICATE".to_owned());
        }
        let record = SourceRecord {
            sequence,
            previous_digest: fields[2].to_owned(),
            operation_id: fields[3].to_owned(),
            state: state_value,
            source_id: fields[5].to_owned(),
            revision_id: fields[6].to_owned(),
            content_digest: fields[7].to_owned(),
            byte_length,
            file_identity_digest: fields[9].to_owned(),
            path_digest: fields[10].to_owned(),
            identity_strength,
            record_digest: fields[12].to_owned(),
        };
        if let Some(previous) = state.latest.get(&record.source_id) {
            if previous.file_identity_digest != record.file_identity_digest {
                return Err("DIRECT_CONTROL_LOG_SOURCE_COLLISION".to_owned());
            }
        }
        state.operations.insert(
            record.operation_id.clone(),
            record.record_digest.clone(),
        );
        state
            .revisions
            .entry(record.revision_id.clone())
            .or_insert_with(|| record.clone());
        state.latest.insert(record.source_id.clone(), record.clone());
        state.last_sequence = sequence;
        state.last_digest = record.record_digest;
        state.event_count = state.event_count.saturating_add(1);
        if state.event_count > MAX_SOURCE_EVENTS {
            return Err("DIRECT_SOURCE_EVENT_LIMIT_EXCEEDED".to_owned());
        }
    }
    Ok(state)
}

fn read_file_snapshot(
    path: &Path,
    data_root: &Path,
    remaining_batch_bytes: usize,
) -> Result<FileSnapshot, String> {
    let max_bytes = remaining_batch_bytes.min(MAX_SCAN_INPUT_BYTES);
    let limit_error = if remaining_batch_bytes < MAX_SCAN_INPUT_BYTES {
        "DIRECT_BATCH_BYTES_EXCEEDED"
    } else {
        "DIRECT_SOURCE_TOO_LARGE"
    };
    let initial = fs::symlink_metadata(path)
        .map_err(|error| format!("DIRECT_SOURCE_OPEN_ERROR:{error}"))?;
    if initial.file_type().is_symlink() || is_reparse(&initial) {
        return Err("DIRECT_SOURCE_LINK_DENIED".to_owned());
    }
    if !initial.is_file() {
        return Err("DIRECT_SOURCE_NOT_REGULAR".to_owned());
    }
    let canonical_path = fs::canonicalize(path)
        .map_err(|error| format!("DIRECT_SOURCE_CANONICALIZE_ERROR:{error}"))?;
    if canonical_path.starts_with(data_root) {
        return Err("DIRECT_SOURCE_INSIDE_DATA_ROOT".to_owned());
    }
    let final_metadata = fs::symlink_metadata(&canonical_path)
        .map_err(|error| format!("DIRECT_SOURCE_OPEN_ERROR:{error}"))?;
    if final_metadata.file_type().is_symlink()
        || is_reparse(&final_metadata)
        || !final_metadata.is_file()
    {
        return Err("DIRECT_SOURCE_FINAL_OBJECT_INVALID".to_owned());
    }

    let mut file = File::open(&canonical_path)
        .map_err(|error| format!("DIRECT_SOURCE_OPEN_ERROR:{error}"))?;
    let before = observe_source_file(&file, &canonical_path)?;
    if before.byte_length > u64::try_from(max_bytes).unwrap_or(u64::MAX) {
        return Err(limit_error.to_owned());
    }
    let mut bytes = Vec::with_capacity(
        usize::try_from(before.byte_length)
            .map_err(|_| limit_error.to_owned())?,
    );
    (&mut file)
        .take(u64::try_from(max_bytes + 1).unwrap_or(u64::MAX))
        .read_to_end(&mut bytes)
        .map_err(|error| format!("DIRECT_SOURCE_READ_ERROR:{error}"))?;
    if bytes.len() > max_bytes {
        return Err(limit_error.to_owned());
    }
    let after = observe_source_file(&file, &canonical_path)?;
    if before != after
        || bytes.len() != usize::try_from(before.byte_length).unwrap_or(usize::MAX)
    {
        return Err("DIRECT_SOURCE_CHANGED_DURING_READ".to_owned());
    }
    let path_digest = sha256::hex(&sha256::digest(&path_identity_bytes(&canonical_path)));
    let content_digest = sha256::hex(&sha256::digest(&bytes));
    Ok(FileSnapshot {
        canonical_path,
        path_digest,
        file_identity_digest: before.file_identity_digest,
        identity_strength: before.identity_strength,
        content_digest,
        bytes,
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SourceFileObservation {
    byte_length: u64,
    modified_nanos: Option<u128>,
    file_identity_digest: String,
    identity_strength: IdentityStrength,
}

fn observe_source_file(file: &File, canonical_path: &Path) -> Result<SourceFileObservation, String> {
    let metadata = file
        .metadata()
        .map_err(|error| format!("DIRECT_SOURCE_METADATA_ERROR:{error}"))?;
    if !metadata.is_file() || is_reparse(&metadata) {
        return Err("DIRECT_SOURCE_FINAL_OBJECT_INVALID".to_owned());
    }
    let modified_nanos = metadata
        .modified()
        .ok()
        .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos());
    let (identity_material, identity_strength) =
        platform_file_identity(file, &metadata, canonical_path)?;
    Ok(SourceFileObservation {
        byte_length: metadata.len(),
        modified_nanos,
        file_identity_digest: sha256::hex(&sha256::digest_parts(
            b"eliot-search/direct-file-identity/v1",
            &[&identity_material],
        )),
        identity_strength,
    })
}

#[cfg(unix)]
fn platform_file_identity(
    _file: &File,
    metadata: &Metadata,
    _path: &Path,
) -> Result<(Vec<u8>, IdentityStrength), String> {
    use std::os::unix::fs::MetadataExt;
    let mut bytes = Vec::with_capacity(16);
    bytes.extend_from_slice(&metadata.dev().to_be_bytes());
    bytes.extend_from_slice(&metadata.ino().to_be_bytes());
    Ok((bytes, IdentityStrength::Native))
}

#[cfg(windows)]
fn platform_file_identity(
    file: &File,
    _metadata: &Metadata,
    _path: &Path,
) -> Result<(Vec<u8>, IdentityStrength), String> {
    let observed = eliot_searchd::native_file::observe(file)
        .map_err(|error| error.code().to_owned())?;
    // Preserve the existing NTFS identity encoding without a path-only fallback.
    Ok((observed.legacy_identity_bytes().to_vec(), IdentityStrength::Native))
}

#[cfg(not(any(unix, windows)))]
fn platform_file_identity(
    _file: &File,
    _metadata: &Metadata,
    path: &Path,
) -> Result<(Vec<u8>, IdentityStrength), String> {
    Ok((path_identity_bytes(path), IdentityStrength::PathBound))
}

#[cfg(unix)]
fn path_identity_bytes(path: &Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    path.as_os_str().as_bytes().to_vec()
}

#[cfg(windows)]
fn path_identity_bytes(path: &Path) -> Vec<u8> {
    use std::os::windows::ffi::OsStrExt;
    path.as_os_str()
        .encode_wide()
        .flat_map(u16::to_le_bytes)
        .collect()
}

#[cfg(not(any(unix, windows)))]
fn path_identity_bytes(path: &Path) -> Vec<u8> {
    path.to_string_lossy().as_bytes().to_vec()
}

fn collect_regular_files(
    directory: &Path,
    data_root: &Path,
    depth: usize,
    output: &mut Vec<PathBuf>,
) -> Result<(), String> {
    if depth > MAX_DIRECTORY_DEPTH {
        return Err("DIRECT_DIRECTORY_DEPTH_EXCEEDED".to_owned());
    }
    let metadata = fs::symlink_metadata(directory)
        .map_err(|error| format!("DIRECT_DIRECTORY_READ_ERROR:{error}"))?;
    if metadata.file_type().is_symlink() || is_reparse(&metadata) || !metadata.is_dir() {
        return Err("DIRECT_DIRECTORY_LINK_OR_TYPE_DENIED".to_owned());
    }
    let canonical = fs::canonicalize(directory)
        .map_err(|error| format!("DIRECT_DIRECTORY_CANONICALIZE_ERROR:{error}"))?;
    if canonical == data_root {
        return Ok(());
    }
    let mut entries = fs::read_dir(&canonical)
        .map_err(|error| format!("DIRECT_DIRECTORY_READ_ERROR:{error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("DIRECT_DIRECTORY_READ_ERROR:{error}"))?;
    entries.sort_by_key(|entry| path_identity_bytes(&entry.path()));
    for entry in entries {
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| format!("DIRECT_DIRECTORY_ENTRY_ERROR:{error}"))?;
        if metadata.file_type().is_symlink() || is_reparse(&metadata) {
            return Err("DIRECT_DIRECTORY_LINK_DENIED".to_owned());
        }
        if metadata.is_dir() {
            let canonical_child = fs::canonicalize(&path)
                .map_err(|error| format!("DIRECT_DIRECTORY_CANONICALIZE_ERROR:{error}"))?;
            if canonical_child == data_root || canonical_child.starts_with(data_root) {
                continue;
            }
            collect_regular_files(&canonical_child, data_root, depth + 1, output)?;
        } else if metadata.is_file() {
            if output.len() >= MAX_DIRECTORY_FILES {
                return Err("DIRECT_DIRECTORY_FILE_LIMIT_EXCEEDED".to_owned());
            }
            output.push(path);
        } else {
            return Err("DIRECT_DIRECTORY_SPECIAL_OBJECT_DENIED".to_owned());
        }
    }
    Ok(())
}

fn revision_path(root: &Path, revision_id: &str) -> Result<PathBuf, String> {
    validate_digest_text(revision_id, "DIRECT_REVISION_ID_INVALID")?;
    Ok(root
        .join(REVISION_DIRECTORY)
        .join(&revision_id[..2])
        .join(format!("{revision_id}.bin")))
}

fn verify_revision_path(
    path: &Path,
    expected_content_digest: &str,
    expected_length: usize,
) -> Result<(), String> {
    ensure_regular_file(path)?;
    let metadata = fs::metadata(path)
        .map_err(|error| format!("DIRECT_REVISION_METADATA_ERROR:{error}"))?;
    if metadata.len() != u64::try_from(expected_length).unwrap_or(u64::MAX) {
        return Err("DIRECT_REVISION_LENGTH_MISMATCH".to_owned());
    }
    let mut bytes = Vec::with_capacity(expected_length);
    File::open(path)
        .and_then(|mut file| {
            file.take(u64::try_from(MAX_SCAN_INPUT_BYTES + 1).unwrap_or(u64::MAX))
                .read_to_end(&mut bytes)
        })
        .map_err(|error| format!("DIRECT_REVISION_READ_ERROR:{error}"))?;
    if bytes.len() != expected_length
        || sha256::hex(&sha256::digest(&bytes)) != expected_content_digest
    {
        return Err("DIRECT_REVISION_CONTENT_MISMATCH".to_owned());
    }
    Ok(())
}

fn validate_digest_text(value: &str, error: &'static str) -> Result<(), String> {
    if sha256::decode_digest(value).is_some() {
        Ok(())
    } else {
        Err(error.to_owned())
    }
}

fn ensure_directory(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("DIRECT_DIRECTORY_METADATA_ERROR:{error}"))?;
    if metadata.file_type().is_symlink() || is_reparse(&metadata) || !metadata.is_dir() {
        return Err("DIRECT_DIRECTORY_INVALID".to_owned());
    }
    Ok(())
}

fn ensure_child_directory(path: &Path) -> Result<(), String> {
    if !path.exists() {
        fs::create_dir(path)
            .map_err(|error| format!("DIRECT_DIRECTORY_CREATE_ERROR:{error}"))?;
    }
    ensure_directory(path)
}

fn ensure_regular_file(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("DIRECT_FILE_METADATA_ERROR:{error}"))?;
    if metadata.file_type().is_symlink() || is_reparse(&metadata) || !metadata.is_file() {
        return Err("DIRECT_FILE_INVALID".to_owned());
    }
    Ok(())
}

#[cfg(windows)]
fn is_reparse(metadata: &Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_reparse(_metadata: &Metadata) -> bool {
    false
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> Result<(), String> {
    File::open(path)
        .and_then(|file| file.sync_all())
        .map_err(|error| format!("DIRECT_DIRECTORY_SYNC_ERROR:{error}"))
}

#[cfg(not(unix))]
fn sync_directory(_path: &Path) -> Result<(), String> {
    Ok(())
}

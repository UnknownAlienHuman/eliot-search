//! Concrete development DIRECT corpus.
//!
//! The store uses an OS-locked data root, immutable revision objects, a
//! SHA-256-chained append-only source log, exact readback verification, stable
//! native file identity where the platform exposes one, and bounded literal
//! search over retained revisions. Its default writer is plaintext; primary
//! composition supplies a verified protected writer before catalog publication.

use std::collections::BTreeMap;
use std::fs::{self, File, Metadata, OpenOptions};
use std::io::Write;
#[cfg(test)]
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::development::MAX_SCAN_INPUT_BYTES;
use crate::safe_reader_adapter::{AdapterError, FullReadError};
use crate::{safe_reader_adapter, sha256};
use search_safe_reader::SafeReadError;

#[path = "direct_store_ingest.rs"]
mod ingest;
#[path = "direct_store_catalog.rs"]
mod catalog;
use catalog::{load_registry, read_namespace};
pub use catalog::{RevisionMetadata, verify_revision_identity};

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SourceState {
    Active,
    Retired,
}

impl SourceState {
    const fn tag(self) -> &'static str {
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
    const fn tag(self) -> &'static str {
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

#[derive(Clone, Debug, Default, Eq, PartialEq)]
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
pub struct IndexedSource {
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
pub struct SourceSummary {
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
pub struct StoredMatch {
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
pub struct StoreGap {
    pub(crate) source_id: String,
    pub(crate) revision_id: String,
    pub(crate) reason: &'static str,
}

/// Truthful corpus-search result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoreSearchResult {
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
pub struct StoreVerification {
    pub(crate) source_events: usize,
    pub(crate) registered_sources: usize,
    pub(crate) active_sources: usize,
    pub(crate) referenced_revisions: usize,
    pub(crate) verified_revisions: usize,
    pub(crate) total_revision_bytes: u64,
}

/// Exact bounded revision slice.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RevisionSlice {
    pub(crate) revision_id: String,
    pub(crate) content_digest: String,
    pub(crate) byte_start: u64,
    pub(crate) byte_end: u64,
    pub(crate) bytes: Vec<u8>,
}

#[derive(Clone, Debug)]
struct FileSnapshot {
    path_digest: String,
    file_identity_digest: String,
    identity_strength: IdentityStrength,
    content_digest: String,
    bytes: Vec<u8>,
}

/// Development retained-revision corpus under one already locked data root.
#[derive(Clone, Debug)]
pub struct DirectStore {
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
    #[cfg(test)]
    pub(crate) fn index_file(&mut self, path: &Path) -> Result<IndexedSource, String> {
        self.index_file_with_writer(path, &mut |store, source, bytes| {
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

    /// Verifies the log chain and every unique referenced immutable revision.
    #[cfg(test)]
    pub(crate) fn verify(&self) -> Result<StoreVerification, String> {
        self.verify_control()?;
        let reloaded = &self.registry;
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

    #[cfg(test)]
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
        #[cfg(unix)]
        sync_directory(&shard)?;
        #[cfg(not(unix))]
        sync_directory(&shard);
        verify_revision_path(&path, expected_content_digest, bytes.len())
    }

    fn append_drafts(&mut self, drafts: Vec<RecordDraft>) -> Result<Vec<SourceRecord>, String> {
        if drafts.is_empty() {
            return Ok(Vec::new());
        }
        // Revalidate after revision-object preparation and before touching the log.
        // This also fences retirement, which does not traverse the ingest planner.
        self.verify_control()?;
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
            record.record_digest.clone_into(&mut previous_digest);
            records.push(record);
        }

        if encoded.is_empty() {
            return Ok(records);
        }
        self.append_encoded_records(&encoded, records)
    }

    /// Appends already validated encoded events to the control log and verifies
    /// exact readback before replacing the in-memory registry.
    fn append_encoded_records(
        &mut self,
        encoded: &str,
        records: Vec<SourceRecord>,
    ) -> Result<Vec<SourceRecord>, String> {
        let log_path = self.root.join(CONTROL_DIRECTORY).join(SOURCE_LOG_FILE);
        ensure_regular_file(&log_path)?;
        let mut file = OpenOptions::new()
            .append(true)
            .open(&log_path)
            .map_err(|error| format!("DIRECT_CONTROL_LOG_OPEN_ERROR:{error}"))?;
        let current_bytes = file.metadata()
            .map_err(|error| format!("DIRECT_CONTROL_LOG_METADATA_ERROR:{error}"))?.len();
        if current_bytes.checked_add(encoded.len() as u64)
            .is_none_or(|bytes| bytes > MAX_LOG_BYTES)
        {
            return Err("DIRECT_CONTROL_LOG_TOO_LARGE".to_owned());
        }
        file.write_all(encoded.as_bytes())
            .and_then(|()| file.sync_all())
            .map_err(|error| format!("DIRECT_CONTROL_LOG_WRITE_ERROR:{error}"))?;
        drop(file);
        #[cfg(unix)]
        sync_directory(&self.root.join(CONTROL_DIRECTORY))?;
        #[cfg(not(unix))]
        sync_directory(&self.root.join(CONTROL_DIRECTORY));

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

    #[cfg(test)]
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
        verify_revision_identity(&RevisionMetadata::from(record))
            .map_err(|_| "DIRECT_REVISION_ID_MISMATCH")?;
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
        return read_namespace(&path);
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
    #[cfg(unix)]
    sync_directory(control)?;
    #[cfg(not(unix))]
    sync_directory(control);
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
    // Primary ingestion reads through the shared safe-reader kernel: the
    // platform adapter proves final-object/ancestor containment on the
    // opened handle and the kernel revalidates the same handle after the
    // read. No path-first byte product exists on this path.
    let absolute = absolutize_source(path)?;
    let parent = absolute
        .parent()
        .ok_or_else(|| "DIRECT_SOURCE_PATH_DENIED".to_owned())?;
    let full = safe_reader_adapter::read_full_file_via_kernel(&absolute, parent, max_bytes)
        .map_err(|error| map_full_read_error(error, limit_error))?;
    if full.canonical_final.starts_with(data_root) {
        return Err("DIRECT_SOURCE_INSIDE_DATA_ROOT".to_owned());
    }
    if full.bytes.len() > max_bytes
        || u64::try_from(full.bytes.len()).unwrap_or(u64::MAX) != full.source_bytes
    {
        return Err("DIRECT_SOURCE_CHANGED_DURING_READ".to_owned());
    }
    let identity_strength = if full.identity_native {
        IdentityStrength::Native
    } else {
        IdentityStrength::PathBound
    };
    let path_digest = sha256::hex(&sha256::digest(&path_identity_bytes(&full.canonical_final)));
    let content_digest = sha256::hex(&sha256::digest(&full.bytes));
    Ok(FileSnapshot {
        path_digest,
        file_identity_digest: sha256::hex(&sha256::digest_parts(
            b"eliot-search/direct-file-identity/v1",
            &[&full.identity_material],
        )),
        identity_strength,
        content_digest,
        bytes: full.bytes,
    })
}

/// Resolves a caller-supplied source locator against the process directory.
///
/// Historical callers pass relative CLI paths; the kernel admits absolute
/// locators only, so relativize here instead of inside the adapter.
fn absolutize_source(path: &Path) -> Result<PathBuf, String> {
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    let current = std::env::current_dir().map_err(|_| "DIRECT_SOURCE_ACCESS_DENIED".to_owned())?;
    Ok(current.join(path))
}

/// Maps a kernel-verified read failure to the DIRECT namespace without
/// paths, bytes or raw OS error text.
fn map_full_read_error(error: FullReadError, limit_error: &str) -> String {
    match error {
        FullReadError::Adapter(adapter) => match adapter {
            AdapterError::PathDenied => "DIRECT_SOURCE_PATH_DENIED".to_owned(),
            AdapterError::LinkDenied | AdapterError::AncestorReparseDenied => {
                "DIRECT_SOURCE_LINK_DENIED".to_owned()
            }
            AdapterError::EscapeDenied => "DIRECT_SOURCE_ESCAPE_DENIED".to_owned(),
            AdapterError::RootRelocated => "DIRECT_SOURCE_ROOT_RELOCATED".to_owned(),
            AdapterError::NotRegular => "DIRECT_SOURCE_NOT_REGULAR".to_owned(),
            AdapterError::FinalObjectInvalid | AdapterError::DeviceDenied => {
                "DIRECT_SOURCE_FINAL_OBJECT_INVALID".to_owned()
            }
            AdapterError::HardlinkDenied => "DIRECT_SOURCE_HARDLINK_DENIED".to_owned(),
            AdapterError::AccessDenied => "DIRECT_SOURCE_ACCESS_DENIED".to_owned(),
            AdapterError::TooLarge => limit_error.to_owned(),
            AdapterError::ReceiptDenied => {
                "DIRECT_SOURCE_METADATA_ERROR:SAFE_ADAPTER_RECEIPT_DENIED".to_owned()
            }
        },
        FullReadError::Kernel(kernel) => match kernel {
            SafeReadError::RangeOutsideSource
            | SafeReadError::EofMismatch
            | SafeReadError::ReadLengthMismatch
            | SafeReadError::StableIdentityMismatch
            | SafeReadError::HandleChangedDuringRead
            | SafeReadError::BackendFailure => {
                "DIRECT_SOURCE_CHANGED_DURING_READ".to_owned()
            }
            SafeReadError::RootIdentityMismatch => "DIRECT_SOURCE_ESCAPE_DENIED".to_owned(),
            SafeReadError::UnsupportedFileKind => {
                "DIRECT_SOURCE_FINAL_OBJECT_INVALID".to_owned()
            }
            SafeReadError::ReparseBoundaryDenied => "DIRECT_SOURCE_LINK_DENIED".to_owned(),
            SafeReadError::SecurityDenied | SafeReadError::SecurityRevisionMismatch => {
                "DIRECT_SOURCE_ACCESS_DENIED".to_owned()
            }
            SafeReadError::SourceSizeInvalid => limit_error.to_owned(),
            SafeReadError::Cancelled => "DIRECT_SOURCE_READ_CANCELLED".to_owned(),
            SafeReadError::InvalidLimits
            | SafeReadError::InvalidPathToken
            | SafeReadError::InvalidReadLength
            | SafeReadError::RangeOverflow
            | SafeReadError::InvalidRetryPolicy
            | SafeReadError::ReceiptMissing => "DIRECT_SOURCE_READ_INVALID".to_owned(),
        },
    }
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

#[cfg(test)]
fn revision_path(root: &Path, revision_id: &str) -> Result<PathBuf, String> {
    validate_digest_text(revision_id, "DIRECT_REVISION_ID_INVALID")?;
    Ok(root
        .join(REVISION_DIRECTORY)
        .join(&revision_id[..2])
        .join(format!("{revision_id}.bin")))
}

#[cfg(test)]
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
        .and_then(|file| {
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
const fn sync_directory(_path: &Path) {}

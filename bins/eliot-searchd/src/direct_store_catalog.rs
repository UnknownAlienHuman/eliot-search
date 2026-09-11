//! One replay of the legacy source journal owns live sources and retained revisions.
//!
//! Filesystem framing and stable-file checks remain in the daemon. Event schema,
//! digest-chain, idempotency and immutable-identity replay semantics are delegated
//! to `search-source-registry`; readers never maintain a second revision map.

use std::io::{BufRead, BufReader, Read};

#[path = "control_migration.rs"]
mod migration;

use super::{
    CONTROL_DIRECTORY, DirectDigest, DirectStore, File, MAX_LOG_BYTES,
    MAX_LOG_LINE_BYTES, MAX_SCAN_INPUT_BYTES, MAX_SOURCE_EVENTS, NAMESPACE_FILE,
    Path, RegistryState, SOURCE_LOG_FILE, SOURCE_LOG_HEADER, SourceRecord,
    SourceState, ZERO_DIGEST, ensure_regular_file, is_reparse, sha256,
};

/// Exact immutable object binding. Global event sequence is not a source revision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RevisionMetadata {
    pub(crate) source_id: String,
    pub(crate) revision_id: String,
    pub(crate) content_digest: String,
    pub(crate) byte_length: u64,
}

impl From<&SourceRecord> for RevisionMetadata {
    fn from(record: &SourceRecord) -> Self {
        Self {
            source_id: record.source_id.clone(),
            revision_id: record.revision_id.clone(),
            content_digest: record.content_digest.clone(),
            byte_length: record.byte_length,
        }
    }
}

impl DirectStore {
    /// Projects the sole accepted revision inventory without retaining another map.
    pub(crate) fn retained_revisions(
        &self,
    ) -> impl ExactSizeIterator<Item = RevisionMetadata> + '_ {
        self.registry.revisions.values().map(RevisionMetadata::from)
    }

    /// Ordered continuation over the same inventory, without cloning or rescanning its prefix.
    pub(crate) fn retained_revisions_after(
        &self,
        after: Option<&str>,
    ) -> impl Iterator<Item = RevisionMetadata> + '_ {
        use std::ops::Bound::{Excluded, Unbounded};
        let lower = after.map_or(Unbounded, |value| Excluded(value.to_owned()));
        self.registry
            .revisions
            .range::<String, _>((lower, Unbounded))
            .map(|(_, record)| RevisionMetadata::from(record))
    }

    /// Snapshot identity for resumable preparation, not an authorization receipt.
    pub(crate) fn preparation_catalog_digest(&self) -> [u8; 32] {
        sha256::digest_parts(
            b"eliot-search/direct-preparation-catalog/v1",
            &[
                &self.namespace_id,
                &self.registry.last_sequence.to_be_bytes(),
                self.registry.last_digest.as_bytes(),
            ],
        )
    }

    pub(crate) fn retained_revision(&self, revision_id: &str) -> Option<RevisionMetadata> {
        self.registry
            .revisions
            .get(revision_id)
            .map(RevisionMetadata::from)
    }

    pub(crate) const fn source_event_count(&self) -> usize {
        self.registry.event_count
    }

    /// Read-only verification. Missing files are never initialized by this path.
    /// The caller still owns the exclusive data-root guard throughout the call.
    pub(crate) fn verify_control(&self) -> Result<(), String> {
        let control = self.root.join(CONTROL_DIRECTORY);
        if read_namespace(&control.join(NAMESPACE_FILE))? != self.namespace_id
            || load_registry(&control.join(SOURCE_LOG_FILE))? != self.registry
        {
            return Err("DIRECT_CONTROL_READBACK_MISMATCH".to_owned());
        }
        Ok(())
    }
}

pub(super) fn read_namespace(path: &Path) -> Result<[u8; 32], String> {
    ensure_regular_file(path)?;
    let file = File::open(path).map_err(|error| format!("DIRECT_NAMESPACE_READ_ERROR:{error}"))?;
    let metadata = file
        .metadata()
        .map_err(|error| format!("DIRECT_NAMESPACE_READ_ERROR:{error}"))?;
    if !metadata.is_file() || is_reparse(&metadata) || metadata.len() > 256 {
        return Err("DIRECT_NAMESPACE_INVALID".to_owned());
    }
    let mut value = String::new();
    file.take(257)
        .read_to_string(&mut value)
        .map_err(|error| format!("DIRECT_NAMESPACE_READ_ERROR:{error}"))?;
    if value.len() > 256 || u64::try_from(value.len()).ok() != Some(metadata.len()) {
        return Err("DIRECT_NAMESPACE_INVALID".to_owned());
    }
    sha256::decode_digest(value.trim()).ok_or_else(|| "DIRECT_NAMESPACE_INVALID".to_owned())
}

pub fn verify_revision_identity(metadata: &RevisionMetadata) -> Result<(), String> {
    search_source_registry::verify_legacy_direct_revision_identity::<DirectDigest>(
        &metadata.source_id,
        &metadata.revision_id,
        &metadata.content_digest,
        metadata.byte_length,
    )
    .map_err(|error| error.code().to_owned())
}

pub(super) fn load_registry(path: &Path) -> Result<RegistryState, String> {
    replay_registry(path, |_, _| Ok(()))
}

/// A read-only migration observer shares the full owner replay validator.
/// Observed entries remain provisional until this function returns successfully.
fn replay_registry(
    path: &Path,
    mut observe: impl FnMut(&SourceRecord, Option<&SourceRecord>) -> Result<(), String>,
) -> Result<RegistryState, String> {
    ensure_regular_file(path)?;
    let file = File::open(path).map_err(|error| format!("DIRECT_CONTROL_LOG_OPEN_ERROR:{error}"))?;
    let before = file
        .metadata()
        .map_err(|error| format!("DIRECT_CONTROL_LOG_METADATA_ERROR:{error}"))?;
    if !before.is_file() || is_reparse(&before) {
        return Err("DIRECT_FILE_INVALID".to_owned());
    }
    if before.len() > MAX_LOG_BYTES {
        return Err("DIRECT_CONTROL_LOG_TOO_LARGE".to_owned());
    }
    let mut reader = BufReader::new(file);
    let mut consumed = 0_u64;
    let mut line = String::new();
    read_log_line(&mut reader, &mut line, &mut consumed)?;
    if line.trim_end_matches(['\r', '\n']) != SOURCE_LOG_HEADER {
        return Err("DIRECT_CONTROL_LOG_HEADER_INVALID".to_owned());
    }
    let mut state = RegistryState::default();
    while read_log_line(&mut reader, &mut line, &mut consumed)? != 0 {
        if state.event_count >= MAX_SOURCE_EVENTS {
            return Err("DIRECT_SOURCE_EVENT_LIMIT_EXCEEDED".to_owned());
        }
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            return Err("DIRECT_CONTROL_LOG_EMPTY_EVENT".to_owned());
        }
        let record = state
            .parse_record::<DirectDigest>(
                trimmed,
                u64::try_from(MAX_SCAN_INPUT_BYTES).unwrap_or(u64::MAX),
            )
            .map_err(|error| error.code().to_owned())?;
        state
            .validate_record::<DirectDigest>(&record)
            .map_err(|error| error.code().to_owned())?;
        observe(&record, state.latest.get(&record.source_id))?;
        state.commit_record(record);
    }
    let after = reader
        .get_ref()
        .metadata()
        .map_err(|error| format!("DIRECT_CONTROL_LOG_METADATA_ERROR:{error}"))?;
    if consumed != before.len()
        || before.len() != after.len()
        || before.modified().ok() != after.modified().ok()
    {
        return Err("DIRECT_CONTROL_READBACK_MISMATCH".to_owned());
    }
    Ok(state)
}

fn read_log_line(
    reader: &mut impl BufRead,
    line: &mut String,
    consumed: &mut u64,
) -> Result<usize, String> {
    line.clear();
    let line_limit = u64::try_from(MAX_LOG_LINE_BYTES)
        .ok()
        .and_then(|value| value.checked_add(1))
        .ok_or_else(|| "DIRECT_CONTROL_LOG_LINE_TOO_LARGE".to_owned())?;
    let read = Read::take(&mut *reader, line_limit)
        .read_line(line)
        .map_err(|error| format!("DIRECT_CONTROL_LOG_READ_ERROR:{error}"))?;
    let read_bytes = u64::try_from(read)
        .map_err(|_| "DIRECT_CONTROL_LOG_TOO_LARGE".to_owned())?;
    *consumed = consumed
        .checked_add(read_bytes)
        .filter(|total| *total <= MAX_LOG_BYTES)
        .ok_or_else(|| "DIRECT_CONTROL_LOG_TOO_LARGE".to_owned())?;
    if read > MAX_LOG_LINE_BYTES {
        return Err("DIRECT_CONTROL_LOG_LINE_TOO_LARGE".to_owned());
    }
    if read != 0 && !line.ends_with('\n') {
        return Err("DIRECT_CONTROL_LOG_UNTERMINATED".to_owned());
    }
    Ok(read)
}

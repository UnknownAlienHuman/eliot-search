//! One replay of the legacy source journal owns live sources and retained revisions.
//!
//! This is the existing DIRECT catalog, not another store or a canonical redb
//! migration. Readers borrow its accepted state; they do not maintain a second
//! revision map or infer event counts by counting unvalidated lines.

use std::io::{BufRead, BufReader, Read};

use super::{
    CONTROL_DIRECTORY, DirectStore, File, IdentityStrength, MAX_LOG_BYTES,
    MAX_LOG_LINE_BYTES, MAX_SCAN_INPUT_BYTES, MAX_SOURCE_EVENTS, NAMESPACE_FILE,
    Path, RegistryState, SOURCE_LOG_FILE, SOURCE_LOG_HEADER, SourceRecord,
    SourceState, ZERO_DIGEST, ensure_regular_file, is_reparse, sha256,
    validate_digest_text,
};

/// Exact immutable object binding. Global event sequence is not a source revision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RevisionMetadata {
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
    pub(crate) fn retained_revisions(&self) -> impl ExactSizeIterator<Item = RevisionMetadata> + '_ {
        self.registry.revisions.values().map(RevisionMetadata::from)
    }

    pub(crate) fn retained_revision(&self, revision_id: &str) -> Option<RevisionMetadata> {
        self.registry.revisions.get(revision_id).map(RevisionMetadata::from)
    }

    pub(crate) fn source_event_count(&self) -> usize {
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
    let file = File::open(path)
        .map_err(|error| format!("DIRECT_NAMESPACE_READ_ERROR:{error}"))?;
    let metadata = file.metadata()
        .map_err(|error| format!("DIRECT_NAMESPACE_READ_ERROR:{error}"))?;
    if !metadata.is_file() || is_reparse(&metadata) || metadata.len() > 256 {
        return Err("DIRECT_NAMESPACE_INVALID".to_owned());
    }
    let mut value = String::new();
    file.take(257).read_to_string(&mut value)
        .map_err(|error| format!("DIRECT_NAMESPACE_READ_ERROR:{error}"))?;
    if value.len() > 256 || u64::try_from(value.len()).ok() != Some(metadata.len()) {
        return Err("DIRECT_NAMESPACE_INVALID".to_owned());
    }
    sha256::decode_digest(value.trim()).ok_or_else(|| "DIRECT_NAMESPACE_INVALID".to_owned())
}

pub(crate) fn verify_revision_identity(metadata: &RevisionMetadata) -> Result<(), String> {
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

pub(super) fn load_registry(path: &Path) -> Result<RegistryState, String> {
    ensure_regular_file(path)?;
    let file = File::open(path)
        .map_err(|error| format!("DIRECT_CONTROL_LOG_OPEN_ERROR:{error}"))?;
    let before = file.metadata()
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
    let mut state = RegistryState {
        last_digest: ZERO_DIGEST.to_owned(),
        ..RegistryState::default()
    };
    while read_log_line(&mut reader, &mut line, &mut consumed)? != 0 {
        if state.event_count >= MAX_SOURCE_EVENTS {
            return Err("DIRECT_SOURCE_EVENT_LIMIT_EXCEEDED".to_owned());
        }
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            return Err("DIRECT_CONTROL_LOG_EMPTY_EVENT".to_owned());
        }
        // Bound field allocation even for a line consisting entirely of tabs.
        let fields = trimmed.splitn(14, '\t').collect::<Vec<_>>();
        if fields.len() != 13 || fields[0] != "V1" {
            return Err("DIRECT_CONTROL_LOG_EVENT_INVALID".to_owned());
        }
        let sequence = fields[1].parse::<u64>()
            .map_err(|_| "DIRECT_CONTROL_LOG_SEQUENCE_INVALID".to_owned())?;
        let expected_sequence = state.last_sequence.checked_add(1)
            .ok_or_else(|| "DIRECT_SOURCE_SEQUENCE_EXHAUSTED".to_owned())?;
        if sequence != expected_sequence || fields[2] != state.last_digest {
            return Err("DIRECT_CONTROL_LOG_CHAIN_INVALID".to_owned());
        }
        for index in [2_usize, 3, 5, 6, 7, 9, 10, 12] {
            validate_digest_text(fields[index], "DIRECT_CONTROL_LOG_DIGEST_INVALID")?;
        }
        let state_value = SourceState::parse(fields[4])
            .ok_or_else(|| "DIRECT_CONTROL_LOG_STATE_INVALID".to_owned())?;
        let byte_length = fields[8].parse::<u64>()
            .map_err(|_| "DIRECT_CONTROL_LOG_LENGTH_INVALID".to_owned())?;
        if byte_length > u64::try_from(MAX_SCAN_INPUT_BYTES).unwrap_or(u64::MAX) {
            return Err("DIRECT_CONTROL_LOG_LENGTH_INVALID".to_owned());
        }
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
        verify_revision_identity(&RevisionMetadata::from(&record))?;
        if let Some(previous) = state.latest.get(&record.source_id) {
            if previous.file_identity_digest != record.file_identity_digest {
                return Err("DIRECT_CONTROL_LOG_SOURCE_COLLISION".to_owned());
            }
        }
        if let Some(previous) = state.revisions.get(&record.revision_id) {
            // Repeated A/B/A occurrences retain their original history; only the
            // immutable object binding must agree, not operation/path/sequence.
            if previous.source_id != record.source_id
                || previous.content_digest != record.content_digest
                || previous.byte_length != record.byte_length
            {
                return Err("DIRECT_CONTROL_LOG_REVISION_COLLISION".to_owned());
            }
        }
        state.operations.insert(record.operation_id.clone(), record.record_digest.clone());
        state.revisions.entry(record.revision_id.clone()).or_insert_with(|| record.clone());
        state.last_sequence = sequence;
        state.last_digest = record.record_digest.clone();
        state.latest.insert(record.source_id.clone(), record);
        state.event_count += 1;
    }
    let after = reader.get_ref().metadata()
        .map_err(|error| format!("DIRECT_CONTROL_LOG_METADATA_ERROR:{error}"))?;
    if consumed != before.len() || before.len() != after.len()
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
    let read = Read::take(&mut *reader, MAX_LOG_LINE_BYTES as u64 + 1)
        .read_line(line)
        .map_err(|error| format!("DIRECT_CONTROL_LOG_READ_ERROR:{error}"))?;
    *consumed = consumed.checked_add(read as u64)
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

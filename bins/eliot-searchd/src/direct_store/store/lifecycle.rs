//! DIRECT store lifecycle, namespace and source-state operations.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::sha256;

use super::super::catalog::{load_registry, read_namespace};
use super::super::model::{
    CONTROL_DIRECTORY, DirectStore, NAMESPACE_FILE, REVISION_DIRECTORY,
    RecordDraft, SOURCE_LOG_FILE, SOURCE_LOG_HEADER, SourceRecord, SourceState,
    SourceSummary,
};
use super::filesystem::{
    ensure_child_directory, ensure_directory, ensure_regular_file,
    path_identity_bytes, sync_directory,
};
use super::validation::validate_digest_text;

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
        let record = self
            .append_drafts(vec![draft])?
            .pop()
            .ok_or_else(|| "DIRECT_RETIRE_EMPTY_RESULT".to_owned())?;
        Ok(summary(&record))
    }

    /// Returns deterministic source summaries.
    pub(crate) fn list_sources(&self) -> Vec<SourceSummary> {
        self.registry.latest.values().map(summary).collect()
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

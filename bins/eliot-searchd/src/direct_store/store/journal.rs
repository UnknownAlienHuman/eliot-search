//! Exact append and readback for the legacy DIRECT source journal.

use std::fs::OpenOptions;
use std::io::Write;

use super::super::catalog::load_registry;
use super::super::model::{
    CONTROL_DIRECTORY, DirectDigest, DirectStore, MAX_LOG_BYTES,
    MAX_LOG_LINE_BYTES, MAX_SOURCE_EVENTS, RecordDraft, SOURCE_LOG_FILE,
    SourceRecord,
};
use super::filesystem::{ensure_regular_file, sync_directory};

impl DirectStore {
    pub(in super::super) fn append_drafts(
        &mut self,
        drafts: Vec<RecordDraft>,
    ) -> Result<Vec<SourceRecord>, String> {
        if drafts.is_empty() {
            return Ok(Vec::new());
        }
        // Revalidate after revision-object preparation and before touching the log.
        // This also fences retirement, which does not traverse the ingest planner.
        self.verify_control()?;
        let plan = self
            .registry
            .plan_append::<DirectDigest>(drafts, MAX_SOURCE_EVENTS, MAX_LOG_LINE_BYTES)
            .map_err(|error| error.code().to_owned())?;
        if plan.encoded.is_empty() {
            return Ok(plan.records);
        }
        self.append_encoded_records(&plan.encoded, plan.records)
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
        let current_bytes = file
            .metadata()
            .map_err(|error| format!("DIRECT_CONTROL_LOG_METADATA_ERROR:{error}"))?
            .len();
        let encoded_bytes = u64::try_from(encoded.len())
            .map_err(|_| "DIRECT_CONTROL_LOG_TOO_LARGE".to_owned())?;
        if current_bytes
            .checked_add(encoded_bytes)
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
}

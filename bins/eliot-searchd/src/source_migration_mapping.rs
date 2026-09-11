//! Adapt the legacy DIRECT source journal to the control-owned deterministic
//! source/revision mapping planner.
//!
//! The daemon still owns legacy replay, source-log validation and textual
//! migration output. Pure UUID derivation, occurrence accounting and redb row
//! construction live in `search-control-redb::migration`.

use std::path::Path;
use std::time::Instant;

use search_contracts::{Sha256Digest32, SourceNamespaceId};
use search_control_redb::migration::{
    LegacySourceMappingEvent, LegacySourceMappingState, MappedSourceEvent,
    SourceImportRow, SourceMappingError, SourceMappingHeader,
    SourceMappingPlanner, SourceMappingSummary, source_mapping_profile_digest,
};

use super::{
    CONTROL_DIRECTORY, DirectStore, MAX_SOURCE_EVENTS, NAMESPACE_FILE,
    SOURCE_LOG_FILE, SourceRecord, SourceState, event_json, read_namespace,
    replay_registry, sha256, snapshot_digest, validate_legacy_event,
};

fn mapping_error(error: SourceMappingError) -> String {
    error.code().to_owned()
}

fn digest(value: &str) -> Result<Sha256Digest32, String> {
    sha256::decode_digest(value)
        .map(Sha256Digest32::from_bytes)
        .ok_or_else(|| "DIRECT_MIGRATION_MAPPING_DIGEST_INVALID".to_owned())
}

fn legacy_event(
    record: &SourceRecord,
    previous: Option<&SourceRecord>,
) -> Result<LegacySourceMappingEvent, String> {
    Ok(LegacySourceMappingEvent {
        sequence: record.sequence,
        operation: digest(&record.operation_id)?,
        legacy_source: digest(&record.source_id)?,
        legacy_revision: digest(&record.revision_id)?,
        content: digest(&record.content_digest)?,
        file_identity: digest(&record.file_identity_digest)?,
        path: digest(&record.path_digest)?,
        event: digest(&record.record_digest)?,
        previous_event: digest(&record.previous_digest)?,
        previous_source_event: previous
            .map(|predecessor| digest(&predecessor.record_digest))
            .transpose()?,
        source_bytes: record.byte_length,
        state: match record.state {
            SourceState::Active => LegacySourceMappingState::Active,
            SourceState::Retired => LegacySourceMappingState::Retired,
        },
        native_identity: record.identity_strength.tag() == "native",
    })
}

fn encode_mapped(
    mapped: &MappedSourceEvent,
    record: &SourceRecord,
    previous: Option<&SourceRecord>,
) -> String {
    let predecessor = mapped.previous_revision_id.map_or_else(
        || "null".to_owned(),
        |id| format!("\"{id}\""),
    );
    format!(
        concat!(
            "{{\"kind\":\"source_event_mapping\",\"source_id\":\"{}\",",
            "\"revision_id\":\"{}\",\"occurrence_sequence\":{},",
            "\"previous_revision_id\":{},\"opens_source\":{},",
            "\"opens_revision\":{},\"retires_source\":{},\"legacy\":{}}}\n"
        ),
        mapped.source_id,
        mapped.revision_id,
        mapped.occurrence_sequence,
        predecessor,
        mapped.opens_source,
        mapped.opens_revision,
        mapped.retires_source,
        event_json(record, previous, mapped.source_event_ordinal),
    )
}

fn encode_header(header: SourceMappingHeader) -> String {
    format!(
        concat!(
            "{{\"kind\":\"source_mapping_header\",",
            "\"schema\":\"eliot.source-mapping.v1\",",
            "\"target_namespace_id\":\"{}\",",
            "\"legacy_namespace_sha256\":\"{}\",",
            "\"catalog_snapshot_sha256\":\"{}\",",
            "\"mapping_profile_sha256\":\"{}\",",
            "\"expected_events\":{},\"expected_sources\":{},",
            "\"identity_kind\":\"imported_object\",",
            "\"acquisition_kind\":\"imported\",",
            "\"draft_only\":true,\"cutover_authorized\":false}}\n"
        ),
        header.namespace,
        sha256::hex(&header.legacy_namespace),
        sha256::hex(&header.catalog_snapshot),
        sha256::hex(&source_mapping_profile_digest()),
        header.expected_events,
        header.expected_sources,
    )
}

fn encode_summary(summary: SourceMappingSummary) -> String {
    format!(
        concat!(
            "{{\"kind\":\"source_mapping_end\",\"events\":{},",
            "\"sources\":{},\"revision_occurrences\":{},",
            "\"retained_revision_events\":{},\"retirements\":{},",
            "\"required_import_inputs\":[",
            "\"content_blake3_readback\",",
            "\"import_observation_and_stability\",",
            "\"residency_policy\",\"root_and_membership_bindings\",",
            "\"namespace_owner_cutover\"],",
            "\"canonical_records_materialized\":false}}\n"
        ),
        summary.events,
        summary.sources,
        summary.occurrences,
        summary.path_only_events,
        summary.retirements,
    )
}

impl DirectStore {
    /// Provisional typed header for the complete admitted legacy snapshot.
    pub(crate) fn source_mapping_header(
        &self,
        namespace: SourceNamespaceId,
    ) -> Result<SourceMappingHeader, String> {
        SourceMappingHeader::new(
            namespace,
            self.namespace_id,
            snapshot_digest(
                &self.namespace_id,
                self.registry.last_sequence,
                &self.registry.last_digest,
            ),
            self.registry.event_count as u64,
            self.registry.latest.len() as u64,
            MAX_SOURCE_EVENTS as u64,
        )
        .map_err(mapping_error)
    }

    /// Compile the same complete mapping stream used by staging/readback.
    pub(crate) fn compile_source_mapping(
        &self,
        namespace: SourceNamespaceId,
        deadline: Instant,
        emit: impl FnMut(&[u8]) -> Result<(), String>,
    ) -> Result<SourceMappingSummary, String> {
        self.compile_source_mapping_inner(namespace, deadline, emit, None)
    }

    /// Compile textual mapping output and the exact typed import rows in one pass.
    pub(crate) fn compile_source_mapping_with_rows(
        &self,
        namespace: SourceNamespaceId,
        deadline: Instant,
        emit: impl FnMut(&[u8]) -> Result<(), String>,
        mut mapped: impl FnMut(SourceImportRow) -> Result<(), String>,
    ) -> Result<SourceMappingSummary, String> {
        self.compile_source_mapping_inner(
            namespace,
            deadline,
            emit,
            Some(&mut mapped),
        )
    }

    fn compile_source_mapping_inner(
        &self,
        namespace: SourceNamespaceId,
        deadline: Instant,
        mut emit: impl FnMut(&[u8]) -> Result<(), String>,
        mut imported: Option<&mut dyn FnMut(SourceImportRow) -> Result<(), String>>,
    ) -> Result<SourceMappingSummary, String> {
        let check = || {
            if Instant::now() >= deadline {
                Err("DIRECT_MIGRATION_DEADLINE_EXCEEDED".to_owned())
            } else {
                Ok(())
            }
        };
        check()?;
        let header = self.source_mapping_header(namespace)?;
        let control = self.root.join(CONTROL_DIRECTORY);
        if read_namespace(&control.join(NAMESPACE_FILE))? != self.namespace_id {
            return Err("DIRECT_MIGRATION_NAMESPACE_MISMATCH".to_owned());
        }
        emit(encode_header(header).as_bytes())?;
        let mut planner = SourceMappingPlanner::new(header, MAX_SOURCE_EVENTS)
            .map_err(mapping_error)?;
        let state = replay_registry(
            &control.join(SOURCE_LOG_FILE),
            |record, previous| {
                check()?;
                validate_legacy_event(&header.legacy_namespace, record, previous)?;
                let mapped = planner
                    .map(legacy_event(record, previous)?)
                    .map_err(mapping_error)?;
                if let Some(imported) = imported.as_mut() {
                    imported(mapped.import_row())?;
                }
                emit(encode_mapped(&mapped, record, previous).as_bytes())
            },
        )?;
        check()?;
        let summary = planner.finish().map_err(mapping_error)?;
        if state != self.registry
            || read_namespace(&control.join(NAMESPACE_FILE))?
                != self.namespace_id
        {
            return Err("DIRECT_CONTROL_READBACK_MISMATCH".to_owned());
        }
        emit(encode_summary(summary).as_bytes())?;
        check()?;
        Ok(summary)
    }

    /// Borrow only the existing immutable legacy source snapshot for mapping.
    pub(crate) fn with_existing_mapping_source<T>(
        root: &Path,
        deadline: Instant,
        inspect: impl FnOnce(&Self) -> Result<T, String>,
    ) -> Result<T, String> {
        let check = || {
            if Instant::now() >= deadline {
                Err("DIRECT_MIGRATION_DEADLINE_EXCEEDED".to_owned())
            } else {
                Ok(())
            }
        };
        check()?;
        crate::catalog_presence::require_existing(root)?;
        let control = root.join(CONTROL_DIRECTORY);
        let namespace_id = read_namespace(&control.join(NAMESPACE_FILE))?;
        let registry = replay_registry(
            &control.join(SOURCE_LOG_FILE),
            |record, previous| {
                check()?;
                validate_legacy_event(&namespace_id, record, previous)
            },
        )?;
        check()?;
        if read_namespace(&control.join(NAMESPACE_FILE))? != namespace_id {
            return Err("DIRECT_MIGRATION_NAMESPACE_MISMATCH".to_owned());
        }
        let source = Self {
            root: root.to_path_buf(),
            namespace_id,
            registry,
        };
        let expected = snapshot_digest(
            &source.namespace_id,
            source.registry.last_sequence,
            &source.registry.last_digest,
        );
        let result = inspect(&source)?;
        if source.verify_migration_snapshot(deadline)? != expected {
            return Err("DIRECT_CONTROL_READBACK_MISMATCH".to_owned());
        }
        Ok(result)
    }
}

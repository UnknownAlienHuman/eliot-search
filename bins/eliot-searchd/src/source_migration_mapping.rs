//! Adapt the legacy DIRECT source journal to the control-owned deterministic
//! source/revision mapping planner.
//!
//! The daemon still owns legacy replay, source-log validation and textual
//! migration output. Pure UUID derivation, occurrence accounting and redb row
//! construction live in `search-control-redb::migration`.

#[cfg(test)]
use std::path::Path;
use std::io::Cursor;
use std::time::Instant;

use search_contracts::{Sha256Digest32, SourceNamespaceId};
use search_control_redb::migration::{
    LegacySourceMappingEvent, LegacySourceMappingState,
    SourceImportRow, SourceMappingError, SourceMappingHeader,
    SourceMappingPlanner, SourceMappingSummary,
    encode_source_mapping_header, encode_source_mapping_event, encode_source_mapping_summary,
};
#[cfg(test)]
use search_control_redb::migration::{MappedSourceEvent, source_mapping_profile_digest};

use super::{
    CONTROL_DIRECTORY, DirectStore, MAX_SOURCE_EVENTS, NAMESPACE_FILE,
    SOURCE_LOG_FILE, SourceRecord, SourceState, read_namespace,
    replay_registry, sha256, snapshot_digest, validate_legacy_event,
};
#[cfg(test)]
use super::event_json;
use super::super::{parse_namespace, replay_registry_reader_checked};

fn check_mapping_deadline(deadline: Instant) -> Result<(), String> {
    if Instant::now() >= deadline {
        Err("DIRECT_MIGRATION_DEADLINE_EXCEEDED".to_owned())
    } else { Ok(()) }
}

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

#[cfg(test)]
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

#[cfg(test)]
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

#[cfg(test)]
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
        self.compile_source_mapping_inner(namespace, emit, None, None, &mut || check_mapping_deadline(deadline))
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
            emit,
            Some(&mut mapped),
            None,
            &mut || check_mapping_deadline(deadline),
        )
    }

    pub(crate) fn compile_source_mapping_from_bytes(
        &self,
        target: SourceNamespaceId,
        namespace: &[u8],
        log: &[u8],
        mut checkpoint: impl FnMut() -> Result<(), String>,
        emit: impl FnMut(&[u8]) -> Result<(), String>,
    ) -> Result<SourceMappingSummary, String> {
        self.compile_source_mapping_inner(target, emit, None, Some((namespace, log)), &mut checkpoint)
    }

    pub(crate) fn compile_source_mapping_with_rows_from_bytes(
        &self,
        target: SourceNamespaceId,
        namespace: &[u8],
        log: &[u8],
        mut checkpoint: impl FnMut() -> Result<(), String>,
        emit: impl FnMut(&[u8]) -> Result<(), String>,
        mut rows: impl FnMut(SourceImportRow) -> Result<(), String>,
    ) -> Result<SourceMappingSummary, String> {
        self.compile_source_mapping_inner(target, emit, Some(&mut rows), Some((namespace, log)), &mut checkpoint)
    }

    pub(crate) fn verify_migration_snapshot_from_bytes(
        &self,
        namespace: &[u8],
        log: &[u8],
        mut checkpoint: impl FnMut() -> Result<(), String>,
    ) -> Result<[u8; 32], String> {
        checkpoint()?;
        let captured = parse_namespace(namespace)?;
        if captured != self.namespace_id { return Err("DIRECT_MIGRATION_NAMESPACE_MISMATCH".to_owned()); }
        let (state, consumed) = replay_registry_reader_checked(
            &mut Cursor::new(log),
            |record, previous| validate_legacy_event(&captured, record, previous),
            &mut checkpoint,
        )?;
        if u64::try_from(log.len()).ok() != Some(consumed) || state != self.registry {
            return Err("DIRECT_CONTROL_READBACK_MISMATCH".to_owned());
        }
        checkpoint()?;
        Ok(snapshot_digest(&captured, state.last_sequence, &state.last_digest))
    }

    fn compile_source_mapping_inner(
        &self,
        namespace: SourceNamespaceId,
        mut emit: impl FnMut(&[u8]) -> Result<(), String>,
        mut imported: Option<&mut dyn FnMut(SourceImportRow) -> Result<(), String>>,
        pinned: Option<(&[u8], &[u8])>,
        check: &mut dyn FnMut() -> Result<(), String>,
    ) -> Result<SourceMappingSummary, String> {
        check()?;
        let header = self.source_mapping_header(namespace)?;
        let control = self.root.join(CONTROL_DIRECTORY);
        let legacy_namespace = match pinned {
            Some((namespace, _)) => parse_namespace(namespace)?,
            None => read_namespace(&control.join(NAMESPACE_FILE))?,
        };
        if legacy_namespace != self.namespace_id {
            return Err("DIRECT_MIGRATION_NAMESPACE_MISMATCH".to_owned());
        }
        emit(&encode_source_mapping_header(header))?;
        let mut planner = SourceMappingPlanner::new(header, MAX_SOURCE_EVENTS)
            .map_err(mapping_error)?;
        let mut observe = |record: &SourceRecord, previous: Option<&SourceRecord>| {
                validate_legacy_event(&header.legacy_namespace, record, previous)?;
                let event = legacy_event(record, previous)?;
                let mapped = planner
                    .map(event)
                    .map_err(mapping_error)?;
                if let Some(imported) = imported.as_mut() {
                    imported(mapped.import_row())?;
                }
                emit(&encode_source_mapping_event(&mapped, &event))
            };
        let state = match pinned {
            Some((_, log)) => {
                let (state, consumed) = replay_registry_reader_checked(&mut Cursor::new(log), &mut observe, &mut *check)?;
                if u64::try_from(log.len()).ok() != Some(consumed) {
                    return Err("DIRECT_CONTROL_READBACK_MISMATCH".to_owned());
                }
                state
            }
            None => replay_registry(&control.join(SOURCE_LOG_FILE), |record, previous| {
                check()?;
                observe(record, previous)
            })?,
        };
        check()?;
        let summary = planner.finish().map_err(mapping_error)?;
        if state != self.registry
            || pinned.is_none() && read_namespace(&control.join(NAMESPACE_FILE))? != self.namespace_id
        {
            return Err("DIRECT_CONTROL_READBACK_MISMATCH".to_owned());
        }
        emit(&encode_source_mapping_summary(summary))?;
        check()?;
        Ok(summary)
    }

    /// Borrow only the existing immutable legacy source snapshot for mapping.
    #[cfg(test)]
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

#[cfg(test)]
mod codec_compatibility {
    use super::*;
    use search_source_registry::LegacyDirectIdentityStrength;

    #[test]
    fn shared_mapping_codec_preserves_legacy_daemon_header_event_and_footer() {
        let header = SourceMappingHeader::new(SourceNamespaceId::from_bytes([1; 16]),
            [2; 32], [9; 32], 1, 1, 16).unwrap();
        assert_eq!(encode_source_mapping_header(header), encode_header(header).as_bytes());
        for identity_strength in [LegacyDirectIdentityStrength::Native, LegacyDirectIdentityStrength::PathBound] {
            let record = SourceRecord { sequence: 1, previous_digest: "00".repeat(32),
                operation_id: "07".repeat(32), state: SourceState::Active,
                source_id: "03".repeat(32), revision_id: "05".repeat(32),
                content_digest: "06".repeat(32), byte_length: 42,
                file_identity_digest: "08".repeat(32), path_digest: "0a".repeat(32),
                identity_strength, record_digest: "04".repeat(32) };
            let event = legacy_event(&record, None).unwrap();
            let mut planner = SourceMappingPlanner::new(header, 16).unwrap();
            let mapped = planner.map(event).unwrap();
            assert_eq!(encode_source_mapping_event(&mapped, &event), encode_mapped(&mapped, &record, None).as_bytes());
            let summary = planner.finish().unwrap();
            assert_eq!(encode_source_mapping_summary(summary), encode_summary(summary).as_bytes());
        }
    }
}

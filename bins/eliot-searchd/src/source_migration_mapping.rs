//! Compile legacy transitions into imported-source and revision-occurrence mappings.
//! This is a migration draft, not admission, a stability receipt, or a SourceRevision.
//! Content SHA-256, unavailable timestamps, and residency are never relabelled.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Instant;

use search_contracts::{SourceId, SourceNamespaceId, SourceRevisionId};

use super::{
    DirectStore, MAX_SOURCE_EVENTS, NAMESPACE_FILE, SOURCE_LOG_FILE, SourceRecord,
    SourceState, CONTROL_DIRECTORY, event_json, read_namespace, replay_registry,
    sha256, snapshot_digest, validate_legacy_event,
};

const PROFILE: &[u8] = b"eliot/source-mapping/v1;imported-object;uuid8-sha256;active-content-change-or-reactivation=new-occurrence;path-only=retain;retirement=retain";

/// Proposed graph edge. Missing canonical evidence is deliberately unrepresentable
/// here; only the importer with real readback/policy inputs may create a SourceRevision.
pub(crate) struct MappedSourceEvent {
    pub(crate) source_id: SourceId,
    pub(crate) revision_id: SourceRevisionId,
    pub(crate) occurrence_sequence: u64,
    pub(crate) previous_revision_id: Option<SourceRevisionId>,
    pub(crate) source_event_ordinal: u64,
    pub(crate) opens_source: bool,
    pub(crate) opens_revision: bool,
    pub(crate) retires_source: bool,
}

impl MappedSourceEvent {
    fn encode(&self, record: &SourceRecord, previous: Option<&SourceRecord>) -> String {
        let predecessor = self.previous_revision_id.map_or_else(
            || "null".to_owned(), |id| format!("\"{id}\""),
        );
        // Everything interpolated is a typed UUID, validated legacy hex/tag, or integer.
        format!(concat!(
            "{{\"kind\":\"source_event_mapping\",\"source_id\":\"{}\",",
            "\"revision_id\":\"{}\",\"occurrence_sequence\":{},",
            "\"previous_revision_id\":{},\"opens_source\":{},\"opens_revision\":{},",
            "\"retires_source\":{},\"legacy\":{}}}\n"
        ), self.source_id, self.revision_id, self.occurrence_sequence, predecessor,
            self.opens_source, self.opens_revision, self.retires_source,
            event_json(record, previous, self.source_event_ordinal))
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) struct SourceMappingHeader {
    pub(crate) namespace: SourceNamespaceId,
    pub(crate) legacy_namespace: [u8; 32],
    pub(crate) catalog_snapshot: [u8; 32],
    pub(crate) expected_events: u64,
    pub(crate) expected_sources: u64,
}

impl SourceMappingHeader {
    pub(crate) fn encode(self) -> String {
        format!(concat!(
            "{{\"kind\":\"source_mapping_header\",\"schema\":\"eliot.source-mapping.v1\",",
            "\"target_namespace_id\":\"{}\",\"legacy_namespace_sha256\":\"{}\",",
            "\"catalog_snapshot_sha256\":\"{}\",\"mapping_profile_sha256\":\"{}\",",
            "\"expected_events\":{},\"expected_sources\":{},",
            "\"identity_kind\":\"imported_object\",\"acquisition_kind\":\"imported\",",
            "\"draft_only\":true,\"cutover_authorized\":false}}\n"
        ), self.namespace, sha256::hex(&self.legacy_namespace), sha256::hex(&self.catalog_snapshot),
            sha256::hex(&sha256::digest(PROFILE)), self.expected_events, self.expected_sources)
    }
}

#[derive(Clone, Copy, Default, Eq, PartialEq)]
pub(crate) struct SourceMappingSummary {
    pub(crate) events: u64,
    pub(crate) sources: u64,
    pub(crate) occurrences: u64,
    pub(crate) path_only_events: u64,
    pub(crate) retirements: u64,
}

impl SourceMappingSummary {
    pub(crate) fn encode(self) -> String {
        format!(concat!(
            "{{\"kind\":\"source_mapping_end\",\"events\":{},\"sources\":{},",
            "\"revision_occurrences\":{},\"retained_revision_events\":{},\"retirements\":{},",
            "\"required_import_inputs\":[\"content_blake3_readback\",\"import_observation_and_stability\",",
            "\"residency_policy\",\"root_and_membership_bindings\",\"namespace_owner_cutover\"],",
            "\"canonical_records_materialized\":false}}\n"
        ), self.events, self.sources, self.occurrences, self.path_only_events, self.retirements)
    }
}

struct SourcePosition {
    id: SourceId,
    revision: SourceRevisionId,
    occurrence: u64,
    ordinal: u64,
    last_event: String,
}

struct Mapper {
    header: SourceMappingHeader,
    sources: BTreeMap<String, SourcePosition>,
    // Keep full derivation identities to reject truncation/UUID-bit collisions.
    ids: BTreeMap<[u8; 16], [u8; 32]>,
    summary: SourceMappingSummary,
}

impl Mapper {
    fn new(header: SourceMappingHeader) -> Self {
        Self { header, sources: BTreeMap::new(), ids: BTreeMap::new(), summary: SourceMappingSummary::default() }
    }

    fn allocate_id(&mut self, domain: &[u8], parts: &[&[u8]]) -> Result<[u8; 16], String> {
        if self.ids.len() >= MAX_SOURCE_EVENTS.saturating_mul(2) {
            return Err("DIRECT_MIGRATION_MAPPING_LIMIT".to_owned());
        }
        let full = sha256::digest_parts(domain, parts);
        let mut id = [0; 16];
        id.copy_from_slice(&full[..16]);
        id[6] = (id[6] & 0x0f) | 0x80; // UUID version 8, explicit application-defined profile.
        id[8] = (id[8] & 0x3f) | 0x80;
        if let Some(previous) = self.ids.insert(id, full) {
            if previous != full { return Err("DIRECT_MIGRATION_MAPPING_ID_COLLISION".to_owned()); }
            // New allocations must correspond to a new source or occurrence.
            return Err("DIRECT_MIGRATION_MAPPING_ID_REUSED".to_owned());
        }
        Ok(id)
    }

    fn map(&mut self, record: &SourceRecord, previous: Option<&SourceRecord>) -> Result<MappedSourceEvent, String> {
        validate_legacy_event(&self.header.legacy_namespace, record, previous)?;
        if record.sequence != self.summary.events + 1 {
            return Err("DIRECT_MIGRATION_MAPPING_ORDER_INVALID".to_owned());
        }
        let prior = self.sources.get(&record.source_id);
        if prior.is_some() != previous.is_some()
            || prior.zip(previous).is_some_and(|(a, b)| a.last_event != b.record_digest)
        {
            return Err("DIRECT_MIGRATION_MAPPING_PREDECESSOR_INVALID".to_owned());
        }
        let opens_source = prior.is_none();
        let old_revision = prior.map(|value| value.revision);
        let old_occurrence = prior.map_or(0, |value| value.occurrence);
        let ordinal = prior.map_or(0, |value| value.ordinal).checked_add(1)
            .ok_or_else(|| "DIRECT_MIGRATION_ORDINAL_EXHAUSTED".to_owned())?;
        let existing_id = prior.map(|value| value.id);
        let target = *self.header.namespace.as_bytes();
        let legacy_namespace = self.header.legacy_namespace;
        let source_id = match existing_id {
            Some(id) => id,
            None => SourceId::from_bytes(self.allocate_id(b"eliot-search/imported-source-uuid/v1", &[
                &target, &legacy_namespace, record.source_id.as_bytes(),
            ])?),
        };
        let opens_revision = record.state == SourceState::Active && previous.is_none_or(|old| {
            old.state == SourceState::Retired || old.revision_id != record.revision_id
        });
        let occurrence_sequence = if opens_revision {
            old_occurrence.checked_add(1).ok_or_else(|| "DIRECT_MIGRATION_ORDINAL_EXHAUSTED".to_owned())?
        } else { old_occurrence };
        let revision_id = if opens_revision {
            SourceRevisionId::from_bytes(self.allocate_id(b"eliot-search/imported-revision-uuid/v1", &[
                &target, source_id.as_bytes(), record.record_digest.as_bytes(), &occurrence_sequence.to_be_bytes(),
            ])?)
        } else {
            old_revision.ok_or_else(|| "DIRECT_MIGRATION_RETIREMENT_INVALID".to_owned())?
        };
        let retires_source = record.state == SourceState::Retired;
        self.sources.insert(record.source_id.clone(), SourcePosition {
            id: source_id, revision: revision_id, occurrence: occurrence_sequence,
            ordinal, last_event: record.record_digest.clone(),
        });
        self.summary.events += 1; // bounded by replay's MAX_SOURCE_EVENTS
        self.summary.sources += u64::from(opens_source);
        self.summary.occurrences += u64::from(opens_revision);
        self.summary.retirements += u64::from(retires_source);
        self.summary.path_only_events += u64::from(!opens_revision && !retires_source);
        Ok(MappedSourceEvent {
            source_id, revision_id, occurrence_sequence, previous_revision_id: old_revision,
            source_event_ordinal: ordinal, opens_source, opens_revision, retires_source,
        })
    }
}

impl DirectStore {
    /// The header is provisional until the complete disk replay returns this state.
    pub(crate) fn source_mapping_header(&self, namespace: SourceNamespaceId) -> Result<SourceMappingHeader, String> {
        if namespace.as_bytes() == &[0; 16] {
            return Err("DIRECT_MIGRATION_TARGET_NAMESPACE_INVALID".to_owned());
        }
        Ok(SourceMappingHeader {
            namespace, legacy_namespace: self.namespace_id,
            catalog_snapshot: snapshot_digest(&self.namespace_id, self.registry.last_sequence, &self.registry.last_digest),
            expected_events: self.registry.event_count as u64,
            expected_sources: self.registry.latest.len() as u64,
        })
    }

    /// A complete mapping pass, shared by staging and its exact-byte readback.
    /// No second source-log parser, per-page replay, payload read or durable owner is created.
    /// The callback may only stage provisional bytes; its effects are not accepted before Ok.
    pub(crate) fn compile_source_mapping(
        &self, namespace: SourceNamespaceId, deadline: Instant,
        mut emit: impl FnMut(&[u8]) -> Result<(), String>,
    ) -> Result<SourceMappingSummary, String> {
        let check = || if Instant::now() >= deadline {
            Err("DIRECT_MIGRATION_DEADLINE_EXCEEDED".to_owned())
        } else { Ok(()) };
        check()?;
        let header = self.source_mapping_header(namespace)?;
        let control = self.root.join(CONTROL_DIRECTORY);
        if read_namespace(&control.join(NAMESPACE_FILE))? != self.namespace_id {
            return Err("DIRECT_MIGRATION_NAMESPACE_MISMATCH".to_owned());
        }
        emit(header.encode().as_bytes())?;
        let mut mapper = Mapper::new(header);
        let state = replay_registry(&control.join(SOURCE_LOG_FILE), |record, previous| {
            check()?;
            let mapped = mapper.map(record, previous)?;
            emit(mapped.encode(record, previous).as_bytes())
        })?;
        check()?;
        if state != self.registry || read_namespace(&control.join(NAMESPACE_FILE))? != self.namespace_id
            || mapper.summary.events != header.expected_events || mapper.summary.sources != header.expected_sources
            || mapper.summary.events != mapper.summary.occurrences + mapper.summary.path_only_events + mapper.summary.retirements
        {
            return Err("DIRECT_CONTROL_READBACK_MISMATCH".to_owned());
        }
        emit(mapper.summary.encode().as_bytes())?;
        check()?;
        Ok(mapper.summary)
    }

    /// Open only the existing journal as an immutable input to a mapping operation.
    /// The caller holds the ordinary root lock for this entire callback. Neither
    /// DirectStore::open nor its initialization/protection/recovery path is invoked.
    /// The callback borrows the admitted snapshot; no mutable store is returned.
    pub(crate) fn with_existing_mapping_source<T>(
        root: &Path, deadline: Instant,
        inspect: impl FnOnce(&Self) -> Result<T, String>,
    ) -> Result<T, String> {
        let check = || if Instant::now() >= deadline {
            Err("DIRECT_MIGRATION_DEADLINE_EXCEEDED".to_owned())
        } else { Ok(()) };
        check()?;
        crate::catalog_presence::require_existing(root)?;
        let control = root.join(CONTROL_DIRECTORY);
        let namespace_id = read_namespace(&control.join(NAMESPACE_FILE))?;
        let registry = replay_registry(&control.join(SOURCE_LOG_FILE), |record, previous| {
            check()?;
            validate_legacy_event(&namespace_id, record, previous)
        })?;
        check()?;
        if read_namespace(&control.join(NAMESPACE_FILE))? != namespace_id {
            return Err("DIRECT_MIGRATION_NAMESPACE_MISMATCH".to_owned());
        }
        let source = Self { root: root.to_path_buf(), namespace_id, registry };
        let expected = snapshot_digest(&source.namespace_id, source.registry.last_sequence, &source.registry.last_digest);
        let result = inspect(&source)?;
        if source.verify_migration_snapshot(deadline)? != expected {
            return Err("DIRECT_CONTROL_READBACK_MISMATCH".to_owned());
        }
        Ok(result)
    }
}

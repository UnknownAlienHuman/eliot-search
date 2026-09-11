use std::collections::BTreeMap;
use std::fmt;

use search_contracts::{
    Sha256Digest32, SourceId, SourceNamespaceId, SourceRevisionId,
};
use sha2::{Digest, Sha256};

use super::{
    SourceImportBinding, SourceImportCounts, SourceImportRow,
    SourceLifecycleFlags,
};

const PARTS_V1: &[u8] = b"eliot-search/sha256-parts/v1\0";
const MAPPING_PROFILE: &[u8] = b"eliot/source-mapping/v1;imported-object;uuid8-sha256;active-content-change-or-reactivation=new-occurrence;path-only=retain;retirement=retain";
const SOURCE_ID_DOMAIN: &[u8] = b"eliot-search/imported-source-uuid/v1";
const REVISION_ID_DOMAIN: &[u8] = b"eliot-search/imported-revision-uuid/v1";

/// Closed pure-mapping failure with the legacy stable reason namespace.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceMappingError {
    /// Target namespace is the all-zero sentinel.
    TargetNamespaceInvalid,
    /// Declared or observed event/cardinality bound is impossible.
    Limit,
    /// Two different full SHA-256 derivations truncate to the same UUID bytes.
    IdCollision,
    /// A supposedly new source/occurrence attempted to allocate the same identity.
    IdReused,
    /// Global event sequence is not exactly contiguous.
    OrderInvalid,
    /// Per-source predecessor presence or digest does not match planner state.
    PredecessorInvalid,
    /// Per-source ordinal or occurrence counter exhausted.
    OrdinalExhausted,
    /// A retirement/path-only event has no prior live occurrence.
    RetirementInvalid,
    /// Final event/source/accounting counts do not match the declared header.
    SummaryMismatch,
}

impl SourceMappingError {
    /// Stable machine-readable reason code retained from the daemon mapper.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::TargetNamespaceInvalid => {
                "DIRECT_MIGRATION_TARGET_NAMESPACE_INVALID"
            }
            Self::Limit => "DIRECT_MIGRATION_MAPPING_LIMIT",
            Self::IdCollision => "DIRECT_MIGRATION_MAPPING_ID_COLLISION",
            Self::IdReused => "DIRECT_MIGRATION_MAPPING_ID_REUSED",
            Self::OrderInvalid => "DIRECT_MIGRATION_MAPPING_ORDER_INVALID",
            Self::PredecessorInvalid => {
                "DIRECT_MIGRATION_MAPPING_PREDECESSOR_INVALID"
            }
            Self::OrdinalExhausted => "DIRECT_MIGRATION_ORDINAL_EXHAUSTED",
            Self::RetirementInvalid => {
                "DIRECT_MIGRATION_RETIREMENT_INVALID"
            }
            Self::SummaryMismatch => "DIRECT_CONTROL_READBACK_MISMATCH",
        }
    }
}

impl fmt::Display for SourceMappingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for SourceMappingError {}

/// Legacy source lifecycle supplied by the DIRECT replay adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LegacySourceMappingState {
    /// Source is active after this event.
    Active,
    /// Source is retired after this event.
    Retired,
}

/// Content-free, fully typed legacy event consumed by the pure mapper.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LegacySourceMappingEvent {
    /// Global source-journal sequence.
    pub sequence: u64,
    /// Original operation digest.
    pub operation: Sha256Digest32,
    /// Original source identity digest.
    pub legacy_source: Sha256Digest32,
    /// Original revision/object identity digest.
    pub legacy_revision: Sha256Digest32,
    /// Actual legacy content SHA-256.
    pub content: Sha256Digest32,
    /// Original stable-object identity fingerprint.
    pub file_identity: Sha256Digest32,
    /// Original path fingerprint.
    pub path: Sha256Digest32,
    /// Exact source-journal event fingerprint.
    pub event: Sha256Digest32,
    /// Global predecessor event fingerprint.
    pub previous_event: Sha256Digest32,
    /// Per-source predecessor event fingerprint, absent for a new source.
    pub previous_source_event: Option<Sha256Digest32>,
    /// Exact source-object length.
    pub source_bytes: u64,
    /// State after this event.
    pub state: LegacySourceMappingState,
    /// Whether the legacy observation had a qualified native identity.
    pub native_identity: bool,
}

/// Immutable identity/count header for one deterministic mapping pass.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceMappingHeader {
    /// Explicit imported namespace.
    pub namespace: SourceNamespaceId,
    /// Original legacy namespace SHA-256 bytes.
    pub legacy_namespace: [u8; 32],
    /// Complete verified source-history snapshot SHA-256 bytes.
    pub catalog_snapshot: [u8; 32],
    /// Exact number of legacy events expected.
    pub expected_events: u64,
    /// Exact number of distinct legacy sources expected.
    pub expected_sources: u64,
}

impl SourceMappingHeader {
    /// Validate and construct one mapping header.
    pub fn new(
        namespace: SourceNamespaceId,
        legacy_namespace: [u8; 32],
        catalog_snapshot: [u8; 32],
        expected_events: u64,
        expected_sources: u64,
        max_events: u64,
    ) -> Result<Self, SourceMappingError> {
        if namespace.as_bytes() == &[0; 16] {
            return Err(SourceMappingError::TargetNamespaceInvalid);
        }
        if expected_events > max_events
            || expected_sources > expected_events
            || (expected_events == 0) != (expected_sources == 0)
        {
            return Err(SourceMappingError::Limit);
        }
        Ok(Self {
            namespace,
            legacy_namespace,
            catalog_snapshot,
            expected_events,
            expected_sources,
        })
    }

    /// Build the inactive redb import binding for an exact plan-chain digest.
    #[must_use]
    pub fn import_binding(self, plan_chain: [u8; 32]) -> SourceImportBinding {
        SourceImportBinding {
            target_namespace: self.namespace,
            legacy_namespace: Sha256Digest32::from_bytes(self.legacy_namespace),
            catalog_snapshot: Sha256Digest32::from_bytes(self.catalog_snapshot),
            mapping_profile: Sha256Digest32::from_bytes(
                source_mapping_profile_digest(),
            ),
            plan_chain: Sha256Digest32::from_bytes(plan_chain),
            events: self.expected_events,
            sources: self.expected_sources,
        }
    }
}

/// One mapped event plus the exact typed redb import row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MappedSourceEvent {
    /// Deterministic imported source UUID.
    pub source_id: SourceId,
    /// Deterministic imported revision-occurrence UUID.
    pub revision_id: SourceRevisionId,
    /// Per-source occurrence sequence.
    pub occurrence_sequence: u64,
    /// Prior imported occurrence, if this source was seen before.
    pub previous_revision_id: Option<SourceRevisionId>,
    /// Per-source event ordinal, including path-only and retirement events.
    pub source_event_ordinal: u64,
    /// True only for the source's first event.
    pub opens_source: bool,
    /// True only when this event allocates a new occurrence.
    pub opens_revision: bool,
    /// True only for a retirement event.
    pub retires_source: bool,
    import_row: SourceImportRow,
}

impl MappedSourceEvent {
    /// Exact content-free row for `SourceMappingImport` or readback comparison.
    #[must_use]
    pub fn import_row(&self) -> SourceImportRow {
        self.import_row.clone()
    }
}

/// Final deterministic accounting for one complete mapping pass.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SourceMappingSummary {
    /// All source events.
    pub events: u64,
    /// Distinct mapped sources.
    pub sources: u64,
    /// Newly allocated revision occurrences.
    pub occurrences: u64,
    /// Active path-only events retaining the previous occurrence.
    pub path_only_events: u64,
    /// Source retirement events.
    pub retirements: u64,
}

impl SourceMappingSummary {
    /// Exact persisted import accounting.
    #[must_use]
    pub const fn import_counts(self) -> SourceImportCounts {
        SourceImportCounts {
            events: self.events,
            sources: self.sources,
            occurrences: self.occurrences,
            retained_events: self.path_only_events,
            retirements: self.retirements,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourcePosition {
    id: SourceId,
    revision: SourceRevisionId,
    legacy_revision: Sha256Digest32,
    occurrence: u64,
    ordinal: u64,
    last_event: Sha256Digest32,
    retired: bool,
}

/// Stateful pure mapper for one exact legacy source-history snapshot.
pub struct SourceMappingPlanner {
    header: SourceMappingHeader,
    max_identifiers: usize,
    sources: BTreeMap<[u8; 32], SourcePosition>,
    // Full derivations reject UUID truncation collisions.
    ids: BTreeMap<[u8; 16], [u8; 32]>,
    summary: SourceMappingSummary,
}

impl SourceMappingPlanner {
    /// Start one bounded deterministic mapping pass.
    pub fn new(
        header: SourceMappingHeader,
        max_events: usize,
    ) -> Result<Self, SourceMappingError> {
        if header.expected_events
            > u64::try_from(max_events).map_err(|_| SourceMappingError::Limit)?
        {
            return Err(SourceMappingError::Limit);
        }
        let max_identifiers = max_events
            .checked_mul(2)
            .ok_or(SourceMappingError::Limit)?;
        Ok(Self {
            header,
            max_identifiers,
            sources: BTreeMap::new(),
            ids: BTreeMap::new(),
            summary: SourceMappingSummary::default(),
        })
    }

    /// Map the next validated legacy event.
    pub fn map(
        &mut self,
        event: LegacySourceMappingEvent,
    ) -> Result<MappedSourceEvent, SourceMappingError> {
        if event.sequence == 0
            || event.sequence != self.summary.events + 1
            || event.sequence > self.header.expected_events
        {
            return Err(SourceMappingError::OrderInvalid);
        }

        let source_key = *event.legacy_source.as_bytes();
        let prior = self.sources.get(&source_key).copied();
        if prior.is_some() != event.previous_source_event.is_some()
            || prior.zip(event.previous_source_event).is_some_and(
                |(position, predecessor)| position.last_event != predecessor,
            )
        {
            return Err(SourceMappingError::PredecessorInvalid);
        }

        let opens_source = prior.is_none();
        let old_revision = prior.map(|position| position.revision);
        let old_occurrence = prior.map_or(0, |position| position.occurrence);
        let source_event_ordinal = prior
            .map_or(0, |position| position.ordinal)
            .checked_add(1)
            .ok_or(SourceMappingError::OrdinalExhausted)?;
        let source_id = if let Some(position) = prior {
            position.id
        } else {
            let target = *self.header.namespace.as_bytes();
            let legacy_namespace = self.header.legacy_namespace;
            let legacy_source_hex = lower_hex_bytes(event.legacy_source.as_bytes());
            SourceId::from_bytes(self.allocate_id(
                SOURCE_ID_DOMAIN,
                &[&target, &legacy_namespace, &legacy_source_hex],
            )?)
        };

        let active = event.state == LegacySourceMappingState::Active;
        let opens_revision = active
            && prior.is_none_or(|position| {
                position.retired
                    || position.legacy_revision != event.legacy_revision
            });
        let occurrence_sequence = if opens_revision {
            old_occurrence
                .checked_add(1)
                .ok_or(SourceMappingError::OrdinalExhausted)?
        } else {
            old_occurrence
        };
        let revision_id = if opens_revision {
            let target = *self.header.namespace.as_bytes();
            let event_hex = lower_hex_bytes(event.event.as_bytes());
            let occurrence_bytes = occurrence_sequence.to_be_bytes();
            SourceRevisionId::from_bytes(self.allocate_id(
                REVISION_ID_DOMAIN,
                &[
                    &target,
                    source_id.as_bytes(),
                    &event_hex,
                    &occurrence_bytes,
                ],
            )?)
        } else {
            old_revision.ok_or(SourceMappingError::RetirementInvalid)?
        };
        let retires_source = event.state == LegacySourceMappingState::Retired;
        let lifecycle = SourceLifecycleFlags::new([
            opens_source,
            opens_revision,
            retires_source,
            event.native_identity,
        ]);
        let import_row = SourceImportRow {
            sequence: event.sequence,
            source: source_id,
            revision: revision_id,
            previous_revision: old_revision,
            occurrence: occurrence_sequence,
            source_event: source_event_ordinal,
            source_bytes: event.source_bytes,
            lifecycle,
            operation: event.operation,
            legacy_source: event.legacy_source,
            legacy_revision: event.legacy_revision,
            content: event.content,
            file_identity: event.file_identity,
            path: event.path,
            event: event.event,
            previous_event: event.previous_event,
            previous_source_event: event
                .previous_source_event
                .unwrap_or(Sha256Digest32::from_bytes([0; 32])),
        };

        self.sources.insert(
            source_key,
            SourcePosition {
                id: source_id,
                revision: revision_id,
                legacy_revision: event.legacy_revision,
                occurrence: occurrence_sequence,
                ordinal: source_event_ordinal,
                last_event: event.event,
                retired: retires_source,
            },
        );
        self.summary.events += 1;
        self.summary.sources += u64::from(opens_source);
        self.summary.occurrences += u64::from(opens_revision);
        self.summary.retirements += u64::from(retires_source);
        self.summary.path_only_events +=
            u64::from(!opens_revision && !retires_source);

        Ok(MappedSourceEvent {
            source_id,
            revision_id,
            occurrence_sequence,
            previous_revision_id: old_revision,
            source_event_ordinal,
            opens_source,
            opens_revision,
            retires_source,
            import_row,
        })
    }

    /// Current accounting, useful for bounded streaming diagnostics.
    #[must_use]
    pub const fn summary(&self) -> SourceMappingSummary {
        self.summary
    }

    /// Finish only after exact event/source and lifecycle accounting matches.
    pub fn finish(self) -> Result<SourceMappingSummary, SourceMappingError> {
        if self.summary.events != self.header.expected_events
            || self.summary.sources != self.header.expected_sources
            || self.summary.events
                != self.summary.occurrences
                    + self.summary.path_only_events
                    + self.summary.retirements
        {
            return Err(SourceMappingError::SummaryMismatch);
        }
        Ok(self.summary)
    }

    fn allocate_id(
        &mut self,
        domain: &[u8],
        parts: &[&[u8]],
    ) -> Result<[u8; 16], SourceMappingError> {
        if self.ids.len() >= self.max_identifiers {
            return Err(SourceMappingError::Limit);
        }
        let full = digest_parts(domain, parts);
        let mut id = [0_u8; 16];
        id.copy_from_slice(&full[..16]);
        id[6] = (id[6] & 0x0f) | 0x80;
        id[8] = (id[8] & 0x3f) | 0x80;
        if let Some(previous) = self.ids.insert(id, full) {
            return if previous == full {
                Err(SourceMappingError::IdReused)
            } else {
                Err(SourceMappingError::IdCollision)
            };
        }
        Ok(id)
    }
}

/// SHA-256 identity of the frozen source-mapping profile.
#[must_use]
pub fn source_mapping_profile_digest() -> [u8; 32] {
    sha256(MAPPING_PROFILE)
}

fn digest_parts(domain: &[u8], parts: &[&[u8]]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(PARTS_V1);
    hasher.update(u64::try_from(domain.len()).unwrap_or(u64::MAX).to_be_bytes());
    hasher.update(domain);
    hasher.update(u64::try_from(parts.len()).unwrap_or(u64::MAX).to_be_bytes());
    for part in parts {
        hasher.update(u64::try_from(part.len()).unwrap_or(u64::MAX).to_be_bytes());
        hasher.update(part);
    }
    hasher.finalize().into()
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn lower_hex_bytes(bytes: &[u8; 32]) -> [u8; 64] {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = [0_u8; 64];
    for (index, byte) in bytes.iter().copied().enumerate() {
        output[index * 2] = HEX[usize::from(byte >> 4)];
        output[index * 2 + 1] = HEX[usize::from(byte & 0x0f)];
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(byte: u8) -> Sha256Digest32 {
        Sha256Digest32::from_bytes([byte; 32])
    }

    fn header(events: u64) -> SourceMappingHeader {
        SourceMappingHeader::new(
            SourceNamespaceId::from_bytes([1; 16]),
            [2; 32],
            [9; 32],
            events,
            1,
            16,
        )
        .expect("valid header")
    }

    fn event(
        sequence: u64,
        event_byte: u8,
        predecessor: Option<u8>,
    ) -> LegacySourceMappingEvent {
        LegacySourceMappingEvent {
            sequence,
            operation: digest(7),
            legacy_source: digest(3),
            legacy_revision: digest(5),
            content: digest(6),
            file_identity: digest(8),
            path: digest(10),
            event: digest(event_byte),
            previous_event: if sequence == 1 {
                digest(0)
            } else {
                digest(event_byte - 1)
            },
            previous_source_event: predecessor.map(digest),
            source_bytes: 42,
            state: LegacySourceMappingState::Active,
            native_identity: true,
        }
    }

    #[test]
    fn frozen_uuid8_derivation_matches_legacy_profile() {
        let mut planner = SourceMappingPlanner::new(header(1), 16)
            .expect("valid planner");
        let mapped = planner.map(event(1, 4, None)).expect("mapped");
        assert_eq!(
            mapped.source_id.to_string(),
            "1a484e7a-e3c2-8c81-aee8-a55ee5edbca0"
        );
        assert_eq!(
            mapped.revision_id.to_string(),
            "da220f10-65d7-860d-b594-81ab4b63d170"
        );
        assert_eq!(planner.finish().expect("complete").occurrences, 1);
    }

    #[test]
    fn path_only_event_reuses_occurrence_and_counts_retained_event() {
        let mut planner = SourceMappingPlanner::new(header(2), 16)
            .expect("valid planner");
        let first = planner.map(event(1, 4, None)).expect("first");
        let second = planner.map(event(2, 5, Some(4))).expect("second");
        assert_eq!(second.source_id, first.source_id);
        assert_eq!(second.revision_id, first.revision_id);
        assert!(!second.opens_revision);
        assert_eq!(second.occurrence_sequence, 1);
        assert_eq!(
            planner.finish().expect("complete"),
            SourceMappingSummary {
                events: 2,
                sources: 1,
                occurrences: 1,
                path_only_events: 1,
                retirements: 0,
            }
        );
    }

    #[test]
    fn wrong_per_source_predecessor_fails_closed() {
        let mut planner = SourceMappingPlanner::new(header(2), 16)
            .expect("valid planner");
        planner.map(event(1, 4, None)).expect("first");
        assert_eq!(
            planner.map(event(2, 5, Some(99))),
            Err(SourceMappingError::PredecessorInvalid)
        );
    }
}

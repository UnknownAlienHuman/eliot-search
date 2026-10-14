//! Provider-neutral pure planning model for one projection membership.
//!
//! Public input coordinates are candidate data, not authority observations.
//! The unitizer owns complete membership and occurrence facts; registry,
//! profile and lineage acceptance still require their owners (#259 pending).
//! No value carries source text, paths, corpus or repository names, access
//! subjects, query text or vendor metadata.

use std::collections::BTreeMap;

use search_contracts::{
    AccessPartitionId, Blake3Digest32, BoundedName, CollectionGenerationId,
    InstallationIncarnationId, ProjectionMembershipId, ProjectionProfileSetId, RepositoryLineageId,
    RepresentationId, ScoringDocumentId, ScoringPartitionId, SourceId, SourceMembershipId,
    SourceRevisionId, UnitId,
};
use search_point_identity::s11::{PointId128, PointIdentity};

/// Conservative finite pure-planning limits.
///
/// The point ceiling and the manifest byte ceiling bound one plan before any
/// uncontrolled allocation. Neither widens the frozen shared vector or payload
/// ceilings owned by `search-contracts`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProjectionBudget {
    /// Maximum planned points in one membership-scoped plan.
    pub max_points: usize,
    /// Maximum bytes of one canonical manifest.
    pub max_manifest_bytes: usize,
}

impl ProjectionBudget {
    /// Baseline finite limits for pure planning.
    pub const BASELINE: Self = Self {
        max_points: 100_000,
        max_manifest_bytes: 64 * 1_024 * 1_024,
    };

    /// Validates every finite dimension as non-zero.
    pub const fn validate(self) -> Result<Self, crate::ProjectionError> {
        if self.max_points == 0 || self.max_manifest_bytes == 0 {
            return Err(crate::ProjectionError::InvalidLimits);
        }
        Ok(self)
    }
}

/// Caller-supplied source-to-projection membership coordinates.
///
/// A candidate plan contains exactly one proposed binding. This record does
/// not prove a registry relationship or grant access/publication authority.
/// Source membership is retained in the control manifest only.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectionMembershipBinding {
    /// Exactly one Qdrant projection membership.
    pub projection_membership_id: ProjectionMembershipId,
    /// Proposed source membership requiring registry validation.
    pub source_membership_id: SourceMembershipId,
    /// Stable source identity the membership projects.
    pub source_id: SourceId,
    /// Retained immutable source revision the membership projects.
    pub source_revision_id: SourceRevisionId,
    /// Exact representation the binding projects.
    pub representation_id: RepresentationId,
    /// Exact projection schema the membership binds.
    pub projection_schema_id: search_contracts::ProfileId,
    /// One exact collection generation.
    pub collection_generation_id: CollectionGenerationId,
    /// One installation incarnation, independent of physical root paths.
    pub installation_incarnation_id: InstallationIncarnationId,
    /// Proposed projection profile set identity requiring owner acceptance.
    pub projection_profile_set_id: ProjectionProfileSetId,
    /// Immutable access partition used for pre-scoring filtering.
    pub access_partition_id: AccessPartitionId,
    /// Immutable scoring/IDF partition.
    pub scoring_partition_id: ScoringPartitionId,
}

/// Caller-supplied lineage coordinate requiring owner verification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LineageObservation {
    /// Proposed lineage coordinate, absent when not supplied.
    pub repository_lineage_id: Option<RepositoryLineageId>,
}

/// One planned unit occurrence and its complete required named-vector set.
///
/// The unit is an occurrence in one exact representation, never a globally
/// stable semantic entity. Vectors are exact encoder products keyed by the
/// closed vector name; the planner never computes or reinterprets them.
#[derive(Clone, Debug, PartialEq)]
pub struct PlannedUnit {
    /// Exact unit occurrence identity in the verified set.
    pub unit_id: UnitId,
    /// Proposed kind, checked against the owner's occurrence kind.
    pub unit_kind: search_contracts::UnitKind,
    /// Content modality of the unit.
    pub modality: search_contracts::Modality,
    /// Immutable language or format profile coordinate.
    pub language_or_format: BoundedName,
    /// Optional normalized entity class.
    pub entity_kind: Option<search_contracts::EntityKind>,
    /// Optional normalized symbol key.
    pub normalized_symbol_key: Option<search_contracts::BoundedSymbolKey>,
    /// Proposed content digest, checked against the owner's exact digest.
    pub unit_digest: Blake3Digest32,
    /// Proposed reference digest, checked against the owner's exact digest.
    pub reference_digest: Blake3Digest32,
    /// One exact named vector per schema requirement, each exactly once.
    pub vectors: Vec<PlannedVector>,
}

/// One exact prepared named vector.
#[derive(Clone, Debug, PartialEq)]
pub struct PlannedVector {
    /// Exact closed vector name.
    pub name: search_contracts::indexed::VectorName,
    /// Exact dense or sparse encoder product.
    pub value: search_contracts::indexed::VectorValue,
}

/// Provider-neutral expected readback shape for publication verification.
///
/// It carries the exact payload identity, the named-vector digests and the
/// unit/reference digests a bridge must observe after publication. No vendor
/// point-id, physical collection string or route is representable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpectedReadbackShape {
    /// Exact canonical payload bytes.
    pub payload_cbor: search_contracts::CanonicalBytes,
    /// Exact internal payload identity.
    pub payload_identity: Blake3Digest32,
    /// Exact named-vector digests in canonical order.
    pub vector_digests: BTreeMap<String, Blake3Digest32>,
    /// Exact unit-content digest.
    pub unit_digest: Blake3Digest32,
    /// Exact native-reference digest.
    pub reference_digest: Blake3Digest32,
}

/// Complete exact point specification.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PointSpec {
    /// Collision-safe provider-neutral address.
    pub point_id: PointId128,
    /// Complete immutable S11 identity and full digest.
    pub identity: PointIdentity,
    /// Exact expected readback shape.
    pub expected_readback: ExpectedReadbackShape,
}

/// One exact projection-manifest entry.
///
/// Source membership is deliberately present here and absent from Qdrant
/// payload: the manifest is control evidence, not indexed projection metadata.
/// Source, revision and representation coordinates are also control-only,
/// because the shared S9.5 payload carries them as opaque UUID observations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectionManifestEntry {
    /// Compact provider-neutral point address.
    pub point_id: PointId128,
    /// Full 256-bit S11 identity digest.
    pub point_identity_digest_256: Blake3Digest32,
    /// Authoritative source membership, control-only.
    pub source_membership_id: SourceMembershipId,
    /// Exactly one projection membership.
    pub projection_membership_id: ProjectionMembershipId,
    /// Exact unit occurrence.
    pub unit_id: UnitId,
    /// Exact collection generation.
    pub collection_generation_id: CollectionGenerationId,
    /// Installation incarnation, independent of physical root paths.
    pub installation_incarnation_id: InstallationIncarnationId,
    /// Access partition used for pre-scoring filtering.
    pub access_partition_id: AccessPartitionId,
    /// Scoring/IDF partition.
    pub scoring_partition_id: ScoringPartitionId,
    /// Membership-independent scoring-document identity.
    pub scoring_document_id: ScoringDocumentId,
    /// Accepted projection profile set.
    pub projection_profile_set_id: ProjectionProfileSetId,
    /// Exact unit-content digest.
    pub unit_digest: Blake3Digest32,
    /// Exact native-reference digest.
    pub reference_digest: Blake3Digest32,
    /// Exact payload identity computed through the shared owner.
    pub payload_identity: Blake3Digest32,
    /// Exact named-vector digests in canonical order.
    pub vector_digests: BTreeMap<String, Blake3Digest32>,
}

/// Immutable CAS-ready exact projection manifest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectionManifest {
    /// Canonically point-address-ordered entries.
    pub entries: Vec<ProjectionManifestEntry>,
    /// Frozen deterministic canonical bytes.
    pub canonical_bytes: Vec<u8>,
}

/// Exact deterministic membership-scoped projection plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectionPlan {
    /// Sole source membership the plan represents.
    pub source_membership_id: SourceMembershipId,
    /// Sole projection membership every point belongs to.
    pub projection_membership_id: ProjectionMembershipId,
    /// Canonically point-address-ordered point specifications.
    pub points: Vec<PointSpec>,
    /// CAS-ready exact manifest.
    pub manifest: ProjectionManifest,
}

/// Exact old/new manifest difference. Broad selectors are structurally absent.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManifestDiff {
    /// Exact new point entries to create.
    pub create: Vec<ProjectionManifestEntry>,
    /// Exact unchanged point entries to retain.
    pub retain: Vec<ProjectionManifestEntry>,
    /// Exact old or changed point entries to retire.
    pub retire: Vec<ProjectionManifestEntry>,
}

use std::collections::BTreeMap;

use search_contracts::{
    AccessPartitionId, Blake3Digest32, BoundedSymbolKey,
    CollectionGenerationId, EntityKind, Epoch, InstallationIncarnationId,
    Modality, ProfileId, ProjectionMembershipId, ProjectionProfileSetId,
    RepositoryLineageId, RepresentationId, ScoringDocumentId,
    ScoringPartitionId, SourceId, SourceMembershipId, SourceRevisionId, UnitId,
    UnitKind,
};
use search_point_identity::{
    PointId128, PointIdentity, PointIdentityKey, PointRole,
};

use crate::ProjectionError;

/// Conservative finite pure-planning limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProjectionBudget {
    /// Maximum points in one exact membership-scoped plan.
    pub max_points: usize,
    /// Maximum named vectors carried by one point.
    pub max_vectors_per_point: usize,
    /// Maximum UTF-8 bytes in one vector name.
    pub max_vector_name_bytes: usize,
    /// Maximum stored dense/sparse values in one point.
    pub max_stored_vector_values_per_point: usize,
    /// Maximum deterministic manifest bytes.
    pub max_manifest_bytes: usize,
}

impl ProjectionBudget {
    /// Baseline finite limits.
    pub const BASELINE: Self = Self {
        max_points: 100_000,
        max_vectors_per_point: 16,
        max_vector_name_bytes: 128,
        max_stored_vector_values_per_point: 65_536,
        max_manifest_bytes: 64 * 1_024 * 1_024,
    };

    /// Validates every finite dimension.
    pub const fn validate(self) -> Result<Self, ProjectionError> {
        if self.max_points == 0
            || self.max_vectors_per_point == 0
            || self.max_vector_name_bytes == 0
            || self.max_stored_vector_values_per_point == 0
            || self.max_manifest_bytes == 0
        {
            Err(ProjectionError::InvalidLimits)
        } else {
            Ok(self)
        }
    }
}

/// Named-vector storage/scoring shape.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum VectorKind {
    /// Fixed-width dense vector.
    Dense,
    /// Sparse vector with explicit increasing dimensions.
    Sparse,
}

/// One immutable named-vector requirement in a projection profile set.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VectorRequirement {
    /// Dense width or sparse logical dimension ceiling.
    pub dimensions: u32,
    /// Dense or sparse representation.
    pub kind: VectorKind,
    /// Whether Qdrant's sparse IDF modifier is part of scoring identity.
    pub idf_enabled: bool,
}

/// Exact immutable projection profile set accepted for one plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectionProfiles {
    /// S9.5/S11 profile-set identity carried by every point.
    pub projection_profile_set_id: ProjectionProfileSetId,
    /// Projection schema identity required by the membership binding.
    pub projection_schema_id: ProfileId,
    /// Exact required named-vector set.
    pub vectors: BTreeMap<String, VectorRequirement>,
}

impl ProjectionProfiles {
    /// Validates one finite, non-empty immutable profile set.
    pub fn validate(&self, budget: ProjectionBudget) -> Result<(), ProjectionError> {
        let budget = budget.validate()?;
        if self.vectors.is_empty() || self.vectors.len() > budget.max_vectors_per_point {
            return Err(ProjectionError::VectorSetMismatch);
        }
        for (name, requirement) in &self.vectors {
            if name.is_empty()
                || name.len() > budget.max_vector_name_bytes
                || requirement.dimensions == 0
                || (requirement.kind == VectorKind::Dense && requirement.idf_enabled)
            {
                return Err(ProjectionError::VectorSetMismatch);
            }
        }
        Ok(())
    }
}

/// Exact dense or sparse values supplied by a prepared encoder contract.
#[derive(Clone, Debug, PartialEq)]
pub enum VectorValue {
    /// Dense finite values.
    Dense(Vec<f32>),
    /// Sparse finite values with strictly increasing indices.
    Sparse {
        /// Strictly increasing dimensions.
        indices: Vec<u32>,
        /// Finite values corresponding one-to-one with `indices`.
        values: Vec<f32>,
    },
}

impl VectorValue {
    /// Dense width or sparse non-zero count.
    #[must_use]
    pub const fn stored_values(&self) -> usize {
        match self {
            Self::Dense(values) | Self::Sparse { values, .. } => values.len(),
        }
    }

    /// Actual dense/sparse representation.
    #[must_use]
    pub const fn kind(&self) -> VectorKind {
        match self {
            Self::Dense(_) => VectorKind::Dense,
            Self::Sparse { .. } => VectorKind::Sparse,
        }
    }

    /// Validates exact shape and finite values against one profile requirement.
    pub fn validate(&self, requirement: VectorRequirement) -> Result<(), ProjectionError> {
        if requirement.dimensions == 0 || self.kind() != requirement.kind {
            return Err(ProjectionError::VectorDimensionMismatch);
        }
        match self {
            Self::Dense(values) => {
                if values.is_empty()
                    || usize::try_from(requirement.dimensions).ok() != Some(values.len())
                    || values.iter().any(|value| !value.is_finite())
                {
                    return Err(ProjectionError::InvalidVector);
                }
            }
            Self::Sparse { indices, values } => {
                if indices.is_empty()
                    || indices.len() != values.len()
                    || values.iter().any(|value| !value.is_finite())
                    || indices.windows(2).any(|pair| pair[0] >= pair[1])
                    || indices
                        .last()
                        .is_some_and(|index| *index >= requirement.dimensions)
                {
                    return Err(ProjectionError::InvalidVector);
                }
            }
        }
        Ok(())
    }
}

/// One prepared named vector before planner-owned digest computation.
#[derive(Clone, Debug, PartialEq)]
pub struct PreparedVector {
    /// Immutable profile-owned vector name.
    pub name: String,
    /// Exact dense or sparse values.
    pub value: VectorValue,
}

/// One admitted source-to-projection membership binding.
///
/// A plan accepts exactly one value. Source membership is retained only in the
/// immutable control manifest; it is deliberately absent from Qdrant payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectionMembershipBinding {
    /// Authoritative source membership for post-readback resolution.
    pub source_membership_id: SourceMembershipId,
    /// Exactly one Qdrant projection membership.
    pub projection_membership_id: ProjectionMembershipId,
    /// Stable source identity.
    pub source_id: SourceId,
    /// Exact representation projected by this membership.
    pub representation_id: RepresentationId,
    /// Immutable access partition identity.
    pub access_partition_id: AccessPartitionId,
    /// Immutable scoring/IDF partition identity.
    pub scoring_partition_id: ScoringPartitionId,
    /// Projection schema identity bound by control state.
    pub projection_schema_id: ProfileId,
}

/// One prepared unit occurrence and its complete required named-vector set.
#[derive(Clone, Debug, PartialEq)]
pub struct PreparedUnit {
    /// Exact unit occurrence in the representation.
    pub unit_id: UnitId,
    /// File/section/symbol/reference/test/document/table/image-region class.
    pub unit_kind: UnitKind,
    /// Unit, relation or auxiliary point role.
    pub point_role: PointRole,
    /// Content modality.
    pub modality: Modality,
    /// Immutable language or format profile.
    pub language_or_format: ProfileId,
    /// Optional normalized entity class.
    pub entity_kind: Option<EntityKind>,
    /// Optional normalized symbol key.
    pub normalized_symbol_key: Option<BoundedSymbolKey>,
    /// Exact unit-content digest retained by the manifest.
    pub unit_digest: Blake3Digest32,
    /// Exact native-reference/coordinate digest retained by the manifest.
    pub reference_digest: Blake3Digest32,
    /// Complete named-vector set required by the profile set.
    pub vectors: Vec<PreparedVector>,
}

/// Complete immutable input for one membership-scoped projection plan.
#[derive(Clone, Debug, PartialEq)]
pub struct ProjectionInput {
    /// Current installation incarnation.
    pub installation_incarnation_id: InstallationIncarnationId,
    /// Immutable physical collection generation.
    pub collection_generation_id: CollectionGenerationId,
    /// Retained immutable source revision.
    pub source_revision_id: SourceRevisionId,
    /// One exact source-to-projection membership binding.
    pub membership: ProjectionMembershipBinding,
    /// Target publication epoch. Zero is reserved for the empty generation.
    pub valid_from_epoch: Epoch,
    /// Optional repository lineage used only as an opaque payload coordinate.
    pub repository_lineage_id: Option<RepositoryLineageId>,
    /// Complete exact point set for the membership.
    pub units: Vec<PreparedUnit>,
}

/// Projection input whose scope, profile and vectors were accepted.
#[derive(Clone, Debug, PartialEq)]
pub struct ValidatedProjectionInput(pub(crate) ProjectionInput);

impl ValidatedProjectionInput {
    /// Borrows the validated input.
    #[must_use]
    pub const fn as_input(&self) -> &ProjectionInput {
        &self.0
    }

    /// Consumes the validated wrapper.
    #[must_use]
    pub fn into_input(self) -> ProjectionInput {
        self.0
    }
}

/// Exact closed S9.5 opaque point payload.
///
/// Source-membership arrays/names, ACL subjects, display paths, raw source or
/// query text, payload digests and vector digests are unrepresentable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MinimalPointPayload {
    /// Current installation incarnation.
    pub installation_incarnation_id: InstallationIncarnationId,
    /// Immutable collection generation.
    pub collection_generation_id: CollectionGenerationId,
    /// Exactly one projection membership.
    pub projection_membership_id: ProjectionMembershipId,
    /// Immutable access partition.
    pub access_partition_id: AccessPartitionId,
    /// Immutable scoring/IDF partition.
    pub scoring_partition_id: ScoringPartitionId,
    /// Stable source identity.
    pub source_id: SourceId,
    /// Retained source revision.
    pub source_revision_id: SourceRevisionId,
    /// Exact representation.
    pub representation_id: RepresentationId,
    /// Exact unit occurrence.
    pub unit_id: UnitId,
    /// Full S11.2 point identity digest.
    pub point_identity_digest_256: Blake3Digest32,
    /// Membership-independent scoring-document identity.
    pub scoring_document_id: ScoringDocumentId,
    /// Immutable named-vector/profile set.
    pub projection_profile_set_id: ProjectionProfileSetId,
    /// Unit class.
    pub unit_kind: UnitKind,
    /// Content modality.
    pub modality: Modality,
    /// Language or format profile.
    pub language_or_format: ProfileId,
    /// Optional entity class.
    pub entity_kind: Option<EntityKind>,
    /// Optional normalized symbol key.
    pub normalized_symbol_key: Option<BoundedSymbolKey>,
    /// Optional repository lineage.
    pub repository_lineage_id: Option<RepositoryLineageId>,
    /// Inclusive first visible epoch.
    pub valid_from_epoch: Epoch,
    /// Exclusive upper epoch; absent for newly active points.
    pub valid_until_epoch_exclusive: Option<Epoch>,
}

/// Planner-validated named vector with its computed immutable digest.
#[derive(Clone, Debug, PartialEq)]
pub struct NamedVector {
    /// Immutable vector name.
    pub name: String,
    /// Exact profile requirement.
    pub requirement: VectorRequirement,
    /// Exact dense or sparse values.
    pub value: VectorValue,
    /// Digest computed from name, shape and exact values.
    pub digest: Blake3Digest32,
}

/// Expected exact payload/vector shape retained for publication readback.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpectedReadbackShape {
    /// Exact S9.5 payload.
    pub payload: MinimalPointPayload,
    /// Digest of the canonical payload fields.
    pub payload_digest: Blake3Digest32,
    /// Exact named-vector digests.
    pub vector_digests: BTreeMap<String, Blake3Digest32>,
    /// Exact unit-content digest.
    pub unit_digest: Blake3Digest32,
    /// Exact native-reference digest.
    pub reference_digest: Blake3Digest32,
}

/// Complete exact point specification for a Qdrant bridge adapter.
#[derive(Clone, Debug, PartialEq)]
pub struct PointSpec {
    /// Compact provider-neutral Qdrant UUID bytes.
    pub point_id: PointId128,
    /// Complete immutable S11 identity.
    pub identity: PointIdentity,
    /// Control-only source-membership mapping; absent from Qdrant payload.
    pub source_membership_id: SourceMembershipId,
    /// Exact closed S9.5 payload.
    pub payload: MinimalPointPayload,
    /// Complete exact named vectors.
    pub vectors: BTreeMap<String, NamedVector>,
    /// Expected readback and manifest digests.
    pub expected_readback: ExpectedReadbackShape,
}

/// Exact immutable S11.3 projection-manifest entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectionManifestEntry {
    /// Exact Qdrant point UUID bytes.
    pub point_id: PointId128,
    /// Complete canonical S11.1 identity key.
    pub identity_key: PointIdentityKey,
    /// Full S11.2 identity digest.
    pub point_identity_digest_256: Blake3Digest32,
    /// Authoritative source membership retained outside Qdrant payload.
    pub source_membership_id: SourceMembershipId,
    /// Exact unit occurrence.
    pub unit_id: UnitId,
    /// Expected canonical payload digest.
    pub payload_digest: Blake3Digest32,
    /// Expected named-vector names and digests.
    pub vector_digests: BTreeMap<String, Blake3Digest32>,
    /// Exact unit-content digest.
    pub unit_digest: Blake3Digest32,
    /// Exact native-reference digest.
    pub reference_digest: Blake3Digest32,
}

/// Immutable CAS-ready exact projection manifest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectionManifest {
    /// Canonically point-ID-ordered entries.
    pub entries: Vec<ProjectionManifestEntry>,
    /// Frozen deterministic bytes used as the CAS object body.
    pub canonical_bytes: Vec<u8>,
}

/// Exact deterministic membership-scoped projection plan.
#[derive(Clone, Debug, PartialEq)]
pub struct ProjectionPlan {
    /// Sole source membership represented by the plan.
    pub source_membership_id: SourceMembershipId,
    /// Sole projection membership represented by every point.
    pub projection_membership_id: ProjectionMembershipId,
    /// Canonically point-ID-ordered point specifications.
    pub points: Vec<PointSpec>,
    /// CAS-ready immutable exact-ID manifest.
    pub manifest: ProjectionManifest,
}

/// Exact old/new manifest difference. Broad selectors are structurally absent.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManifestDiff {
    /// Exact new point entries to create.
    pub create: Vec<ProjectionManifestEntry>,
    /// Exact unchanged entries to retain.
    pub retain: Vec<ProjectionManifestEntry>,
    /// Exact old point entries to retire.
    pub retire: Vec<ProjectionManifestEntry>,
}

/// Immutable payload-index type.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PayloadIndexKind {
    /// UUID exact-match index.
    Uuid,
    /// Keyword exact-match index.
    Keyword,
    /// Signed integer range index.
    Integer,
}

/// Provider-neutral qualified collection schema description.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CollectionSchema {
    /// Named-vector schemas admitted by the collection generation.
    pub named_vectors: BTreeMap<String, VectorRequirement>,
    /// Exact payload-index name/type set.
    pub payload_indexes: BTreeMap<String, PayloadIndexKind>,
}

use std::collections::BTreeMap;

use search_contracts::{
    Blake3Digest32, BoundedSymbolKey, CollectionGenerationId, EntityKind, Epoch,
    InstallationIncarnationId, Modality, ProfileId, ProjectionMembership,
    ProjectionProfileSetId, RepositoryLineageId, ScoringDocumentId, SourceId,
    SourceRevisionId, UnitId, UnitKind,
};
use search_point_identity::{
    PointId128, PointIdentity, PointIdentityPayload, PointRole,
};

use crate::{ProjectionError, ProjectionManifest};

/// Pure BLAKE3-256 boundary used for payload, vector, and manifest digests.
///
/// The planner includes the exact versioned domain tag in every canonical byte
/// sequence before calling this port. Implementations must return the ordinary
/// 32-byte BLAKE3 digest of the supplied bytes with no additional prefix.
pub trait ProjectionDigestPort {
    /// Hashes one complete canonical byte sequence with BLAKE3-256.
    fn blake3_256(
        &mut self,
        canonical_bytes: &[u8],
    ) -> Result<Blake3Digest32, ProjectionError>;
}

/// Dense or sparse named-vector encoding supplied by an admitted encoder.
#[derive(Clone, Debug, PartialEq)]
pub enum VectorValue {
    /// Dense finite vector.
    Dense(Vec<f32>),
    /// Sparse finite vector with strictly increasing indices.
    Sparse {
        /// Strictly increasing dimensions.
        indices: Vec<u32>,
        /// Finite values corresponding one-to-one with `indices`.
        values: Vec<f32>,
    },
}

impl VectorValue {
    /// Number of stored dense values or sparse non-zero entries.
    #[must_use]
    pub const fn stored_values(&self) -> usize {
        match self {
            Self::Dense(values) | Self::Sparse { values, .. } => values.len(),
        }
    }

    /// Whether the supplied representation is sparse.
    #[must_use]
    pub const fn is_sparse(&self) -> bool {
        matches!(self, Self::Sparse { .. })
    }

    /// Validates exact dimensions, finite values, and sparse ordering.
    pub fn validate(&self, dimensions: u32) -> Result<(), ProjectionError> {
        if dimensions == 0 {
            return Err(ProjectionError::VectorDimensionMismatch);
        }
        match self {
            Self::Dense(values) => {
                if usize::try_from(dimensions).ok() != Some(values.len())
                    || values.is_empty()
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
                    || indices.last().is_some_and(|index| *index >= dimensions)
                {
                    return Err(ProjectionError::InvalidVector);
                }
            }
        }
        Ok(())
    }
}

/// One exact named vector supplied to the pure planner.
#[derive(Clone, Debug, PartialEq)]
pub struct NamedVectorInput {
    /// Stable profile-owned vector name.
    pub name: String,
    /// Declared vector dimensions.
    pub dimensions: u32,
    /// Exact finite vector values.
    pub value: VectorValue,
}

/// One validated named vector and its planner-owned expected digest.
#[derive(Clone, Debug, PartialEq)]
pub struct PlannedVector {
    /// Stable profile-owned vector name.
    pub name: String,
    /// Declared vector dimensions.
    pub dimensions: u32,
    /// Exact finite vector values.
    pub value: VectorValue,
    /// Digest of the versioned canonical vector encoding.
    pub digest: Blake3Digest32,
}

/// Required vector shape for one accepted projection profile set.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VectorRequirement {
    /// Required dimensions.
    pub dimensions: u32,
    /// Whether the vector uses sparse storage.
    pub sparse: bool,
}

/// Accepted immutable projection profile set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectionProfiles {
    /// Exact profile-set identity encoded into every point identity and payload.
    pub profile_set_id: ProjectionProfileSetId,
    /// Digest of the complete accepted profile behavior.
    pub profile_set_digest: Blake3Digest32,
    /// Projection schema identity required by the membership binding.
    pub projection_schema_id: ProfileId,
    /// Exact named-vector set required on every point in the plan.
    pub vectors: BTreeMap<String, VectorRequirement>,
}

impl ProjectionProfiles {
    /// Validates the finite exact profile set.
    pub fn validate(&self, budget: ProjectionBudget) -> Result<(), ProjectionError> {
        budget.validate()?;
        if self.vectors.is_empty() || self.vectors.len() > budget.max_vectors_per_point {
            return Err(ProjectionError::VectorSetMismatch);
        }
        for (name, requirement) in &self.vectors {
            if name.is_empty()
                || name.len() > budget.max_vector_name_bytes
                || requirement.dimensions == 0
            {
                return Err(ProjectionError::VectorSetMismatch);
            }
        }
        Ok(())
    }
}

/// Finite pure-planning and canonical-encoding limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProjectionBudget {
    /// Maximum points in one exact membership-scoped plan.
    pub max_points: usize,
    /// Maximum named vectors per point.
    pub max_vectors_per_point: usize,
    /// Maximum UTF-8 bytes in one vector name.
    pub max_vector_name_bytes: usize,
    /// Maximum stored dense/sparse values per point.
    pub max_stored_vector_values_per_point: usize,
    /// Maximum canonical payload bytes per point.
    pub max_payload_bytes: usize,
    /// Maximum canonical vector bytes per vector.
    pub max_vector_bytes: usize,
    /// Maximum canonical manifest bytes.
    pub max_manifest_bytes: usize,
}

impl ProjectionBudget {
    /// Conservative baseline limits.
    pub const BASELINE: Self = Self {
        max_points: 100_000,
        max_vectors_per_point: 16,
        max_vector_name_bytes: 128,
        max_stored_vector_values_per_point: 65_536,
        max_payload_bytes: 16 * 1_024,
        max_vector_bytes: 4 * 1_024 * 1_024,
        max_manifest_bytes: 64 * 1_024 * 1_024,
    };

    /// Validates every finite dimension.
    pub const fn validate(self) -> Result<Self, ProjectionError> {
        if self.max_points == 0
            || self.max_vectors_per_point == 0
            || self.max_vector_name_bytes == 0
            || self.max_stored_vector_values_per_point == 0
            || self.max_payload_bytes == 0
            || self.max_vector_bytes == 0
            || self.max_manifest_bytes == 0
        {
            Err(ProjectionError::InvalidLimits)
        } else {
            Ok(self)
        }
    }
}

/// One immutable collection/membership/source scope for an exact point plan.
///
/// `ProjectionMembership` carries the authoritative one-to-one source
/// membership, representation, access partition, scoring partition, and
/// projection schema binding. Source membership is retained only in this
/// control-side scope and is never copied into the Qdrant point payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectionScope {
    /// Current installation incarnation.
    pub installation_incarnation_id: InstallationIncarnationId,
    /// Exact target collection generation.
    pub collection_generation_id: CollectionGenerationId,
    /// Authoritative immutable projection-membership binding.
    pub membership: ProjectionMembership,
    /// Stable source identity represented by this membership.
    pub source_id: SourceId,
    /// Exact retained source revision represented by this plan.
    pub source_revision_id: SourceRevisionId,
    /// Epoch at which newly staged points become eligible after control commit.
    pub valid_from_epoch: Epoch,
}

/// One prepared unit and all payload/vector metadata required by S9.5.
#[derive(Clone, Debug, PartialEq)]
pub struct ProjectionUnitInput {
    /// Exact occurrence unit identity.
    pub unit_id: UnitId,
    /// Representation identity; must equal the projection membership binding.
    pub representation_id: search_contracts::RepresentationId,
    /// Closed unit kind.
    pub unit_kind: UnitKind,
    /// Membership-independent scoring-document identity.
    pub scoring_document_id: ScoringDocumentId,
    /// Unit modality.
    pub modality: Modality,
    /// Exact language/format profile keyword.
    pub language_or_format: ProfileId,
    /// Optional closed entity kind.
    pub entity_kind: Option<EntityKind>,
    /// Optional normalized symbol key.
    pub normalized_symbol_key: Option<BoundedSymbolKey>,
    /// Optional repository lineage identity.
    pub repository_lineage_id: Option<RepositoryLineageId>,
    /// Logical role encoded into the S11 point identity.
    pub point_role: PointRole,
    /// Complete named-vector set supplied by admitted encoders.
    pub vectors: Vec<NamedVectorInput>,
}

/// Complete immutable input for one membership-scoped projection plan.
#[derive(Clone, Debug, PartialEq)]
pub struct ProjectionInput {
    /// Exact collection/membership/source scope.
    pub scope: ProjectionScope,
    /// Complete prepared unit set for that scope.
    pub units: Vec<ProjectionUnitInput>,
}

/// Projection input whose profile, scope, vector set, and finite bounds passed.
#[derive(Clone, Debug, PartialEq)]
pub struct ValidatedProjectionInput(pub(crate) ProjectionInput);

impl ValidatedProjectionInput {
    /// Borrows the exact validated input.
    #[must_use]
    pub const fn as_input(&self) -> &ProjectionInput {
        &self.0
    }

    /// Consumes the wrapper.
    #[must_use]
    pub fn into_input(self) -> ProjectionInput {
        self.0
    }
}

/// Exact closed S9.5 Qdrant point payload.
///
/// Source membership, corpus/repository display names, ACL subjects, paths,
/// raw source text, query text, payload digests, and vector digests are not
/// representable in this type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MinimalPointPayload {
    /// Current installation incarnation.
    pub installation_incarnation_id: InstallationIncarnationId,
    /// Exact physical collection generation.
    pub collection_generation_id: CollectionGenerationId,
    /// One immutable projection membership.
    pub projection_membership_id: search_contracts::ProjectionMembershipId,
    /// Immutable access partition.
    pub access_partition_id: search_contracts::AccessPartitionId,
    /// Immutable scoring partition.
    pub scoring_partition_id: search_contracts::ScoringPartitionId,
    /// Stable source identity.
    pub source_id: SourceId,
    /// Exact retained source revision.
    pub source_revision_id: SourceRevisionId,
    /// Exact canonical representation.
    pub representation_id: search_contracts::RepresentationId,
    /// Exact occurrence unit.
    pub unit_id: UnitId,
    /// Full BLAKE3-256 canonical point-identity digest.
    pub point_identity_digest_256: Blake3Digest32,
    /// Membership-independent scoring-document identity.
    pub scoring_document_id: ScoringDocumentId,
    /// Immutable vector/analyzer/profile-set identity.
    pub projection_profile_set_id: ProjectionProfileSetId,
    /// Closed unit kind.
    pub unit_kind: UnitKind,
    /// Closed modality.
    pub modality: Modality,
    /// Exact language/format keyword.
    pub language_or_format: ProfileId,
    /// Optional closed entity kind.
    pub entity_kind: Option<EntityKind>,
    /// Optional normalized symbol key.
    pub normalized_symbol_key: Option<BoundedSymbolKey>,
    /// Optional repository lineage identity.
    pub repository_lineage_id: Option<RepositoryLineageId>,
    /// Inclusive epoch lower bound.
    pub valid_from_epoch: Epoch,
    /// Exclusive epoch upper bound; absent on newly staged active points.
    pub valid_until_epoch_exclusive: Option<Epoch>,
}

impl MinimalPointPayload {
    /// Validates the open-ended or finite validity interval.
    pub fn validate(&self) -> Result<(), ProjectionError> {
        if self
            .valid_until_epoch_exclusive
            .is_some_and(|until| until <= self.valid_from_epoch)
        {
            return Err(ProjectionError::PointPayloadInvalid);
        }
        Ok(())
    }
}

/// Expected exact readback shape retained in the immutable manifest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpectedReadbackShape {
    /// S11.2 identity fields expected from exact payload readback.
    pub identity_payload: PointIdentityPayload,
    /// Digest of the complete canonical S9.5 payload encoding.
    pub payload_digest: Blake3Digest32,
    /// Exact named-vector digests.
    pub vector_digests: BTreeMap<String, Blake3Digest32>,
}

/// Complete exact point specification ready for a vendor-neutral bridge adapter.
#[derive(Clone, Debug, PartialEq)]
pub struct PointSpec {
    /// Compact provider-neutral Qdrant point address.
    pub point_id: PointId128,
    /// Complete immutable S11 logical identity.
    pub identity: PointIdentity,
    /// Exact closed S9.5 payload.
    pub payload: MinimalPointPayload,
    /// Complete named vectors with planner-owned expected digests.
    pub vectors: BTreeMap<String, PlannedVector>,
    /// Exact expected payload/vector readback shape.
    pub expected_readback: ExpectedReadbackShape,
}

/// Complete deterministic plan for exactly one projection membership.
#[derive(Clone, Debug, PartialEq)]
pub struct ProjectionPlan {
    /// Exact control-side scope, including source-membership mapping.
    pub scope: ProjectionScope,
    /// Exact accepted profile set.
    pub profiles: ProjectionProfiles,
    /// Canonically point-ID-ordered specifications.
    pub points: Vec<PointSpec>,
    /// CAS-ready immutable exact manifest.
    pub manifest: ProjectionManifest,
}

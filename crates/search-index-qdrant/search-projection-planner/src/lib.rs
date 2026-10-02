//! Pure deterministic planning for canonical Qdrant projections.
//!
//! This package performs no database, filesystem, network, admission or access
//! decision. It converts already-admitted typed projection inputs into exact
//! S9.5 payloads, named vectors and immutable S11.3 manifests.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![allow(
    clippy::large_enum_variant,
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::too_many_arguments,
    clippy::too_many_lines
)]

use core::fmt;
use std::collections::{BTreeMap, BTreeSet};

use search_contracts::{
    AccessPartitionId, Blake3Digest32, BoundedSymbolKey, CollectionGenerationId,
    EntityKind, Epoch, InstallationIncarnationId, Modality, ProfileId,
    ProjectionMembershipId, ProjectionProfileSetId, RepositoryLineageId,
    RepresentationId, ScoringDocumentId, ScoringPartitionId, SourceId,
    SourceMembershipId, SourceRevisionId, UnitId, UnitKind,
};
use search_point_identity::{
    POINT_KEY_SCHEMA_VERSION, PointId128, PointIdentity, PointIdentityError,
    PointIdentityLimits, PointRole, ProjectionPointKey, canonical_point_key_bytes,
    derive_point_identity,
};

const PAYLOAD_DIGEST_DOMAIN: &[u8] = b"eliot-search/projection-payload/v1\0";
const VECTOR_DIGEST_DOMAIN: &[u8] = b"eliot-search/projection-vector/v1\0";
const MANIFEST_DOMAIN: &[u8] = b"eliot-search/projection-manifest/v2\0";

/// Exact S9.5 payload-index field set.
pub const REQUIRED_PAYLOAD_INDEX_FIELDS: [&str; 19] = [
    "installation_incarnation_id",
    "collection_generation_id",
    "projection_membership_id",
    "access_partition_id",
    "scoring_partition_id",
    "source_id",
    "source_revision_id",
    "representation_id",
    "unit_id",
    "scoring_document_id",
    "projection_profile_set_id",
    "unit_kind",
    "modality",
    "language_or_format",
    "entity_kind",
    "normalized_symbol_key",
    "repository_lineage_id",
    "valid_from_epoch",
    "valid_until_epoch_exclusive",
];

/// Compatibility name for the exact payload-index set.
pub const REQUIRED_FILTER_FIELDS: [&str; 19] = REQUIRED_PAYLOAD_INDEX_FIELDS;

/// Closed projection-planning failure.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ProjectionError {
    /// A finite planning limit is zero or internally inconsistent.
    InvalidLimits,
    /// Point identity input is invalid or collided.
    PointIdentity,
    /// Source/projection membership binding is inconsistent.
    MembershipMismatch,
    /// More than one membership was supplied where exactly one is required.
    MembershipArrayForbidden,
    /// A raw public/vendor collection identifier reached a boundary.
    RawCollectionIdForbidden,
    /// Inputs span inequivalent security/scoring/generation scope.
    ScopeMismatch,
    /// Unit residency does not match the admitted binding.
    ResidencyMismatch,
    /// An expected unit has no admitted input.
    MissingUnitReceipt,
    /// An input unit is outside the declared complete set.
    UnexpectedUnit,
    /// Unit byte range is empty or inverted.
    InvalidUnitRange,
    /// Named vector set differs from the accepted profile.
    VectorSetMismatch,
    /// A vector name appears more than once.
    DuplicateVectorName,
    /// Vector dimensions differ from the accepted schema.
    VectorDimensionMismatch,
    /// Dense or sparse vector values are invalid.
    InvalidVector,
    /// Supplied vector digest differs from canonical values.
    VectorDigestMismatch,
    /// A plan exceeds its finite budget.
    BudgetExceeded,
    /// Two specs resolve to the same compact point ID.
    DuplicatePointId,
    /// One unit/point role appears more than once.
    DuplicateUnitRole,
    /// Manifest ordering, entries or reconstruction are invalid.
    InvalidManifest,
    /// Canonical manifest encoding overflowed its ceiling.
    ManifestTooLarge,
    /// Required named vectors are absent from collection schema.
    CollectionVectorMissing,
    /// Collection vector dimensions are incompatible.
    CollectionVectorMismatch,
    /// A required payload field lacks an index.
    PayloadIndexMissing,
    /// Collection carries indexes outside the accepted closed schema.
    PayloadIndexUnexpected,
}

impl ProjectionError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidLimits => "PROJECTION_INVALID_LIMITS",
            Self::PointIdentity => "PROJECTION_POINT_IDENTITY_INVALID",
            Self::MembershipMismatch => "PROJECTION_MEMBERSHIP_MISMATCH",
            Self::MembershipArrayForbidden => "PROJECTION_MEMBERSHIP_ARRAY_FORBIDDEN",
            Self::RawCollectionIdForbidden => "PROJECTION_RAW_COLLECTION_ID_FORBIDDEN",
            Self::ScopeMismatch => "PROJECTION_SCOPE_MISMATCH",
            Self::ResidencyMismatch => "PROJECTION_RESIDENCY_MISMATCH",
            Self::MissingUnitReceipt => "PROJECTION_MISSING_UNIT_RECEIPT",
            Self::UnexpectedUnit => "PROJECTION_UNEXPECTED_UNIT",
            Self::InvalidUnitRange => "PROJECTION_INVALID_UNIT_RANGE",
            Self::VectorSetMismatch => "PROJECTION_VECTOR_SET_MISMATCH",
            Self::DuplicateVectorName => "PROJECTION_DUPLICATE_VECTOR_NAME",
            Self::VectorDimensionMismatch => "PROJECTION_VECTOR_DIMENSION_MISMATCH",
            Self::InvalidVector => "PROJECTION_INVALID_VECTOR",
            Self::VectorDigestMismatch => "PROJECTION_VECTOR_DIGEST_MISMATCH",
            Self::BudgetExceeded => "PROJECTION_BUDGET_EXCEEDED",
            Self::DuplicatePointId => "PROJECTION_DUPLICATE_POINT_ID",
            Self::DuplicateUnitRole => "PROJECTION_DUPLICATE_UNIT_ROLE",
            Self::InvalidManifest => "PROJECTION_MANIFEST_MISMATCH",
            Self::ManifestTooLarge => "PROJECTION_MANIFEST_TOO_LARGE",
            Self::CollectionVectorMissing => "PROJECTION_COLLECTION_VECTOR_MISSING",
            Self::CollectionVectorMismatch => "PROJECTION_COLLECTION_VECTOR_MISMATCH",
            Self::PayloadIndexMissing => "PROJECTION_PAYLOAD_INDEX_MISSING",
            Self::PayloadIndexUnexpected => "PROJECTION_PAYLOAD_INDEX_UNEXPECTED",
        }
    }
}

impl fmt::Display for ProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for ProjectionError {}

impl From<PointIdentityError> for ProjectionError {
    fn from(_: PointIdentityError) -> Self {
        Self::PointIdentity
    }
}

/// Dense or sparse named-vector encoding.
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
    /// Number of supplied dense values or sparse non-zero entries.
    #[must_use]
    pub const fn stored_values(&self) -> usize {
        match self {
            Self::Dense(values) | Self::Sparse { values, .. } => values.len(),
        }
    }

    /// Validates finite values and sparse ordering against declared width.
    pub fn validate(&self, declared_dimensions: u32) -> Result<(), ProjectionError> {
        if declared_dimensions == 0 {
            return Err(ProjectionError::VectorDimensionMismatch);
        }
        match self {
            Self::Dense(values) => {
                if usize::try_from(declared_dimensions).ok() != Some(values.len())
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
                    || indices.last().is_some_and(|index| *index >= declared_dimensions)
                {
                    return Err(ProjectionError::InvalidVector);
                }
            }
        }
        Ok(())
    }
}

/// One exact named vector and its canonical digest.
#[derive(Clone, Debug, PartialEq)]
pub struct NamedVector {
    /// Stable profile-owned vector name.
    pub name: String,
    /// Declared vector dimensions.
    pub dimensions: u32,
    /// Exact finite vector values.
    pub value: VectorValue,
    /// Expected digest of canonical name/dimension/kind/value encoding.
    pub digest: Blake3Digest32,
}

impl NamedVector {
    /// Computes the canonical digest from exact vector values.
    #[must_use]
    pub fn canonical_digest(&self) -> Blake3Digest32 {
        let mut hasher = blake3::Hasher::new();
        hasher.update(VECTOR_DIGEST_DOMAIN);
        hash_text(&mut hasher, &self.name);
        hasher.update(&self.dimensions.to_be_bytes());
        match &self.value {
            VectorValue::Dense(values) => {
                hasher.update(&[0]);
                hasher.update(&u64::try_from(values.len()).unwrap_or(u64::MAX).to_be_bytes());
                for value in values {
                    hasher.update(&value.to_bits().to_be_bytes());
                }
            }
            VectorValue::Sparse { indices, values } => {
                hasher.update(&[1]);
                hasher.update(&u64::try_from(values.len()).unwrap_or(u64::MAX).to_be_bytes());
                for (index, value) in indices.iter().zip(values) {
                    hasher.update(&index.to_be_bytes());
                    hasher.update(&value.to_bits().to_be_bytes());
                }
            }
        }
        Blake3Digest32::from_bytes(*hasher.finalize().as_bytes())
    }
}

/// Required vector shape for one accepted profile set.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VectorRequirement {
    /// Required dimensions.
    pub dimensions: u32,
    /// Whether this vector is sparse.
    pub sparse: bool,
}

/// Accepted exact projection profile set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectionProfiles {
    /// Stable immutable profile-set identifier.
    pub profile_set_id: ProjectionProfileSetId,
    /// Digest of the complete accepted profile configuration.
    pub profile_set_digest: Blake3Digest32,
    /// Exact required named-vector set.
    pub vectors: BTreeMap<String, VectorRequirement>,
}

impl ProjectionProfiles {
    /// Validates a finite non-empty vector schema.
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

/// Finite pure-planning limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProjectionBudget {
    /// Maximum points in one exact plan.
    pub max_points: usize,
    /// Maximum named vectors per point.
    pub max_vectors_per_point: usize,
    /// Maximum UTF-8 bytes in one vector name.
    pub max_vector_name_bytes: usize,
    /// Maximum stored dense/sparse values per point.
    pub max_stored_vector_values_per_point: usize,
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

/// Immutable admitted unit prepared for one point role.
#[derive(Clone, Debug, PartialEq)]
pub struct ProjectionInput {
    /// Current installation incarnation.
    pub installation_incarnation_id: InstallationIncarnationId,
    /// Target collection generation.
    pub collection_generation_id: CollectionGenerationId,
    /// Authoritative source membership retained only in manifest/control data.
    pub source_membership_id: SourceMembershipId,
    /// Exactly one projection membership stored in payload.
    pub projection_membership_id: ProjectionMembershipId,
    /// Immutable access partition.
    pub access_partition_id: AccessPartitionId,
    /// Immutable scoring partition.
    pub scoring_partition_id: ScoringPartitionId,
    /// Stable source identity.
    pub source_id: SourceId,
    /// Retained source revision.
    pub source_revision_id: SourceRevisionId,
    /// Immutable representation identity.
    pub representation_id: RepresentationId,
    /// Immutable unit occurrence identity.
    pub unit_id: UnitId,
    /// Opaque scoring-document identity used for duplicate routing.
    pub scoring_document_id: ScoringDocumentId,
    /// Exact profile-set identity; must equal [`ProjectionProfiles`].
    pub projection_profile_set_id: ProjectionProfileSetId,
    /// Exact point role used by S11 identity.
    pub point_role: PointRole,
    /// Unit kind metadata.
    pub unit_kind: UnitKind,
    /// Modality metadata.
    pub modality: Modality,
    /// Language or format profile.
    pub language_or_format: ProfileId,
    /// Optional entity classification.
    pub entity_kind: Option<EntityKind>,
    /// Optional normalized symbol key.
    pub normalized_symbol_key: Option<BoundedSymbolKey>,
    /// Optional repository lineage.
    pub repository_lineage_id: Option<RepositoryLineageId>,
    /// First epoch in which this point may be visible.
    pub valid_from_epoch: Epoch,
    /// Optional exclusive final epoch; normally absent for a staged point.
    pub valid_until_epoch_exclusive: Option<Epoch>,
    /// Deterministic unit ordinal for complete-set checking only.
    pub unit_ordinal: u64,
    /// Inclusive exact source byte start for source-backed validation.
    pub source_byte_start: u64,
    /// Exclusive exact source byte end.
    pub source_byte_end: u64,
    /// Exact unit bytes digest.
    pub unit_digest: Blake3Digest32,
    /// Exact native-reference digest.
    pub reference_digest: Blake3Digest32,
    /// Admitted residency binding digest.
    pub residency_digest: Blake3Digest32,
    /// Complete named-vector set.
    pub vectors: Vec<NamedVector>,
}

/// Projection input whose profile, vectors and finite bounds were accepted.
#[derive(Clone, Debug, PartialEq)]
pub struct ValidatedProjectionInput(ProjectionInput);

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

/// Exact closed S9.5 opaque point payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MinimalPointPayload {
    /// Current installation incarnation.
    pub installation_incarnation_id: InstallationIncarnationId,
    /// Collection generation.
    pub collection_generation_id: CollectionGenerationId,
    /// Exactly one projection membership.
    pub projection_membership_id: ProjectionMembershipId,
    /// Immutable access partition.
    pub access_partition_id: AccessPartitionId,
    /// Immutable scoring partition.
    pub scoring_partition_id: ScoringPartitionId,
    /// Stable source identity.
    pub source_id: SourceId,
    /// Retained source revision.
    pub source_revision_id: SourceRevisionId,
    /// Immutable representation identity.
    pub representation_id: RepresentationId,
    /// Immutable unit identity.
    pub unit_id: UnitId,
    /// Full BLAKE3-256 S11 identity digest.
    pub point_identity_digest_256: Blake3Digest32,
    /// Opaque scoring-document identity.
    pub scoring_document_id: ScoringDocumentId,
    /// Projection profile set.
    pub projection_profile_set_id: ProjectionProfileSetId,
    /// Unit kind.
    pub unit_kind: UnitKind,
    /// Modality.
    pub modality: Modality,
    /// Language or format profile.
    pub language_or_format: ProfileId,
    /// Optional entity kind.
    pub entity_kind: Option<EntityKind>,
    /// Optional normalized symbol key.
    pub normalized_symbol_key: Option<BoundedSymbolKey>,
    /// Optional repository lineage.
    pub repository_lineage_id: Option<RepositoryLineageId>,
    /// First valid epoch.
    pub valid_from_epoch: Epoch,
    /// Optional exclusive final epoch.
    pub valid_until_epoch_exclusive: Option<Epoch>,
}

impl MinimalPointPayload {
    /// Validates the open-ended epoch interval.
    pub fn validate(&self) -> Result<(), ProjectionError> {
        if self
            .valid_until_epoch_exclusive
            .is_some_and(|until| until <= self.valid_from_epoch)
        {
            Err(ProjectionError::ScopeMismatch)
        } else {
            Ok(())
        }
    }

    /// Canonical expected payload digest retained in the manifest.
    #[must_use]
    pub fn canonical_digest(&self) -> Blake3Digest32 {
        let mut hasher = blake3::Hasher::new();
        hasher.update(PAYLOAD_DIGEST_DOMAIN);
        hash_bytes(&mut hasher, self.installation_incarnation_id.as_bytes());
        hash_bytes(&mut hasher, self.collection_generation_id.as_bytes());
        hash_bytes(&mut hasher, self.projection_membership_id.as_bytes());
        hash_bytes(&mut hasher, self.access_partition_id.as_bytes());
        hash_bytes(&mut hasher, self.scoring_partition_id.as_bytes());
        hash_bytes(&mut hasher, self.source_id.as_bytes());
        hash_bytes(&mut hasher, self.source_revision_id.as_bytes());
        hash_bytes(&mut hasher, self.representation_id.as_bytes());
        hash_bytes(&mut hasher, self.unit_id.as_bytes());
        hash_bytes(&mut hasher, self.point_identity_digest_256.as_bytes());
        hash_bytes(&mut hasher, self.scoring_document_id.as_bytes());
        hash_text(&mut hasher, self.projection_profile_set_id.as_str());
        hash_text(&mut hasher, self.unit_kind.as_str());
        hash_text(&mut hasher, self.modality.as_str());
        hash_text(&mut hasher, self.language_or_format.as_str());
        hash_optional_text(&mut hasher, self.entity_kind.map(EntityKind::as_str));
        hash_optional_text(
            &mut hasher,
            self.normalized_symbol_key.as_ref().map(BoundedSymbolKey::as_str),
        );
        hash_optional_bytes(
            &mut hasher,
            self.repository_lineage_id.as_ref().map(RepositoryLineageId::as_bytes),
        );
        hasher.update(&self.valid_from_epoch.get().to_be_bytes());
        match self.valid_until_epoch_exclusive {
            Some(until) => {
                hasher.update(&[1]);
                hasher.update(&until.get().to_be_bytes());
            }
            None => {
                hasher.update(&[0]);
            }
        }
        Blake3Digest32::from_bytes(*hasher.finalize().as_bytes())
    }
}

/// Expected exact readback shape for publication verification.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpectedReadbackShape {
    /// Complete canonical identity key.
    pub identity_key: ProjectionPointKey,
    /// Full S11 identity digest.
    pub point_identity_digest_256: Blake3Digest32,
    /// Exact payload digest retained outside Qdrant payload.
    pub payload_digest: Blake3Digest32,
    /// Exact named-vector digests.
    pub vector_digests: BTreeMap<String, Blake3Digest32>,
    /// Exact unit digest.
    pub unit_digest: Blake3Digest32,
    /// Exact native-reference digest.
    pub reference_digest: Blake3Digest32,
}

/// Complete exact point specification ready for a bridge adapter.
#[derive(Clone, Debug, PartialEq)]
pub struct PointSpec {
    /// Compact provider-neutral point identifier.
    pub point_id: PointId128,
    /// Complete immutable point identity.
    pub identity: PointIdentity,
    /// Exact S9.5 payload.
    pub payload: MinimalPointPayload,
    /// Complete exact named vectors.
    pub vectors: BTreeMap<String, NamedVector>,
    /// Expected exact readback shape.
    pub expected_readback: ExpectedReadbackShape,
    /// Authoritative source membership retained outside Qdrant payload.
    pub source_membership_id: SourceMembershipId,
    /// Inclusive source byte start retained for source validation.
    pub source_byte_start: u64,
    /// Exclusive source byte end retained for source validation.
    pub source_byte_end: u64,
    /// Residency binding retained in the manifest.
    pub residency_digest: Blake3Digest32,
}

/// Exact point manifest entry without raw vector values.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectionManifestEntry {
    /// Compact point identifier.
    pub point_id: PointId128,
    /// Complete canonical identity key.
    pub identity_key: ProjectionPointKey,
    /// Full S11 identity digest.
    pub point_identity_digest_256: Blake3Digest32,
    /// Authoritative source membership resolved after readback.
    pub source_membership_id: SourceMembershipId,
    /// Projection membership stored in payload.
    pub projection_membership_id: ProjectionMembershipId,
    /// Retained source revision.
    pub source_revision_id: SourceRevisionId,
    /// Representation identity.
    pub representation_id: RepresentationId,
    /// Unit identity.
    pub unit_id: UnitId,
    /// Source byte start.
    pub source_byte_start: u64,
    /// Source byte end.
    pub source_byte_end: u64,
    /// Exact unit digest.
    pub unit_digest: Blake3Digest32,
    /// Exact reference digest.
    pub reference_digest: Blake3Digest32,
    /// Residency binding digest.
    pub residency_digest: Blake3Digest32,
    /// Exact canonical payload digest.
    pub payload_digest: Blake3Digest32,
    /// Exact named-vector digests.
    pub vector_digests: BTreeMap<String, Blake3Digest32>,
}

/// One expected unit receipt in a complete scoped point set.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExpectedUnit {
    /// Exact unit identity.
    pub unit_id: UnitId,
    /// Deterministic ordinal for completeness diagnostics.
    pub unit_ordinal: u64,
    /// Exact unit bytes digest.
    pub unit_digest: Blake3Digest32,
}

/// Exact admitted scope one plan must satisfy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScopeExpectation {
    /// Installation incarnation.
    pub installation_incarnation_id: InstallationIncarnationId,
    /// Collection generation.
    pub collection_generation_id: CollectionGenerationId,
    /// Authoritative source membership.
    pub source_membership_id: SourceMembershipId,
    /// Projection membership.
    pub projection_membership_id: ProjectionMembershipId,
    /// Access partition.
    pub access_partition_id: AccessPartitionId,
    /// Scoring partition.
    pub scoring_partition_id: ScoringPartitionId,
    /// Source identity.
    pub source_id: SourceId,
    /// Retained source revision.
    pub source_revision_id: SourceRevisionId,
    /// Representation identity.
    pub representation_id: RepresentationId,
    /// Projection profile set.
    pub projection_profile_set_id: ProjectionProfileSetId,
    /// Complete profile configuration digest.
    pub profile_set_digest: Blake3Digest32,
    /// Admitted residency binding digest.
    pub residency_digest: Blake3Digest32,
}

/// Immutable CAS-ready exact projection manifest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectionManifest {
    /// Canonically point-ID-ordered entries.
    pub entries: Vec<ProjectionManifestEntry>,
    /// Frozen deterministic canonical bytes.
    pub canonical_bytes: Vec<u8>,
}

/// Exact deterministic projection plan.
#[derive(Clone, Debug, PartialEq)]
pub struct ProjectionPlan {
    /// Canonically point-ID-ordered specs.
    pub points: Vec<PointSpec>,
    /// CAS-ready exact manifest.
    pub manifest: ProjectionManifest,
}

/// Exact old/new manifest difference.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManifestDiff {
    /// New or changed points to create.
    pub create: Vec<ProjectionManifestEntry>,
    /// Unchanged exact points to retain.
    pub retain: Vec<ProjectionManifestEntry>,
    /// Old or changed points to retire.
    pub retire: Vec<ProjectionManifestEntry>,
}

/// Provider-neutral collection schema requirements.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CollectionSchema {
    /// Named-vector dimensions.
    pub named_vectors: BTreeMap<String, u32>,
    /// Exact payload index names.
    pub indexed_payload_fields: BTreeSet<String>,
}

/// Validates immutable input against the accepted profile set.
pub fn validate_projection_input(
    input: ProjectionInput,
    profiles: &ProjectionProfiles,
    budget: ProjectionBudget,
) -> Result<ValidatedProjectionInput, ProjectionError> {
    let budget = budget.validate()?;
    profiles.validate(budget)?;
    if input.projection_profile_set_id != profiles.profile_set_id {
        return Err(ProjectionError::ScopeMismatch);
    }
    if input.source_byte_start >= input.source_byte_end {
        return Err(ProjectionError::InvalidUnitRange);
    }
    if input
        .valid_until_epoch_exclusive
        .is_some_and(|until| until <= input.valid_from_epoch)
    {
        return Err(ProjectionError::ScopeMismatch);
    }
    if input.vectors.len() != profiles.vectors.len()
        || input.vectors.len() > budget.max_vectors_per_point
    {
        return Err(ProjectionError::VectorSetMismatch);
    }
    let mut names = BTreeSet::new();
    let mut stored_values = 0_usize;
    for vector in &input.vectors {
        if vector.name.is_empty() || vector.name.len() > budget.max_vector_name_bytes {
            return Err(ProjectionError::VectorSetMismatch);
        }
        if !names.insert(vector.name.clone()) {
            return Err(ProjectionError::DuplicateVectorName);
        }
        let requirement = profiles
            .vectors
            .get(&vector.name)
            .ok_or(ProjectionError::VectorSetMismatch)?;
        if vector.dimensions != requirement.dimensions {
            return Err(ProjectionError::VectorDimensionMismatch);
        }
        let sparse = matches!(vector.value, VectorValue::Sparse { .. });
        if sparse != requirement.sparse {
            return Err(ProjectionError::VectorSetMismatch);
        }
        vector.value.validate(vector.dimensions)?;
        if vector.digest != vector.canonical_digest() {
            return Err(ProjectionError::VectorDigestMismatch);
        }
        stored_values = stored_values
            .checked_add(vector.value.stored_values())
            .ok_or(ProjectionError::BudgetExceeded)?;
    }
    if stored_values > budget.max_stored_vector_values_per_point
        || names != profiles.vectors.keys().cloned().collect()
    {
        return Err(ProjectionError::BudgetExceeded);
    }
    Ok(ValidatedProjectionInput(input))
}

/// Validates an already-built exact S9.5 payload.
pub fn validate_minimal_payload(
    payload: &MinimalPointPayload,
) -> Result<(), ProjectionError> {
    payload.validate()
}

/// Builds the only payload shape allowed for ordinary publication.
pub fn build_minimal_payload(
    input: &ValidatedProjectionInput,
    identity: &PointIdentity,
) -> Result<MinimalPointPayload, ProjectionError> {
    let input = input.as_input();
    let payload = MinimalPointPayload {
        installation_incarnation_id: input.installation_incarnation_id,
        collection_generation_id: input.collection_generation_id,
        projection_membership_id: input.projection_membership_id,
        access_partition_id: input.access_partition_id,
        scoring_partition_id: input.scoring_partition_id,
        source_id: input.source_id,
        source_revision_id: input.source_revision_id,
        representation_id: input.representation_id,
        unit_id: input.unit_id,
        point_identity_digest_256: Blake3Digest32::from_bytes(
            *identity.full_digest.as_bytes(),
        ),
        scoring_document_id: input.scoring_document_id,
        projection_profile_set_id: input.projection_profile_set_id.clone(),
        unit_kind: input.unit_kind,
        modality: input.modality,
        language_or_format: input.language_or_format.clone(),
        entity_kind: input.entity_kind,
        normalized_symbol_key: input.normalized_symbol_key.clone(),
        repository_lineage_id: input.repository_lineage_id,
        valid_from_epoch: input.valid_from_epoch,
        valid_until_epoch_exclusive: input.valid_until_epoch_exclusive,
    };
    payload.validate()?;
    Ok(payload)
}

/// Builds one exact point specification.
pub fn build_point_spec(
    input: ValidatedProjectionInput,
    point_identity_limits: PointIdentityLimits,
) -> Result<PointSpec, ProjectionError> {
    let key = ProjectionPointKey {
        schema_version: POINT_KEY_SCHEMA_VERSION,
        installation_incarnation_id: input.0.installation_incarnation_id,
        collection_generation_id: input.0.collection_generation_id,
        projection_membership_id: input.0.projection_membership_id,
        representation_id: input.0.representation_id,
        unit_id: input.0.unit_id,
        projection_profile_set_id: input.0.projection_profile_set_id.clone(),
        point_role: input.0.point_role,
    };
    let identity = derive_point_identity(key, point_identity_limits)?;
    let payload = build_minimal_payload(&input, &identity)?;
    let payload_digest = payload.canonical_digest();
    let source_membership_id = input.0.source_membership_id;
    let source_byte_start = input.0.source_byte_start;
    let source_byte_end = input.0.source_byte_end;
    let residency_digest = input.0.residency_digest;
    let unit_digest = input.0.unit_digest;
    let reference_digest = input.0.reference_digest;
    let vectors = input
        .0
        .vectors
        .into_iter()
        .map(|vector| (vector.name.clone(), vector))
        .collect::<BTreeMap<_, _>>();
    let vector_digests = vectors
        .iter()
        .map(|(name, vector)| (name.clone(), vector.digest))
        .collect();
    let expected_readback = ExpectedReadbackShape {
        identity_key: identity.key.clone(),
        point_identity_digest_256: payload.point_identity_digest_256,
        payload_digest,
        vector_digests,
        unit_digest,
        reference_digest,
    };
    Ok(PointSpec {
        point_id: identity.point_id,
        identity,
        payload,
        vectors,
        expected_readback,
        source_membership_id,
        source_byte_start,
        source_byte_end,
        residency_digest,
    })
}

/// Creates a deterministic exact plan and manifest.
pub fn plan_projection(
    inputs: Vec<ProjectionInput>,
    profiles: &ProjectionProfiles,
    budget: ProjectionBudget,
    point_identity_limits: PointIdentityLimits,
) -> Result<ProjectionPlan, ProjectionError> {
    let budget = budget.validate()?;
    if inputs.is_empty() || inputs.len() > budget.max_points {
        return Err(ProjectionError::BudgetExceeded);
    }
    let scope = scope_from_input(&inputs[0], profiles);
    let mut points = Vec::with_capacity(inputs.len());
    let mut point_ids = BTreeSet::new();
    let mut unit_roles = BTreeSet::new();
    for input in inputs {
        if !input_matches_scope(&input, &scope, profiles) {
            return Err(ProjectionError::ScopeMismatch);
        }
        if !unit_roles.insert((input.unit_id, input.point_role)) {
            return Err(ProjectionError::DuplicateUnitRole);
        }
        let validated = validate_projection_input(input, profiles, budget)?;
        let point = build_point_spec(validated, point_identity_limits)?;
        if !point_ids.insert(point.point_id) {
            return Err(ProjectionError::DuplicatePointId);
        }
        points.push(point);
    }
    points.sort_by_key(|point| point.point_id);
    let manifest = canonicalize_manifest(&points, budget)?;
    Ok(ProjectionPlan { points, manifest })
}

/// Requires exactly one typed projection membership.
pub fn require_single_membership(
    memberships: &[ProjectionMembershipId],
) -> Result<ProjectionMembershipId, ProjectionError> {
    match memberships {
        [membership] => Ok(*membership),
        _ => Err(ProjectionError::MembershipArrayForbidden),
    }
}

/// Rejects raw vendor collection identifiers at adapter boundaries.
pub fn reject_raw_collection_id(value: &str) -> Result<(), ProjectionError> {
    if value.is_empty()
        || value.contains(['/', '\\'])
        || value.contains(char::is_whitespace)
        || value.contains("..")
        || value.starts_with("qdrant:")
        || value.starts_with("qdrant/")
        || value.starts_with("collections:")
        || value.starts_with("collections/")
        || value.starts_with("collection:")
        || value.starts_with("collection/")
    {
        Err(ProjectionError::RawCollectionIdForbidden)
    } else {
        Ok(())
    }
}

/// Validates one typed source/projection membership binding.
pub const fn validate_membership_binding(
    _source_membership_id: SourceMembershipId,
    _projection_membership_id: ProjectionMembershipId,
) -> Result<(), ProjectionError> {
    Ok(())
}

/// Returns the exact payload fields the bridge must index.
#[must_use]
pub const fn expected_payload_indexes() -> [&'static str; 19] {
    REQUIRED_PAYLOAD_INDEX_FIELDS
}

/// Proves a manifest reconstructs exactly its point specs.
pub fn verify_manifest_reconstruction(
    manifest: &ProjectionManifest,
    points: &[PointSpec],
) -> Result<(), ProjectionError> {
    validate_manifest_entries(&manifest.entries)?;
    if manifest.entries.len() != points.len() {
        return Err(ProjectionError::InvalidManifest);
    }
    for (entry, point) in manifest.entries.iter().zip(points) {
        if entry != &manifest_entry(point) {
            return Err(ProjectionError::InvalidManifest);
        }
    }
    Ok(())
}

/// Plans one complete membership-scoped point set.
#[allow(clippy::too_many_arguments)]
pub fn plan_scoped_projection(
    inputs: Vec<ProjectionInput>,
    expected_units: &[ExpectedUnit],
    scope: &ScopeExpectation,
    profiles: &ProjectionProfiles,
    budget: ProjectionBudget,
    point_identity_limits: PointIdentityLimits,
) -> Result<ProjectionPlan, ProjectionError> {
    let budget = budget.validate()?;
    if inputs.is_empty()
        || inputs.len() > budget.max_points
        || expected_units.is_empty()
        || expected_units.len() > budget.max_points
        || scope.profile_set_digest != profiles.profile_set_digest
        || scope.projection_profile_set_id != profiles.profile_set_id
    {
        return Err(ProjectionError::ScopeMismatch);
    }
    for input in &inputs {
        if !input_matches_scope(input, scope, profiles) {
            return Err(ProjectionError::ScopeMismatch);
        }
        if input.residency_digest != scope.residency_digest {
            return Err(ProjectionError::ResidencyMismatch);
        }
    }
    let expected = expected_units
        .iter()
        .map(|unit| ((unit.unit_id, unit.unit_ordinal), unit.unit_digest))
        .collect::<BTreeMap<_, _>>();
    if expected.len() != expected_units.len() {
        return Err(ProjectionError::DuplicateUnitRole);
    }
    for input in &inputs {
        match expected.get(&(input.unit_id, input.unit_ordinal)) {
            Some(digest) if *digest == input.unit_digest => {}
            Some(_) => return Err(ProjectionError::ScopeMismatch),
            None => return Err(ProjectionError::UnexpectedUnit),
        }
    }
    for unit in expected_units {
        if !inputs.iter().any(|input| {
            input.unit_id == unit.unit_id
                && input.unit_ordinal == unit.unit_ordinal
                && input.unit_digest == unit.unit_digest
        }) {
            return Err(ProjectionError::MissingUnitReceipt);
        }
    }
    plan_projection(inputs, profiles, budget, point_identity_limits)
}

/// Produces a CAS-ready exact manifest from point specs.
pub fn canonicalize_manifest(
    points: &[PointSpec],
    budget: ProjectionBudget,
) -> Result<ProjectionManifest, ProjectionError> {
    let budget = budget.validate()?;
    if points.len() > budget.max_points {
        return Err(ProjectionError::BudgetExceeded);
    }
    let mut entries = points.iter().map(manifest_entry).collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.point_id);
    validate_manifest_entries(&entries)?;
    let canonical_bytes = encode_manifest(&entries, budget)?;
    Ok(ProjectionManifest { entries, canonical_bytes })
}

/// Returns exact create, retain and retire sets.
pub fn diff_manifests(
    old: &ProjectionManifest,
    new: &ProjectionManifest,
) -> Result<ManifestDiff, ProjectionError> {
    validate_manifest_entries(&old.entries)?;
    validate_manifest_entries(&new.entries)?;
    let old_by_id = old
        .entries
        .iter()
        .map(|entry| (entry.point_id, entry))
        .collect::<BTreeMap<_, _>>();
    let new_by_id = new
        .entries
        .iter()
        .map(|entry| (entry.point_id, entry))
        .collect::<BTreeMap<_, _>>();
    let mut create = Vec::new();
    let mut retain = Vec::new();
    let mut retire = Vec::new();
    for (point_id, new_entry) in &new_by_id {
        match old_by_id.get(point_id) {
            Some(old_entry) if *old_entry == *new_entry => {
                retain.push((*new_entry).clone());
            }
            Some(old_entry) => {
                retire.push((*old_entry).clone());
                create.push((*new_entry).clone());
            }
            None => create.push((*new_entry).clone()),
        }
    }
    for (point_id, old_entry) in old_by_id {
        if !new_by_id.contains_key(&point_id) {
            retire.push(old_entry.clone());
        }
    }
    Ok(ManifestDiff { create, retain, retire })
}

/// Proves named-vector and exact payload-index completeness.
pub fn validate_schema_requirements(
    manifest: &ProjectionManifest,
    schema: &CollectionSchema,
) -> Result<(), ProjectionError> {
    validate_manifest_entries(&manifest.entries)?;
    let required = REQUIRED_PAYLOAD_INDEX_FIELDS
        .into_iter()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    if !required.is_subset(&schema.indexed_payload_fields) {
        return Err(ProjectionError::PayloadIndexMissing);
    }
    if schema.indexed_payload_fields != required {
        return Err(ProjectionError::PayloadIndexUnexpected);
    }
    for entry in &manifest.entries {
        for name in entry.vector_digests.keys() {
            if !schema.named_vectors.contains_key(name) {
                return Err(ProjectionError::CollectionVectorMissing);
            }
        }
    }
    Ok(())
}

/// Proves schema dimensions against the accepted profile set.
pub fn validate_schema_dimensions(
    profiles: &ProjectionProfiles,
    schema: &CollectionSchema,
) -> Result<(), ProjectionError> {
    if schema.named_vectors.len() != profiles.vectors.len() {
        return Err(ProjectionError::CollectionVectorMismatch);
    }
    for (name, requirement) in &profiles.vectors {
        match schema.named_vectors.get(name) {
            Some(dimensions) if *dimensions == requirement.dimensions => {}
            Some(_) => return Err(ProjectionError::CollectionVectorMismatch),
            None => return Err(ProjectionError::CollectionVectorMissing),
        }
    }
    Ok(())
}

fn scope_from_input(
    input: &ProjectionInput,
    profiles: &ProjectionProfiles,
) -> ScopeExpectation {
    ScopeExpectation {
        installation_incarnation_id: input.installation_incarnation_id,
        collection_generation_id: input.collection_generation_id,
        source_membership_id: input.source_membership_id,
        projection_membership_id: input.projection_membership_id,
        access_partition_id: input.access_partition_id,
        scoring_partition_id: input.scoring_partition_id,
        source_id: input.source_id,
        source_revision_id: input.source_revision_id,
        representation_id: input.representation_id,
        projection_profile_set_id: input.projection_profile_set_id.clone(),
        profile_set_digest: profiles.profile_set_digest,
        residency_digest: input.residency_digest,
    }
}

fn input_matches_scope(
    input: &ProjectionInput,
    scope: &ScopeExpectation,
    profiles: &ProjectionProfiles,
) -> bool {
    input.installation_incarnation_id == scope.installation_incarnation_id
        && input.collection_generation_id == scope.collection_generation_id
        && input.source_membership_id == scope.source_membership_id
        && input.projection_membership_id == scope.projection_membership_id
        && input.access_partition_id == scope.access_partition_id
        && input.scoring_partition_id == scope.scoring_partition_id
        && input.source_id == scope.source_id
        && input.source_revision_id == scope.source_revision_id
        && input.representation_id == scope.representation_id
        && input.projection_profile_set_id == scope.projection_profile_set_id
        && input.projection_profile_set_id == profiles.profile_set_id
}

fn manifest_entry(point: &PointSpec) -> ProjectionManifestEntry {
    ProjectionManifestEntry {
        point_id: point.point_id,
        identity_key: point.identity.key.clone(),
        point_identity_digest_256: point.payload.point_identity_digest_256,
        source_membership_id: point.source_membership_id,
        projection_membership_id: point.payload.projection_membership_id,
        source_revision_id: point.payload.source_revision_id,
        representation_id: point.payload.representation_id,
        unit_id: point.payload.unit_id,
        source_byte_start: point.source_byte_start,
        source_byte_end: point.source_byte_end,
        unit_digest: point.expected_readback.unit_digest,
        reference_digest: point.expected_readback.reference_digest,
        residency_digest: point.residency_digest,
        payload_digest: point.expected_readback.payload_digest,
        vector_digests: point.expected_readback.vector_digests.clone(),
    }
}

fn encode_manifest(
    entries: &[ProjectionManifestEntry],
    budget: ProjectionBudget,
) -> Result<Vec<u8>, ProjectionError> {
    let mut output = Vec::new();
    append_bytes(&mut output, MANIFEST_DOMAIN, budget)?;
    append_u64(
        &mut output,
        u64::try_from(entries.len()).map_err(|_| ProjectionError::ManifestTooLarge)?,
        budget,
    )?;
    for entry in entries {
        append_bytes(&mut output, entry.point_id.as_bytes(), budget)?;
        let key = canonical_point_key_bytes(
            &entry.identity_key,
            PointIdentityLimits {
                max_identifier_bytes: 4_096,
                max_canonical_bytes: 32_768,
                max_registered_points: budget.max_points,
            },
        )?;
        append_bytes(&mut output, key.as_slice(), budget)?;
        append_bytes(&mut output, entry.point_identity_digest_256.as_bytes(), budget)?;
        append_bytes(&mut output, entry.source_membership_id.as_bytes(), budget)?;
        append_bytes(&mut output, entry.projection_membership_id.as_bytes(), budget)?;
        append_bytes(&mut output, entry.source_revision_id.as_bytes(), budget)?;
        append_bytes(&mut output, entry.representation_id.as_bytes(), budget)?;
        append_bytes(&mut output, entry.unit_id.as_bytes(), budget)?;
        append_u64(&mut output, entry.source_byte_start, budget)?;
        append_u64(&mut output, entry.source_byte_end, budget)?;
        append_bytes(&mut output, entry.unit_digest.as_bytes(), budget)?;
        append_bytes(&mut output, entry.reference_digest.as_bytes(), budget)?;
        append_bytes(&mut output, entry.residency_digest.as_bytes(), budget)?;
        append_bytes(&mut output, entry.payload_digest.as_bytes(), budget)?;
        append_u64(
            &mut output,
            u64::try_from(entry.vector_digests.len())
                .map_err(|_| ProjectionError::ManifestTooLarge)?,
            budget,
        )?;
        for (name, digest) in &entry.vector_digests {
            append_text(&mut output, name, budget)?;
            append_bytes(&mut output, digest.as_bytes(), budget)?;
        }
    }
    Ok(output)
}

fn validate_manifest_entries(entries: &[ProjectionManifestEntry]) -> Result<(), ProjectionError> {
    if entries
        .windows(2)
        .any(|pair| pair[0].point_id >= pair[1].point_id)
        || entries.iter().any(|entry| {
            entry.source_byte_start >= entry.source_byte_end
                || entry.identity_key.projection_membership_id
                    != entry.projection_membership_id
                || entry.identity_key.representation_id != entry.representation_id
                || entry.identity_key.unit_id != entry.unit_id
        })
    {
        return Err(ProjectionError::InvalidManifest);
    }
    Ok(())
}

fn hash_text(hasher: &mut blake3::Hasher, value: &str) {
    hash_bytes(hasher, value.as_bytes());
}

fn hash_bytes(hasher: &mut blake3::Hasher, value: &[u8]) {
    hasher.update(&u64::try_from(value.len()).unwrap_or(u64::MAX).to_be_bytes());
    hasher.update(value);
}

fn hash_optional_text(hasher: &mut blake3::Hasher, value: Option<&str>) {
    match value {
        Some(value) => {
            hasher.update(&[1]);
            hash_text(hasher, value);
        }
        None => {
            hasher.update(&[0]);
        }
    }
}

fn hash_optional_bytes(hasher: &mut blake3::Hasher, value: Option<&[u8; 16]>) {
    match value {
        Some(value) => {
            hasher.update(&[1]);
            hash_bytes(hasher, value);
        }
        None => {
            hasher.update(&[0]);
        }
    }
}

fn append_text(
    output: &mut Vec<u8>,
    value: &str,
    budget: ProjectionBudget,
) -> Result<(), ProjectionError> {
    append_bytes(output, value.as_bytes(), budget)
}

fn append_bytes(
    output: &mut Vec<u8>,
    value: &[u8],
    budget: ProjectionBudget,
) -> Result<(), ProjectionError> {
    append_u64(
        output,
        u64::try_from(value.len()).map_err(|_| ProjectionError::ManifestTooLarge)?,
        budget,
    )?;
    extend_checked(output, value, budget)
}

fn append_u64(
    output: &mut Vec<u8>,
    value: u64,
    budget: ProjectionBudget,
) -> Result<(), ProjectionError> {
    extend_checked(output, &value.to_be_bytes(), budget)
}

fn extend_checked(
    output: &mut Vec<u8>,
    value: &[u8],
    budget: ProjectionBudget,
) -> Result<(), ProjectionError> {
    let length = output
        .len()
        .checked_add(value.len())
        .ok_or(ProjectionError::ManifestTooLarge)?;
    if length > budget.max_manifest_bytes {
        return Err(ProjectionError::ManifestTooLarge);
    }
    output.extend_from_slice(value);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use search_point_identity::DEFAULT_POINT_IDENTITY_LIMITS;

    fn digest(byte: u8) -> Blake3Digest32 {
        Blake3Digest32::from_bytes([byte; 32])
    }

    fn vector() -> NamedVector {
        let mut vector = NamedVector {
            name: "lex_code_v1".to_owned(),
            dimensions: 8,
            value: VectorValue::Sparse {
                indices: vec![1, 3],
                values: vec![1.0, 2.0],
            },
            digest: digest(0),
        };
        vector.digest = vector.canonical_digest();
        vector
    }

    fn profiles() -> ProjectionProfiles {
        ProjectionProfiles {
            profile_set_id: ProjectionProfileSetId::new("profile-set-v1")
                .expect("profile"),
            profile_set_digest: digest(20),
            vectors: BTreeMap::from([(
                "lex_code_v1".to_owned(),
                VectorRequirement {
                    dimensions: 8,
                    sparse: true,
                },
            )]),
        }
    }

    fn input(unit: u8) -> ProjectionInput {
        ProjectionInput {
            installation_incarnation_id:
                InstallationIncarnationId::from_bytes([1; 16]),
            collection_generation_id:
                CollectionGenerationId::from_bytes([2; 16]),
            source_membership_id: SourceMembershipId::from_bytes([3; 16]),
            projection_membership_id:
                ProjectionMembershipId::from_bytes([4; 16]),
            access_partition_id: AccessPartitionId::from_bytes([5; 16]),
            scoring_partition_id: ScoringPartitionId::from_bytes([6; 16]),
            source_id: SourceId::from_bytes([7; 16]),
            source_revision_id: SourceRevisionId::from_bytes([8; 16]),
            representation_id: RepresentationId::from_bytes([9; 16]),
            unit_id: UnitId::from_bytes([unit; 16]),
            scoring_document_id: ScoringDocumentId::from_bytes([unit + 1; 16]),
            projection_profile_set_id: ProjectionProfileSetId::new(
                "profile-set-v1",
            )
            .expect("profile"),
            point_role: PointRole::Unit,
            unit_kind: UnitKind::File,
            modality: Modality::Code,
            language_or_format: ProfileId::new("rust-v1").expect("language"),
            entity_kind: None,
            normalized_symbol_key: None,
            repository_lineage_id: None,
            valid_from_epoch: Epoch::new(1).expect("epoch"),
            valid_until_epoch_exclusive: None,
            unit_ordinal: u64::from(unit),
            source_byte_start: u64::from(unit) * 10,
            source_byte_end: u64::from(unit) * 10 + 10,
            unit_digest: digest(unit),
            reference_digest: digest(unit + 1),
            residency_digest: digest(30),
            vectors: vec![vector()],
        }
    }

    #[test]
    fn exact_s95_payload_and_manifest_are_deterministic() {
        let plan = plan_projection(
            vec![input(1), input(2)],
            &profiles(),
            ProjectionBudget::BASELINE,
            DEFAULT_POINT_IDENTITY_LIMITS,
        )
        .expect("plan");
        verify_manifest_reconstruction(&plan.manifest, &plan.points)
            .expect("reconstruction");
        assert_eq!(expected_payload_indexes().len(), 19);
        for point in &plan.points {
            assert_eq!(
                point.payload.point_identity_digest_256,
                Blake3Digest32::from_bytes(*point.identity.full_digest.as_bytes())
            );
            assert_eq!(
                point.expected_readback.payload_digest,
                point.payload.canonical_digest()
            );
        }
    }

    #[test]
    fn membership_isolation_changes_projection_identity() {
        let first = plan_projection(
            vec![input(1)],
            &profiles(),
            ProjectionBudget::BASELINE,
            DEFAULT_POINT_IDENTITY_LIMITS,
        )
        .expect("first");
        let mut other = input(1);
        other.source_membership_id = SourceMembershipId::from_bytes([99; 16]);
        other.projection_membership_id =
            ProjectionMembershipId::from_bytes([98; 16]);
        let second = plan_projection(
            vec![other],
            &profiles(),
            ProjectionBudget::BASELINE,
            DEFAULT_POINT_IDENTITY_LIMITS,
        )
        .expect("second");
        assert_ne!(first.points[0].point_id, second.points[0].point_id);
        assert_ne!(
            first.manifest.entries[0].source_membership_id,
            second.manifest.entries[0].source_membership_id
        );
    }

    #[test]
    fn vector_digest_is_computed_not_trusted() {
        let mut bad = input(1);
        bad.vectors[0].digest = digest(77);
        assert_eq!(
            plan_projection(
                vec![bad],
                &profiles(),
                ProjectionBudget::BASELINE,
                DEFAULT_POINT_IDENTITY_LIMITS,
            ),
            Err(ProjectionError::VectorDigestMismatch)
        );
    }

    #[test]
    fn manifest_diff_uses_exact_ids_only() {
        let old = plan_projection(
            vec![input(1)],
            &profiles(),
            ProjectionBudget::BASELINE,
            DEFAULT_POINT_IDENTITY_LIMITS,
        )
        .expect("old");
        let new = plan_projection(
            vec![input(1), input(2)],
            &profiles(),
            ProjectionBudget::BASELINE,
            DEFAULT_POINT_IDENTITY_LIMITS,
        )
        .expect("new");
        let diff = diff_manifests(&old.manifest, &new.manifest).expect("diff");
        assert_eq!(diff.retain.len(), 1);
        assert_eq!(diff.create.len(), 1);
        assert!(diff.retire.is_empty());
    }
}

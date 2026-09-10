//! Pure deterministic planning for Qdrant point projections.
//!
//! This package performs no database, filesystem, network, admission, or access
//! decision. It converts immutable admitted units into exact point specs and
//! CAS-ready manifests using identities owned by `search-point-identity`.

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

use search_contracts::{Blake3Digest32, Epoch, NonZeroRevision, OpaqueId};
use search_point_identity::{
    PointId128, PointIdentity, PointIdentityError, PointIdentityKey, PointIdentityLimits,
    ProjectionKind, derive_point_identity,
};

/// Required indexed payload field names.
pub const REQUIRED_FILTER_FIELDS: [&str; 6] = [
    "source_membership_id",
    "projection_membership_id",
    "source_revision",
    "unit_ordinal",
    "visible_epoch",
    "access_partition_digest",
];

/// Closed projection-planning failure.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ProjectionError {
    /// A finite planning limit is zero or internally inconsistent.
    InvalidLimits,
    /// Point identity input is invalid or collided.
    PointIdentity,
    /// Membership or immutable source identity is empty or mismatched.
    MembershipMismatch,
    /// More than one membership was supplied where exactly one is required.
    MembershipArrayForbidden,
    /// A raw public/vendor collection identifier reached a scope boundary.
    RawCollectionIdForbidden,
    /// Inputs in one exact plan span inequivalent scoring/security/generation
    /// domains and must never share a manifest.
    ScopeMismatch,
    /// Unit residency does not match the admitted residency binding.
    ResidencyMismatch,
    /// An expected unit has no admitted receipt in the exact plan.
    MissingUnitReceipt,
    /// An input unit is outside the declared complete unit set.
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
    /// A plan exceeds its point, vector, or byte budget.
    BudgetExceeded,
    /// Two point specs resolve to the same compact identity.
    DuplicatePointId,
    /// One unit/projection role appears more than once.
    DuplicateUnitRole,
    /// Manifest input is not canonically ordered or contains duplicates.
    InvalidManifest,
    /// Canonical manifest encoding overflowed its finite ceiling.
    ManifestTooLarge,
    /// Required named vectors are absent from the collection schema.
    CollectionVectorMissing,
    /// A collection named-vector dimension is incompatible.
    CollectionVectorMismatch,
    /// A required payload field lacks an index.
    PayloadIndexMissing,
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
            Self::BudgetExceeded => "PROJECTION_BUDGET_EXCEEDED",
            Self::DuplicatePointId => "PROJECTION_DUPLICATE_POINT_ID",
            Self::DuplicateUnitRole => "PROJECTION_DUPLICATE_UNIT_ROLE",
            Self::InvalidManifest => "PROJECTION_INVALID_MANIFEST",
            Self::ManifestTooLarge => "PROJECTION_MANIFEST_TOO_LARGE",
            Self::CollectionVectorMissing => "PROJECTION_COLLECTION_VECTOR_MISSING",
            Self::CollectionVectorMismatch => "PROJECTION_COLLECTION_VECTOR_MISMATCH",
            Self::PayloadIndexMissing => "PROJECTION_PAYLOAD_INDEX_MISSING",
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

    /// Validates finite values and sparse ordering against the declared width.
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
                    || indices
                        .last()
                        .is_some_and(|index| *index >= declared_dimensions)
                {
                    return Err(ProjectionError::InvalidVector);
                }
            }
        }
        Ok(())
    }
}

/// One exact named vector and its immutable digest.
#[derive(Clone, Debug, PartialEq)]
pub struct NamedVector {
    /// Stable profile-owned vector name.
    pub name: String,
    /// Declared vector dimensions.
    pub dimensions: u32,
    /// Exact finite vector values.
    pub value: VectorValue,
    /// Digest of the canonical vector encoding.
    pub digest: Blake3Digest32,
}

/// Required vector shape for one accepted projection profile.
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
    /// Stable profile-set identifier.
    pub profile_set_id: OpaqueId,
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

/// Immutable admitted unit prepared for one projection role.
///
/// The T26 scope tail (`representation_digest` through `residency_digest`)
/// binds the exact preparation/analyzer generation, the IDF/security domain
/// and the admitted residency. Reuse across inequivalent domains is rejected;
/// a profile change mints a new collection generation instead of reusing
/// point identities.
#[derive(Clone, Debug, PartialEq)]
pub struct ProjectionInput {
    /// Stable source namespace identity.
    pub namespace_id: OpaqueId,
    /// Stable source identity.
    pub source_id: OpaqueId,
    /// Exact source membership; one point belongs to exactly one membership.
    pub source_membership_id: OpaqueId,
    /// Exact projection membership.
    pub projection_membership_id: OpaqueId,
    /// Retained immutable source revision.
    pub source_revision: NonZeroRevision,
    /// Deterministic unit ordinal.
    pub unit_ordinal: u64,
    /// Inclusive source byte start.
    pub source_byte_start: u64,
    /// Exclusive source byte end.
    pub source_byte_end: u64,
    /// Logical projection family.
    pub projection_kind: ProjectionKind,
    /// Complete projection/analyzer configuration fingerprint.
    pub projection_fingerprint: Blake3Digest32,
    /// Monotone projection schema revision.
    pub projection_schema_revision: NonZeroRevision,
    /// Visible target epoch.
    pub visible_epoch: Epoch,
    /// Exact access partition digest.
    pub access_partition_digest: Blake3Digest32,
    /// T16 canonical representation digest bound to source bytes and profiles.
    pub representation_digest: Blake3Digest32,
    /// Scoring-partition digest (IDF/security domain).
    pub scoring_partition_digest: Blake3Digest32,
    /// Opaque collection-generation digest.
    pub collection_generation_digest: Blake3Digest32,
    /// Admitted residency binding digest.
    pub residency_digest: Blake3Digest32,
    /// Exact unit bytes digest.
    pub unit_digest: Blake3Digest32,
    /// Exact native-reference digest.
    pub reference_digest: Blake3Digest32,
    /// Digest of the minimal point payload.
    pub payload_digest: Blake3Digest32,
    /// Complete named-vector set.
    pub vectors: Vec<NamedVector>,
}

/// Projection input whose profile, vector set, and finite bounds were accepted.
#[derive(Clone, Debug, PartialEq)]
pub struct ValidatedProjectionInput(ProjectionInput);

impl ValidatedProjectionInput {
    /// Borrow the exact validated input.
    #[must_use]
    pub const fn as_input(&self) -> &ProjectionInput {
        &self.0
    }

    /// Consume the wrapper.
    #[must_use]
    pub fn into_input(self) -> ProjectionInput {
        self.0
    }
}

/// Minimal opaque point payload. No source text, path, ACL subject, display
/// name, repository name, or vendor metadata is representable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MinimalPointPayload {
    /// Exact source membership.
    pub source_membership_id: OpaqueId,
    /// Exact projection membership.
    pub projection_membership_id: OpaqueId,
    /// Retained source revision.
    pub source_revision: NonZeroRevision,
    /// Deterministic unit ordinal.
    pub unit_ordinal: u64,
    /// Visible epoch.
    pub visible_epoch: Epoch,
    /// Access partition digest used for pre-scoring filtering.
    pub access_partition_digest: Blake3Digest32,
    /// Digest of exact minimal payload bytes.
    pub payload_digest: Blake3Digest32,
}

/// Expected exact readback shape for publication verification.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpectedReadbackShape {
    /// Complete point identity key.
    pub identity_key: PointIdentityKey,
    /// Exact minimal payload digest.
    pub payload_digest: Blake3Digest32,
    /// Exact named vector digests.
    pub vector_digests: BTreeMap<String, Blake3Digest32>,
    /// Exact unit digest.
    pub unit_digest: Blake3Digest32,
    /// Exact native-reference digest.
    pub reference_digest: Blake3Digest32,
}

/// Complete exact point specification ready for a Qdrant bridge.
#[derive(Clone, Debug, PartialEq)]
pub struct PointSpec {
    /// Compact provider-neutral point identifier.
    pub point_id: PointId128,
    /// Complete immutable logical identity.
    pub identity: PointIdentity,
    /// Minimal filterable payload.
    pub payload: MinimalPointPayload,
    /// Complete exact named vectors.
    pub vectors: BTreeMap<String, NamedVector>,
    /// Expected exact readback shape.
    pub expected_readback: ExpectedReadbackShape,
}

/// Exact point manifest entry without raw vector values.
///
/// The scope tail mirrors the private point identity so manifest
/// reconstruction can prove representation/scoring/generation binding without
/// reopening another package's internals.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectionManifestEntry {
    /// Compact provider-neutral point identifier.
    pub point_id: PointId128,
    /// Complete immutable identity key.
    pub identity_key: PointIdentityKey,
    /// Exact source membership.
    pub source_membership_id: OpaqueId,
    /// Exact projection membership.
    pub projection_membership_id: OpaqueId,
    /// Exact unit digest.
    pub unit_digest: Blake3Digest32,
    /// Exact reference digest.
    pub reference_digest: Blake3Digest32,
    /// T16 canonical representation digest.
    pub representation_digest: Blake3Digest32,
    /// Scoring-partition digest (IDF/security domain).
    pub scoring_partition_digest: Blake3Digest32,
    /// Opaque collection-generation digest.
    pub collection_generation_digest: Blake3Digest32,
    /// Exact payload digest.
    pub payload_digest: Blake3Digest32,
    /// Exact named-vector digests.
    pub vector_digests: BTreeMap<String, Blake3Digest32>,
}

/// One expected unit receipt in a complete scoped point set.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExpectedUnit {
    /// Deterministic unit ordinal within the retained revision.
    pub unit_ordinal: u64,
    /// Exact unit bytes digest the admitted receipt must carry.
    pub unit_digest: Blake3Digest32,
}

/// The exact admitted scope one plan must satisfy.
///
/// Every input in a plan must match each field; inequivalent
/// scoring/security/generation domains never share a manifest, and a profile
/// change mints a new collection generation instead of reusing identities.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScopeExpectation {
    /// Stable source namespace identity.
    pub namespace_id: OpaqueId,
    /// Stable source identity.
    pub source_id: OpaqueId,
    /// Retained immutable source revision.
    pub source_revision: NonZeroRevision,
    /// Exact source membership.
    pub source_membership_id: OpaqueId,
    /// Exact projection membership.
    pub projection_membership_id: OpaqueId,
    /// Complete projection/analyzer configuration fingerprint.
    pub projection_fingerprint: Blake3Digest32,
    /// Monotone projection schema revision.
    pub projection_schema_revision: NonZeroRevision,
    /// T16 canonical representation digest.
    pub representation_digest: Blake3Digest32,
    /// Scoring-partition digest (IDF/security domain).
    pub scoring_partition_digest: Blake3Digest32,
    /// Opaque collection-generation digest.
    pub collection_generation_digest: Blake3Digest32,
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
    /// Payload fields with exact-match/range indexes.
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
    validate_membership_binding(&input.source_membership_id, &input.projection_membership_id)?;
    if input.source_byte_start >= input.source_byte_end {
        return Err(ProjectionError::InvalidUnitRange);
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

/// Builds the only payload shape allowed for ordinary point publication.
#[must_use]
pub fn build_minimal_payload(input: &ValidatedProjectionInput) -> MinimalPointPayload {
    let input = input.as_input();
    MinimalPointPayload {
        source_membership_id: input.source_membership_id.clone(),
        projection_membership_id: input.projection_membership_id.clone(),
        source_revision: input.source_revision,
        unit_ordinal: input.unit_ordinal,
        visible_epoch: input.visible_epoch,
        access_partition_digest: input.access_partition_digest,
        payload_digest: input.payload_digest,
    }
}

/// Builds one exact point specification.
pub fn build_point_spec(
    input: ValidatedProjectionInput,
    point_identity_limits: PointIdentityLimits,
) -> Result<PointSpec, ProjectionError> {
    let input = input.into_input();
    let key = PointIdentityKey {
        namespace_id: input.namespace_id,
        source_id: input.source_id,
        source_revision: input.source_revision,
        unit_ordinal: input.unit_ordinal,
        source_byte_start: input.source_byte_start,
        source_byte_end: input.source_byte_end,
        projection_kind: input.projection_kind,
        projection_fingerprint: input.projection_fingerprint,
        projection_schema_revision: input.projection_schema_revision,
        source_membership_id: input.source_membership_id.clone(),
        projection_membership_id: input.projection_membership_id.clone(),
        representation_digest: input.representation_digest,
        unit_digest: input.unit_digest,
        scoring_partition_digest: input.scoring_partition_digest,
        collection_generation_digest: input.collection_generation_digest,
    };
    let identity = derive_point_identity(key, point_identity_limits)?;
    let payload = MinimalPointPayload {
        source_membership_id: input.source_membership_id,
        projection_membership_id: input.projection_membership_id,
        source_revision: input.source_revision,
        unit_ordinal: input.unit_ordinal,
        visible_epoch: input.visible_epoch,
        access_partition_digest: input.access_partition_digest,
        payload_digest: input.payload_digest,
    };
    let vectors = input
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
        payload_digest: payload.payload_digest,
        vector_digests,
        unit_digest: input.unit_digest,
        reference_digest: input.reference_digest,
    };
    Ok(PointSpec {
        point_id: identity.point_id,
        identity,
        payload,
        vectors,
        expected_readback,
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
    // One exact plan covers one admitted scope: equivalent scoring/security
    // domains and one collection generation. Mixing inequivalent domains in a
    // single manifest fails closed instead of aliasing point sets.
    let scope = &inputs[0];
    for input in &inputs[1..] {
        if input.namespace_id != scope.namespace_id
            || input.source_id != scope.source_id
            || input.source_revision != scope.source_revision
            || input.source_membership_id != scope.source_membership_id
            || input.projection_membership_id != scope.projection_membership_id
            || input.projection_kind != scope.projection_kind
            || input.projection_fingerprint != scope.projection_fingerprint
            || input.projection_schema_revision != scope.projection_schema_revision
            || input.representation_digest != scope.representation_digest
            || input.scoring_partition_digest != scope.scoring_partition_digest
            || input.collection_generation_digest != scope.collection_generation_digest
            || input.residency_digest != scope.residency_digest
        {
            return Err(ProjectionError::ScopeMismatch);
        }
    }

    let mut points = Vec::with_capacity(inputs.len());
    let mut point_ids = BTreeSet::new();
    let mut unit_roles = BTreeSet::new();
    for input in inputs {
        let role = (
            input.source_membership_id.clone(),
            input.unit_ordinal,
            input.projection_kind,
            input.projection_fingerprint,
        );
        if !unit_roles.insert(role) {
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

/// Requires exactly one membership where an array could alias point sets.
///
/// Membership arrays are forbidden by invariant 4: one point has exactly one
/// `ProjectionMembership`. Callers holding a list must resolve it to a single
/// authorized binding before planning; passing zero or several fails closed.
pub fn require_single_membership(memberships: &[OpaqueId]) -> Result<OpaqueId, ProjectionError> {
    if memberships.len() != 1 {
        return Err(ProjectionError::MembershipArrayForbidden);
    }
    memberships
        .first()
        .cloned()
        .ok_or(ProjectionError::MembershipArrayForbidden)
}

/// Rejects raw public/vendor collection identifiers at scope boundaries.
///
/// Only opaque membership identifiers and digests cross public boundaries.
/// Vendor collection names, paths, whitespace-bearing display strings and
/// parent-directory escapes are never valid scope inputs. Qdrant payload
/// indexes are created by T24 from [`expected_payload_indexes`]; no raw
/// collection name enters a plan or manifest.
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
        return Err(ProjectionError::RawCollectionIdForbidden);
    }
    Ok(())
}

/// Validates one source/projection membership binding.
///
/// Both identifiers must be opaque (no raw vendor collection IDs). Empty
/// identifiers are unrepresentable through [`OpaqueId`]; a verified binding
/// still requires the T13 accepted membership receipt, which the daemon
/// composition checks before planning.
pub fn validate_membership_binding(
    source_membership_id: &OpaqueId,
    projection_membership_id: &OpaqueId,
) -> Result<(), ProjectionError> {
    reject_raw_collection_id(source_membership_id.as_str())?;
    reject_raw_collection_id(projection_membership_id.as_str())?;
    Ok(())
}

/// Returns the exact filterable payload fields T24 must index.
///
/// The manifest reconstruction proof ([`verify_manifest_reconstruction`]) and
/// the collection schema gate ([`validate_schema_requirements`]) share this
/// set, so the daemon hands T24 one authoritative list instead of a second
/// ad-hoc enumeration. No Qdrant call happens here.
#[must_use]
pub const fn expected_payload_indexes() -> [&'static str; 6] {
    REQUIRED_FILTER_FIELDS
}

/// Proves a manifest reconstructs exactly its point specs.
///
/// Checks strict point-ID ordering, duplicate freedom and full entry equality
/// (identity keys, memberships, representation/scoring/generation digests,
/// unit/reference/payload/vector digests) against freshly derived entries.
/// Any drift — reordered bytes, dropped points, swapped digests — fails
/// closed with [`ProjectionError::InvalidManifest`].
pub fn verify_manifest_reconstruction(
    manifest: &ProjectionManifest,
    points: &[PointSpec],
) -> Result<(), ProjectionError> {
    validate_manifest_entries(&manifest.entries)?;
    if manifest.entries.len() != points.len() {
        return Err(ProjectionError::InvalidManifest);
    }
    let mut derived = points
        .iter()
        .map(|point| ProjectionManifestEntry {
            point_id: point.point_id,
            identity_key: point.identity.key.clone(),
            source_membership_id: point.payload.source_membership_id.clone(),
            projection_membership_id: point.payload.projection_membership_id.clone(),
            unit_digest: point.expected_readback.unit_digest,
            reference_digest: point.expected_readback.reference_digest,
            representation_digest: point.identity.key.representation_digest,
            scoring_partition_digest: point.identity.key.scoring_partition_digest,
            collection_generation_digest: point.identity.key.collection_generation_digest,
            payload_digest: point.expected_readback.payload_digest,
            vector_digests: point.expected_readback.vector_digests.clone(),
        })
        .collect::<Vec<_>>();
    derived.sort_by_key(|entry| entry.point_id);
    if derived
        .windows(2)
        .any(|pair| pair[0].point_id == pair[1].point_id)
    {
        return Err(ProjectionError::InvalidManifest);
    }
    if manifest.entries != derived {
        return Err(ProjectionError::InvalidManifest);
    }
    Ok(())
}

/// Plans one complete membership-scoped point set.
///
/// Beyond [`plan_projection`], this proves completeness against the admitted
/// scope: every input must match the expected namespace/source/revision,
/// membership pair, projection fingerprint/schema, representation,
/// scoring-partition and generation bindings (residency drift reports
/// [`ProjectionError::ResidencyMismatch` specifically), every expected unit
/// receipt must be present exactly once
/// ([`ProjectionError::MissingUnitReceipt`]) and no undeclared unit may enter
/// ([`ProjectionError::UnexpectedUnit`]).
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
    if inputs.is_empty() || inputs.len() > budget.max_points {
        return Err(ProjectionError::BudgetExceeded);
    }
    if expected_units.is_empty() || expected_units.len() > budget.max_points {
        return Err(ProjectionError::BudgetExceeded);
    }
    for input in &inputs {
        validate_membership_binding(&input.source_membership_id, &input.projection_membership_id)?;
        if input.namespace_id != scope.namespace_id
            || input.source_id != scope.source_id
            || input.source_revision != scope.source_revision
            || input.source_membership_id != scope.source_membership_id
            || input.projection_membership_id != scope.projection_membership_id
            || input.projection_fingerprint != scope.projection_fingerprint
            || input.projection_schema_revision != scope.projection_schema_revision
            || input.representation_digest != scope.representation_digest
            || input.scoring_partition_digest != scope.scoring_partition_digest
            || input.collection_generation_digest != scope.collection_generation_digest
        {
            return Err(ProjectionError::ScopeMismatch);
        }
        if input.residency_digest != scope.residency_digest {
            return Err(ProjectionError::ResidencyMismatch);
        }
    }
    let expected_by_ordinal = expected_units
        .iter()
        .map(|unit| (unit.unit_ordinal, unit.unit_digest))
        .collect::<BTreeMap<_, _>>();
    if expected_by_ordinal.len() != expected_units.len() {
        return Err(ProjectionError::DuplicateUnitRole);
    }
    for input in &inputs {
        match expected_by_ordinal.get(&input.unit_ordinal) {
            Some(expected_digest) if *expected_digest == input.unit_digest => {}
            Some(_) => return Err(ProjectionError::ScopeMismatch),
            None => return Err(ProjectionError::UnexpectedUnit),
        }
    }
    for expected in expected_units {
        let present = inputs.iter().any(|input| {
            input.unit_ordinal == expected.unit_ordinal && input.unit_digest == expected.unit_digest
        });
        if !present {
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

    let mut entries = points
        .iter()
        .map(|point| ProjectionManifestEntry {
            point_id: point.point_id,
            identity_key: point.identity.key.clone(),
            source_membership_id: point.payload.source_membership_id.clone(),
            projection_membership_id: point.payload.projection_membership_id.clone(),
            unit_digest: point.expected_readback.unit_digest,
            reference_digest: point.expected_readback.reference_digest,
            representation_digest: point.identity.key.representation_digest,
            scoring_partition_digest: point.identity.key.scoring_partition_digest,
            collection_generation_digest: point.identity.key.collection_generation_digest,
            payload_digest: point.expected_readback.payload_digest,
            vector_digests: point.expected_readback.vector_digests.clone(),
        })
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.point_id);
    if entries
        .windows(2)
        .any(|pair| pair[0].point_id == pair[1].point_id)
    {
        return Err(ProjectionError::InvalidManifest);
    }

    let mut canonical = Vec::new();
    append_bytes(
        &mut canonical,
        b"eliot-search/projection-manifest/v1",
        budget,
    )?;
    append_u64(
        &mut canonical,
        u64::try_from(entries.len()).map_err(|_| ProjectionError::ManifestTooLarge)?,
        budget,
    )?;
    for entry in &entries {
        append_bytes(&mut canonical, entry.point_id.as_bytes(), budget)?;
        let identity = entry
            .identity_key
            .canonical_bytes(PointIdentityLimits {
                max_identifier_bytes: 4_096,
                max_canonical_bytes: 32_768,
                max_registered_points: budget.max_points,
            })
            .map_err(ProjectionError::from)?;
        append_bytes(&mut canonical, &identity, budget)?;
        append_text(&mut canonical, entry.source_membership_id.as_str(), budget)?;
        append_text(
            &mut canonical,
            entry.projection_membership_id.as_str(),
            budget,
        )?;
        append_bytes(&mut canonical, entry.unit_digest.as_bytes(), budget)?;
        append_bytes(&mut canonical, entry.reference_digest.as_bytes(), budget)?;
        append_bytes(
            &mut canonical,
            entry.representation_digest.as_bytes(),
            budget,
        )?;
        append_bytes(
            &mut canonical,
            entry.scoring_partition_digest.as_bytes(),
            budget,
        )?;
        append_bytes(
            &mut canonical,
            entry.collection_generation_digest.as_bytes(),
            budget,
        )?;
        append_bytes(&mut canonical, entry.payload_digest.as_bytes(), budget)?;
        append_u64(
            &mut canonical,
            u64::try_from(entry.vector_digests.len())
                .map_err(|_| ProjectionError::ManifestTooLarge)?,
            budget,
        )?;
        for (name, digest) in &entry.vector_digests {
            append_text(&mut canonical, name, budget)?;
            append_bytes(&mut canonical, digest.as_bytes(), budget)?;
        }
    }
    Ok(ProjectionManifest {
        entries,
        canonical_bytes: canonical,
    })
}

/// Returns exact create, retain, and retire sets.
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
            Some(old_entry) if *old_entry == *new_entry => retain.push((*new_entry).clone()),
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
    Ok(ManifestDiff {
        create,
        retain,
        retire,
    })
}

/// Proves collection named-vector and payload-index completeness.
pub fn validate_schema_requirements(
    manifest: &ProjectionManifest,
    schema: &CollectionSchema,
) -> Result<(), ProjectionError> {
    validate_manifest_entries(&manifest.entries)?;
    for field in REQUIRED_FILTER_FIELDS {
        if !schema.indexed_payload_fields.contains(field) {
            return Err(ProjectionError::PayloadIndexMissing);
        }
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
    for (name, requirement) in &profiles.vectors {
        match schema.named_vectors.get(name) {
            Some(dimensions) if *dimensions == requirement.dimensions => {}
            Some(_) => return Err(ProjectionError::CollectionVectorMismatch),
            None => return Err(ProjectionError::CollectionVectorMissing),
        }
    }
    Ok(())
}

fn validate_manifest_entries(entries: &[ProjectionManifestEntry]) -> Result<(), ProjectionError> {
    if entries
        .windows(2)
        .any(|pair| pair[0].point_id >= pair[1].point_id)
    {
        return Err(ProjectionError::InvalidManifest);
    }
    Ok(())
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
    let length = u64::try_from(value.len()).map_err(|_| ProjectionError::ManifestTooLarge)?;
    append_u64(output, length, budget)?;
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

    fn oid(value: &str) -> OpaqueId {
        OpaqueId::new(value).expect("opaque id")
    }

    fn digest(byte: u8) -> Blake3Digest32 {
        Blake3Digest32::from_bytes([byte; 32])
    }

    fn profiles() -> ProjectionProfiles {
        ProjectionProfiles {
            profile_set_id: oid("profile-set:test"),
            profile_set_digest: digest(0xA0),
            vectors: BTreeMap::from([(
                "lexical-sparse".to_owned(),
                VectorRequirement {
                    dimensions: 8,
                    sparse: true,
                },
            )]),
        }
    }

    fn input(
        source: &str,
        source_membership: &str,
        projection_membership: &str,
        unit_ordinal: u64,
        fingerprint_byte: u8,
        generation_byte: u8,
    ) -> ProjectionInput {
        ProjectionInput {
            namespace_id: oid("namespace:test"),
            source_id: oid(source),
            source_membership_id: oid(source_membership),
            projection_membership_id: oid(projection_membership),
            source_revision: NonZeroRevision::new(3).expect("revision"),
            unit_ordinal,
            source_byte_start: unit_ordinal * 100,
            source_byte_end: unit_ordinal * 100 + 50,
            projection_kind: ProjectionKind::Lexical,
            projection_fingerprint: digest(fingerprint_byte),
            projection_schema_revision: NonZeroRevision::new(2).expect("revision"),
            visible_epoch: Epoch::new(7).expect("epoch"),
            access_partition_digest: digest(0xB0),
            representation_digest: digest(0xB1),
            scoring_partition_digest: digest(0xB2),
            collection_generation_digest: digest(generation_byte),
            residency_digest: digest(0xB4),
            unit_digest: Blake3Digest32::from_bytes({
                let mut bytes = [0xC0; 32];
                bytes[0] = u8::try_from(unit_ordinal).unwrap_or(u8::MAX);
                bytes
            }),
            reference_digest: digest(0xC1),
            payload_digest: digest(0xC2),
            vectors: vec![NamedVector {
                name: "lexical-sparse".to_owned(),
                dimensions: 8,
                value: VectorValue::Sparse {
                    indices: vec![1, 4],
                    values: vec![1.0, 2.0],
                },
                digest: digest(0xD0),
            }],
        }
    }

    fn expected_scope() -> ScopeExpectation {
        ScopeExpectation {
            namespace_id: oid("namespace:test"),
            source_id: oid("source:test"),
            source_revision: NonZeroRevision::new(3).expect("revision"),
            source_membership_id: oid("membership:source:a"),
            projection_membership_id: oid("membership:projection:a"),
            projection_fingerprint: digest(0x07),
            projection_schema_revision: NonZeroRevision::new(2).expect("revision"),
            representation_digest: digest(0xB1),
            scoring_partition_digest: digest(0xB2),
            collection_generation_digest: digest(0xC0),
            residency_digest: digest(0xB4),
        }
    }

    #[test]
    fn t26_one_source_two_memberships_yield_distinct_point_sets() {
        let profiles = profiles();
        let budget = ProjectionBudget::BASELINE;
        let limits = PointIdentityLimits {
            max_registered_points: 1024,
            ..search_point_identity::DEFAULT_POINT_IDENTITY_LIMITS
        };
        let plan_a = plan_projection(
            vec![input(
                "source:test",
                "membership:source:a",
                "membership:projection:a",
                0,
                0x07,
                0xC0,
            )],
            &profiles,
            budget,
            limits,
        )
        .expect("plan a");
        let plan_b = plan_projection(
            vec![input(
                "source:test",
                "membership:source:b",
                "membership:projection:b",
                0,
                0x07,
                0xC0,
            )],
            &profiles,
            budget,
            limits,
        )
        .expect("plan b");
        assert_ne!(plan_a.points[0].point_id, plan_b.points[0].point_id);
    }

    #[test]
    fn t26_shared_scoring_leg_does_not_alias_units() {
        let profiles = profiles();
        let budget = ProjectionBudget::BASELINE;
        let limits = search_point_identity::DEFAULT_POINT_IDENTITY_LIMITS;
        // Two units under one scoring partition: two points, one shared leg.
        let plan = plan_projection(
            vec![
                input(
                    "source:test",
                    "membership:source:a",
                    "membership:projection:a",
                    0,
                    0x07,
                    0xC0,
                ),
                input(
                    "source:test",
                    "membership:source:a",
                    "membership:projection:a",
                    1,
                    0x07,
                    0xC0,
                ),
            ],
            &profiles,
            budget,
            limits,
        )
        .expect("shared-leg plan");
        assert_eq!(plan.points.len(), 2);
        assert_ne!(plan.points[0].point_id, plan.points[1].point_id);
    }

    #[test]
    fn t26_same_content_distinct_source_stays_distinct() {
        let profiles = profiles();
        let budget = ProjectionBudget::BASELINE;
        let limits = search_point_identity::DEFAULT_POINT_IDENTITY_LIMITS;
        // One source per exact plan (a projection membership binds one source
        // membership binds one source): same bytes under distinct sources are
        // distinct point sets in distinct plans.
        let left = plan_projection(
            vec![input(
                "source:left",
                "membership:source:a",
                "membership:projection:a",
                0,
                0x07,
                0xC0,
            )],
            &profiles,
            budget,
            limits,
        )
        .expect("left plan");
        let right = plan_projection(
            vec![input(
                "source:right",
                "membership:source:a",
                "membership:projection:a",
                0,
                0x07,
                0xC0,
            )],
            &profiles,
            budget,
            limits,
        )
        .expect("right plan");
        assert_ne!(left.points[0].point_id, right.points[0].point_id);
    }

    #[test]
    fn t26_profile_and_generation_change_replace_points() {
        let profiles = profiles();
        let budget = ProjectionBudget::BASELINE;
        let limits = search_point_identity::DEFAULT_POINT_IDENTITY_LIMITS;
        let baseline = plan_projection(
            vec![input(
                "source:test",
                "membership:source:a",
                "membership:projection:a",
                0,
                0x07,
                0xC0,
            )],
            &profiles,
            budget,
            limits,
        )
        .expect("baseline");
        let changed_profile = plan_projection(
            vec![input(
                "source:test",
                "membership:source:a",
                "membership:projection:a",
                0,
                0x08,
                0xC0,
            )],
            &profiles,
            budget,
            limits,
        )
        .expect("changed profile");
        assert_ne!(
            baseline.points[0].point_id,
            changed_profile.points[0].point_id
        );
        let changed_generation = plan_projection(
            vec![input(
                "source:test",
                "membership:source:a",
                "membership:projection:a",
                0,
                0x07,
                0xC1,
            )],
            &profiles,
            budget,
            limits,
        )
        .expect("changed generation");
        assert_ne!(
            baseline.points[0].point_id,
            changed_generation.points[0].point_id
        );
        let diff = diff_manifests(&baseline.manifest, &changed_generation.manifest)
            .expect("generation diff");
        assert!(diff.retain.is_empty());
        assert_eq!(diff.create.len(), 1);
        assert_eq!(diff.retire.len(), 1);
    }

    #[test]
    fn t26_same_unit_different_vectors_is_rejected_not_aliased() {
        let profiles = profiles();
        let budget = ProjectionBudget::BASELINE;
        let limits = search_point_identity::DEFAULT_POINT_IDENTITY_LIMITS;
        let first = input(
            "source:test",
            "membership:source:a",
            "membership:projection:a",
            0,
            0x07,
            0xC0,
        );
        let mut second = first.clone();
        second.vectors[0].digest = digest(0xD1);
        second.payload_digest = digest(0xC3);
        // Same unit role with divergent encodings: the plan fails closed on
        // the duplicate role instead of silently aliasing one point.
        let result = plan_projection(vec![first, second], &profiles, budget, limits);
        assert_eq!(result, Err(ProjectionError::DuplicateUnitRole));
    }

    #[test]
    fn t26_missing_unit_receipt_fails_closed() {
        let profiles = profiles();
        let budget = ProjectionBudget::BASELINE;
        let limits = search_point_identity::DEFAULT_POINT_IDENTITY_LIMITS;
        let inputs = vec![input(
            "source:test",
            "membership:source:a",
            "membership:projection:a",
            0,
            0x07,
            0xC0,
        )];
        let expected = vec![
            ExpectedUnit {
                unit_ordinal: 0,
                unit_digest: inputs[0].unit_digest,
            },
            ExpectedUnit {
                unit_ordinal: 1,
                unit_digest: digest(0xE0),
            },
        ];
        assert_eq!(
            plan_scoped_projection(
                inputs,
                &expected,
                &expected_scope(),
                &profiles,
                budget,
                limits
            ),
            Err(ProjectionError::MissingUnitReceipt)
        );
    }

    #[test]
    fn t26_unexpected_unit_fails_closed() {
        let profiles = profiles();
        let budget = ProjectionBudget::BASELINE;
        let limits = search_point_identity::DEFAULT_POINT_IDENTITY_LIMITS;
        let inputs = vec![
            input(
                "source:test",
                "membership:source:a",
                "membership:projection:a",
                0,
                0x07,
                0xC0,
            ),
            input(
                "source:test",
                "membership:source:a",
                "membership:projection:a",
                9,
                0x07,
                0xC0,
            ),
        ];
        let expected = vec![ExpectedUnit {
            unit_ordinal: 0,
            unit_digest: inputs[0].unit_digest,
        }];
        assert_eq!(
            plan_scoped_projection(
                inputs,
                &expected,
                &expected_scope(),
                &profiles,
                budget,
                limits
            ),
            Err(ProjectionError::UnexpectedUnit)
        );
    }

    #[test]
    fn t26_wrong_residency_fails_closed() {
        let profiles = profiles();
        let budget = ProjectionBudget::BASELINE;
        let limits = search_point_identity::DEFAULT_POINT_IDENTITY_LIMITS;
        let mut bad = input(
            "source:test",
            "membership:source:a",
            "membership:projection:a",
            0,
            0x07,
            0xC0,
        );
        bad.residency_digest = digest(0xFF);
        let expected = vec![ExpectedUnit {
            unit_ordinal: 0,
            unit_digest: bad.unit_digest,
        }];
        assert_eq!(
            plan_scoped_projection(
                vec![bad],
                &expected,
                &expected_scope(),
                &profiles,
                budget,
                limits
            ),
            Err(ProjectionError::ResidencyMismatch)
        );
    }

    #[test]
    fn t26_cross_domain_reuse_is_rejected() {
        let profiles = profiles();
        let budget = ProjectionBudget::BASELINE;
        let limits = search_point_identity::DEFAULT_POINT_IDENTITY_LIMITS;
        // Mixed scoring partitions inside one plan: inequivalent IDF/security
        // domains must never share a manifest.
        let mut second = input(
            "source:test",
            "membership:source:a",
            "membership:projection:a",
            1,
            0x07,
            0xC0,
        );
        second.scoring_partition_digest = digest(0xFE);
        let result = plan_projection(
            vec![
                input(
                    "source:test",
                    "membership:source:a",
                    "membership:projection:a",
                    0,
                    0x07,
                    0xC0,
                ),
                second,
            ],
            &profiles,
            budget,
            limits,
        );
        assert_eq!(result, Err(ProjectionError::ScopeMismatch));
        // Mixed collection generations likewise.
        let mut third = input(
            "source:test",
            "membership:source:a",
            "membership:projection:a",
            1,
            0x07,
            0xC0,
        );
        third.collection_generation_digest = digest(0xFD);
        let result = plan_projection(
            vec![
                input(
                    "source:test",
                    "membership:source:a",
                    "membership:projection:a",
                    0,
                    0x07,
                    0xC0,
                ),
                third,
            ],
            &profiles,
            budget,
            limits,
        );
        assert_eq!(result, Err(ProjectionError::ScopeMismatch));
    }

    #[test]
    fn t26_duplicate_point_and_duplicate_role_fail_closed() {
        let profiles = profiles();
        let budget = ProjectionBudget::BASELINE;
        let limits = search_point_identity::DEFAULT_POINT_IDENTITY_LIMITS;
        let twice = vec![
            input(
                "source:test",
                "membership:source:a",
                "membership:projection:a",
                0,
                0x07,
                0xC0,
            ),
            input(
                "source:test",
                "membership:source:a",
                "membership:projection:a",
                0,
                0x07,
                0xC0,
            ),
        ];
        assert_eq!(
            plan_projection(twice, &profiles, budget, limits),
            Err(ProjectionError::DuplicateUnitRole)
        );
    }

    #[test]
    fn t26_reordered_inputs_yield_identical_manifest_bytes() {
        let profiles = profiles();
        let budget = ProjectionBudget::BASELINE;
        let limits = search_point_identity::DEFAULT_POINT_IDENTITY_LIMITS;
        let forward = plan_projection(
            vec![
                input(
                    "source:test",
                    "membership:source:a",
                    "membership:projection:a",
                    0,
                    0x07,
                    0xC0,
                ),
                input(
                    "source:test",
                    "membership:source:a",
                    "membership:projection:a",
                    1,
                    0x07,
                    0xC0,
                ),
            ],
            &profiles,
            budget,
            limits,
        )
        .expect("forward");
        let backward = plan_projection(
            vec![
                input(
                    "source:test",
                    "membership:source:a",
                    "membership:projection:a",
                    1,
                    0x07,
                    0xC0,
                ),
                input(
                    "source:test",
                    "membership:source:a",
                    "membership:projection:a",
                    0,
                    0x07,
                    0xC0,
                ),
            ],
            &profiles,
            budget,
            limits,
        )
        .expect("backward");
        assert_eq!(forward.manifest.entries, backward.manifest.entries);
        assert_eq!(
            forward.manifest.canonical_bytes,
            backward.manifest.canonical_bytes
        );
        verify_manifest_reconstruction(&backward.manifest, &backward.points).expect("reconstruct");
    }

    #[test]
    fn t26_manifest_reconstruction_matches_points_and_payload_indexes() {
        let profiles = profiles();
        let budget = ProjectionBudget::BASELINE;
        let limits = search_point_identity::DEFAULT_POINT_IDENTITY_LIMITS;
        let plan = plan_projection(
            vec![input(
                "source:test",
                "membership:source:a",
                "membership:projection:a",
                0,
                0x07,
                0xC0,
            )],
            &profiles,
            budget,
            limits,
        )
        .expect("plan");
        verify_manifest_reconstruction(&plan.manifest, &plan.points).expect("reconstruct");
        // Payload indexes handed to T24 cover every filterable field.
        let indexes = expected_payload_indexes();
        assert_eq!(indexes.len(), REQUIRED_FILTER_FIELDS.len());
        for field in REQUIRED_FILTER_FIELDS {
            assert!(indexes.contains(&field), "missing index {field}");
        }
        let schema = CollectionSchema {
            named_vectors: BTreeMap::from([("lexical-sparse".to_owned(), 8)]),
            indexed_payload_fields: indexes.iter().map(ToString::to_string).collect(),
        };
        validate_schema_requirements(&plan.manifest, &schema).expect("schema complete");
    }

    #[test]
    fn t26_membership_arrays_are_forbidden() {
        assert_eq!(
            require_single_membership(&[]),
            Err(ProjectionError::MembershipArrayForbidden)
        );
        assert_eq!(
            require_single_membership(&[oid("membership:source:a"), oid("membership:source:b")]),
            Err(ProjectionError::MembershipArrayForbidden)
        );
        assert_eq!(
            require_single_membership(&[oid("membership:source:a")]).expect("single"),
            oid("membership:source:a")
        );
    }

    #[test]
    fn t26_raw_public_collection_ids_are_rejected() {
        for raw in [
            "qdrant:collection/main",
            "collections/main",
            "main collection",
            "qdrant/main",
            "../escape",
        ] {
            assert_eq!(
                reject_raw_collection_id(raw),
                Err(ProjectionError::RawCollectionIdForbidden),
                "raw {raw:?}"
            );
        }
        reject_raw_collection_id("membership:projection:a").expect("opaque ok");
    }

    #[test]
    fn t26_minimal_payload_discloses_no_source_text_or_acl_subjects() {
        let profiles = profiles();
        let budget = ProjectionBudget::BASELINE;
        let input = input(
            "source:test",
            "membership:source:a",
            "membership:projection:a",
            0,
            0x07,
            0xC0,
        );
        let validated = validate_projection_input(input, &profiles, budget).expect("valid");
        let payload = build_minimal_payload(&validated);
        let debug = format!("{payload:?}");
        assert!(!debug.contains("source:test"));
        assert!(debug.contains("membership:source:a"));
    }
}

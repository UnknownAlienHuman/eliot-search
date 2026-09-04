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
    pub fn stored_values(&self) -> usize {
        match self {
            Self::Dense(values) => values.len(),
            Self::Sparse { values, .. } => values.len(),
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
    /// Exact payload digest.
    pub payload_digest: Blake3Digest32,
    /// Exact named-vector digests.
    pub vector_digests: BTreeMap<String, Blake3Digest32>,
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
    append_bytes(&mut canonical, b"eliot-search/projection-manifest/v1", budget)?;
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
        append_text(
            &mut canonical,
            entry.source_membership_id.as_str(),
            budget,
        )?;
        append_text(
            &mut canonical,
            entry.projection_membership_id.as_str(),
            budget,
        )?;
        append_bytes(&mut canonical, entry.unit_digest.as_bytes(), budget)?;
        append_bytes(&mut canonical, entry.reference_digest.as_bytes(), budget)?;
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

fn validate_manifest_entries(
    entries: &[ProjectionManifestEntry],
) -> Result<(), ProjectionError> {
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

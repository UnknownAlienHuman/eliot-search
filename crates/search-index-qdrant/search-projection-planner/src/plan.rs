//! Pure membership-scoped planning, vector admission and point derivation.
//!
//! This module performs no filesystem, redb, Qdrant, registry read, access
//! decision or publication effect. It converts one owner-verified unit set and
//! one exact prepared vector product per unit into exact point specs, one
//! exact S9.5 payload and one canonical manifest.

use std::collections::{BTreeMap, BTreeSet};

use search_contracts::indexed::{
    MAX_STORED_VECTOR_VALUES, PointPayload, VectorRequirement, VectorValue,
};
use search_contracts::{
    BoundedBytes, BoundedList, BoundedMap, Blake3Digest32, CanonicalDigestDomain, CanonicalKey,
    CanonicalText, CanonicalValue, DigestInputLimit, Epoch, RepresentationId, ScoringDocumentId,
    SourceRevisionId, UnitId, blake3_canonical, MAX_CANONICAL_BYTES,
};
use search_point_identity::s11::{
    POINT_KEY_SCHEMA_VERSION, PointIdentityLimits, PointRole, ProjectionPointKey,
    derive_point_identity,
};
use search_unitizer::{UnitDescriptor, VerifiedUnitSet};

use super::ProjectionBudget;
use super::ProjectionManifest;
use super::manifest::manifest_canonical_bytes;
use super::validate::VerifiedUnitIndex;
use super::{
    ExpectedReadbackShape, LineageObservation, PlannedUnit, PlannedVector, PointSpec,
    ProjectionManifestEntry, ProjectionMembershipBinding, ProjectionPlan,
};
use crate::error::ProjectionError;

/// Frozen domain for the membership-independent scoring-document identity.
const SCORING_DOCUMENT_DOMAIN: &str = "eliot/cbor/projection-scoring-document/v1";

/// Frozen domain for one named-vector identity.
const NAMED_VECTOR_DOMAIN: &str = "eliot/cbor/projection-named-vector/v1";

/// Complete exact planning scope for one membership-scoped plan.
///
/// Every field is an opaque verified observation from its owner; ordinary
/// constructible data is never described as authority here.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanningScope {
    /// Exactly one admitted source-to-projection membership binding.
    pub membership: ProjectionMembershipBinding,
    /// Exact target publication epoch, at least 1.
    pub valid_from_epoch: Epoch,
    /// Optional exclusive upper epoch for points retired by this plan.
    pub valid_until_epoch_exclusive: Option<Epoch>,
    /// Optional repository lineage observation.
    pub lineage: LineageObservation,
    /// Exact immutable point identity limits.
    pub identity_limits: PointIdentityLimits,
}

/// Proves the unit set binds the exact representation the membership projects.
///
/// The complete set is package-verified and bound to
/// `Representation.unit_manifest_digest`; no caller list, count or digest can
/// substitute for that verification.
pub fn validate_scope_binding(
    unit_set: &VerifiedUnitSet,
    scope: &PlanningScope,
    budget: ProjectionBudget,
) -> Result<(), ProjectionError> {
    let budget = budget.validate()?;
    if unit_set.units().is_empty() {
        return Err(ProjectionError::EmptyUnitSet);
    }
    if unit_set.units().len() > budget.max_points {
        return Err(ProjectionError::UnitSetTooLarge);
    }
    if unit_set.representation().representation_id != scope.membership.representation_id {
        return Err(ProjectionError::RepresentationMismatch);
    }
    Ok(())
}

/// Validates one unit's exact vector set against the accepted requirements.
///
/// Every required named vector must be present exactly once with the exact
/// shape, finiteness and order the schema declares. A subset, duplicate,
/// extra or foreign vector name cannot pass.
fn validate_unit_vectors(
    vectors: &[PlannedVector],
    requirements: &BTreeMap<String, VectorRequirement>,
) -> Result<BTreeMap<String, Blake3Digest32>, ProjectionError> {
    if vectors.len() != requirements.len() {
        return Err(ProjectionError::MissingVector);
    }
    let mut digests: BTreeMap<String, Blake3Digest32> = BTreeMap::new();
    let mut total_stored = 0_usize;
    for vector in vectors {
        let name = vector.name.as_str();
        if name.is_empty() || digests.contains_key(name) {
            return Err(ProjectionError::VectorNameMismatch);
        }
        let requirement = requirements
            .get(name)
            .ok_or(ProjectionError::VectorNameMismatch)?;
        vector
            .value
            .validate(requirement)
            .map_err(|_| ProjectionError::VectorShapeMismatch)?;
        total_stored = total_stored
            .checked_add(vector.value.stored_values())
            .ok_or(ProjectionError::StoredVectorValuesExceeded)?;
        if total_stored > MAX_STORED_VECTOR_VALUES as usize {
            return Err(ProjectionError::StoredVectorValuesExceeded);
        }
        digests.insert(name.to_owned(), vector_digest(name, *requirement, &vector.value)?);
    }
    if digests.len() != requirements.len() {
        return Err(ProjectionError::MissingVector);
    }
    Ok(digests)
}

/// Computes one named-vector digest through the shared canonical owner.
fn vector_digest(
    name: &str,
    requirement: VectorRequirement,
    value: &VectorValue,
) -> Result<Blake3Digest32, ProjectionError> {
    let domain = CanonicalDigestDomain::parse(NAMED_VECTOR_DOMAIN)
        .map_err(|_| ProjectionError::InvalidLimits)?;
    let limit =
        DigestInputLimit::new(MAX_CANONICAL_BYTES).map_err(|_| ProjectionError::InvalidLimits)?;
    let mut entries: Vec<(CanonicalKey, CanonicalValue)> = Vec::new();
    entries.push((canonical_key("name")?, canonical_text(name)?));
    entries.push((
        canonical_key("requirement")?,
        requirement
            .to_canonical_value()
            .map_err(|_| ProjectionError::InvalidManifest)?,
    ));
    let kind = match value {
        VectorValue::Dense(_) => "dense",
        VectorValue::Sparse { .. } => "sparse",
    };
    entries.push((canonical_key("value_kind")?, canonical_text(kind)?));
    match value {
        VectorValue::Dense(values) => {
            let mut stored = Vec::with_capacity(values.len());
            for value in values {
                stored.push(CanonicalValue::Bytes(
                    BoundedBytes::new(value.to_bits().to_be_bytes().to_vec())
                        .map_err(|_| ProjectionError::InvalidManifest)?,
                ));
            }
            entries.push((
                canonical_key("dense_values")?,
                CanonicalValue::Array(
                    BoundedList::new(stored).map_err(|_| ProjectionError::InvalidManifest)?,
                ),
            ));
        }
        VectorValue::Sparse { indices, values } => {
            let mut stored = Vec::with_capacity(indices.len());
            for (index, value) in indices.iter().zip(values) {
                let pair = BoundedList::new(vec![
                    CanonicalValue::U64(u64::from(*index)),
                    CanonicalValue::Bytes(
                        BoundedBytes::new(value.to_bits().to_be_bytes().to_vec())
                            .map_err(|_| ProjectionError::InvalidManifest)?,
                    ),
                ])
                .map_err(|_| ProjectionError::InvalidManifest)?;
                stored.push(CanonicalValue::Array(pair));
            }
            entries.push((
                canonical_key("sparse_values")?,
                CanonicalValue::Array(
                    BoundedList::new(stored).map_err(|_| ProjectionError::InvalidManifest)?,
                ),
            ));
        }
    }
    let preimage = CanonicalValue::Object(
        BoundedMap::from_entries(entries).map_err(|_| ProjectionError::InvalidManifest)?,
    );
    blake3_canonical(&domain, &preimage, limit).map_err(|_| ProjectionError::InvalidManifest)
}

/// Derives one membership-independent scoring document identity.
///
/// Membership, access and epoch state are deliberately excluded so the same
/// scoring population is not fragmented by authorization changes.
fn derive_scoring_document_id(
    source_revision_id: SourceRevisionId,
    representation_id: RepresentationId,
    unit_id: UnitId,
    profile: &search_contracts::ProjectionProfileSetId,
) -> Result<ScoringDocumentId, ProjectionError> {
    let domain = CanonicalDigestDomain::parse(SCORING_DOCUMENT_DOMAIN)
        .map_err(|_| ProjectionError::InvalidLimits)?;
    let limit =
        DigestInputLimit::new(MAX_CANONICAL_BYTES).map_err(|_| ProjectionError::InvalidLimits)?;
    let coordinates = BoundedList::new(vec![
        uuid_value(source_revision_id.as_bytes())?,
        uuid_value(representation_id.as_bytes())?,
        uuid_value(unit_id.as_bytes())?,
        canonical_text(profile.as_str())?,
    ])
    .map_err(|_| ProjectionError::InvalidManifest)?;
    let digest = blake3_canonical(&domain, &CanonicalValue::Array(coordinates), limit)
        .map_err(|_| ProjectionError::InvalidManifest)?;
    let mut truncated = [0_u8; 16];
    truncated.copy_from_slice(&digest.as_bytes()[..16]);
    Ok(ScoringDocumentId::from_bytes(truncated))
}

/// Builds the exact S9.5 payload through the shared contract owner.
#[allow(clippy::too_many_arguments)]
fn build_payload(
    scope: &PlanningScope,
    unit_id: UnitId,
    point_identity_digest_256: Blake3Digest32,
    scoring_document_id: ScoringDocumentId,
    planned: &PlannedUnit,
) -> Result<PointPayload, ProjectionError> {
    let payload = PointPayload::new(
        scope.membership.installation_incarnation_id,
        scope.membership.collection_generation_id,
        scope.membership.projection_membership_id,
        scope.membership.access_partition_id,
        scope.membership.scoring_partition_id,
        scope.membership.source_id,
        scope.membership.source_revision_id,
        scope.membership.representation_id,
        unit_id,
        point_identity_digest_256,
        scoring_document_id,
        scope.membership.projection_profile_set_id.clone(),
        planned.unit_kind,
        planned.modality,
        planned.language_or_format.clone(),
        planned.entity_kind,
        planned.normalized_symbol_key.clone(),
        scope.lineage.repository_lineage_id,
        scope.valid_from_epoch,
        scope.valid_until_epoch_exclusive,
    );
    payload
        .validate()
        .map_err(|_| ProjectionError::ScopeMismatch)?;
    Ok(payload)
}

/// Builds one exact point specification from one planned unit.
fn build_point(
    scope: &PlanningScope,
    planned: &PlannedUnit,
    requirements: &BTreeMap<String, VectorRequirement>,
) -> Result<PointSpec, ProjectionError> {
    let vector_digests = validate_unit_vectors(&planned.vectors, requirements)?;
    let scoring_document_id = derive_scoring_document_id(
        scope.membership.source_revision_id,
        scope.membership.representation_id,
        planned.unit_id,
        &scope.membership.projection_profile_set_id,
    )?;
    let key = ProjectionPointKey {
        schema_version: POINT_KEY_SCHEMA_VERSION,
        installation_incarnation_id: scope.membership.installation_incarnation_id,
        collection_generation_id: scope.membership.collection_generation_id,
        projection_membership_id: scope.membership.projection_membership_id,
        representation_id: scope.membership.representation_id,
        unit_id: planned.unit_id,
        projection_profile_set_id: scope.membership.projection_profile_set_id.clone(),
        point_role: PointRole::Unit,
    };
    let identity = derive_point_identity(&key, scope.identity_limits)?;
    let payload = build_payload(
        scope,
        planned.unit_id,
        identity.full_digest(),
        scoring_document_id,
        planned,
    )?;
    let payload_identity = payload.identity().map_err(|_| ProjectionError::InvalidManifest)?;
    let payload_cbor = payload
        .to_canonical_cbor()
        .map_err(|_| ProjectionError::InvalidManifest)?;
    Ok(PointSpec {
        point_id: identity.point_id(),
        identity,
        expected_readback: ExpectedReadbackShape {
            payload_cbor,
            payload_identity,
            vector_digests,
            unit_digest: planned.unit_digest,
            reference_digest: planned.reference_digest,
        },
    })
}

/// Builds one exact manifest entry for one point specification.
fn manifest_entry(
    point: &PointSpec,
    scope: &PlanningScope,
) -> Result<ProjectionManifestEntry, ProjectionError> {
    let unit_id = point.identity.key().unit_id;
    let scoring_document_id = derive_scoring_document_id(
        scope.membership.source_revision_id,
        scope.membership.representation_id,
        unit_id,
        &scope.membership.projection_profile_set_id,
    )?;
    Ok(ProjectionManifestEntry {
        point_id: point.point_id,
        point_identity_digest_256: point.identity.full_digest(),
        source_membership_id: scope.membership.source_membership_id,
        projection_membership_id: scope.membership.projection_membership_id,
        unit_id,
        collection_generation_id: scope.membership.collection_generation_id,
        installation_incarnation_id: scope.membership.installation_incarnation_id,
        access_partition_id: scope.membership.access_partition_id,
        scoring_partition_id: scope.membership.scoring_partition_id,
        scoring_document_id,
        projection_profile_set_id: scope.membership.projection_profile_set_id.clone(),
        unit_digest: point.expected_readback.unit_digest,
        reference_digest: point.expected_readback.reference_digest,
        payload_identity: point.expected_readback.payload_identity,
        vector_digests: point.expected_readback.vector_digests.clone(),
    })
}

/// Locates one verified descriptor by its exact unit identity.
fn find_descriptor(
    unit_set: &VerifiedUnitSet,
    unit_id: UnitId,
) -> Result<&UnitDescriptor, ProjectionError> {
    unit_set
        .units()
        .iter()
        .find(|descriptor| descriptor.unit_id() == unit_id)
        .ok_or(ProjectionError::ScopeMismatch)
}

/// Plans one complete membership-scoped projection.
///
/// Every verified unit must contribute exactly one planned unit carrying its
/// complete required named-vector set. The result is deterministic: input
/// order never changes the canonical point order, the manifest entries or the
/// frozen bytes.
///
/// # Errors
///
/// Returns a typed [`ProjectionError`] when the unit set does not bind the
/// membership representation, when the planned units do not cover the verified
/// set exactly, when a vector set is incomplete or malformed, when point
/// identity derivation refuses a key, or when the manifest exceeds a finite
/// ceiling. No partial plan is ever produced.
pub fn plan_projection(
    unit_set: &VerifiedUnitSet,
    scope: &PlanningScope,
    planned_units: &[PlannedUnit],
    requirements: &BTreeMap<String, VectorRequirement>,
    budget: ProjectionBudget,
) -> Result<ProjectionPlan, ProjectionError> {
    let budget = budget.validate()?;
    validate_scope_binding(unit_set, scope, budget)?;
    if planned_units.is_empty() || planned_units.len() != unit_set.units().len() {
        return Err(ProjectionError::UnitSetTooLarge);
    }

    // One indexed membership pass replaces every per-unit linear rescan, so
    // admitted coverage costs one lookup per unit rather than
    // expected_units x planned_units comparisons.
    let index = VerifiedUnitIndex::build(unit_set)?;
    let mut covered: BTreeSet<u64> = BTreeSet::new();
    let mut points: Vec<PointSpec> = Vec::with_capacity(planned_units.len());
    for planned in planned_units {
        let descriptor = find_descriptor(unit_set, planned.unit_id)?;
        index.admit(descriptor.occurrence().ordinal, planned.unit_id, &mut covered)?;
        points.push(build_point(scope, planned, requirements)?);
    }
    index.prove_complete(&covered)?;

    points.sort_by_key(|point| point.point_id);
    let manifest = rebuild_manifest(&points, scope, budget)?;
    Ok(ProjectionPlan {
        source_membership_id: scope.membership.source_membership_id,
        projection_membership_id: scope.membership.projection_membership_id,
        points,
        manifest,
    })
}

/// Rebuilds one canonical manifest from exact point specifications.
pub fn rebuild_manifest(
    points: &[PointSpec],
    scope: &PlanningScope,
    budget: ProjectionBudget,
) -> Result<ProjectionManifest, ProjectionError> {
    let budget = budget.validate()?;
    let mut entries = Vec::with_capacity(points.len());
    for point in points {
        entries.push(manifest_entry(point, scope)?);
    }
    entries.sort_by_key(|entry| entry.point_id);
    let canonical = manifest_canonical_bytes(
        &ProjectionManifest {
            entries: entries.clone(),
            canonical_bytes: Vec::new(),
        },
        budget,
    )?;
    Ok(ProjectionManifest {
        entries,
        canonical_bytes: canonical.as_slice().to_vec(),
    })
}

/// Builds one bounded canonical key.
fn canonical_key(name: &str) -> Result<CanonicalKey, ProjectionError> {
    CanonicalKey::new_non_empty(name).map_err(|_| ProjectionError::InvalidManifest)
}

/// Builds one bounded canonical text value.
fn canonical_text(value: &str) -> Result<CanonicalValue, ProjectionError> {
    CanonicalText::new(value)
        .map(CanonicalValue::Text)
        .map_err(|_| ProjectionError::InvalidManifest)
}

/// Encodes one UUID coordinate as exact canonical bytes.
fn uuid_value(bytes: &[u8; 16]) -> Result<CanonicalValue, ProjectionError> {
    BoundedBytes::new(bytes.to_vec())
        .map(CanonicalValue::Bytes)
        .map_err(|_| ProjectionError::InvalidManifest)
}

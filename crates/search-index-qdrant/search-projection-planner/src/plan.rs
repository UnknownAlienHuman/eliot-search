use std::collections::{BTreeMap, BTreeSet};

use search_contracts::Blake3Digest32;
use search_point_identity::{
    CollisionDecision, POINT_IDENTITY_SCHEMA_VERSION, PointIdentity,
    PointIdentityKey, PointIdentityLimits, PointIdentityRegistry,
    derive_point_identity,
};

use crate::canonical::{canonical_payload_bytes, canonical_vector_bytes};
use crate::{
    ExpectedReadbackShape, MinimalPointPayload, NamedVectorInput, PlannedVector,
    PointSpec, ProjectionBudget, ProjectionDigestPort, ProjectionError,
    ProjectionInput, ProjectionPlan, ProjectionProfiles, ProjectionScope,
    ProjectionUnitInput, ValidatedProjectionInput, canonicalize_manifest,
};

/// Validates one complete membership-scoped projection input.
pub fn validate_projection_input(
    input: ProjectionInput,
    profiles: &ProjectionProfiles,
    budget: ProjectionBudget,
) -> Result<ValidatedProjectionInput, ProjectionError> {
    let budget = budget.validate()?;
    profiles.validate(budget)?;
    validate_scope(&input.scope, profiles)?;
    if input.units.is_empty() || input.units.len() > budget.max_points {
        return Err(ProjectionError::BudgetExceeded);
    }

    let expected_names = profiles.vectors.keys().cloned().collect::<BTreeSet<_>>();
    let mut unit_roles = BTreeSet::new();
    let mut scoring_documents = BTreeSet::new();
    for unit in &input.units {
        if unit.representation_id != input.scope.membership.representation_id {
            return Err(ProjectionError::MembershipMismatch);
        }
        if !unit_roles.insert((unit.unit_id, unit.point_role)) {
            return Err(ProjectionError::DuplicateUnitRole);
        }
        if !scoring_documents.insert(unit.scoring_document_id) {
            return Err(ProjectionError::DuplicateScoringDocument);
        }
        validate_unit(unit, profiles, budget, &expected_names)?;
    }
    Ok(ValidatedProjectionInput(input))
}

/// Proves every point in a plan belongs to exactly the plan's one projection
/// membership and immutable access/scoring partitions.
pub fn validate_membership_isolation(
    plan: &ProjectionPlan,
) -> Result<(), ProjectionError> {
    let scope = &plan.scope;
    for point in &plan.points {
        if point.payload.installation_incarnation_id
            != scope.installation_incarnation_id
            || point.payload.collection_generation_id
                != scope.collection_generation_id
            || point.payload.projection_membership_id
                != scope.membership.projection_membership_id
            || point.payload.access_partition_id
                != scope.membership.access_partition_id
            || point.payload.scoring_partition_id
                != scope.membership.scoring_partition_id
            || point.payload.source_id != scope.source_id
            || point.payload.source_revision_id != scope.source_revision_id
            || point.payload.representation_id
                != scope.membership.representation_id
            || point.payload.projection_profile_set_id
                != plan.profiles.profile_set_id
        {
            return Err(ProjectionError::MembershipMismatch);
        }
    }
    Ok(())
}

/// Builds the exact closed S9.5 payload for one derived point identity.
pub fn build_minimal_payload(
    scope: &ProjectionScope,
    unit: &ProjectionUnitInput,
    profiles: &ProjectionProfiles,
    identity: &PointIdentity,
) -> Result<MinimalPointPayload, ProjectionError> {
    let payload = MinimalPointPayload {
        installation_incarnation_id: scope.installation_incarnation_id,
        collection_generation_id: scope.collection_generation_id,
        projection_membership_id: scope.membership.projection_membership_id,
        access_partition_id: scope.membership.access_partition_id,
        scoring_partition_id: scope.membership.scoring_partition_id,
        source_id: scope.source_id,
        source_revision_id: scope.source_revision_id,
        representation_id: unit.representation_id,
        unit_id: unit.unit_id,
        point_identity_digest_256: Blake3Digest32::from_bytes(
            *identity.full_digest.as_bytes(),
        ),
        scoring_document_id: unit.scoring_document_id,
        projection_profile_set_id: profiles.profile_set_id.clone(),
        unit_kind: unit.unit_kind,
        modality: unit.modality,
        language_or_format: unit.language_or_format.clone(),
        entity_kind: unit.entity_kind,
        normalized_symbol_key: unit.normalized_symbol_key.clone(),
        repository_lineage_id: unit.repository_lineage_id,
        valid_from_epoch: scope.valid_from_epoch,
        valid_until_epoch_exclusive: None,
    };
    payload.validate()?;
    Ok(payload)
}

/// Builds one exact point specification and planner-owned expected digests.
pub fn build_point_spec<D: ProjectionDigestPort>(
    scope: &ProjectionScope,
    unit: ProjectionUnitInput,
    profiles: &ProjectionProfiles,
    budget: ProjectionBudget,
    point_identity_limits: PointIdentityLimits,
    digest_port: &mut D,
) -> Result<PointSpec, ProjectionError> {
    let budget = budget.validate()?;
    profiles.validate(budget)?;
    validate_scope(scope, profiles)?;
    let expected_names = profiles.vectors.keys().cloned().collect::<BTreeSet<_>>();
    validate_unit(&unit, profiles, budget, &expected_names)?;
    build_point_spec_validated(
        scope,
        unit,
        profiles,
        budget,
        point_identity_limits,
        digest_port,
    )
}

/// Creates a deterministic exact plan and immutable manifest for one
/// projection membership.
pub fn plan_projection<D: ProjectionDigestPort>(
    input: ProjectionInput,
    profiles: &ProjectionProfiles,
    budget: ProjectionBudget,
    point_identity_limits: PointIdentityLimits,
    digest_port: &mut D,
) -> Result<ProjectionPlan, ProjectionError> {
    let validated = validate_projection_input(input, profiles, budget)?;
    let budget = budget.validate()?;
    let ProjectionInput { scope, units } = validated.into_input();
    let mut registry = PointIdentityRegistry::new(point_identity_limits)?;
    let mut point_ids = BTreeSet::new();
    let mut points = Vec::with_capacity(units.len());

    for unit in units {
        let point = build_point_spec_validated(
            &scope,
            unit,
            profiles,
            budget,
            point_identity_limits,
            digest_port,
        )?;
        match registry.register(point.identity.clone())? {
            CollisionDecision::Vacant => {}
            CollisionDecision::SameFullIdentity | CollisionDecision::CollisionBlock => {
                return Err(ProjectionError::DuplicatePointId);
            }
        }
        if !point_ids.insert(point.point_id) {
            return Err(ProjectionError::DuplicatePointId);
        }
        points.push(point);
    }
    points.sort_by_key(|point| point.point_id);
    let manifest = canonicalize_manifest(
        &scope,
        profiles,
        &points,
        budget,
        digest_port,
    )?;
    let plan = ProjectionPlan {
        scope,
        profiles: profiles.clone(),
        points,
        manifest,
    };
    validate_membership_isolation(&plan)?;
    Ok(plan)
}

fn build_point_spec_validated<D: ProjectionDigestPort>(
    scope: &ProjectionScope,
    unit: ProjectionUnitInput,
    profiles: &ProjectionProfiles,
    budget: ProjectionBudget,
    point_identity_limits: PointIdentityLimits,
    digest_port: &mut D,
) -> Result<PointSpec, ProjectionError> {
    let key = PointIdentityKey {
        schema_version: POINT_IDENTITY_SCHEMA_VERSION,
        installation_incarnation_id: scope.installation_incarnation_id,
        collection_generation_id: scope.collection_generation_id,
        projection_membership_id: scope.membership.projection_membership_id,
        representation_id: unit.representation_id,
        unit_id: unit.unit_id,
        projection_profile_set_id: profiles.profile_set_id.clone(),
        point_role: unit.point_role,
    };
    let identity = derive_point_identity(key, point_identity_limits)?;
    let payload = build_minimal_payload(scope, &unit, profiles, &identity)?;
    let payload_bytes = canonical_payload_bytes(&payload, budget)?;
    let payload_digest = digest_port.blake3_256(&payload_bytes)?;

    let mut vectors = BTreeMap::new();
    let mut vector_digests = BTreeMap::new();
    for input in unit.vectors {
        let canonical = canonical_vector_bytes(
            &input.name,
            input.dimensions,
            &input.value,
            budget,
        )?;
        let digest = digest_port.blake3_256(&canonical)?;
        let vector = PlannedVector {
            name: input.name.clone(),
            dimensions: input.dimensions,
            value: input.value,
            digest,
        };
        if vectors.insert(input.name.clone(), vector).is_some()
            || vector_digests.insert(input.name, digest).is_some()
        {
            return Err(ProjectionError::DuplicateVectorName);
        }
    }
    let expected_readback = ExpectedReadbackShape {
        identity_payload: identity.payload(),
        payload_digest,
        vector_digests,
    };
    Ok(PointSpec {
        point_id: identity.point_id,
        identity,
        payload,
        vectors,
        expected_readback,
    })
}

fn validate_scope(
    scope: &ProjectionScope,
    profiles: &ProjectionProfiles,
) -> Result<(), ProjectionError> {
    if scope.membership.projection_schema_id != profiles.projection_schema_id {
        return Err(ProjectionError::ProfileSetMismatch);
    }
    if scope.valid_from_epoch.get() == 0 {
        return Err(ProjectionError::PointPayloadInvalid);
    }
    Ok(())
}

fn validate_unit(
    unit: &ProjectionUnitInput,
    profiles: &ProjectionProfiles,
    budget: ProjectionBudget,
    expected_names: &BTreeSet<String>,
) -> Result<(), ProjectionError> {
    if unit.vectors.len() != profiles.vectors.len()
        || unit.vectors.len() > budget.max_vectors_per_point
    {
        return Err(ProjectionError::VectorSetMismatch);
    }
    let mut names = BTreeSet::new();
    let mut stored_values = 0_usize;
    for vector in &unit.vectors {
        validate_vector(vector, profiles, budget)?;
        if !names.insert(vector.name.clone()) {
            return Err(ProjectionError::DuplicateVectorName);
        }
        stored_values = stored_values
            .checked_add(vector.value.stored_values())
            .ok_or(ProjectionError::BudgetExceeded)?;
    }
    if names != *expected_names
        || stored_values > budget.max_stored_vector_values_per_point
    {
        return Err(ProjectionError::VectorSetMismatch);
    }
    Ok(())
}

fn validate_vector(
    vector: &NamedVectorInput,
    profiles: &ProjectionProfiles,
    budget: ProjectionBudget,
) -> Result<(), ProjectionError> {
    if vector.name.is_empty() || vector.name.len() > budget.max_vector_name_bytes {
        return Err(ProjectionError::VectorSetMismatch);
    }
    let requirement = profiles
        .vectors
        .get(&vector.name)
        .ok_or(ProjectionError::VectorSetMismatch)?;
    if vector.dimensions != requirement.dimensions
        || vector.value.is_sparse() != requirement.sparse
    {
        return Err(ProjectionError::VectorDimensionMismatch);
    }
    vector.value.validate(vector.dimensions)
}

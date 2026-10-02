use std::collections::{BTreeMap, BTreeSet};

use search_point_identity::{
    POINT_IDENTITY_SCHEMA_VERSION, PointIdentity, PointIdentityLimits,
    ProjectionPointKey, derive_point_identity,
};

use crate::digest::{
    derive_scoring_document_id, planned_vector,
};
use crate::{
    ExpectedReadbackShape, MinimalPointPayload, PointSpec, PreparedUnit,
    ProjectionBudget, ProjectionError, ProjectionInput, ProjectionPlan,
    ProjectionProfiles, ValidatedProjectionInput, canonicalize_manifest,
    payload_digest,
};

/// Validates one complete membership-scoped projection input.
pub fn validate_projection_input(
    input: ProjectionInput,
    profiles: &ProjectionProfiles,
    budget: ProjectionBudget,
) -> Result<ValidatedProjectionInput, ProjectionError> {
    let budget = budget.validate()?;
    profiles.validate(budget)?;
    if input.membership.projection_schema_id != profiles.projection_schema_id {
        return Err(ProjectionError::ProfileSetMismatch);
    }
    if input.valid_from_epoch.get() == 0 {
        return Err(ProjectionError::InvalidEpoch);
    }
    if input.units.is_empty() {
        return Err(ProjectionError::EmptyPointSet);
    }
    if input.units.len() > budget.max_points {
        return Err(ProjectionError::BudgetExceeded);
    }

    let expected_names: BTreeSet<String> = profiles.vectors.keys().cloned().collect();
    let mut unit_roles = BTreeSet::new();
    for unit in &input.units {
        if !unit_roles.insert((unit.unit_id, unit.point_role)) {
            return Err(ProjectionError::DuplicateUnitRole);
        }
        if unit.vectors.len() != profiles.vectors.len()
            || unit.vectors.len() > budget.max_vectors_per_point
        {
            return Err(ProjectionError::VectorSetMismatch);
        }
        let mut names = BTreeSet::new();
        let mut stored_values = 0_usize;
        for vector in &unit.vectors {
            if vector.name.is_empty() || vector.name.len() > budget.max_vector_name_bytes {
                return Err(ProjectionError::VectorSetMismatch);
            }
            if !names.insert(vector.name.clone()) {
                return Err(ProjectionError::DuplicateVectorName);
            }
            let requirement = profiles
                .vectors
                .get(&vector.name)
                .copied()
                .ok_or(ProjectionError::VectorSetMismatch)?;
            vector.value.validate(requirement)?;
            stored_values = stored_values
                .checked_add(vector.value.stored_values())
                .ok_or(ProjectionError::BudgetExceeded)?;
        }
        if names != expected_names {
            return Err(ProjectionError::VectorSetMismatch);
        }
        if stored_values > budget.max_stored_vector_values_per_point {
            return Err(ProjectionError::BudgetExceeded);
        }
    }

    Ok(ValidatedProjectionInput(input))
}

/// Builds the exact closed S9.5 point payload for one validated unit.
pub fn build_minimal_payload(
    input: &ValidatedProjectionInput,
    unit: &PreparedUnit,
    profiles: &ProjectionProfiles,
    identity: &PointIdentity,
) -> Result<MinimalPointPayload, ProjectionError> {
    let input = input.as_input();
    if !input.units.iter().any(|candidate| candidate == unit)
        || identity.key.installation_incarnation_id
            != input.installation_incarnation_id
        || identity.key.collection_generation_id != input.collection_generation_id
        || identity.key.projection_membership_id
            != input.membership.projection_membership_id
        || identity.key.representation_id != input.membership.representation_id
        || identity.key.unit_id != unit.unit_id
        || identity.key.projection_profile_set_id
            != profiles.projection_profile_set_id
        || identity.key.point_role != unit.point_role
    {
        return Err(ProjectionError::MembershipIsolationViolation);
    }
    let scoring_document_id = derive_scoring_document_id(
        input.source_revision_id,
        input.membership.representation_id,
        unit.unit_id,
        &profiles.projection_profile_set_id,
    );
    Ok(MinimalPointPayload {
        installation_incarnation_id: input.installation_incarnation_id,
        collection_generation_id: input.collection_generation_id,
        projection_membership_id: input.membership.projection_membership_id,
        access_partition_id: input.membership.access_partition_id,
        scoring_partition_id: input.membership.scoring_partition_id,
        source_id: input.membership.source_id,
        source_revision_id: input.source_revision_id,
        representation_id: input.membership.representation_id,
        unit_id: unit.unit_id,
        point_identity_digest_256: identity.full_digest.as_contract_digest(),
        scoring_document_id,
        projection_profile_set_id: profiles.projection_profile_set_id.clone(),
        unit_kind: unit.unit_kind,
        modality: unit.modality,
        language_or_format: unit.language_or_format.clone(),
        entity_kind: unit.entity_kind,
        normalized_symbol_key: unit.normalized_symbol_key.clone(),
        repository_lineage_id: input.repository_lineage_id,
        valid_from_epoch: input.valid_from_epoch,
        valid_until_epoch_exclusive: None,
    })
}

/// Builds one exact point specification from a validated unit.
pub fn build_point_spec(
    input: &ValidatedProjectionInput,
    unit: &PreparedUnit,
    profiles: &ProjectionProfiles,
    budget: ProjectionBudget,
    identity_limits: PointIdentityLimits,
) -> Result<PointSpec, ProjectionError> {
    let budget = budget.validate()?;
    let source = input.as_input();
    let identity = derive_point_identity(
        ProjectionPointKey {
            schema_version: POINT_IDENTITY_SCHEMA_VERSION,
            installation_incarnation_id: source.installation_incarnation_id,
            collection_generation_id: source.collection_generation_id,
            projection_membership_id: source.membership.projection_membership_id,
            representation_id: source.membership.representation_id,
            unit_id: unit.unit_id,
            projection_profile_set_id: profiles.projection_profile_set_id.clone(),
            point_role: unit.point_role,
        },
        identity_limits,
    )?;
    let payload = build_minimal_payload(input, unit, profiles, &identity)?;
    let payload_digest = payload_digest(&payload);

    let mut vectors = BTreeMap::new();
    let mut stored_values = 0_usize;
    for prepared in &unit.vectors {
        let requirement = profiles
            .vectors
            .get(&prepared.name)
            .copied()
            .ok_or(ProjectionError::VectorSetMismatch)?;
        prepared.value.validate(requirement)?;
        stored_values = stored_values
            .checked_add(prepared.value.stored_values())
            .ok_or(ProjectionError::BudgetExceeded)?;
        let vector = planned_vector(prepared, requirement);
        if vectors.insert(prepared.name.clone(), vector).is_some() {
            return Err(ProjectionError::DuplicateVectorName);
        }
    }
    if stored_values > budget.max_stored_vector_values_per_point
        || vectors.len() != profiles.vectors.len()
    {
        return Err(ProjectionError::BudgetExceeded);
    }
    let vector_digests = vectors
        .iter()
        .map(|(name, vector)| (name.clone(), vector.digest))
        .collect();
    let expected_readback = ExpectedReadbackShape {
        payload: payload.clone(),
        payload_digest,
        vector_digests,
        unit_digest: unit.unit_digest,
        reference_digest: unit.reference_digest,
    };
    Ok(PointSpec {
        point_id: identity.point_id,
        identity,
        source_membership_id: source.membership.source_membership_id,
        payload,
        vectors,
        expected_readback,
    })
}

/// Creates one deterministic exact membership-scoped plan and manifest.
pub fn plan_projection(
    input: ProjectionInput,
    profiles: &ProjectionProfiles,
    budget: ProjectionBudget,
    identity_limits: PointIdentityLimits,
) -> Result<ProjectionPlan, ProjectionError> {
    let budget = budget.validate()?;
    let validated = validate_projection_input(input, profiles, budget)?;
    let scope = validated.as_input().membership.clone();
    let mut points = Vec::with_capacity(validated.as_input().units.len());
    let mut point_ids = BTreeSet::new();
    for unit in &validated.as_input().units {
        let point = build_point_spec(&validated, unit, profiles, budget, identity_limits)?;
        if !point_ids.insert(point.point_id) {
            return Err(ProjectionError::DuplicatePointId);
        }
        points.push(point);
    }
    points.sort_by_key(|point| point.point_id);
    let manifest = canonicalize_manifest(&points, profiles, budget, identity_limits)?;
    let plan = ProjectionPlan {
        source_membership_id: scope.source_membership_id,
        projection_membership_id: scope.projection_membership_id,
        points,
        manifest,
    };
    validate_membership_isolation(&plan)?;
    Ok(plan)
}

/// Proves one plan and manifest contain exactly one projection membership.
pub fn validate_membership_isolation(
    plan: &ProjectionPlan,
) -> Result<(), ProjectionError> {
    if plan.points.is_empty()
        || plan.points.len() != plan.manifest.entries.len()
        || plan.points.iter().any(|point| {
            point.source_membership_id != plan.source_membership_id
                || point.payload.projection_membership_id
                    != plan.projection_membership_id
                || point.identity.key.projection_membership_id
                    != plan.projection_membership_id
        })
        || plan.manifest.entries.iter().any(|entry| {
            entry.source_membership_id != plan.source_membership_id
                || entry.identity_key.projection_membership_id
                    != plan.projection_membership_id
        })
    {
        return Err(ProjectionError::MembershipIsolationViolation);
    }
    Ok(())
}

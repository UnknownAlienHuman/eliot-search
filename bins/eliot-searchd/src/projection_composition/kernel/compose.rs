//! Pure membership-scoped projection planning and collision verification.

use search_point_identity::{
    PointIdentityLimits, PointIdentityRegistry, PointRole,
};
use search_projection_planner::{
    ProjectionBudget, ProjectionError, ProjectionInput, ProjectionPlan,
    ProjectionProfiles, expected_payload_indexes, plan_scoped_projection,
    verify_manifest_reconstruction,
};

use super::error::ProjectionCompositionError;
use super::model::CompositionRequest;

/// Composes one complete membership-scoped projection plan (pure, no I/O).
///
/// Scope identities are already admitted by T13/T16 owners. This function
/// only proves exact equality, builds typed S9.5 inputs and delegates payload,
/// identity, digest and manifest production to `search-projection-planner`.
pub fn compose_scoped_projection(
    request: &CompositionRequest,
    profiles: &ProjectionProfiles,
    budget: ProjectionBudget,
    point_identity_limits: PointIdentityLimits,
) -> Result<ProjectionPlan, ProjectionCompositionError> {
    let budget = budget
        .validate()
        .map_err(|_| ProjectionCompositionError::from(ProjectionError::InvalidLimits))?;
    point_identity_limits
        .validate()
        .map_err(ProjectionCompositionError::from)?;
    if request.membership.source_membership_id
        != request.scope.source_membership_id
        || request.membership.projection_membership_id
            != request.scope.projection_membership_id
    {
        return Err(ProjectionCompositionError::MembershipMismatch);
    }
    if request.units.is_empty() || request.units.len() > budget.max_points {
        return Err(ProjectionCompositionError::BudgetExceeded);
    }

    let mut inputs = Vec::with_capacity(request.units.len());
    for unit in &request.units {
        let receipt = &unit.receipt;
        if receipt.source_byte_start >= receipt.source_byte_end {
            return Err(ProjectionCompositionError::from(
                ProjectionError::InvalidUnitRange,
            ));
        }
        if receipt.residency_digest != request.scope.residency_digest {
            return Err(ProjectionCompositionError::ResidencyMismatch);
        }
        inputs.push(ProjectionInput {
            installation_incarnation_id:
                request.scope.installation_incarnation_id,
            collection_generation_id: request.scope.collection_generation_id,
            source_membership_id: request.scope.source_membership_id,
            projection_membership_id: request.scope.projection_membership_id,
            access_partition_id: request.scope.access_partition_id,
            scoring_partition_id: request.scope.scoring_partition_id,
            source_id: request.scope.source_id,
            source_revision_id: request.scope.source_revision_id,
            representation_id: request.scope.representation_id,
            unit_id: receipt.unit_id,
            scoring_document_id: receipt.scoring_document_id,
            projection_profile_set_id:
                request.scope.projection_profile_set_id.clone(),
            point_role: PointRole::Unit,
            unit_kind: receipt.unit_kind,
            modality: receipt.modality,
            language_or_format: receipt.language_or_format.clone(),
            entity_kind: receipt.entity_kind,
            normalized_symbol_key: receipt.normalized_symbol_key.clone(),
            repository_lineage_id: receipt.repository_lineage_id,
            valid_from_epoch: request.visible_epoch,
            valid_until_epoch_exclusive: None,
            unit_ordinal: receipt.unit_ordinal,
            source_byte_start: receipt.source_byte_start,
            source_byte_end: receipt.source_byte_end,
            unit_digest: receipt.unit_digest,
            reference_digest: receipt.reference_digest,
            residency_digest: receipt.residency_digest,
            vectors: unit.vectors.clone(),
        });
    }

    let plan = plan_scoped_projection(
        inputs,
        &request.expected_units,
        &request.scope,
        profiles,
        budget,
        point_identity_limits,
    )
    .map_err(ProjectionCompositionError::from)?;
    verify_no_collisions(&plan, point_identity_limits)?;
    verify_manifest_reconstruction(&plan.manifest, &plan.points)
        .map_err(ProjectionCompositionError::from)?;
    Ok(plan)
}

/// Returns the exact S9.5 payload fields T24 must index.
#[must_use]
pub const fn expected_payload_indexes_for_bridge() -> [&'static str; 19] {
    expected_payload_indexes()
}

fn verify_no_collisions(
    plan: &ProjectionPlan,
    limits: PointIdentityLimits,
) -> Result<(), ProjectionCompositionError> {
    let registry_limits = PointIdentityLimits {
        max_registered_points: plan.points.len().max(1),
        ..limits
    };
    let mut registry = PointIdentityRegistry::new(registry_limits)?;
    for point in &plan.points {
        registry.register(point.identity.clone())?;
    }
    Ok(())
}

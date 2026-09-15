//! Pure membership-scoped projection planning and collision verification.

use search_point_identity::{
    PointIdentityLimits, PointIdentityRegistry, ProjectionKind,
};
use search_projection_planner::{
    ProjectionBudget, ProjectionError, ProjectionInput, ProjectionPlan,
    ProjectionProfiles, expected_payload_indexes, plan_scoped_projection,
    verify_manifest_reconstruction,
};

use super::digest::compute_payload_digest;
use super::error::ProjectionCompositionError;
use super::model::CompositionRequest;

/// Composes one complete membership-scoped projection plan (pure, no I/O).
///
/// # Errors
///
/// Returns the typed [`ProjectionCompositionError`] for scope drift, missing
/// or unexpected receipts, duplicates, collisions, invalid manifests and
/// exhausted budgets. Performs no Qdrant, CAS or redb I/O.
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
    if request.membership.source_membership_id != request.scope.source_membership_id
        || request.membership.projection_membership_id != request.scope.projection_membership_id
    {
        return Err(ProjectionCompositionError::MembershipMismatch);
    }
    if request.units.is_empty() || request.units.len() > budget.max_points {
        return Err(ProjectionCompositionError::BudgetExceeded);
    }
    let mut inputs = Vec::with_capacity(request.units.len());
    for unit in &request.units {
        if unit.receipt.source_byte_start >= unit.receipt.source_byte_end {
            return Err(ProjectionCompositionError::from(
                ProjectionError::InvalidUnitRange,
            ));
        }
        inputs.push(ProjectionInput {
            namespace_id: request.scope.namespace_id.clone(),
            source_id: request.scope.source_id.clone(),
            source_membership_id: request.scope.source_membership_id.clone(),
            projection_membership_id: request.scope.projection_membership_id.clone(),
            source_revision: request.scope.source_revision,
            unit_ordinal: unit.receipt.unit_ordinal,
            source_byte_start: unit.receipt.source_byte_start,
            source_byte_end: unit.receipt.source_byte_end,
            projection_kind: ProjectionKind::Lexical,
            projection_fingerprint: request.scope.projection_fingerprint,
            projection_schema_revision: request.scope.projection_schema_revision,
            visible_epoch: request.visible_epoch,
            access_partition_digest: unit.receipt.access_partition_digest,
            representation_digest: unit.receipt.representation_digest,
            scoring_partition_digest: request.scope.scoring_partition_digest,
            collection_generation_digest: request.scope.collection_generation_digest,
            residency_digest: unit.receipt.residency_digest,
            unit_digest: unit.receipt.unit_digest,
            reference_digest: unit.receipt.reference_digest,
            payload_digest: compute_payload_digest(
                &request.scope,
                request.visible_epoch,
                &unit.receipt,
            ),
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

/// Returns the exact filterable payload fields T24 must index.
#[must_use]
pub const fn expected_payload_indexes_for_bridge() -> [&'static str; 6] {
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

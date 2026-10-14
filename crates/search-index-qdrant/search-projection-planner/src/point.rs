//! Exact schema requirements for one qualified collection generation.
//!
//! The field, index and vector tables are read from the shared contract
//! owner; none is copied here. Readback validation is performed by the
//! contract's own `CollectionSchema::validate_readback`, so the planner only
//! proves the schema is the accepted one for this membership.

use search_contracts::indexed::CollectionSchema;
use search_contracts::{
    CollectionGenerationId, InstallationIncarnationId, ProjectionProfileSetId,
};

use crate::error::ProjectionError;
use crate::model::{ProjectionBudget, ProjectionManifest, ProjectionPlan};

/// Proves every manifest entry's named vectors exist in the qualified schema.
///
/// The complete 19-index plan and the named-vector requirement comparison are
/// owned by the contract's own readback validator; this check adds the
/// manifest-side guarantee that no extra or foreign vector name is planned.
pub fn validate_schema_requirements(
    manifest: &ProjectionManifest,
    schema: &CollectionSchema,
) -> Result<(), ProjectionError> {
    super::diff::validate_entries(&manifest.entries)?;
    let requirements = schema_requirement_names(schema);
    for entry in &manifest.entries {
        if entry.vector_digests.len() != requirements.len() {
            return Err(ProjectionError::CollectionVectorMissing);
        }
        for name in entry.vector_digests.keys() {
            if !requirements.contains(&name.as_str()) {
                return Err(ProjectionError::CollectionVectorMissing);
            }
        }
    }
    Ok(())
}

/// Proves one schema is the qualified one for this membership.
///
/// A schema bound to another installation incarnation, collection generation
/// or profile set fails closed instead of substituting a second schema table.
pub fn validate_schema_binding(
    schema: &CollectionSchema,
    plan: &ProjectionPlan,
    budget: ProjectionBudget,
) -> Result<(), ProjectionError> {
    let _ = budget.validate()?;
    let Some(first) = plan.manifest.entries.first() else {
        return Err(ProjectionError::MembershipArrayForbidden);
    };
    if schema.installation_incarnation_id() != &first.installation_incarnation_id
        || schema.collection_generation_id() != &first.collection_generation_id
        || schema.projection_profile_set_id() != &first.projection_profile_set_id
    {
        return Err(ProjectionError::CollectionSchemaMismatch);
    }
    Ok(())
}

/// Proves one plan carries exactly one projection membership.
pub fn validate_membership_isolation(plan: &ProjectionPlan) -> Result<(), ProjectionError> {
    if plan.points.is_empty()
        || plan.points.len() != plan.manifest.entries.len()
        || plan.points.iter().any(|point| {
            point.identity.key().projection_membership_id != plan.projection_membership_id
        })
        || plan.manifest.entries.iter().any(|entry| {
            entry.projection_membership_id != plan.projection_membership_id
                || entry.source_membership_id != plan.source_membership_id
        })
    {
        return Err(ProjectionError::MembershipArrayForbidden);
    }
    Ok(())
}

/// Returns the schema's exact named-vector identifiers.
fn schema_requirement_names(schema: &CollectionSchema) -> Vec<&str> {
    schema
        .vectors()
        .iter()
        .map(|requirement| requirement.name().as_str())
        .collect()
}

/// Re-exported identifier types used by the binding comparison above.
const _: fn() -> (InstallationIncarnationId, CollectionGenerationId, ProjectionProfileSetId) = || {
    unreachable!()
};

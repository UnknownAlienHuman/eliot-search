//! Pure deterministic planning for Qdrant point projections.
//!
//! This package performs no database, filesystem, network, admission, access
//! decision or publication effect. It converts one owner-verified
//! `VerifiedUnitSet` and one exact prepared vector product per unit into exact
//! S9.5 payloads, S11 point identities and one canonical immutable manifest.
//!
//! Ownership boundaries enforced here:
//!
//! - point identity, its full digest and its compact address belong to
//!   `search-point-identity`; this package never derives either locally;
//! - the 20-field payload, the 19-index plan, the collection schema, the
//!   vector requirement vocabulary and every canonical/digest computation
//!   belong to `search-contracts`; no field table is copied here;
//! - complete unit-set membership belongs to `search-unitizer`; no caller
//!   `expected_units`, count or digest can substitute for its verification;
//! - Qdrant transport, source truth and access authority are not owned here.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![allow(
    clippy::large_enum_variant,
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::too_many_arguments,
    clippy::too_many_lines
)]

mod diff;
mod error;
mod manifest;
mod model;
mod plan;
mod point;
mod validate;

pub use diff::diff_manifests;
pub use error::ProjectionError;
pub use model::{
    ExpectedReadbackShape, LineageObservation, ManifestDiff, PlannedUnit, PlannedVector, PointSpec,
    ProjectionBudget, ProjectionManifest, ProjectionManifestEntry, ProjectionMembershipBinding,
    ProjectionPlan,
};
pub use plan::{PlanningScope, plan_projection, validate_scope_binding};
pub use point::validate_membership_isolation;
pub use validate::VerifiedUnitIndex;

use search_contracts::indexed::CollectionSchema;
use search_contracts::ProjectionMembershipId;
use search_unitizer::VerifiedUnitSet;

/// Requires exactly one membership where an array could alias point sets.
pub fn require_single_membership(
    memberships: &[ProjectionMembershipId],
) -> Result<ProjectionMembershipId, ProjectionError> {
    if memberships.len() != 1 {
        return Err(ProjectionError::MembershipArrayForbidden);
    }
    memberships
        .first()
        .copied()
        .ok_or(ProjectionError::MembershipArrayForbidden)
}

/// Builds the exact ordered membership index over the verified unit set.
///
/// The index is derived from the authoritative set with a private constructor,
/// so a caller can neither mint nor substitute one.
pub fn build_verified_unit_index(
    unit_set: &VerifiedUnitSet,
) -> Result<VerifiedUnitIndex, ProjectionError> {
    VerifiedUnitIndex::build(unit_set)
}

/// Proves one manifest reconstructs exactly its point specifications.
///
/// Recomputes every entry from the exact point specs and compares full
/// equality. Any drift fails closed.
pub fn verify_manifest_reconstruction(
    manifest: &ProjectionManifest,
    points: &[PointSpec],
    scope: &PlanningScope,
    budget: ProjectionBudget,
) -> Result<(), ProjectionError> {
    let rebuilt =
        plan::rebuild_manifest(points, scope, budget).map_err(|_| ProjectionError::ManifestMismatch)?;
    if rebuilt == *manifest {
        Ok(())
    } else {
        Err(ProjectionError::ManifestMismatch)
    }
}

/// Validates one qualified collection schema against one plan.
///
/// Delegates the provider readback shape to the shared contract owner and adds
/// the manifest-side vector completeness and binding checks.
pub fn validate_schema(
    schema: &CollectionSchema,
    plan: &ProjectionPlan,
    budget: ProjectionBudget,
) -> Result<(), ProjectionError> {
    let budget = budget.validate()?;
    schema
        .validate()
        .map_err(|_| ProjectionError::CollectionSchemaMismatch)?;
    point::validate_schema_binding(schema, plan, budget)?;
    point::validate_schema_requirements(&plan.manifest, schema)
}

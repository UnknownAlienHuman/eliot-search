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
//!
//! Scope coordinates and vector requirements are caller-supplied candidate
//! inputs. Their consistency with the verified unit set is checked here;
//! registry admission, profile acceptance and lineage verification are still
//! required from their owners before publication (#259 acceptance pending).

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
// Canonical encoding is data only; publication still verifies source/scope/units.
pub use manifest::manifest_canonical_bytes;
pub use model::{
    ExpectedReadbackShape, LineageObservation, ManifestDiff, PlannedUnit, PlannedVector, PointSpec,
    ProjectionBudget, ProjectionManifest, ProjectionManifestEntry, ProjectionMembershipBinding,
    ProjectionPlan,
};
pub use plan::{PlanningScope, plan_projection, validate_scope_binding};
pub use point::validate_membership_isolation;

use search_contracts::indexed::CollectionSchema;
use search_contracts::{
    Blake3Digest32, CanonicalDigestDomain, DigestInputLimit, MAX_CANONICAL_BYTES,
    ProjectionMembershipId, blake3_raw,
};

const MANIFEST_SEMANTIC_DIGEST_DOMAIN: &str = "eliot/raw/projection-manifest-semantic/v1";

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
    let rebuilt = plan::rebuild_manifest(points, scope, budget)
        .map_err(|_| ProjectionError::ManifestMismatch)?;
    if rebuilt == *manifest {
        Ok(())
    } else {
        Err(ProjectionError::ManifestMismatch)
    }
}

/// Returns C13's semantic digest only after the manifest exactly reconstructs
/// from the caller's actual point specifications and scope.
///
/// The public manifest fields are mutable, so this verifies both entries and
/// their frozen canonical bytes before hashing. The result uses C00's
/// `eliot/raw/projection-manifest-semantic/v1` domain and is distinct from an
/// unframed digest of the encoded body or from the profile-set identity.
pub fn verified_projection_manifest_semantic_digest(
    manifest: &ProjectionManifest,
    points: &[PointSpec],
    scope: &PlanningScope,
    budget: ProjectionBudget,
) -> Result<Blake3Digest32, ProjectionError> {
    verify_manifest_reconstruction(manifest, points, scope, budget)?;
    manifest_semantic_digest(manifest)
}

/// Recomputes canonical manifest bytes before issuing their C13 semantic digest.
///
/// This establishes encoded-data consistency, not source/scope qualification;
/// publication must independently establish the complete `UnitSet` and owner scope.
pub fn canonical_projection_manifest_semantic_digest(
    manifest: &ProjectionManifest,
    budget: ProjectionBudget,
) -> Result<Blake3Digest32, ProjectionError> {
    if manifest_canonical_bytes(manifest, budget)?.as_slice() != manifest.canonical_bytes {
        return Err(ProjectionError::ManifestMismatch);
    }
    manifest_semantic_digest(manifest)
}

fn manifest_semantic_digest(manifest: &ProjectionManifest) -> Result<Blake3Digest32, ProjectionError> {
    let domain = CanonicalDigestDomain::parse(MANIFEST_SEMANTIC_DIGEST_DOMAIN)
        .map_err(|_| ProjectionError::InvalidManifest)?;
    let limit =
        DigestInputLimit::new(MAX_CANONICAL_BYTES).map_err(|_| ProjectionError::InvalidLimits)?;
    blake3_raw(&domain, &manifest.canonical_bytes, limit)
        .map_err(|_| ProjectionError::ManifestTooLarge)
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

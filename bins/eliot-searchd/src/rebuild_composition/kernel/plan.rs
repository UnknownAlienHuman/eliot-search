//! Deterministic rebuild planning and full backend readback verification.

use search_contracts::{
    Blake3Digest32, CollectionGenerationId, CollectionRouteRevision,
};
use search_epoch_pins::RouteIdentity;

use super::error::RebuildError;
use super::manifest::{
    RetainedManifest, RetainedPoint, validate_retained_manifest,
};

/// Finite rebuild tuning.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RebuildBudget {
    /// Maximum retained points in one rebuild plan.
    pub max_points: usize,
    /// Maximum exact-ID batches in one rebuild plan.
    pub max_batches: usize,
}

/// One exact rebuild plan: fresh generation and deterministic point batches.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RebuildPlan {
    /// Route being replaced.
    pub old_route: RouteIdentity,
    /// Fresh physical collection generation under construction.
    pub new_generation: CollectionGenerationId,
    /// Exact successor of the replaced route revision.
    pub new_revision: CollectionRouteRevision,
    /// Retained-manifest digest under replay.
    pub manifest_digest: Blake3Digest32,
    /// Deterministic exact-ID batches in canonical order.
    pub batches: Vec<Vec<[u8; 16]>>,
    /// Digest binding old route, new generation, manifest and batching.
    pub plan_digest: Blake3Digest32,
}

fn rebuild_plan_digest(
    old_route: RouteIdentity,
    new_generation: CollectionGenerationId,
    new_revision: CollectionRouteRevision,
    manifest_digest: Blake3Digest32,
    batch_size: usize,
) -> Result<Blake3Digest32, RebuildError> {
    let batch_size =
        u64::try_from(batch_size).map_err(|_| RebuildError::BudgetExceeded)?;
    let mut input = Vec::with_capacity(32 + 16 + 8 + 16 + 8 + 32 + 8);
    input.extend_from_slice(b"eliot-search/rebuild-plan/v1\x00");
    input.extend_from_slice(old_route.collection_generation_id.as_bytes());
    input.extend_from_slice(&old_route.route_revision.get().to_le_bytes());
    input.extend_from_slice(new_generation.as_bytes());
    input.extend_from_slice(&new_revision.get().to_le_bytes());
    input.extend_from_slice(manifest_digest.as_bytes());
    input.extend_from_slice(&batch_size.to_le_bytes());
    Ok(Blake3Digest32::from_bytes(*blake3::hash(&input).as_bytes()))
}

/// Plans a new generation from one validated retained manifest.
///
/// The manifest is borrowed read-only. The new route revision must be exactly
/// the successor of the active route and the new generation must differ.
pub fn propose_rebuild(
    old_route: RouteIdentity,
    new_generation: CollectionGenerationId,
    new_revision: CollectionRouteRevision,
    manifest: &RetainedManifest,
    batch_size: usize,
    budget: RebuildBudget,
) -> Result<RebuildPlan, RebuildError> {
    if batch_size == 0 || budget.max_points == 0 || budget.max_batches == 0 {
        return Err(RebuildError::InvalidLimits);
    }
    if new_generation == old_route.collection_generation_id {
        return Err(RebuildError::GenerationReuse);
    }
    let expected_revision = old_route
        .route_revision
        .checked_next()
        .map_err(|_| RebuildError::StaleRevision)?;
    if new_revision != expected_revision {
        return Err(RebuildError::StaleRevision);
    }
    if manifest.generation != old_route.collection_generation_id
        || manifest.route_revision != old_route.route_revision
    {
        return Err(RebuildError::GenerationMismatch);
    }
    validate_retained_manifest(manifest)?;
    if manifest.points.len() > budget.max_points {
        return Err(RebuildError::BudgetExceeded);
    }
    let batch_count = manifest.points.len().div_ceil(batch_size);
    if batch_count > budget.max_batches {
        return Err(RebuildError::BudgetExceeded);
    }
    let mut batches = Vec::with_capacity(batch_count);
    for chunk in manifest.points.chunks(batch_size) {
        batches.push(chunk.iter().map(|point| point.id).collect());
    }
    let plan_digest = rebuild_plan_digest(
        old_route,
        new_generation,
        new_revision,
        manifest.manifest_digest,
        batch_size,
    )?;
    Ok(RebuildPlan {
        old_route,
        new_generation,
        new_revision,
        manifest_digest: manifest.manifest_digest,
        batches,
        plan_digest,
    })
}

/// Caller-observed backend state over the planned point set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndexReadbackView {
    /// Points found with their live digests.
    pub present: Vec<RetainedPoint>,
    /// Planned identifiers with no stored point.
    pub missing: Vec<[u8; 16]>,
    /// Points returned for unplanned identifiers.
    pub unexpected: Vec<[u8; 16]>,
}

/// Proof that the backend holds exactly the planned generation content.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FullReadbackProof {
    /// Plan the proof was verified against.
    pub plan_digest: Blake3Digest32,
    /// Number of exactly verified points.
    pub verified_points: usize,
}

/// Verifies all planned points and rejects missing, unexpected or changed data.
pub fn verify_full_readback(
    plan: &RebuildPlan,
    manifest: &RetainedManifest,
    readback: &IndexReadbackView,
) -> Result<FullReadbackProof, RebuildError> {
    if manifest.manifest_digest != plan.manifest_digest {
        return Err(RebuildError::ReadbackMismatch);
    }
    if !readback.missing.is_empty() || !readback.unexpected.is_empty() {
        return Err(RebuildError::ReadbackMismatch);
    }
    let mut expected: Vec<RetainedPoint> = manifest.points.clone();
    expected.sort();
    let mut observed = readback.present.clone();
    observed.sort();
    if expected != observed {
        return Err(RebuildError::ReadbackMismatch);
    }
    Ok(FullReadbackProof {
        plan_digest: plan.plan_digest,
        verified_points: expected.len(),
    })
}

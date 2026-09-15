//! Verified staged and committed route cutover records.

use search_contracts::{
    Blake3Digest32, CollectionGenerationId, CollectionRouteRevision,
};
use search_epoch_pins::RouteIdentity;

use super::error::RebuildError;
use super::plan::{FullReadbackProof, RebuildPlan};

/// Staged but uncommitted route cutover.
///
/// This value authorizes nothing by itself. A commit requires a fresh full
/// readback proof after any interruption or restart.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StagedCutover {
    /// Plan the cutover was staged for.
    pub plan_digest: Blake3Digest32,
    /// Route being replaced.
    pub old_route: RouteIdentity,
    /// Fresh generation under construction.
    pub new_generation: CollectionGenerationId,
    /// Exact successor revision under construction.
    pub new_revision: CollectionRouteRevision,
    /// Digest binding the staged cutover record.
    pub staged_digest: Blake3Digest32,
}

/// Committed route cutover: one verified publication of a rebuild.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommittedCutover {
    /// Plan the cutover committed.
    pub plan_digest: Blake3Digest32,
    /// Published generation.
    pub new_generation: CollectionGenerationId,
    /// Published route revision.
    pub new_revision: CollectionRouteRevision,
    /// Digest binding the commit record to its readback proof.
    pub commit_digest: Blake3Digest32,
}

/// Stages one cutover after a full backend readback.
pub fn stage_cutover(
    plan: &RebuildPlan,
    proof: &FullReadbackProof,
) -> Result<StagedCutover, RebuildError> {
    if proof.plan_digest != plan.plan_digest {
        return Err(RebuildError::CutoverMismatch);
    }
    let mut input = Vec::with_capacity(32 + 32 + 16 + 16 + 8);
    input.extend_from_slice(b"eliot-search/rebuild-staged/v1\x00");
    input.extend_from_slice(plan.plan_digest.as_bytes());
    input.extend_from_slice(plan.old_route.collection_generation_id.as_bytes());
    input.extend_from_slice(plan.new_generation.as_bytes());
    input.extend_from_slice(&plan.new_revision.get().to_le_bytes());
    Ok(StagedCutover {
        plan_digest: plan.plan_digest,
        old_route: plan.old_route,
        new_generation: plan.new_generation,
        new_revision: plan.new_revision,
        staged_digest: Blake3Digest32::from_bytes(*blake3::hash(&input).as_bytes()),
    })
}

/// Commits a staged cutover under a fresh full-readback proof.
pub fn commit_cutover(
    staged: &StagedCutover,
    proof: &FullReadbackProof,
) -> Result<CommittedCutover, RebuildError> {
    if proof.plan_digest != staged.plan_digest {
        return Err(RebuildError::CutoverMismatch);
    }
    let verified = u64::try_from(proof.verified_points)
        .map_err(|_| RebuildError::BudgetExceeded)?;
    let mut input = Vec::with_capacity(32 + 32 + 8);
    input.extend_from_slice(b"eliot-search/rebuild-commit/v1\x00");
    input.extend_from_slice(staged.staged_digest.as_bytes());
    input.extend_from_slice(&verified.to_le_bytes());
    Ok(CommittedCutover {
        plan_digest: staged.plan_digest,
        new_generation: staged.new_generation,
        new_revision: staged.new_revision,
        commit_digest: Blake3Digest32::from_bytes(*blake3::hash(&input).as_bytes()),
    })
}

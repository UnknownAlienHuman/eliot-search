//! Ordinary exact-ID reclaim authorization after route retirement.

use search_contracts::CollectionGenerationId;
use search_epoch_pins::{
    PinRegistry, ReclamationWatermark, RetiredVisibilityFence, RouteIdentity,
    compute_reclamation_watermark,
};
use search_index_reclaimer::{
    CommittedRetiredManifest, PublicationCommitProof, ReclaimBudget,
    ReclaimError, ReclaimPlan, ReclaimReceipt, ReclaimReceiptKind,
    ReclaimSettings, RetiredPointManifest,
};

use super::error::RebuildError;

/// Finite reclaim tuning for one ordinary reclaim authorization.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReclaimTuning {
    /// Exact point identifiers per delete/readback batch.
    pub batch_size: usize,
    /// Maximum identifiers in one plan.
    pub max_points: usize,
    /// Maximum batches in one plan.
    pub max_batches: usize,
}

/// Authorizes ordinary reclaim of a committed retired manifest.
///
/// The route and generation must own the manifest and the live pin watermark
/// must prove no active query can still observe it. Security purge cannot be
/// represented by this API.
pub fn authorize_reclaim(
    manifest: RetiredPointManifest,
    publication: &PublicationCommitProof,
    caller_route: RouteIdentity,
    caller_generation: CollectionGenerationId,
    registry: &PinRegistry,
    tuning: ReclaimTuning,
) -> Result<ReclaimPlan, RebuildError> {
    if manifest.route != caller_route {
        return Err(RebuildError::StaleRoute);
    }
    if manifest.collection_generation_id != caller_generation {
        return Err(RebuildError::GenerationMismatch);
    }
    let committed: CommittedRetiredManifest =
        search_index_reclaimer::validate_retired_manifest(manifest, publication)
            .map_err(|error| match error {
                ReclaimError::PublicationMismatch => {
                    RebuildError::PublicationMismatch
                }
                ReclaimError::EmptyManifest | ReclaimError::InvalidPointSet => {
                    RebuildError::ManifestNotCanonical
                }
                other => RebuildError::ReclaimDenied(other),
            })?;
    let snapshot = registry.snapshot().map_err(RebuildError::PinDenied)?;
    let fence = RetiredVisibilityFence {
        route: committed.manifest().route,
        retirement_epoch_exclusive: committed
            .manifest()
            .retirement_epoch_exclusive,
    };
    let watermark: ReclamationWatermark =
        compute_reclamation_watermark(fence, &snapshot);
    if tuning.batch_size == 0
        || tuning.max_points == 0
        || tuning.max_batches == 0
    {
        return Err(RebuildError::InvalidLimits);
    }
    search_index_reclaimer::plan(
        committed,
        watermark,
        ReclaimSettings {
            batch_size: tuning.batch_size,
        },
        ReclaimBudget {
            max_points: tuning.max_points,
            max_batches: tuning.max_batches,
        },
    )
    .map_err(|error| match error {
        ReclaimError::StillPinned => RebuildError::StillPinned,
        ReclaimError::BudgetExceeded => RebuildError::BudgetExceeded,
        other => RebuildError::ReclaimDenied(other),
    })
}

/// Whether a receipt proves ordinary retired-point reclaim.
///
/// The exhaustive match intentionally has no wildcard so a future purge-like
/// receipt cannot silently pass this gate.
#[must_use]
pub const fn is_ordinary_reclaim_receipt(
    receipt: &ReclaimReceipt,
) -> bool {
    match receipt.kind {
        ReclaimReceiptKind::OrdinaryRetiredPointReclaim => true,
    }
}

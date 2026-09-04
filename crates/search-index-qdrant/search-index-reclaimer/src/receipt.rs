//! Exact batch receipts, checkpoints, resume, and completion.

use std::collections::{BTreeMap, BTreeSet};

use search_contracts::{Blake3Digest32, OpaqueId, ReceiptRef};
use search_epoch_pins::ReclamationWatermark;

use crate::{
    ReclaimBatch, ReclaimError, ReclaimPlan, ReclaimPlanDigest, ReclaimPointId,
};

/// Terminal observation for one exact delete/readback batch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReclaimBatchOutcome {
    /// Every requested identifier is verified absent.
    Complete,
    /// Delete may have committed but exact readback is unresolved.
    OutcomeUnknown,
    /// Exact readback contradicted the requested batch.
    Rejected,
}

/// Exact batch deletion/readback receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReclaimBatchReceipt {
    /// Matching plan digest.
    pub plan_digest: ReclaimPlanDigest,
    /// Matching batch index.
    pub batch_index: usize,
    /// Matching immutable operation identity.
    pub operation_id: OpaqueId,
    /// Identifiers verified absent after deletion.
    pub missing_ids: Vec<ReclaimPointId>,
    /// Unexpected identifiers returned by exact readback.
    pub unexpected_ids: Vec<ReclaimPointId>,
    /// Terminal outcome.
    pub outcome: ReclaimBatchOutcome,
}

/// Verifies an exact batch receipt against its plan.
///
/// # Errors
///
/// Distinguishes absent batches, unknown mutation outcomes, unexpected IDs,
/// and mismatched operation or point sets.
pub fn verify_batch_receipt(
    plan: &ReclaimPlan,
    receipt: &ReclaimBatchReceipt,
) -> Result<(), ReclaimError> {
    let batch = plan
        .batches
        .get(receipt.batch_index)
        .ok_or(ReclaimError::BatchNotFound)?;
    if receipt.plan_digest != plan.plan_digest
        || receipt.operation_id != batch.operation_id
        || receipt.batch_index != batch.batch_index
    {
        return Err(ReclaimError::BatchReceiptMismatch);
    }
    match receipt.outcome {
        ReclaimBatchOutcome::OutcomeUnknown => return Err(ReclaimError::BatchOutcomeUnknown),
        ReclaimBatchOutcome::Rejected => return Err(ReclaimError::BatchReceiptMismatch),
        ReclaimBatchOutcome::Complete => {}
    }
    if !receipt.unexpected_ids.is_empty() {
        return Err(ReclaimError::UnexpectedReadback);
    }
    if receipt.missing_ids != batch.point_ids {
        return Err(ReclaimError::BatchReceiptMismatch);
    }
    Ok(())
}

/// Durable content-free reclaim checkpoint.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReclaimCheckpoint {
    /// Exact plan digest.
    pub plan_digest: ReclaimPlanDigest,
    /// Exact retired-manifest digest.
    pub manifest_digest: Blake3Digest32,
    /// Verified batch receipts keyed by deterministic index.
    pub completed: BTreeMap<usize, ReclaimBatchReceipt>,
}

/// Builds a checkpoint only from verified batch receipts.
///
/// # Errors
///
/// Rejects duplicate batch indices and every invalid receipt.
pub fn checkpoint(
    plan: &ReclaimPlan,
    receipts: Vec<ReclaimBatchReceipt>,
) -> Result<ReclaimCheckpoint, ReclaimError> {
    let mut completed = BTreeMap::new();
    for receipt in receipts {
        verify_batch_receipt(plan, &receipt)?;
        let batch_index = receipt.batch_index;
        if completed.insert(batch_index, receipt).is_some() {
            return Err(ReclaimError::BatchReceiptMismatch);
        }
    }
    Ok(ReclaimCheckpoint {
        plan_digest: plan.plan_digest,
        manifest_digest: plan.manifest.manifest().manifest_digest,
        completed,
    })
}

/// Revalidates a checkpoint and returns exact unfinished batches.
///
/// # Errors
///
/// Rejects foreign checkpoints, stale pin watermarks, or invalid completed
/// receipts. Already verified batches are omitted rather than blindly replayed.
pub fn resume(
    checkpoint: &ReclaimCheckpoint,
    plan: &ReclaimPlan,
    watermark: ReclamationWatermark,
) -> Result<Vec<ReclaimBatch>, ReclaimError> {
    if checkpoint.plan_digest != plan.plan_digest
        || checkpoint.manifest_digest != plan.manifest.manifest().manifest_digest
    {
        return Err(ReclaimError::CheckpointMismatch);
    }
    if !watermark.reclaimable
        || watermark.blocking_epoch_pins != 0
        || watermark.blocking_route_pins != 0
    {
        return Err(ReclaimError::StillPinned);
    }
    for receipt in checkpoint.completed.values() {
        verify_batch_receipt(plan, receipt)?;
    }
    Ok(plan
        .batches
        .iter()
        .filter(|batch| !checkpoint.completed.contains_key(&batch.batch_index))
        .cloned()
        .collect())
}

/// Closed receipt kind. It cannot be confused with a security purge receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReclaimReceiptKind {
    /// Ordinary deletion of rebuildable retired index points.
    OrdinaryRetiredPointReclaim,
}

/// Terminal ordinary-reclaim receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReclaimReceipt {
    /// Fixed ordinary-reclaim kind.
    pub kind: ReclaimReceiptKind,
    /// Exact plan digest.
    pub plan_digest: ReclaimPlanDigest,
    /// Exact retired-manifest digest.
    pub manifest_digest: Blake3Digest32,
    /// Number of exact identifiers verified absent.
    pub reclaimed_points: usize,
    /// Publication receipt inherited from the committed manifest.
    pub publication_receipt_ref: ReceiptRef,
}

/// Completes only when every exact planned identifier is verified absent.
///
/// # Errors
///
/// Rejects missing, duplicate, unknown, contradictory, or mismatched receipts.
pub fn complete(
    plan: &ReclaimPlan,
    receipts: &[ReclaimBatchReceipt],
) -> Result<ReclaimReceipt, ReclaimError> {
    if receipts.len() != plan.batches.len() {
        return Err(ReclaimError::IncompleteReclaim);
    }
    let mut seen = BTreeSet::new();
    let mut reclaimed_points = 0_usize;
    for receipt in receipts {
        verify_batch_receipt(plan, receipt)?;
        if !seen.insert(receipt.batch_index) {
            return Err(ReclaimError::BatchReceiptMismatch);
        }
        reclaimed_points = reclaimed_points
            .checked_add(receipt.missing_ids.len())
            .ok_or(ReclaimError::BudgetExceeded)?;
    }
    if reclaimed_points != plan.manifest.manifest().point_ids.len() {
        return Err(ReclaimError::IncompleteReclaim);
    }
    Ok(ReclaimReceipt {
        kind: ReclaimReceiptKind::OrdinaryRetiredPointReclaim,
        plan_digest: plan.plan_digest,
        manifest_digest: plan.manifest.manifest().manifest_digest,
        reclaimed_points,
        publication_receipt_ref: plan
            .manifest
            .manifest()
            .publication_receipt_ref
            .clone(),
    })
}

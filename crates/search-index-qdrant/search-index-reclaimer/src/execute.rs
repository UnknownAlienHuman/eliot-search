//! Exact batch execution through a vendor-neutral index-admin port.
//!
//! [`execute_batch`] deletes only the batch's exact identifiers, then proves
//! absence through an exact readback. A delete that may have committed but
//! reports no usable acknowledgement is resolved through the readback: full
//! absence completes, any residual or contradictory observation stays
//! [`ReclaimError::BatchOutcomeUnknown`] or fails closed. The port carries no
//! collection names, filters, or credentials; the caller binds the route.

use search_contracts::OpaqueId;

use crate::{ReclaimBatch, ReclaimBatchOutcome, ReclaimBatchReceipt, ReclaimError, ReclaimPlan};

/// Immutable exact mutation identity for one batch execution.
///
/// Retries reuse the batch's own [`ReclaimBatch::operation_id`] byte-identical
/// with the same point set; the admin ledger rejects the same identity with
/// different identifiers as a conflict.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdminMutation {
    /// Caller-assigned operation identity; must equal the batch identity.
    pub operation_id: OpaqueId,
    /// Digest of the exact canonical execution input.
    pub input_digest: [u8; 32],
}

/// Exact acknowledgement returned by the index-admin delete path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdminDeleteAck {
    /// Echo of the executed operation identity.
    pub operation_id: OpaqueId,
    /// Identifiers the admin reports deleted.
    pub deleted_ids: Vec<crate::ReclaimPointId>,
    /// Whether the receipt came from idempotent replay.
    pub replayed: bool,
}

/// Exact readback over the batch identifiers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdminReadback {
    /// Requested identifiers with no stored point.
    pub missing_ids: Vec<crate::ReclaimPointId>,
    /// Identifiers returned for unrequested points.
    pub unexpected_ids: Vec<crate::ReclaimPointId>,
}

/// Closed index-admin failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdminError {
    /// The delete may have committed; only an exact readback resolves it.
    Unknown,
    /// The same identity was reused with different identifiers.
    Conflict,
    /// The admin deterministically contradicts the planned expectation.
    Mismatch,
    /// The transport failed without a usable acknowledgement.
    Transport,
}

impl AdminError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Unknown => "RECLAIM_ADMIN_OUTCOME_UNKNOWN",
            Self::Conflict => "RECLAIM_ADMIN_CONFLICT",
            Self::Mismatch => "RECLAIM_ADMIN_MISMATCH",
            Self::Transport => "RECLAIM_ADMIN_TRANSPORT_FAILED",
        }
    }
}

impl core::fmt::Display for AdminError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for AdminError {}

/// Vendor-neutral exact-ID index administration.
///
/// The implementor deletes only the batch's exact identifiers and reads back
/// only requested identifiers. No broad-filter operation exists on this port.
pub trait IndexAdmin {
    /// Deletes only the batch's exact identifiers.
    ///
    /// # Errors
    ///
    /// Returns [`AdminError`] for ambiguous, conflicting, contradictory, or
    /// transport failures. Absence is proven by the later readback, never by
    /// this acknowledgement alone.
    fn delete_exact(
        &mut self,
        batch: &ReclaimBatch,
        mutation: &AdminMutation,
    ) -> Result<AdminDeleteAck, AdminError>;

    /// Reads back exactly the requested identifiers.
    ///
    /// # Errors
    ///
    /// Returns [`AdminError`] when the observation cannot be produced; the
    /// caller treats an unobservable readback as an unknown outcome.
    fn readback_exact(&self, ids: &[crate::ReclaimPointId]) -> Result<AdminReadback, AdminError>;
}

/// Executes one exact batch: delete by exact identifiers, then prove absence.
///
/// A usable delete acknowledgement still requires the readback proof; an
/// ambiguous delete (`Unknown`, `Transport`) is resolved through the same
/// readback. Full absence completes, residual points after an ambiguous
/// delete stay unknown, and any contradiction fails closed.
///
/// # Errors
///
/// Returns [`ReclaimError::BatchNotFound`] for an unknown batch,
/// [`ReclaimError::BatchReceiptMismatch`] for a foreign operation identity or
/// a mismatched acknowledgement, [`ReclaimError::BatchOutcomeUnknown`] when
/// the outcome cannot be resolved, and [`ReclaimError::UnexpectedReadback`]
/// for contradictory observations.
pub fn execute_batch(
    plan: &ReclaimPlan,
    batch_index: usize,
    admin: &mut impl IndexAdmin,
    mutation: &AdminMutation,
) -> Result<ReclaimBatchReceipt, ReclaimError> {
    let batch = plan
        .batches
        .get(batch_index)
        .ok_or(ReclaimError::BatchNotFound)?;
    if mutation.operation_id != batch.operation_id {
        return Err(ReclaimError::BatchReceiptMismatch);
    }
    let delete_acked = match admin.delete_exact(batch, mutation) {
        Ok(ack) => {
            if ack.operation_id != batch.operation_id {
                return Err(ReclaimError::BatchReceiptMismatch);
            }
            if ack.deleted_ids != batch.point_ids {
                return Err(ReclaimError::BatchReceiptMismatch);
            }
            true
        }
        Err(error) => match error {
            AdminError::Unknown | AdminError::Transport => false,
            AdminError::Conflict => return Err(ReclaimError::BatchReceiptMismatch),
            AdminError::Mismatch => return Err(ReclaimError::UnexpectedReadback),
        },
    };
    let readback = admin
        .readback_exact(&batch.point_ids)
        .map_err(|_| ReclaimError::BatchOutcomeUnknown)?;
    if !readback.unexpected_ids.is_empty() {
        return Err(ReclaimError::UnexpectedReadback);
    }
    if readback.missing_ids == batch.point_ids {
        let receipt = ReclaimBatchReceipt {
            plan_digest: plan.plan_digest,
            batch_index: batch.batch_index,
            operation_id: batch.operation_id.clone(),
            missing_ids: readback.missing_ids,
            unexpected_ids: Vec::new(),
            outcome: ReclaimBatchOutcome::Complete,
        };
        crate::verify_batch_receipt(plan, &receipt)?;
        return Ok(receipt);
    }
    if delete_acked {
        return Err(ReclaimError::UnexpectedReadback);
    }
    Err(ReclaimError::BatchOutcomeUnknown)
}

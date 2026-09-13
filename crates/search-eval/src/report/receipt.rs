//! Content-free Product Pulse receipt issuance.

use search_contracts::{Blake3Digest32, OpaqueId, ReceiptRef};

use crate::{EvalError, ValidatedAcceptancePolicy};

use super::model::ProductPulseReport;
use super::review::{IndependentReview, validate_independent_review};
use super::verdict::{AcceptanceVerdict, VerdictKind};

/// Immutable content-free Product Pulse receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductPulseReceipt {
    /// Exact frozen run digest.
    pub run_digest: Blake3Digest32,
    /// Exact report digest.
    pub report_digest: Blake3Digest32,
    /// Exact policy digest.
    pub policy_digest: Blake3Digest32,
    /// Exact review identity.
    pub review_id: OpaqueId,
    /// Exact verdict digest.
    pub verdict_digest: Blake3Digest32,
    /// Terminal verdict.
    pub verdict: VerdictKind,
    /// Number of hard blockers.
    pub hard_blocker_count: usize,
    /// Content-free receipt identity.
    pub receipt: ReceiptRef,
}

/// Issues a receipt only for an exactly bound report, review, policy, and verdict.
pub fn issue_product_pulse_receipt(
    report: &ProductPulseReport,
    policy: &ValidatedAcceptancePolicy,
    review: &IndependentReview,
    verdict: &AcceptanceVerdict,
    receipt: ReceiptRef,
) -> Result<ProductPulseReceipt, EvalError> {
    validate_independent_review(report, policy, review)?;
    if verdict.run_digest != report.run_digest
        || verdict.report_digest != report.report_digest
        || verdict.policy_digest != policy.policy().policy_digest
        || verdict.review_id != review.review_id
        || verdict.hard_blockers != report.hard_blockers
        || receipt.as_str().is_empty()
    {
        return Err(EvalError::ReceiptMismatch);
    }
    Ok(ProductPulseReceipt {
        run_digest: report.run_digest,
        report_digest: report.report_digest,
        policy_digest: policy.policy().policy_digest,
        review_id: review.review_id.clone(),
        verdict_digest: verdict.verdict_digest,
        verdict: verdict.kind,
        hard_blocker_count: report.hard_blockers.len(),
        receipt,
    })
}

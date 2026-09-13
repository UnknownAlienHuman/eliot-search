//! Independent Product Pulse report review bound to one report and policy.

use search_contracts::{Blake3Digest32, OpaqueId, ReceiptRef};

use crate::{EvalError, ValidatedAcceptancePolicy};

use super::model::ProductPulseReport;

/// Independent report review fixed to one exact policy and report digest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndependentReview {
    /// Stable review identity.
    pub review_id: OpaqueId,
    /// Exact frozen run digest.
    pub run_digest: Blake3Digest32,
    /// Exact Product Pulse report digest.
    pub report_digest: Blake3Digest32,
    /// Exact acceptance-policy digest.
    pub policy_digest: Blake3Digest32,
    /// Evaluation/report producer identity.
    pub producer_id: OpaqueId,
    /// Independent reviewer identity.
    pub reviewer_id: OpaqueId,
    /// Whether conflicts of interest were declared absent.
    pub conflict_free: bool,
    /// Whether raw evidence references were available to the reviewer.
    pub raw_evidence_available: bool,
    /// Whether the reviewer approved the report for policy evaluation.
    pub approved: bool,
    /// Monotone evidence sequence of review completion.
    pub completed_sequence: u64,
    /// Immutable review evidence.
    pub review_receipt: ReceiptRef,
}

/// Validates reviewer independence and exact report/policy binding.
pub fn validate_independent_review(
    report: &ProductPulseReport,
    policy: &ValidatedAcceptancePolicy,
    review: &IndependentReview,
) -> Result<(), EvalError> {
    if review.run_digest != report.run_digest
        || review.report_digest != report.report_digest
        || review.policy_digest != policy.policy().policy_digest
        || review.producer_id != policy.policy().producer_id
        || review.reviewer_id != policy.policy().approver_id
        || review.producer_id == review.reviewer_id
        || !review.conflict_free
        || !review.raw_evidence_available
        || review.completed_sequence <= policy.policy().registered_sequence
        || review.review_receipt.as_str().is_empty()
    {
        return Err(if review.producer_id == review.reviewer_id {
            EvalError::SelfAcceptanceForbidden
        } else {
            EvalError::IndependentReviewRequired
        });
    }
    Ok(())
}

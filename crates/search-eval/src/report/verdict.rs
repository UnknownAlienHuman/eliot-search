//! Preregistered acceptance verdict over one independently reviewed report.

use std::collections::BTreeSet;

use search_contracts::{Blake3Digest32, OpaqueId};

use crate::{
    BaselineComparisonClass, EvalError, FingerprintBuilder, HardBlocker,
    ValidatedAcceptancePolicy,
};

use super::model::ProductPulseReport;
use super::review::{IndependentReview, validate_independent_review};
use super::support::{blocker_class_tag, opaque_reason, verdict_tag};

/// Closed acceptance verdict.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum VerdictKind {
    /// All preregistered gates passed with complete independently reviewed evidence.
    Accepted,
    /// Complete evidence contains a failed gate or hard blocker.
    Rejected,
    /// Evidence is incomplete; acceptance and rejection are not inferred.
    Incomplete,
}

/// Independent policy verdict over one exact Product Pulse report.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptanceVerdict {
    /// Exact frozen run digest.
    pub run_digest: Blake3Digest32,
    /// Exact report digest.
    pub report_digest: Blake3Digest32,
    /// Exact policy digest.
    pub policy_digest: Blake3Digest32,
    /// Exact independent review identity.
    pub review_id: OpaqueId,
    /// Terminal verdict.
    pub kind: VerdictKind,
    /// Every machine-readable reason; an empty set is permitted only for ACCEPTED.
    pub reasons: BTreeSet<OpaqueId>,
    /// Hard blockers copied without weighting or aggregation.
    pub hard_blockers: Vec<HardBlocker>,
    /// Deterministic verdict digest.
    pub verdict_digest: Blake3Digest32,
}

/// Applies preregistered gates only after exact independent review.
pub fn decide_acceptance(
    report: &ProductPulseReport,
    policy: &ValidatedAcceptancePolicy,
    review: &IndependentReview,
) -> Result<AcceptanceVerdict, EvalError> {
    validate_independent_review(report, policy, review)?;
    let mut reasons = BTreeSet::new();
    let kind = if report.complete {
        if !report.hard_blockers.is_empty() {
            reasons.insert(opaque_reason("HARD_BLOCKER_PRESENT")?);
        }
        if !review.approved {
            reasons.insert(opaque_reason("INDEPENDENT_REVIEW_REJECTED")?);
        }
        if policy.policy().require_complete_case_families && !report.case_coverage.complete {
            reasons.insert(opaque_reason("CASE_COVERAGE_INCOMPLETE")?);
        }
        if policy.policy().require_slo_success && !report.candidate_slos.mandatory_passed {
            reasons.insert(opaque_reason("MANDATORY_SLO_FAILED")?);
        }
        if report.comparison.classification == BaselineComparisonClass::Regresses {
            reasons.insert(opaque_reason("CANDIDATE_REGRESSED")?);
        }
        if report.comparison.classification == BaselineComparisonClass::Incomplete {
            reasons.insert(opaque_reason("COMPARISON_INCOMPLETE")?);
        }
        if policy.policy().require_material_value
            && !matches!(
                report.comparison.classification,
                BaselineComparisonClass::Dominates | BaselineComparisonClass::Complements
            )
        {
            reasons.insert(opaque_reason("NO_REGISTERED_MATERIAL_GAIN")?);
        }
        for delta in report.comparison.metric_deltas.values() {
            if !delta.gates.threshold_passed || !delta.gates.non_inferior {
                reasons.insert(opaque_reason("METRIC_GATE_FAILED")?);
            }
        }
        if reasons.is_empty() {
            VerdictKind::Accepted
        } else {
            VerdictKind::Rejected
        }
    } else {
        reasons.insert(opaque_reason("EVALUATION_INCOMPLETE")?);
        VerdictKind::Incomplete
    };
    if kind == VerdictKind::Accepted && (!report.hard_blockers.is_empty() || !review.approved) {
        return Err(EvalError::ReceiptMismatch);
    }
    let mut fingerprint = FingerprintBuilder::new(b"eliot-search/eval/acceptance-verdict/v1");
    fingerprint.push_digest(report.run_digest);
    fingerprint.push_digest(report.report_digest);
    fingerprint.push_digest(policy.policy().policy_digest);
    fingerprint.push_text(review.review_id.as_str());
    fingerprint.push_u64(verdict_tag(kind));
    for reason in &reasons {
        fingerprint.push_text(reason.as_str());
    }
    for blocker in &report.hard_blockers {
        fingerprint.push_u64(blocker_class_tag(blocker.class));
        fingerprint.push_text(blocker.check_id.as_str());
        fingerprint.push_text(blocker.reason.as_str());
    }
    Ok(AcceptanceVerdict {
        run_digest: report.run_digest,
        report_digest: report.report_digest,
        policy_digest: policy.policy().policy_digest,
        review_id: review.review_id.clone(),
        kind,
        reasons,
        hard_blockers: report.hard_blockers.clone(),
        verdict_digest: fingerprint.finish(),
    })
}

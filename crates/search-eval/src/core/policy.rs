//! Preregistered acceptance-policy model and validation.

use std::collections::BTreeMap;

use search_contracts::{Blake3Digest32, OpaqueId, ReceiptRef};

use crate::{EvalError, FingerprintBuilder};
use super::limits::EvalLimits;
use super::registry::ValidatedMetricRegistry;

/// One preregistered acceptance rule.
#[derive(Clone, Debug, PartialEq)]
pub struct AcceptanceRule {
    /// Registered metric identity.
    pub metric_id: OpaqueId,
    /// Absolute candidate acceptance threshold.
    pub candidate_threshold: f64,
    /// Maximum tolerated regression against the strongest applicable baseline.
    pub maximum_regression: f64,
    /// Minimum practical improvement needed to claim material gain.
    pub practical_effect: f64,
    /// Whether the metric is primary.
    pub primary: bool,
}

/// Complete acceptance policy fixed before candidate results.
#[derive(Clone, Debug, PartialEq)]
pub struct AcceptancePolicy {
    /// Stable policy identity.
    pub policy_id: OpaqueId,
    /// Monotone policy revision.
    pub revision: u64,
    /// Exact policy digest.
    pub policy_digest: Blake3Digest32,
    /// Evidence sequence at which the policy was registered.
    pub registered_sequence: u64,
    /// First candidate-result sequence, or zero if no result existed yet.
    pub first_candidate_result_sequence: u64,
    /// Evaluation producer identity.
    pub producer_id: OpaqueId,
    /// Independent policy approver identity.
    pub approver_id: OpaqueId,
    /// Registered metric rules in canonical order.
    pub rules: Vec<AcceptanceRule>,
    /// Whether every mandatory case family must have measured evidence.
    pub require_complete_case_families: bool,
    /// Whether all registered SLOs must pass.
    pub require_slo_success: bool,
    /// Whether final comparison must be DOMINATES or COMPLEMENTS.
    pub require_material_value: bool,
    /// Immutable policy approval receipt.
    pub approval_receipt: ReceiptRef,
}

/// Acceptance policy validated against an exact metric registry.
#[derive(Clone, Debug, PartialEq)]
pub struct ValidatedAcceptancePolicy {
    policy: AcceptancePolicy,
    rules: BTreeMap<OpaqueId, AcceptanceRule>,
    validation_digest: Blake3Digest32,
}

impl ValidatedAcceptancePolicy {
    /// Exact accepted policy.
    #[must_use]
    pub const fn policy(&self) -> &AcceptancePolicy {
        &self.policy
    }

    /// Reads one registered rule.
    #[must_use]
    pub fn rule(&self, metric_id: &OpaqueId) -> Option<&AcceptanceRule> {
        self.rules.get(metric_id)
    }

    /// Deterministic policy-validation fingerprint.
    #[must_use]
    pub const fn validation_digest(&self) -> Blake3Digest32 {
        self.validation_digest
    }
}

/// Validates pre-registration, independent approval, and metric references.
pub fn validate_acceptance_policy(
    policy: AcceptancePolicy,
    metrics: &ValidatedMetricRegistry,
    limits: EvalLimits,
) -> Result<ValidatedAcceptancePolicy, EvalError> {
    let limits = limits.validate()?;
    if policy.revision == 0
        || policy.rules.is_empty()
        || policy.rules.len() > limits.max_policy_rules
        || policy.approval_receipt.as_str().is_empty()
        || policy.producer_id == policy.approver_id
        || policy
            .rules
            .windows(2)
            .any(|pair| pair[0].metric_id >= pair[1].metric_id)
    {
        return Err(if policy.producer_id == policy.approver_id {
            EvalError::IndependentReviewRequired
        } else {
            EvalError::AcceptancePolicyInvalid
        });
    }
    if policy.first_candidate_result_sequence != 0
        && policy.registered_sequence >= policy.first_candidate_result_sequence
    {
        return Err(EvalError::PolicyNotPreregistered);
    }

    let mut rules = BTreeMap::new();
    let mut primary_rules = 0_usize;
    let mut fingerprint = FingerprintBuilder::new(b"eliot-search/eval/acceptance-policy/v1");
    fingerprint.push_text(policy.policy_id.as_str());
    fingerprint.push_u64(policy.revision);
    fingerprint.push_digest(policy.policy_digest);
    fingerprint.push_u64(policy.registered_sequence);
    for rule in &policy.rules {
        if !rule.candidate_threshold.is_finite()
            || !rule.maximum_regression.is_finite()
            || !rule.practical_effect.is_finite()
            || rule.maximum_regression < 0.0
            || rule.practical_effect < 0.0
            || metrics.metric(&rule.metric_id).is_none()
        {
            return Err(EvalError::AcceptancePolicyInvalid);
        }
        if rule.primary {
            primary_rules = primary_rules.saturating_add(1);
        }
        if rules.insert(rule.metric_id.clone(), rule.clone()).is_some() {
            return Err(EvalError::AcceptancePolicyInvalid);
        }
        fingerprint.push_text(rule.metric_id.as_str());
        fingerprint.push_f64(rule.candidate_threshold);
        fingerprint.push_f64(rule.maximum_regression);
        fingerprint.push_f64(rule.practical_effect);
        fingerprint.push_bool(rule.primary);
    }
    if primary_rules == 0 {
        return Err(EvalError::AcceptancePolicyInvalid);
    }
    fingerprint.push_bool(policy.require_complete_case_families);
    fingerprint.push_bool(policy.require_slo_success);
    fingerprint.push_bool(policy.require_material_value);
    Ok(ValidatedAcceptancePolicy {
        policy,
        rules,
        validation_digest: fingerprint.finish(),
    })
}

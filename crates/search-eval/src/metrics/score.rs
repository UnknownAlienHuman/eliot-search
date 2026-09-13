//! Attempt-level metric observation validation and scoring.

use std::collections::BTreeMap;

use search_contracts::{Blake3Digest32, OpaqueId, ReceiptRef};

use crate::{
    AttemptStatus, EvalError, EvalLimits, FingerprintBuilder, MetricDefinition,
    MissingValuePolicy, ValidatedCaseEvidence, ValidatedMetricRegistry,
};

use super::support::{
    attempt_status_tag, canonicalize_receipts, conservative_failure_value,
    fingerprint_metric_value,
};

/// Terminal state of one caller-supplied metric observation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum MetricObservationState {
    /// Finite numerator and positive denominator were measured.
    Measured,
    /// The metric was in scope but its value was missing.
    Missing,
    /// The underlying attempt failed for this metric.
    Failed,
    /// The baseline or capability could not produce this metric.
    Unavailable,
}

/// One metric observation derived from immutable raw evidence and an oracle.
///
/// `Measured` requires finite `numerator` and a finite positive `denominator`.
/// Every other state requires both numeric fields to be absent. Evidence
/// references are content-free immutable receipts, not raw output locations.
#[derive(Clone, Debug, PartialEq)]
pub struct MetricObservation {
    /// Registered metric identity.
    pub metric_id: OpaqueId,
    /// Observation state.
    pub state: MetricObservationState,
    /// Measured numerator.
    pub numerator: Option<f64>,
    /// Measured denominator.
    pub denominator: Option<f64>,
    /// Immutable content-free evidence references.
    pub evidence_refs: Vec<ReceiptRef>,
}

/// One case/attempt-level metric value.
#[derive(Clone, Debug, PartialEq)]
pub struct CaseMetricValue {
    /// Registered metric identity.
    pub metric_id: OpaqueId,
    /// Computed value when the registered missing-value policy permits one.
    pub value: Option<f64>,
    /// Measured numerator retained for safe aggregation.
    pub numerator: Option<f64>,
    /// Measured denominator retained for safe aggregation.
    pub denominator: Option<f64>,
    /// Number of measured observations represented by this value.
    pub measured_count: u64,
    /// Number of missing or unavailable observations.
    pub missing_count: u64,
    /// Number of failed observations.
    pub failed_count: u64,
    /// Whether this metric is complete for the exact attempt.
    pub complete: bool,
    /// Immutable content-free evidence references.
    pub evidence_refs: Vec<ReceiptRef>,
}

/// Exact metric set for one validated attempt.
#[derive(Clone, Debug, PartialEq)]
pub struct CaseMetricSet {
    /// Frozen run digest.
    pub run_digest: Blake3Digest32,
    /// Stable control-case identity.
    pub case_id: OpaqueId,
    /// Stable baseline/candidate identity.
    pub baseline_id: OpaqueId,
    /// Exact immutable attempt digest.
    pub attempt_digest: Blake3Digest32,
    /// Attempt terminal state.
    pub status: AttemptStatus,
    /// Whether this is an excluded warm-up observation.
    pub warmup: bool,
    /// Metric values keyed by registered metric identity.
    pub metrics: BTreeMap<OpaqueId, CaseMetricValue>,
    /// Whether the complete registered metric denominator was measured.
    pub complete: bool,
    /// Deterministic metric-set digest.
    pub metric_set_digest: Blake3Digest32,
}

/// Scores one validated attempt against the preregistered metric registry.
///
/// Metric observations must be derived by a dev/test driver from the immutable
/// raw-output reference and private oracle. The evaluation package validates
/// their shape, registry membership, missing-value semantics, and evidence
/// accounting; it does not receive raw source or query content.
pub fn score_case(
    evidence: &ValidatedCaseEvidence,
    observations: Vec<MetricObservation>,
    registry: &ValidatedMetricRegistry,
    limits: EvalLimits,
) -> Result<CaseMetricSet, EvalError> {
    let limits = limits.validate()?;
    if observations.len() > limits.max_metrics {
        return Err(EvalError::BudgetExceeded);
    }

    let attempt = evidence.evidence();
    let mut by_metric = BTreeMap::new();
    for mut observation in observations {
        if registry.metric(&observation.metric_id).is_none()
            || by_metric.contains_key(&observation.metric_id)
        {
            return Err(if registry.metric(&observation.metric_id).is_none() {
                EvalError::MetricRegistryInvalid
            } else {
                EvalError::DuplicateMetric
            });
        }
        canonicalize_receipts(&mut observation.evidence_refs, limits.max_receipts)?;
        by_metric.insert(observation.metric_id.clone(), observation);
    }

    let mut metrics = BTreeMap::new();
    for definition in &registry.registry().definitions {
        let observation = by_metric.remove(&definition.metric_id);
        let value = score_observation(
            definition,
            observation,
            attempt.status,
            &attempt.invocation_receipt,
            limits.max_receipts,
        )?;
        metrics.insert(definition.metric_id.clone(), value);
    }
    if !by_metric.is_empty() {
        return Err(EvalError::MetricRegistryInvalid);
    }

    let complete =
        attempt.status == AttemptStatus::Success && metrics.values().all(|metric| metric.complete);
    let mut fingerprint = FingerprintBuilder::new(b"eliot-search/eval/case-metrics/v1");
    fingerprint.push_digest(attempt.run_digest);
    fingerprint.push_text(attempt.case_id.as_str());
    fingerprint.push_text(attempt.baseline_id.as_str());
    fingerprint.push_digest(attempt.attempt_digest);
    fingerprint.push_u64(attempt_status_tag(attempt.status));
    fingerprint.push_bool(attempt.warmup);
    fingerprint.push_digest(evidence.validation_digest());
    for metric in metrics.values() {
        fingerprint_metric_value(&mut fingerprint, metric);
    }
    fingerprint.push_bool(complete);

    Ok(CaseMetricSet {
        run_digest: attempt.run_digest,
        case_id: attempt.case_id.clone(),
        baseline_id: attempt.baseline_id.clone(),
        attempt_digest: attempt.attempt_digest,
        status: attempt.status,
        warmup: attempt.warmup,
        metrics,
        complete,
        metric_set_digest: fingerprint.finish(),
    })
}

fn score_observation(
    definition: &MetricDefinition,
    observation: Option<MetricObservation>,
    attempt_status: AttemptStatus,
    invocation_receipt: &ReceiptRef,
    max_receipts: usize,
) -> Result<CaseMetricValue, EvalError> {
    let Some(observation) = observation else {
        return missing_metric(
            definition,
            MetricObservationState::Missing,
            vec![invocation_receipt.clone()],
        );
    };
    let mut evidence_refs = observation.evidence_refs;
    evidence_refs.push(invocation_receipt.clone());
    canonicalize_receipts(&mut evidence_refs, max_receipts)?;

    match observation.state {
        MetricObservationState::Measured => {
            if !matches!(
                attempt_status,
                AttemptStatus::Success | AttemptStatus::Partial
            ) {
                return Err(EvalError::EvidenceStatusInvalid);
            }
            let numerator = observation.numerator.ok_or(EvalError::MetricUnavailable)?;
            let denominator = observation
                .denominator
                .ok_or(EvalError::MetricUnavailable)?;
            if !numerator.is_finite() || !denominator.is_finite() || denominator <= 0.0 {
                return Err(EvalError::MetricUnavailable);
            }
            let value = numerator / denominator;
            if !value.is_finite() {
                return Err(EvalError::MetricUnavailable);
            }
            Ok(CaseMetricValue {
                metric_id: definition.metric_id.clone(),
                value: Some(value),
                numerator: Some(numerator),
                denominator: Some(denominator),
                measured_count: 1,
                missing_count: 0,
                failed_count: 0,
                complete: attempt_status == AttemptStatus::Success,
                evidence_refs,
            })
        }
        state => {
            if observation.numerator.is_some() || observation.denominator.is_some() {
                return Err(EvalError::MetricUnavailable);
            }
            missing_metric(definition, state, evidence_refs)
        }
    }
}

fn missing_metric(
    definition: &MetricDefinition,
    state: MetricObservationState,
    evidence_refs: Vec<ReceiptRef>,
) -> Result<CaseMetricValue, EvalError> {
    let failed_count = u64::from(state == MetricObservationState::Failed);
    let missing_count = u64::from(state != MetricObservationState::Failed);
    match definition.missing_value_policy {
        MissingValuePolicy::FailRun => Err(EvalError::MetricUnavailable),
        MissingValuePolicy::CountAsFailure => Ok(CaseMetricValue {
            metric_id: definition.metric_id.clone(),
            value: Some(conservative_failure_value(definition.direction)),
            numerator: None,
            denominator: None,
            measured_count: 0,
            missing_count,
            failed_count,
            complete: false,
            evidence_refs,
        }),
        MissingValuePolicy::UnavailableAndIncomplete => Ok(CaseMetricValue {
            metric_id: definition.metric_id.clone(),
            value: None,
            numerator: None,
            denominator: None,
            measured_count: 0,
            missing_count,
            failed_count,
            complete: false,
            evidence_refs,
        }),
    }
}

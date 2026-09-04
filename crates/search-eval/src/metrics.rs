//! Deterministic metric scoring, aggregation, comparison, SLOs, and resources.
//!
//! This module consumes already-validated immutable attempt evidence and
//! caller-supplied metric observations. It never reads raw source/query output,
//! never selects criteria after a run, and never drops failed, partial,
//! cancelled, timed-out, or unavailable attempts from denominators.

use std::collections::{BTreeMap, BTreeSet};

use search_contracts::{Blake3Digest32, OpaqueId, ReceiptRef};

use crate::{
    AttemptStatus, EvalError, EvalLimits, FingerprintBuilder, FrozenRunManifest,
    MetricDefinition, MetricDirection, MissingValuePolicy, ResourceSample,
    ValidatedAcceptancePolicy, ValidatedCaseEvidence, ValidatedMetricRegistry,
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

    let complete = attempt.status == AttemptStatus::Success
        && metrics.values().all(|metric| metric.complete);
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
            if !matches!(attempt_status, AttemptStatus::Success | AttemptStatus::Partial) {
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

fn conservative_failure_value(direction: MetricDirection) -> f64 {
    match direction {
        MetricDirection::HigherIsBetter => -f64::MAX,
        MetricDirection::LowerIsBetter | MetricDirection::ZeroTolerance => f64::MAX,
    }
}

/// Aggregated registered metric for one baseline/candidate.
#[derive(Clone, Debug, PartialEq)]
pub struct AggregatedMetric {
    /// Registered metric identity.
    pub metric_id: OpaqueId,
    /// Aggregate value when available.
    pub value: Option<f64>,
    /// Sum of measured numerators when no failure sentinel was required.
    pub numerator: Option<f64>,
    /// Sum of measured denominators when no failure sentinel was required.
    pub denominator: Option<f64>,
    /// Number of measured attempt observations.
    pub measured_count: u64,
    /// Number of missing or unavailable observations.
    pub missing_count: u64,
    /// Number of failed observations.
    pub failed_count: u64,
    /// Whether every measured attempt supplied a complete value.
    pub complete: bool,
    /// Canonical immutable evidence references.
    pub evidence_refs: Vec<ReceiptRef>,
}

/// Complete aggregate metric report for one A/B/C identity.
#[derive(Clone, Debug, PartialEq)]
pub struct BaselineMetricReport {
    /// Frozen run digest.
    pub run_digest: Blake3Digest32,
    /// Stable baseline/candidate identity.
    pub baseline_id: OpaqueId,
    /// Registered aggregate metrics.
    pub metrics: BTreeMap<OpaqueId, AggregatedMetric>,
    /// Number of distinct measured control cases.
    pub measured_case_count: usize,
    /// Number of measured non-warm-up attempts.
    pub measured_attempt_count: usize,
    /// Number of failed, cancelled, or timed-out attempts.
    pub failed_attempt_count: usize,
    /// Number of unavailable attempts.
    pub unavailable_attempt_count: usize,
    /// Whether every measured attempt and registered metric is complete.
    pub complete: bool,
    /// Deterministic report digest.
    pub report_digest: Blake3Digest32,
}

/// Aggregates one baseline/candidate's non-warm-up metric sets.
///
/// Failed and unavailable attempts remain in the denominator. Warm-up attempts
/// are excluded explicitly rather than being mixed with measured evidence.
pub fn aggregate_block(
    run: &FrozenRunManifest,
    baseline_id: &OpaqueId,
    case_metrics: &[CaseMetricSet],
    registry: &ValidatedMetricRegistry,
    limits: EvalLimits,
) -> Result<BaselineMetricReport, EvalError> {
    let limits = limits.validate()?;
    let max_attempts = limits
        .max_cases
        .checked_mul(usize::try_from(limits.max_repetitions).unwrap_or(usize::MAX))
        .ok_or(EvalError::BudgetExceeded)?;
    if case_metrics.is_empty() || case_metrics.len() > max_attempts {
        return Err(EvalError::BudgetExceeded);
    }

    let measured = case_metrics
        .iter()
        .filter(|metrics| !metrics.warmup)
        .collect::<Vec<_>>();
    if measured.is_empty() {
        return Err(EvalError::MetricUnavailable);
    }
    let mut attempts = BTreeSet::new();
    let mut cases = BTreeSet::new();
    for item in &measured {
        if item.run_digest != run.run_digest() || &item.baseline_id != baseline_id {
            return Err(EvalError::AggregateIdentityMismatch);
        }
        if !attempts.insert(item.attempt_digest) {
            return Err(EvalError::AttemptConflict);
        }
        cases.insert(item.case_id.clone());
    }

    let mut aggregate = BTreeMap::new();
    for definition in &registry.registry().definitions {
        let metric = aggregate_metric(definition, &measured, limits.max_receipts)?;
        aggregate.insert(definition.metric_id.clone(), metric);
    }

    let failed_attempt_count = measured
        .iter()
        .filter(|item| {
            matches!(
                item.status,
                AttemptStatus::Failed | AttemptStatus::Cancelled | AttemptStatus::TimedOut
            )
        })
        .count();
    let unavailable_attempt_count = measured
        .iter()
        .filter(|item| item.status == AttemptStatus::Unavailable)
        .count();
    let complete = measured.iter().all(|item| item.complete)
        && aggregate.values().all(|metric| metric.complete);

    let mut fingerprint = FingerprintBuilder::new(b"eliot-search/eval/baseline-metrics/v1");
    fingerprint.push_digest(run.run_digest());
    fingerprint.push_text(baseline_id.as_str());
    fingerprint.push_u64(u64::try_from(cases.len()).map_err(|_| EvalError::ContractExhausted)?);
    fingerprint.push_u64(
        u64::try_from(measured.len()).map_err(|_| EvalError::ContractExhausted)?,
    );
    fingerprint.push_u64(
        u64::try_from(failed_attempt_count).map_err(|_| EvalError::ContractExhausted)?,
    );
    fingerprint.push_u64(
        u64::try_from(unavailable_attempt_count)
            .map_err(|_| EvalError::ContractExhausted)?,
    );
    for metric in aggregate.values() {
        fingerprint_aggregate(&mut fingerprint, metric);
    }
    fingerprint.push_bool(complete);

    Ok(BaselineMetricReport {
        run_digest: run.run_digest(),
        baseline_id: baseline_id.clone(),
        metrics: aggregate,
        measured_case_count: cases.len(),
        measured_attempt_count: measured.len(),
        failed_attempt_count,
        unavailable_attempt_count,
        complete,
        report_digest: fingerprint.finish(),
    })
}

fn aggregate_metric(
    definition: &MetricDefinition,
    case_metrics: &[&CaseMetricSet],
    max_receipts: usize,
) -> Result<AggregatedMetric, EvalError> {
    let mut numerator = 0.0_f64;
    let mut denominator = 0.0_f64;
    let mut measured_count = 0_u64;
    let mut missing_count = 0_u64;
    let mut failed_count = 0_u64;
    let mut complete = true;
    let mut failure_sentinel_required = false;
    let mut evidence_refs = Vec::new();

    for item in case_metrics {
        let metric = item
            .metrics
            .get(&definition.metric_id)
            .ok_or(EvalError::MetricUnavailable)?;
        measured_count = measured_count
            .checked_add(metric.measured_count)
            .ok_or(EvalError::ContractExhausted)?;
        missing_count = missing_count
            .checked_add(metric.missing_count)
            .ok_or(EvalError::ContractExhausted)?;
        failed_count = failed_count
            .checked_add(metric.failed_count)
            .ok_or(EvalError::ContractExhausted)?;
        complete &= metric.complete;
        evidence_refs.extend(metric.evidence_refs.iter().cloned());

        match (metric.numerator, metric.denominator, metric.value) {
            (Some(item_numerator), Some(item_denominator), Some(_)) => {
                numerator += item_numerator;
                denominator += item_denominator;
                if !numerator.is_finite() || !denominator.is_finite() {
                    return Err(EvalError::MetricUnavailable);
                }
            }
            (None, None, Some(_)) => failure_sentinel_required = true,
            (None, None, None) => {}
            _ => return Err(EvalError::MetricUnavailable),
        }
    }
    canonicalize_receipts(&mut evidence_refs, max_receipts)?;

    if definition.missing_value_policy == MissingValuePolicy::FailRun
        && (missing_count != 0 || failed_count != 0)
    {
        return Err(EvalError::MetricUnavailable);
    }
    let (value, aggregate_numerator, aggregate_denominator) = if failure_sentinel_required {
        (
            Some(conservative_failure_value(definition.direction)),
            None,
            None,
        )
    } else if denominator > 0.0 {
        let value = numerator / denominator;
        if !value.is_finite() {
            return Err(EvalError::MetricUnavailable);
        }
        (Some(value), Some(numerator), Some(denominator))
    } else {
        (None, None, None)
    };
    complete &= missing_count == 0
        && failed_count == 0
        && measured_count
            == u64::try_from(case_metrics.len()).map_err(|_| EvalError::ContractExhausted)?
        && value.is_some();

    Ok(AggregatedMetric {
        metric_id: definition.metric_id.clone(),
        value,
        numerator: aggregate_numerator,
        denominator: aggregate_denominator,
        measured_count,
        missing_count,
        failed_count,
        complete,
        evidence_refs,
    })
}

/// Closed preregistered A/B/C comparison classification.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum BaselineComparisonClass {
    /// Candidate C passes every gate and materially improves every primary metric.
    Dominates,
    /// Candidate C is non-inferior and materially improves at least one metric.
    Complements,
    /// Candidate C is non-inferior but has no preregistered material gain.
    NonInferiorWithoutMaterialGain,
    /// Candidate C violates an absolute threshold or non-inferiority rule.
    Regresses,
    /// One or more registered values or denominators are unavailable.
    Incomplete,
}

/// One preregistered metric comparison against the strongest applicable baseline.
#[derive(Clone, Debug, PartialEq)]
pub struct MetricDelta {
    /// Registered metric identity.
    pub metric_id: OpaqueId,
    /// Baseline A value.
    pub baseline_a_value: Option<f64>,
    /// Baseline B value.
    pub baseline_b_value: Option<f64>,
    /// Candidate C value.
    pub candidate_value: Option<f64>,
    /// Strongest baseline value under the metric direction.
    pub strongest_baseline_value: Option<f64>,
    /// Signed improvement; positive is always better.
    pub improvement: Option<f64>,
    /// Whether the absolute candidate threshold passed.
    pub threshold_passed: bool,
    /// Whether maximum preregistered regression was respected.
    pub non_inferior: bool,
    /// Whether preregistered practical effect was reached.
    pub material_gain: bool,
    /// Whether all three coherent complete metric values were present.
    pub complete: bool,
}

/// Complete deterministic A/B/C comparison.
#[derive(Clone, Debug, PartialEq)]
pub struct BaselineComparison {
    /// Frozen run digest.
    pub run_digest: Blake3Digest32,
    /// Baseline A identity.
    pub baseline_a: OpaqueId,
    /// Baseline B identity.
    pub baseline_b: OpaqueId,
    /// Candidate C identity.
    pub candidate_c: OpaqueId,
    /// Closed relationship classification.
    pub classification: BaselineComparisonClass,
    /// Metric deltas keyed by preregistered metric identity.
    pub metric_deltas: BTreeMap<OpaqueId, MetricDelta>,
    /// Deterministic comparison digest.
    pub comparison_digest: Blake3Digest32,
}

/// Compares exact A/B/C reports under the frozen acceptance policy.
pub fn compare_abc(
    baseline_a: &BaselineMetricReport,
    baseline_b: &BaselineMetricReport,
    candidate_c: &BaselineMetricReport,
    metrics: &ValidatedMetricRegistry,
    policy: &ValidatedAcceptancePolicy,
) -> Result<BaselineComparison, EvalError> {
    if baseline_a.run_digest != baseline_b.run_digest
        || baseline_a.run_digest != candidate_c.run_digest
        || baseline_a.baseline_id == baseline_b.baseline_id
        || baseline_a.baseline_id == candidate_c.baseline_id
        || baseline_b.baseline_id == candidate_c.baseline_id
    {
        return Err(EvalError::AggregateIdentityMismatch);
    }

    let mut deltas = BTreeMap::new();
    let mut primary_count = 0_usize;
    let mut primary_material = 0_usize;
    let mut any_material = false;
    let mut any_regression = false;
    let mut any_incomplete = false;

    for rule in &policy.policy().rules {
        let definition = metrics
            .metric(&rule.metric_id)
            .ok_or(EvalError::AcceptancePolicyInvalid)?;
        let a = baseline_a.metrics.get(&rule.metric_id);
        let b = baseline_b.metrics.get(&rule.metric_id);
        let c = candidate_c.metrics.get(&rule.metric_id);
        let complete = a.is_some_and(|value| value.complete && value.value.is_some())
            && b.is_some_and(|value| value.complete && value.value.is_some())
            && c.is_some_and(|value| value.complete && value.value.is_some());

        let baseline_a_value = a.and_then(|value| value.value);
        let baseline_b_value = b.and_then(|value| value.value);
        let candidate_value = c.and_then(|value| value.value);
        let strongest_baseline_value = match (baseline_a_value, baseline_b_value) {
            (Some(left), Some(right)) => Some(strongest_baseline(
                definition.direction,
                left,
                right,
            )),
            _ => None,
        };
        let improvement = candidate_value.zip(strongest_baseline_value).map(
            |(candidate, strongest)| improvement_amount(
                definition.direction,
                candidate,
                strongest,
            ),
        );
        let threshold_passed = candidate_value.is_some_and(|candidate| {
            passes_threshold(definition.direction, candidate, rule.candidate_threshold)
        });
        let non_inferior = improvement.is_some_and(|value| {
            value >= -rule.maximum_regression
        });
        let material_gain = improvement.is_some_and(|value| value >= rule.practical_effect);

        if rule.primary {
            primary_count = primary_count.saturating_add(1);
            if material_gain {
                primary_material = primary_material.saturating_add(1);
            }
        }
        any_material |= material_gain;
        any_regression |= !threshold_passed || !non_inferior;
        any_incomplete |= !complete;
        deltas.insert(
            rule.metric_id.clone(),
            MetricDelta {
                metric_id: rule.metric_id.clone(),
                baseline_a_value,
                baseline_b_value,
                candidate_value,
                strongest_baseline_value,
                improvement,
                threshold_passed,
                non_inferior,
                material_gain,
                complete,
            },
        );
    }

    let classification = if any_incomplete {
        BaselineComparisonClass::Incomplete
    } else if any_regression {
        BaselineComparisonClass::Regresses
    } else if primary_count > 0 && primary_material == primary_count {
        BaselineComparisonClass::Dominates
    } else if any_material {
        BaselineComparisonClass::Complements
    } else {
        BaselineComparisonClass::NonInferiorWithoutMaterialGain
    };

    let mut fingerprint = FingerprintBuilder::new(b"eliot-search/eval/abc-comparison/v1");
    fingerprint.push_digest(baseline_a.run_digest);
    fingerprint.push_text(baseline_a.baseline_id.as_str());
    fingerprint.push_text(baseline_b.baseline_id.as_str());
    fingerprint.push_text(candidate_c.baseline_id.as_str());
    fingerprint.push_digest(policy.validation_digest());
    for delta in deltas.values() {
        fingerprint.push_text(delta.metric_id.as_str());
        fingerprint_optional_f64(&mut fingerprint, delta.baseline_a_value);
        fingerprint_optional_f64(&mut fingerprint, delta.baseline_b_value);
        fingerprint_optional_f64(&mut fingerprint, delta.candidate_value);
        fingerprint_optional_f64(&mut fingerprint, delta.strongest_baseline_value);
        fingerprint_optional_f64(&mut fingerprint, delta.improvement);
        fingerprint.push_bool(delta.threshold_passed);
        fingerprint.push_bool(delta.non_inferior);
        fingerprint.push_bool(delta.material_gain);
        fingerprint.push_bool(delta.complete);
    }
    fingerprint.push_u64(comparison_class_tag(classification));

    Ok(BaselineComparison {
        run_digest: baseline_a.run_digest,
        baseline_a: baseline_a.baseline_id.clone(),
        baseline_b: baseline_b.baseline_id.clone(),
        candidate_c: candidate_c.baseline_id.clone(),
        classification,
        metric_deltas: deltas,
        comparison_digest: fingerprint.finish(),
    })
}

fn strongest_baseline(direction: MetricDirection, left: f64, right: f64) -> f64 {
    match direction {
        MetricDirection::HigherIsBetter => left.max(right),
        MetricDirection::LowerIsBetter | MetricDirection::ZeroTolerance => left.min(right),
    }
}

fn improvement_amount(direction: MetricDirection, candidate: f64, baseline: f64) -> f64 {
    match direction {
        MetricDirection::HigherIsBetter => candidate - baseline,
        MetricDirection::LowerIsBetter | MetricDirection::ZeroTolerance => baseline - candidate,
    }
}

fn passes_threshold(direction: MetricDirection, candidate: f64, threshold: f64) -> bool {
    candidate.is_finite()
        && threshold.is_finite()
        && match direction {
            MetricDirection::HigherIsBetter => candidate >= threshold,
            MetricDirection::LowerIsBetter | MetricDirection::ZeroTolerance => {
                candidate <= threshold
            }
        }
}

/// Direction of one preregistered SLO threshold.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SloDirection {
    /// Observed value must be no greater than the threshold.
    AtMost,
    /// Observed value must be no less than the threshold.
    AtLeast,
}

/// One preregistered SLO definition.
#[derive(Clone, Debug, PartialEq)]
pub struct SloDefinition {
    /// Stable SLO identity.
    pub slo_id: OpaqueId,
    /// Registered metric used by the SLO.
    pub metric_id: OpaqueId,
    /// Threshold direction.
    pub direction: SloDirection,
    /// Finite threshold.
    pub threshold: f64,
    /// Minimum measured samples needed for a claim.
    pub minimum_samples: u64,
    /// Whether failure or unavailability blocks acceptance.
    pub mandatory: bool,
}

/// Closed SLO outcome.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SloStatus {
    /// Complete evidence satisfies the threshold.
    Pass,
    /// Complete evidence violates the threshold.
    Fail,
    /// Evidence is missing, incomplete, or below the sample floor.
    Unavailable,
}

/// One SLO result.
#[derive(Clone, Debug, PartialEq)]
pub struct SloOutcome {
    /// Stable SLO identity.
    pub slo_id: OpaqueId,
    /// Registered metric identity.
    pub metric_id: OpaqueId,
    /// Threshold direction.
    pub direction: SloDirection,
    /// Exact preregistered threshold.
    pub threshold: f64,
    /// Aggregate observed value when available.
    pub observed_value: Option<f64>,
    /// Measured sample count.
    pub measured_samples: u64,
    /// Whether this SLO is mandatory.
    pub mandatory: bool,
    /// Closed outcome.
    pub status: SloStatus,
}

/// Complete SLO report for one candidate/baseline.
#[derive(Clone, Debug, PartialEq)]
pub struct SloReport {
    /// Frozen run digest.
    pub run_digest: Blake3Digest32,
    /// Baseline/candidate identity.
    pub baseline_id: OpaqueId,
    /// SLO outcomes ordered by SLO identity.
    pub outcomes: Vec<SloOutcome>,
    /// Whether every mandatory SLO passed.
    pub mandatory_passed: bool,
    /// Whether every SLO had complete sufficient evidence.
    pub complete: bool,
    /// Deterministic report digest.
    pub report_digest: Blake3Digest32,
}

/// Evaluates preregistered SLOs against one exact aggregate report.
pub fn evaluate_candidate_slos(
    report: &BaselineMetricReport,
    mut definitions: Vec<SloDefinition>,
    limits: EvalLimits,
) -> Result<SloReport, EvalError> {
    let limits = limits.validate()?;
    if definitions.is_empty() || definitions.len() > limits.max_policy_rules {
        return Err(EvalError::BudgetExceeded);
    }
    definitions.sort_by(|left, right| left.slo_id.cmp(&right.slo_id));
    if definitions
        .windows(2)
        .any(|pair| pair[0].slo_id == pair[1].slo_id)
    {
        return Err(EvalError::AcceptancePolicyInvalid);
    }

    let mut outcomes = Vec::with_capacity(definitions.len());
    for definition in definitions {
        if !definition.threshold.is_finite() || definition.minimum_samples == 0 {
            return Err(EvalError::AcceptancePolicyInvalid);
        }
        let metric = report.metrics.get(&definition.metric_id);
        let observed_value = metric.and_then(|metric| metric.value);
        let measured_samples = metric.map_or(0, |metric| metric.measured_count);
        let status = if metric.is_none_or(|metric| !metric.complete)
            || observed_value.is_none()
            || measured_samples < definition.minimum_samples
        {
            SloStatus::Unavailable
        } else {
            let observed = observed_value.expect("availability checked");
            let passed = match definition.direction {
                SloDirection::AtMost => observed <= definition.threshold,
                SloDirection::AtLeast => observed >= definition.threshold,
            };
            if passed {
                SloStatus::Pass
            } else {
                SloStatus::Fail
            }
        };
        outcomes.push(SloOutcome {
            slo_id: definition.slo_id,
            metric_id: definition.metric_id,
            direction: definition.direction,
            threshold: definition.threshold,
            observed_value,
            measured_samples,
            mandatory: definition.mandatory,
            status,
        });
    }

    let mandatory_passed = outcomes
        .iter()
        .all(|outcome| !outcome.mandatory || outcome.status == SloStatus::Pass);
    let complete = outcomes
        .iter()
        .all(|outcome| outcome.status != SloStatus::Unavailable);
    let mut fingerprint = FingerprintBuilder::new(b"eliot-search/eval/slo-report/v1");
    fingerprint.push_digest(report.run_digest);
    fingerprint.push_text(report.baseline_id.as_str());
    for outcome in &outcomes {
        fingerprint.push_text(outcome.slo_id.as_str());
        fingerprint.push_text(outcome.metric_id.as_str());
        fingerprint.push_u64(slo_direction_tag(outcome.direction));
        fingerprint.push_f64(outcome.threshold);
        fingerprint_optional_f64(&mut fingerprint, outcome.observed_value);
        fingerprint.push_u64(outcome.measured_samples);
        fingerprint.push_bool(outcome.mandatory);
        fingerprint.push_u64(slo_status_tag(outcome.status));
    }
    fingerprint.push_bool(mandatory_passed);
    fingerprint.push_bool(complete);

    Ok(SloReport {
        run_digest: report.run_digest,
        baseline_id: report.baseline_id.clone(),
        outcomes,
        mandatory_passed,
        complete,
        report_digest: fingerprint.finish(),
    })
}

/// Resource lane kept separate during aggregation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ResourceLane {
    /// Excluded warm-up attempts.
    Warmup,
    /// Measured attempts.
    Measured,
}

/// Deterministic resource report for one baseline and lane.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceReport {
    /// Frozen run digest.
    pub run_digest: Blake3Digest32,
    /// Baseline/candidate identity.
    pub baseline_id: OpaqueId,
    /// Warm-up or measured lane.
    pub lane: ResourceLane,
    /// Number of represented attempts.
    pub attempt_count: usize,
    /// Number of resource samples.
    pub sample_count: usize,
    /// Sum of per-attempt CPU deltas.
    pub cpu_millis: u64,
    /// Maximum observed resident-memory bytes.
    pub peak_memory_bytes: u64,
    /// Sum of per-attempt read-byte deltas.
    pub read_bytes: u64,
    /// Sum of per-attempt write-byte deltas.
    pub write_bytes: u64,
    /// Number of attempts without a usable sample series.
    pub missing_sample_attempts: usize,
    /// Whether every selected attempt succeeded with a coherent sample series.
    pub complete: bool,
    /// Immutable attempt evidence references.
    pub evidence_refs: Vec<ReceiptRef>,
    /// Deterministic report digest.
    pub report_digest: Blake3Digest32,
}

/// Computes the measured-lane resource report.
pub fn compute_resource_report(
    run: &FrozenRunManifest,
    baseline_id: &OpaqueId,
    evidence: &[ValidatedCaseEvidence],
    limits: EvalLimits,
) -> Result<ResourceReport, EvalError> {
    compute_resource_report_for_lane(
        run,
        baseline_id,
        evidence,
        ResourceLane::Measured,
        limits,
    )
}

/// Computes one exact warm-up or measured resource lane.
pub fn compute_resource_report_for_lane(
    run: &FrozenRunManifest,
    baseline_id: &OpaqueId,
    evidence: &[ValidatedCaseEvidence],
    lane: ResourceLane,
    limits: EvalLimits,
) -> Result<ResourceReport, EvalError> {
    let limits = limits.validate()?;
    if evidence.len() > limits.max_cases.saturating_mul(3) {
        return Err(EvalError::BudgetExceeded);
    }
    let selected = evidence
        .iter()
        .filter(|item| {
            item.evidence().warmup == (lane == ResourceLane::Warmup)
                && &item.evidence().baseline_id == baseline_id
        })
        .collect::<Vec<_>>();
    if selected.is_empty() {
        return Err(EvalError::ResourceReportIncomplete);
    }

    let mut sample_count = 0_usize;
    let mut cpu_millis = 0_u64;
    let mut peak_memory_bytes = 0_u64;
    let mut read_bytes = 0_u64;
    let mut write_bytes = 0_u64;
    let mut missing_sample_attempts = 0_usize;
    let mut complete = true;
    let mut attempt_ids = BTreeSet::new();
    let mut evidence_refs = Vec::new();

    for item in &selected {
        let attempt = item.evidence();
        if attempt.run_digest != run.run_digest()
            || &attempt.baseline_id != baseline_id
            || !attempt_ids.insert(attempt.attempt_digest)
        {
            return Err(EvalError::AggregateIdentityMismatch);
        }
        evidence_refs.push(attempt.invocation_receipt.clone());
        complete &= attempt.status == AttemptStatus::Success;
        if attempt.resource_samples.is_empty() {
            missing_sample_attempts = missing_sample_attempts.saturating_add(1);
            complete = false;
            continue;
        }
        sample_count = sample_count
            .checked_add(attempt.resource_samples.len())
            .ok_or(EvalError::BudgetExceeded)?;
        if sample_count > limits.max_resource_samples {
            return Err(EvalError::BudgetExceeded);
        }
        let summary = summarize_samples(&attempt.resource_samples)?;
        cpu_millis = cpu_millis
            .checked_add(summary.cpu_millis)
            .ok_or(EvalError::ContractExhausted)?;
        read_bytes = read_bytes
            .checked_add(summary.read_bytes)
            .ok_or(EvalError::ContractExhausted)?;
        write_bytes = write_bytes
            .checked_add(summary.write_bytes)
            .ok_or(EvalError::ContractExhausted)?;
        peak_memory_bytes = peak_memory_bytes.max(summary.peak_memory_bytes);
    }
    canonicalize_receipts(&mut evidence_refs, limits.max_receipts)?;

    let mut fingerprint = FingerprintBuilder::new(b"eliot-search/eval/resource-report/v1");
    fingerprint.push_digest(run.run_digest());
    fingerprint.push_text(baseline_id.as_str());
    fingerprint.push_u64(resource_lane_tag(lane));
    fingerprint.push_u64(
        u64::try_from(selected.len()).map_err(|_| EvalError::ContractExhausted)?,
    );
    fingerprint.push_u64(
        u64::try_from(sample_count).map_err(|_| EvalError::ContractExhausted)?,
    );
    fingerprint.push_u64(cpu_millis);
    fingerprint.push_u64(peak_memory_bytes);
    fingerprint.push_u64(read_bytes);
    fingerprint.push_u64(write_bytes);
    fingerprint.push_u64(
        u64::try_from(missing_sample_attempts)
            .map_err(|_| EvalError::ContractExhausted)?,
    );
    for reference in &evidence_refs {
        fingerprint.push_text(reference.as_str());
    }
    fingerprint.push_bool(complete);

    Ok(ResourceReport {
        run_digest: run.run_digest(),
        baseline_id: baseline_id.clone(),
        lane,
        attempt_count: selected.len(),
        sample_count,
        cpu_millis,
        peak_memory_bytes,
        read_bytes,
        write_bytes,
        missing_sample_attempts,
        complete,
        evidence_refs,
        report_digest: fingerprint.finish(),
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ResourceSummary {
    cpu_millis: u64,
    peak_memory_bytes: u64,
    read_bytes: u64,
    write_bytes: u64,
}

fn summarize_samples(samples: &[ResourceSample]) -> Result<ResourceSummary, EvalError> {
    let first = samples.first().ok_or(EvalError::ResourceReportIncomplete)?;
    let last = samples.last().ok_or(EvalError::ResourceReportIncomplete)?;
    if samples.windows(2).any(|pair| {
        pair[0].tick >= pair[1].tick
            || pair[0].cpu_millis > pair[1].cpu_millis
            || pair[0].read_bytes > pair[1].read_bytes
            || pair[0].write_bytes > pair[1].write_bytes
    }) {
        return Err(EvalError::ResourceReportIncomplete);
    }
    Ok(ResourceSummary {
        cpu_millis: last
            .cpu_millis
            .checked_sub(first.cpu_millis)
            .ok_or(EvalError::ResourceReportIncomplete)?,
        peak_memory_bytes: samples
            .iter()
            .map(|sample| sample.memory_bytes)
            .max()
            .unwrap_or(0),
        read_bytes: last
            .read_bytes
            .checked_sub(first.read_bytes)
            .ok_or(EvalError::ResourceReportIncomplete)?,
        write_bytes: last
            .write_bytes
            .checked_sub(first.write_bytes)
            .ok_or(EvalError::ResourceReportIncomplete)?,
    })
}

fn canonicalize_receipts(
    receipts: &mut Vec<ReceiptRef>,
    maximum: usize,
) -> Result<(), EvalError> {
    receipts.sort();
    receipts.dedup();
    if receipts.len() > maximum
        || receipts.iter().any(|receipt| receipt.as_str().is_empty())
    {
        return Err(EvalError::BudgetExceeded);
    }
    Ok(())
}

fn fingerprint_metric_value(fingerprint: &mut FingerprintBuilder, metric: &CaseMetricValue) {
    fingerprint.push_text(metric.metric_id.as_str());
    fingerprint_optional_f64(fingerprint, metric.value);
    fingerprint_optional_f64(fingerprint, metric.numerator);
    fingerprint_optional_f64(fingerprint, metric.denominator);
    fingerprint.push_u64(metric.measured_count);
    fingerprint.push_u64(metric.missing_count);
    fingerprint.push_u64(metric.failed_count);
    fingerprint.push_bool(metric.complete);
    for receipt in &metric.evidence_refs {
        fingerprint.push_text(receipt.as_str());
    }
}

fn fingerprint_aggregate(fingerprint: &mut FingerprintBuilder, metric: &AggregatedMetric) {
    fingerprint.push_text(metric.metric_id.as_str());
    fingerprint_optional_f64(fingerprint, metric.value);
    fingerprint_optional_f64(fingerprint, metric.numerator);
    fingerprint_optional_f64(fingerprint, metric.denominator);
    fingerprint.push_u64(metric.measured_count);
    fingerprint.push_u64(metric.missing_count);
    fingerprint.push_u64(metric.failed_count);
    fingerprint.push_bool(metric.complete);
    for receipt in &metric.evidence_refs {
        fingerprint.push_text(receipt.as_str());
    }
}

fn fingerprint_optional_f64(fingerprint: &mut FingerprintBuilder, value: Option<f64>) {
    fingerprint.push_bool(value.is_some());
    if let Some(value) = value {
        fingerprint.push_f64(value);
    }
}

fn attempt_status_tag(status: AttemptStatus) -> u64 {
    match status {
        AttemptStatus::Success => 1,
        AttemptStatus::Partial => 2,
        AttemptStatus::Failed => 3,
        AttemptStatus::Cancelled => 4,
        AttemptStatus::TimedOut => 5,
        AttemptStatus::Unavailable => 6,
    }
}

fn comparison_class_tag(classification: BaselineComparisonClass) -> u64 {
    match classification {
        BaselineComparisonClass::Dominates => 1,
        BaselineComparisonClass::Complements => 2,
        BaselineComparisonClass::NonInferiorWithoutMaterialGain => 3,
        BaselineComparisonClass::Regresses => 4,
        BaselineComparisonClass::Incomplete => 5,
    }
}

fn slo_direction_tag(direction: SloDirection) -> u64 {
    match direction {
        SloDirection::AtMost => 1,
        SloDirection::AtLeast => 2,
    }
}

fn slo_status_tag(status: SloStatus) -> u64 {
    match status {
        SloStatus::Pass => 1,
        SloStatus::Fail => 2,
        SloStatus::Unavailable => 3,
    }
}

fn resource_lane_tag(lane: ResourceLane) -> u64 {
    match lane {
        ResourceLane::Warmup => 1,
        ResourceLane::Measured => 2,
    }
}
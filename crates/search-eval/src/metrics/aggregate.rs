//! Deterministic non-warm-up baseline/candidate metric aggregation.

use std::collections::{BTreeMap, BTreeSet};

use search_contracts::{Blake3Digest32, OpaqueId, ReceiptRef};

use crate::{
    AttemptStatus, EvalError, EvalLimits, FingerprintBuilder, FrozenRunManifest,
    MetricDefinition, MissingValuePolicy, ValidatedMetricRegistry,
};

use super::score::CaseMetricSet;
use super::support::{
    canonicalize_receipts, conservative_failure_value, fingerprint_aggregate,
};

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
    fingerprint.push_u64(u64::try_from(measured.len()).map_err(|_| EvalError::ContractExhausted)?);
    fingerprint
        .push_u64(u64::try_from(failed_attempt_count).map_err(|_| EvalError::ContractExhausted)?);
    fingerprint.push_u64(
        u64::try_from(unavailable_attempt_count).map_err(|_| EvalError::ContractExhausted)?,
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

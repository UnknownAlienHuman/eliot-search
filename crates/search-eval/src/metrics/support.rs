//! Shared deterministic metric fingerprint and receipt helpers.

use search_contracts::ReceiptRef;

use crate::{AttemptStatus, EvalError, FingerprintBuilder, MetricDirection};

use super::aggregate::AggregatedMetric;
use super::compare::BaselineComparisonClass;
use super::resource::ResourceLane;
use super::score::CaseMetricValue;
use super::slo::{SloDirection, SloStatus};

pub(super) fn canonicalize_receipts(
    receipts: &mut Vec<ReceiptRef>,
    maximum: usize,
) -> Result<(), EvalError> {
    receipts.sort();
    receipts.dedup();
    if receipts.len() > maximum || receipts.iter().any(|receipt| receipt.as_str().is_empty()) {
        return Err(EvalError::BudgetExceeded);
    }
    Ok(())
}

pub(super) fn conservative_failure_value(direction: MetricDirection) -> f64 {
    match direction {
        MetricDirection::HigherIsBetter => -f64::MAX,
        MetricDirection::LowerIsBetter | MetricDirection::ZeroTolerance => f64::MAX,
    }
}

pub(super) fn fingerprint_metric_value(
    fingerprint: &mut FingerprintBuilder,
    metric: &CaseMetricValue,
) {
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

pub(super) fn fingerprint_aggregate(
    fingerprint: &mut FingerprintBuilder,
    metric: &AggregatedMetric,
) {
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

pub(super) fn fingerprint_optional_f64(
    fingerprint: &mut FingerprintBuilder,
    value: Option<f64>,
) {
    fingerprint.push_bool(value.is_some());
    if let Some(value) = value {
        fingerprint.push_f64(value);
    }
}

pub(super) const fn attempt_status_tag(status: AttemptStatus) -> u64 {
    match status {
        AttemptStatus::Success => 1,
        AttemptStatus::Partial => 2,
        AttemptStatus::Failed => 3,
        AttemptStatus::Cancelled => 4,
        AttemptStatus::TimedOut => 5,
        AttemptStatus::Unavailable => 6,
    }
}

pub(super) const fn comparison_class_tag(classification: BaselineComparisonClass) -> u64 {
    match classification {
        BaselineComparisonClass::Dominates => 1,
        BaselineComparisonClass::Complements => 2,
        BaselineComparisonClass::NonInferiorWithoutMaterialGain => 3,
        BaselineComparisonClass::Regresses => 4,
        BaselineComparisonClass::Incomplete => 5,
    }
}

pub(super) const fn slo_direction_tag(direction: SloDirection) -> u64 {
    match direction {
        SloDirection::AtMost => 1,
        SloDirection::AtLeast => 2,
    }
}

pub(super) const fn slo_status_tag(status: SloStatus) -> u64 {
    match status {
        SloStatus::Pass => 1,
        SloStatus::Fail => 2,
        SloStatus::Unavailable => 3,
    }
}

pub(super) const fn resource_lane_tag(lane: ResourceLane) -> u64 {
    match lane {
        ResourceLane::Warmup => 1,
        ResourceLane::Measured => 2,
    }
}

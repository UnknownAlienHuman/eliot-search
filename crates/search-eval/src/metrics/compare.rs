//! Closed preregistered A/B/C metric comparison.

use std::collections::BTreeMap;

use search_contracts::{Blake3Digest32, OpaqueId};

use crate::{
    EvalError, FingerprintBuilder, MetricDirection, ValidatedAcceptancePolicy,
    ValidatedMetricRegistry,
};

use super::aggregate::BaselineMetricReport;
use super::support::{comparison_class_tag, fingerprint_optional_f64};

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
    /// Purpose-grouped acceptance gates.
    pub gates: MetricGates,
    /// Whether all three coherent complete metric values were present.
    pub complete: bool,
}

/// Purpose-grouped acceptance gates for one metric comparison.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MetricGates {
    /// Whether the absolute candidate threshold passed.
    pub threshold_passed: bool,
    /// Whether maximum preregistered regression was respected.
    pub non_inferior: bool,
    /// Whether preregistered practical effect was reached.
    pub material_gain: bool,
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

        let left_value = a.and_then(|value| value.value);
        let right_value = b.and_then(|value| value.value);
        let candidate_value = c.and_then(|value| value.value);
        let strongest_baseline_value = match (left_value, right_value) {
            (Some(left), Some(right)) => {
                Some(strongest_baseline(definition.direction, left, right))
            }
            _ => None,
        };
        let improvement =
            candidate_value
                .zip(strongest_baseline_value)
                .map(|(candidate, strongest)| {
                    improvement_amount(definition.direction, candidate, strongest)
                });
        let threshold_passed = candidate_value.is_some_and(|candidate| {
            passes_threshold(definition.direction, candidate, rule.candidate_threshold)
        });
        let non_inferior = improvement.is_some_and(|value| value >= -rule.maximum_regression);
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
                baseline_a_value: left_value,
                baseline_b_value: right_value,
                candidate_value,
                strongest_baseline_value,
                improvement,
                gates: MetricGates {
                    threshold_passed,
                    non_inferior,
                    material_gain,
                },
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
        fingerprint.push_bool(delta.gates.threshold_passed);
        fingerprint.push_bool(delta.gates.non_inferior);
        fingerprint.push_bool(delta.gates.material_gain);
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

const fn strongest_baseline(direction: MetricDirection, left: f64, right: f64) -> f64 {
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

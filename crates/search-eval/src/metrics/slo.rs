//! Preregistered candidate SLO evaluation.

use search_contracts::{Blake3Digest32, OpaqueId};

use crate::{EvalError, EvalLimits, FingerprintBuilder};

use super::aggregate::BaselineMetricReport;
use super::support::{fingerprint_optional_f64, slo_direction_tag, slo_status_tag};

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
        let status = if let Some(observed) = observed_value
            && metric.is_some_and(|metric| metric.complete)
            && measured_samples >= definition.minimum_samples
        {
            let passed = match definition.direction {
                SloDirection::AtMost => observed <= definition.threshold,
                SloDirection::AtLeast => observed >= definition.threshold,
            };
            if passed {
                SloStatus::Pass
            } else {
                SloStatus::Fail
            }
        } else {
            SloStatus::Unavailable
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

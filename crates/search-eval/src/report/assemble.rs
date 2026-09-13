//! Fail-closed Product Pulse assembly and deterministic report digest.

use std::collections::BTreeSet;

use crate::{
    BaselineComparisonClass, EvalError, EvalLimits, FingerprintBuilder,
    FrozenRunManifest, HardBlocker, HardBlockerClass, MetricDirection,
    ProbeStatus, ValidatedMetricRegistry,
};

use super::model::{ProductPulseInputs, ProductPulseReport};
use super::support::{blocker_class_tag, canonicalize_blockers, opaque_reason};

/// Assembles Product Pulse without upgrading incomplete evidence to success.
pub fn assemble_product_pulse(
    run: &FrozenRunManifest,
    metrics: &ValidatedMetricRegistry,
    inputs: ProductPulseInputs,
    limits: EvalLimits,
) -> Result<ProductPulseReport, EvalError> {
    let limits = limits.validate()?;
    if inputs.metric_reports.len() != 3
        || inputs.probes.len() > limits.max_audit_items
        || inputs.assembly_receipt.as_str().is_empty()
    {
        return Err(EvalError::ProductReportIncomplete);
    }
    let mut report_ids = BTreeSet::new();
    for report in &inputs.metric_reports {
        if report.run_digest != run.run_digest()
            || !report_ids.insert(report.baseline_id.clone())
        {
            return Err(EvalError::AggregateIdentityMismatch);
        }
    }
    let candidate_metrics = inputs
        .metric_reports
        .iter()
        .find(|report| report.baseline_id == inputs.candidate_id)
        .ok_or(EvalError::ProductReportIncomplete)?;
    if inputs.comparison.candidate_c != inputs.candidate_id
        || inputs.candidate_slos.baseline_id != inputs.candidate_id
        || inputs.candidate_resources.baseline_id != inputs.candidate_id
        || inputs.candidate_resources.run_digest != run.run_digest()
        || inputs.case_coverage.run_digest != run.run_digest()
        || inputs.leakage.run_digest != run.run_digest()
        || inputs.admission.run_digest != run.run_digest()
        || inputs.faults.run_digest != run.run_digest()
        || inputs.protocol.evidence.run_digest != run.run_digest()
        || inputs.reproducibility.run_digest != run.run_digest()
    {
        return Err(EvalError::EvidenceBindingMismatch);
    }

    let mut blockers = Vec::new();
    blockers.extend(inputs.leakage.blockers.iter().cloned());
    blockers.extend(inputs.admission.blockers.iter().cloned());
    blockers.extend(inputs.faults.blockers.iter().cloned());
    blockers.extend(inputs.protocol.blockers.iter().cloned());
    blockers.extend(inputs.reproducibility.blockers.iter().cloned());

    for definition in &metrics.registry().definitions {
        if definition.direction != MetricDirection::ZeroTolerance {
            continue;
        }
        let metric = candidate_metrics.metrics.get(&definition.metric_id);
        if metric.is_none_or(|metric| {
            !metric.complete || metric.value.is_none_or(|value| value != 0.0)
        }) {
            blockers.push(HardBlocker {
                class: HardBlockerClass::Correctness,
                check_id: definition.metric_id.clone(),
                reason: opaque_reason("ZERO_TOLERANCE_METRIC_FAILED")?,
                evidence_ref: candidate_metrics
                    .metrics
                    .get(&definition.metric_id)
                    .and_then(|metric| metric.evidence_refs.first())
                    .cloned()
                    .unwrap_or_else(|| inputs.assembly_receipt.clone()),
            });
        }
    }
    for outcome in &inputs.candidate_slos.outcomes {
        if outcome.mandatory && outcome.status != crate::SloStatus::Pass {
            blockers.push(HardBlocker {
                class: HardBlockerClass::ServiceLevelObjective,
                check_id: outcome.slo_id.clone(),
                reason: opaque_reason("MANDATORY_SLO_FAILED")?,
                evidence_ref: inputs.assembly_receipt.clone(),
            });
        }
    }
    if inputs.comparison.classification == BaselineComparisonClass::Regresses {
        blockers.push(HardBlocker {
            class: HardBlockerClass::CandidateRegression,
            check_id: inputs.candidate_id.clone(),
            reason: opaque_reason("CANDIDATE_REGRESSED")?,
            evidence_ref: inputs.assembly_receipt.clone(),
        });
    }
    for probe in &inputs.probes {
        let probe = probe.evidence();
        if probe.run_digest != run.run_digest() {
            return Err(EvalError::EvidenceBindingMismatch);
        }
        if probe.mandatory && probe.status != ProbeStatus::Pass {
            blockers.push(HardBlocker {
                class: HardBlockerClass::EvidenceIntegrity,
                check_id: probe.probe_id.clone(),
                reason: opaque_reason(match probe.status {
                    ProbeStatus::Pass => "UNREACHABLE_PASS",
                    ProbeStatus::Fail => "MANDATORY_PROBE_FAILED",
                    ProbeStatus::Unavailable => "MANDATORY_PROBE_UNAVAILABLE",
                })?,
                evidence_ref: probe
                    .raw_evidence_ref
                    .clone()
                    .unwrap_or_else(|| inputs.assembly_receipt.clone()),
            });
        }
    }
    canonicalize_blockers(&mut blockers)?;

    let complete = inputs.metric_reports.iter().all(|report| report.complete)
        && inputs.comparison.classification != BaselineComparisonClass::Incomplete
        && inputs.case_coverage.complete
        && inputs.candidate_resources.complete
        && inputs.leakage.observations.len()
            == inputs
                .leakage
                .required_canaries
                .len()
                .saturating_mul(inputs.leakage.required_surfaces.len())
        && inputs.admission.probes.len() >= crate::AdmissionScenario::MANDATORY.len()
        && !inputs.faults.cells.is_empty()
        && inputs.protocol.evidence.complete
        && !inputs.reproducibility.observations.is_empty()
        && inputs
            .probes
            .iter()
            .all(|probe| !probe.evidence().mandatory || probe.evidence().status != ProbeStatus::Unavailable);

    let mut fingerprint = FingerprintBuilder::new(b"eliot-search/eval/product-pulse/v1");
    fingerprint.push_digest(run.run_digest());
    fingerprint.push_text(inputs.candidate_id.as_str());
    for report in &inputs.metric_reports {
        fingerprint.push_digest(report.report_digest);
    }
    fingerprint.push_digest(inputs.comparison.comparison_digest);
    fingerprint.push_digest(inputs.candidate_slos.report_digest);
    fingerprint.push_digest(inputs.candidate_resources.report_digest);
    fingerprint.push_digest(inputs.case_coverage.coverage_digest);
    fingerprint.push_digest(inputs.leakage.audit_digest);
    fingerprint.push_digest(inputs.admission.audit_digest);
    fingerprint.push_digest(inputs.faults.matrix_digest);
    fingerprint.push_digest(inputs.protocol.report_digest);
    fingerprint.push_digest(inputs.reproducibility.report_digest);
    for blocker in &blockers {
        fingerprint.push_u64(blocker_class_tag(blocker.class));
        fingerprint.push_text(blocker.check_id.as_str());
        fingerprint.push_text(blocker.reason.as_str());
    }
    fingerprint.push_bool(complete);
    let report_digest = fingerprint.finish();
    Ok(ProductPulseReport {
        run_digest: run.run_digest(),
        candidate_id: inputs.candidate_id,
        metric_reports: inputs.metric_reports,
        comparison: inputs.comparison,
        candidate_slos: inputs.candidate_slos,
        candidate_resources: inputs.candidate_resources,
        case_coverage: inputs.case_coverage,
        leakage: inputs.leakage,
        admission: inputs.admission,
        faults: inputs.faults,
        protocol: inputs.protocol,
        reproducibility: inputs.reproducibility,
        probes: inputs.probes,
        hard_blockers: blockers,
        complete,
        report_digest,
        assembly_receipt: inputs.assembly_receipt,
    })
}

//! Product Pulse assembly, independent review, acceptance verdict, and receipt.

use std::collections::{BTreeMap, BTreeSet};

use search_contracts::{Blake3Digest32, OpaqueId, ReceiptRef};

use crate::{
    AdmissionAudit, AttemptStatus, BaselineComparison, BaselineComparisonClass,
    BaselineMetricReport, EvalError, EvalLimits, FaultMatrixReport,
    FingerprintBuilder, FrozenRunManifest, HardBlocker, HardBlockerClass,
    LeakageAudit, MetricDirection, ProbeStatus, ProtocolStressReport,
    ReproducibilityReport, ResourceReport, SloReport, ValidatedAcceptancePolicy,
    ValidatedCaseEvidence, ValidatedControlCorpus, ValidatedMetricRegistry,
    ValidatedProbeEvidence,
};

/// Exact measured case coverage for all three A/B/C identities.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CaseCoverageReport {
    /// Exact frozen run digest.
    pub run_digest: Blake3Digest32,
    /// Number of cases in the frozen denominator.
    pub expected_cases: usize,
    /// Measured distinct cases by baseline identity.
    pub measured_cases: BTreeMap<OpaqueId, usize>,
    /// Missing case identities by baseline identity.
    pub missing_cases: BTreeMap<OpaqueId, BTreeSet<OpaqueId>>,
    /// Whether every expected case has at least one measured terminal attempt for A/B/C.
    pub complete: bool,
    /// Deterministic coverage digest.
    pub coverage_digest: Blake3Digest32,
}

/// Audits measured case coverage against the frozen control-corpus denominator.
pub fn audit_case_coverage(
    run: &FrozenRunManifest,
    corpus: &ValidatedControlCorpus,
    baseline_ids: BTreeSet<OpaqueId>,
    evidence: &[ValidatedCaseEvidence],
) -> Result<CaseCoverageReport, EvalError> {
    if baseline_ids.len() != 3 {
        return Err(EvalError::ProductReportIncomplete);
    }
    let expected = corpus
        .manifest()
        .cases
        .iter()
        .map(|case| case.case_id.clone())
        .collect::<BTreeSet<_>>();
    let mut observed: BTreeMap<OpaqueId, BTreeSet<OpaqueId>> = baseline_ids
        .iter()
        .map(|baseline| (baseline.clone(), BTreeSet::new()))
        .collect();
    for item in evidence {
        let attempt = item.evidence();
        if attempt.warmup {
            continue;
        }
        if attempt.run_digest != run.run_digest()
            || !baseline_ids.contains(&attempt.baseline_id)
            || !expected.contains(&attempt.case_id)
        {
            return Err(EvalError::EvidenceBindingMismatch);
        }
        if attempt.terminal_events != 1 {
            return Err(EvalError::EvidenceStatusInvalid);
        }
        observed
            .get_mut(&attempt.baseline_id)
            .expect("baseline validated")
            .insert(attempt.case_id.clone());
    }
    let measured_cases = observed
        .iter()
        .map(|(baseline, cases)| (baseline.clone(), cases.len()))
        .collect::<BTreeMap<_, _>>();
    let missing_cases = observed
        .iter()
        .map(|(baseline, cases)| {
            (
                baseline.clone(),
                expected.difference(cases).cloned().collect::<BTreeSet<_>>(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let complete = missing_cases.values().all(BTreeSet::is_empty);
    let mut fingerprint = FingerprintBuilder::new(b"eliot-search/eval/case-coverage/v1");
    fingerprint.push_digest(run.run_digest());
    fingerprint.push_u64(u64::try_from(expected.len()).unwrap_or(u64::MAX));
    for (baseline, missing) in &missing_cases {
        fingerprint.push_text(baseline.as_str());
        for case in missing {
            fingerprint.push_text(case.as_str());
        }
    }
    Ok(CaseCoverageReport {
        run_digest: run.run_digest(),
        expected_cases: expected.len(),
        measured_cases,
        missing_cases,
        complete,
        coverage_digest: fingerprint.finish(),
    })
}

/// All already-validated inputs required for one Product Pulse report.
#[derive(Clone, Debug, PartialEq)]
pub struct ProductPulseInputs {
    /// A/B/C metric reports.
    pub metric_reports: Vec<BaselineMetricReport>,
    /// Exact candidate C identity.
    pub candidate_id: OpaqueId,
    /// Preregistered A/B/C comparison.
    pub comparison: BaselineComparison,
    /// Candidate C SLO report.
    pub candidate_slos: SloReport,
    /// Candidate C resource report.
    pub candidate_resources: ResourceReport,
    /// Frozen case coverage.
    pub case_coverage: CaseCoverageReport,
    /// Zero-tolerance leakage audit.
    pub leakage: LeakageAudit,
    /// Unsafe-source admission audit.
    pub admission: AdmissionAudit,
    /// Mandatory fault-recovery matrix.
    pub faults: FaultMatrixReport,
    /// Protocol stress report.
    pub protocol: ProtocolStressReport,
    /// Repeated-run reproducibility report.
    pub reproducibility: ReproducibilityReport,
    /// Additional mandatory/optional external probes.
    pub probes: Vec<ValidatedProbeEvidence>,
    /// Content-free assembly receipt.
    pub assembly_receipt: ReceiptRef,
}

/// Complete Product Pulse report before independent acceptance.
#[derive(Clone, Debug, PartialEq)]
pub struct ProductPulseReport {
    /// Exact frozen run digest.
    pub run_digest: Blake3Digest32,
    /// Exact candidate C identity.
    pub candidate_id: OpaqueId,
    /// A/B/C metric reports.
    pub metric_reports: Vec<BaselineMetricReport>,
    /// A/B/C relationship.
    pub comparison: BaselineComparison,
    /// Candidate SLOs.
    pub candidate_slos: SloReport,
    /// Candidate resources.
    pub candidate_resources: ResourceReport,
    /// Frozen case coverage.
    pub case_coverage: CaseCoverageReport,
    /// Leakage audit.
    pub leakage: LeakageAudit,
    /// Source-admission audit.
    pub admission: AdmissionAudit,
    /// Fault matrix.
    pub faults: FaultMatrixReport,
    /// Protocol stress.
    pub protocol: ProtocolStressReport,
    /// Reproducibility.
    pub reproducibility: ReproducibilityReport,
    /// Additional external probes.
    pub probes: Vec<ValidatedProbeEvidence>,
    /// Every zero-tolerance blocker, never averaged into a score.
    pub hard_blockers: Vec<HardBlocker>,
    /// Whether every mandatory evidence section is complete.
    pub complete: bool,
    /// Deterministic report digest.
    pub report_digest: Blake3Digest32,
    /// Content-free assembly receipt.
    pub assembly_receipt: ReceiptRef,
}

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
    let kind = if !report.complete {
        reasons.insert(opaque_reason("EVALUATION_INCOMPLETE")?);
        VerdictKind::Incomplete
    } else {
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
            if !delta.threshold_passed || !delta.non_inferior {
                reasons.insert(opaque_reason("METRIC_GATE_FAILED")?);
            }
        }
        if reasons.is_empty() {
            VerdictKind::Accepted
        } else {
            VerdictKind::Rejected
        }
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

/// Immutable content-free Product Pulse receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductPulseReceipt {
    /// Exact frozen run digest.
    pub run_digest: Blake3Digest32,
    /// Exact report digest.
    pub report_digest: Blake3Digest32,
    /// Exact policy digest.
    pub policy_digest: Blake3Digest32,
    /// Exact review identity.
    pub review_id: OpaqueId,
    /// Exact verdict digest.
    pub verdict_digest: Blake3Digest32,
    /// Terminal verdict.
    pub verdict: VerdictKind,
    /// Number of hard blockers.
    pub hard_blocker_count: usize,
    /// Content-free receipt identity.
    pub receipt: ReceiptRef,
}

/// Issues a receipt only for an exactly bound report, review, policy, and verdict.
pub fn issue_product_pulse_receipt(
    report: &ProductPulseReport,
    policy: &ValidatedAcceptancePolicy,
    review: &IndependentReview,
    verdict: &AcceptanceVerdict,
    receipt: ReceiptRef,
) -> Result<ProductPulseReceipt, EvalError> {
    validate_independent_review(report, policy, review)?;
    if verdict.run_digest != report.run_digest
        || verdict.report_digest != report.report_digest
        || verdict.policy_digest != policy.policy().policy_digest
        || verdict.review_id != review.review_id
        || verdict.hard_blockers != report.hard_blockers
        || receipt.as_str().is_empty()
    {
        return Err(EvalError::ReceiptMismatch);
    }
    Ok(ProductPulseReceipt {
        run_digest: report.run_digest,
        report_digest: report.report_digest,
        policy_digest: policy.policy().policy_digest,
        review_id: review.review_id.clone(),
        verdict_digest: verdict.verdict_digest,
        verdict: verdict.kind,
        hard_blocker_count: report.hard_blockers.len(),
        receipt,
    })
}

fn canonicalize_blockers(blockers: &mut Vec<HardBlocker>) -> Result<(), EvalError> {
    blockers.sort_by(|left, right| {
        (left.class, &left.check_id, &left.reason).cmp(&(
            right.class,
            &right.check_id,
            &right.reason,
        ))
    });
    if blockers.windows(2).any(|pair| {
        pair[0].class == pair[1].class
            && pair[0].check_id == pair[1].check_id
            && pair[0].reason == pair[1].reason
    }) {
        return Err(EvalError::ReceiptMismatch);
    }
    Ok(())
}

fn opaque_reason(value: &str) -> Result<OpaqueId, EvalError> {
    OpaqueId::new(value.to_owned()).map_err(|_| EvalError::ContractExhausted)
}

fn blocker_class_tag(value: HardBlockerClass) -> u64 {
    match value {
        HardBlockerClass::Leakage => 1,
        HardBlockerClass::SourceAdmission => 2,
        HardBlockerClass::FaultRecovery => 3,
        HardBlockerClass::ProtocolSafety => 4,
        HardBlockerClass::Correctness => 5,
        HardBlockerClass::ServiceLevelObjective => 6,
        HardBlockerClass::Reproducibility => 7,
        HardBlockerClass::EvidenceIntegrity => 8,
        HardBlockerClass::CandidateRegression => 9,
    }
}

fn verdict_tag(value: VerdictKind) -> u64 {
    match value {
        VerdictKind::Accepted => 1,
        VerdictKind::Rejected => 2,
        VerdictKind::Incomplete => 3,
    }
}

#[allow(dead_code)]
fn _attempt_status_is_terminal(status: AttemptStatus) -> bool {
    matches!(
        status,
        AttemptStatus::Success
            | AttemptStatus::Partial
            | AttemptStatus::Failed
            | AttemptStatus::Cancelled
            | AttemptStatus::TimedOut
            | AttemptStatus::Unavailable
    )
}

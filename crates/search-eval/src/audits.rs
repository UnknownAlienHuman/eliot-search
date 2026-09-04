//! Zero-tolerance leakage, admission, fault-recovery, and protocol audits.

use std::collections::{BTreeMap, BTreeSet};

use search_contracts::{Blake3Digest32, OpaqueId, ReceiptRef};

use crate::{EvalError, EvalLimits, FingerprintBuilder, FrozenRunManifest};

/// Hard blocker category that cannot be averaged away by quality metrics.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum HardBlockerClass {
    /// Source, query, path, secret, token, or oracle material leaked.
    Leakage,
    /// Unauthorized or structurally unsafe source was admitted.
    SourceAdmission,
    /// Crash recovery violated identity, visibility, or idempotency invariants.
    FaultRecovery,
    /// Framing, replay, terminal-response, flow-control, or cleanup failed.
    ProtocolSafety,
    /// A zero-tolerance registered correctness metric was non-zero.
    Correctness,
    /// Mandatory SLO failed.
    ServiceLevelObjective,
    /// Frozen-run or repeated-run reproducibility failed.
    Reproducibility,
    /// Required evidence was missing or could not be independently reviewed.
    EvidenceIntegrity,
    /// Candidate C materially regressed against a preregistered gate.
    CandidateRegression,
}

/// One immutable hard blocker.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HardBlocker {
    /// Closed blocker class.
    pub class: HardBlockerClass,
    /// Stable originating check identity.
    pub check_id: OpaqueId,
    /// Bounded machine-readable reason identity.
    pub reason: OpaqueId,
    /// Immutable evidence reference.
    pub evidence_ref: ReceiptRef,
}

/// Closed prohibited canary class.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CanaryClass {
    /// Source body or excerpt bytes.
    SourceContent,
    /// User query text.
    QueryText,
    /// Absolute or canonical path text.
    AbsolutePath,
    /// Plaintext secret or credential.
    Secret,
    /// Bearer token or opaque-handle plaintext.
    BearerToken,
    /// Private handle/continuation authority record.
    AuthorityRecord,
    /// Private evaluation oracle.
    Oracle,
    /// Forbidden vendor-specific internal metadata.
    VendorMetadata,
}

/// Closed observable leakage surface.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum LeakageSurface {
    /// Process/application logs.
    Logs,
    /// Durable control store.
    ControlStore,
    /// Search-index payload.
    IndexPayload,
    /// Metrics, traces, or telemetry.
    Telemetry,
    /// Protocol error/result body.
    Protocol,
    /// Temporary filesystem objects.
    TemporaryFiles,
    /// Crash artifacts and dumps.
    CrashArtifacts,
    /// Optional model-provider input.
    ModelInput,
    /// Evaluation feedback visible to production ranking/training.
    EvaluationFeedback,
}

/// One immutable canary/surface observation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LeakageObservation {
    /// Stable observation identity.
    pub observation_id: OpaqueId,
    /// Canary class searched for.
    pub canary_class: CanaryClass,
    /// Exact observed surface.
    pub surface: LeakageSurface,
    /// Digest of the secret canary retained outside candidate-visible state.
    pub canary_digest: Blake3Digest32,
    /// Whether the canary or an explicitly forbidden derivative was detected.
    pub detected: bool,
    /// Whether the inspected surface inventory was complete.
    pub complete_surface: bool,
    /// Immutable raw audit evidence.
    pub evidence_ref: ReceiptRef,
}

/// Complete zero-tolerance leakage audit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LeakageAudit {
    /// Exact frozen run digest.
    pub run_digest: Blake3Digest32,
    /// Required canary classes.
    pub required_canaries: BTreeSet<CanaryClass>,
    /// Required observable surfaces.
    pub required_surfaces: BTreeSet<LeakageSurface>,
    /// Exact observations in canonical pair order.
    pub observations: Vec<LeakageObservation>,
    /// Hard blockers emitted for every detection or incomplete surface.
    pub blockers: Vec<HardBlocker>,
    /// Whether all required pairs were observed and no canary was detected.
    pub passed: bool,
    /// Deterministic audit digest.
    pub audit_digest: Blake3Digest32,
}

/// Audits the complete required canary-by-surface matrix.
pub fn audit_leakage(
    run: &FrozenRunManifest,
    required_canaries: BTreeSet<CanaryClass>,
    required_surfaces: BTreeSet<LeakageSurface>,
    mut observations: Vec<LeakageObservation>,
    limits: EvalLimits,
) -> Result<LeakageAudit, EvalError> {
    let limits = limits.validate()?;
    if required_canaries.is_empty()
        || required_surfaces.is_empty()
        || observations.len() > limits.max_audit_items
    {
        return Err(EvalError::InvalidLimits);
    }
    observations.sort_by(|left, right| {
        (left.canary_class, left.surface, &left.observation_id).cmp(&(
            right.canary_class,
            right.surface,
            &right.observation_id,
        ))
    });
    let mut observed_pairs = BTreeSet::new();
    let mut blockers = Vec::new();
    for observation in &observations {
        if !required_canaries.contains(&observation.canary_class)
            || !required_surfaces.contains(&observation.surface)
            || observation.evidence_ref.as_str().is_empty()
            || !observed_pairs.insert((observation.canary_class, observation.surface))
        {
            return Err(EvalError::EvidenceBindingMismatch);
        }
        if observation.detected || !observation.complete_surface {
            blockers.push(HardBlocker {
                class: HardBlockerClass::Leakage,
                check_id: observation.observation_id.clone(),
                reason: bounded_reason(if observation.detected {
                    "CANARY_DETECTED"
                } else {
                    "SURFACE_INCOMPLETE"
                })?,
                evidence_ref: observation.evidence_ref.clone(),
            });
        }
    }
    for canary in &required_canaries {
        for surface in &required_surfaces {
            if !observed_pairs.contains(&(*canary, *surface)) {
                return Err(EvalError::ProductReportIncomplete);
            }
        }
    }
    let mut fingerprint = FingerprintBuilder::new(b"eliot-search/eval/leakage-audit/v1");
    fingerprint.push_digest(run.run_digest());
    for observation in &observations {
        fingerprint.push_text(observation.observation_id.as_str());
        fingerprint.push_u64(canary_tag(observation.canary_class));
        fingerprint.push_u64(surface_tag(observation.surface));
        fingerprint.push_digest(observation.canary_digest);
        fingerprint.push_bool(observation.detected);
        fingerprint.push_bool(observation.complete_surface);
    }
    Ok(LeakageAudit {
        run_digest: run.run_digest(),
        required_canaries,
        required_surfaces,
        observations,
        passed: blockers.is_empty(),
        blockers,
        audit_digest: fingerprint.finish(),
    })
}

/// Mandatory unsafe-admission scenario.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum AdmissionScenario {
    /// Unknown requested source or membership.
    UnknownScope,
    /// Symbolic-link escape.
    SymlinkEscape,
    /// Windows reparse-point/junction escape.
    ReparseEscape,
    /// Remote, device, or disallowed root.
    RemoteRoot,
    /// Source exceeds finite byte policy.
    OversizedSource,
    /// Restricted source lacks an exact grant.
    RestrictedWithoutGrant,
    /// Runtime owner epoch is stale.
    StaleOwnerEpoch,
    /// Admission policy revision is stale.
    StalePolicyRevision,
    /// Live purge/security barrier denies the source.
    PurgeFenced,
    /// Virtual buffer lacks authenticated immutable attestation.
    VirtualWithoutAttestation,
}

impl AdmissionScenario {
    /// Baseline mandatory admission-denial matrix.
    pub const MANDATORY: [Self; 10] = [
        Self::UnknownScope,
        Self::SymlinkEscape,
        Self::ReparseEscape,
        Self::RemoteRoot,
        Self::OversizedSource,
        Self::RestrictedWithoutGrant,
        Self::StaleOwnerEpoch,
        Self::StalePolicyRevision,
        Self::PurgeFenced,
        Self::VirtualWithoutAttestation,
    ];
}

/// One immutable source-admission probe.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmissionProbe {
    /// Stable probe identity.
    pub probe_id: OpaqueId,
    /// Unsafe scenario under test.
    pub scenario: AdmissionScenario,
    /// Whether the source was actually admitted.
    pub admitted: bool,
    /// Whether denial happened before source bytes entered preparation/indexing.
    pub denied_before_content_processing: bool,
    /// Closed observed reason identity.
    pub observed_reason: OpaqueId,
    /// Immutable raw evidence.
    pub evidence_ref: ReceiptRef,
}

/// Complete unsafe-source admission audit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmissionAudit {
    /// Exact frozen run digest.
    pub run_digest: Blake3Digest32,
    /// Canonical probes.
    pub probes: Vec<AdmissionProbe>,
    /// Hard blockers for unsafe admission or late denial.
    pub blockers: Vec<HardBlocker>,
    /// Whether every mandatory scenario denied before content processing.
    pub passed: bool,
    /// Deterministic audit digest.
    pub audit_digest: Blake3Digest32,
}

/// Validates the complete mandatory unsafe-admission matrix.
pub fn audit_source_admission(
    run: &FrozenRunManifest,
    mut probes: Vec<AdmissionProbe>,
    limits: EvalLimits,
) -> Result<AdmissionAudit, EvalError> {
    let limits = limits.validate()?;
    if probes.len() > limits.max_audit_items {
        return Err(EvalError::BudgetExceeded);
    }
    probes.sort_by(|left, right| {
        (left.scenario, &left.probe_id).cmp(&(right.scenario, &right.probe_id))
    });
    let mut scenarios = BTreeSet::new();
    let mut blockers = Vec::new();
    for probe in &probes {
        if probe.evidence_ref.as_str().is_empty() || !scenarios.insert(probe.scenario) {
            return Err(EvalError::EvidenceBindingMismatch);
        }
        if probe.admitted || !probe.denied_before_content_processing {
            blockers.push(HardBlocker {
                class: HardBlockerClass::SourceAdmission,
                check_id: probe.probe_id.clone(),
                reason: bounded_reason(if probe.admitted {
                    "UNSAFE_SOURCE_ADMITTED"
                } else {
                    "DENIAL_AFTER_CONTENT_PROCESSING"
                })?,
                evidence_ref: probe.evidence_ref.clone(),
            });
        }
    }
    if !AdmissionScenario::MANDATORY
        .into_iter()
        .all(|scenario| scenarios.contains(&scenario))
    {
        return Err(EvalError::ProductReportIncomplete);
    }
    let mut fingerprint = FingerprintBuilder::new(b"eliot-search/eval/admission-audit/v1");
    fingerprint.push_digest(run.run_digest());
    for probe in &probes {
        fingerprint.push_text(probe.probe_id.as_str());
        fingerprint.push_u64(admission_tag(probe.scenario));
        fingerprint.push_bool(probe.admitted);
        fingerprint.push_bool(probe.denied_before_content_processing);
        fingerprint.push_text(probe.observed_reason.as_str());
    }
    Ok(AdmissionAudit {
        run_digest: run.run_digest(),
        probes,
        passed: blockers.is_empty(),
        blockers,
        audit_digest: fingerprint.finish(),
    })
}

/// Mandatory mutation/fault boundary.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum FaultPoint {
    /// Before runtime-owner acquisition mutation.
    BeforeOwnerAcquire,
    /// After OS lock but before durable owner readback.
    AfterOwnerLock,
    /// During control transaction commit.
    ControlCommit,
    /// During encrypted revision commit.
    RevisionCommit,
    /// During index stage mutation.
    IndexStage,
    /// During index close/retire mutation.
    IndexClose,
    /// During visible-epoch control CAS.
    VisibleEpochCommit,
    /// During handle/continuation mint.
    CapabilityMint,
    /// During purge live-deny commit.
    PurgeBarrier,
    /// During purge physical/object deletion.
    PurgeDelete,
    /// During graceful drain.
    Drain,
    /// During owner release.
    OwnerRelease,
}

impl FaultPoint {
    /// Baseline mandatory fault matrix.
    pub const MANDATORY: [Self; 12] = [
        Self::BeforeOwnerAcquire,
        Self::AfterOwnerLock,
        Self::ControlCommit,
        Self::RevisionCommit,
        Self::IndexStage,
        Self::IndexClose,
        Self::VisibleEpochCommit,
        Self::CapabilityMint,
        Self::PurgeBarrier,
        Self::PurgeDelete,
        Self::Drain,
        Self::OwnerRelease,
    ];
}

/// Terminal fault-cell state.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum FaultCellStatus {
    /// Exact recovery completed and all invariants held.
    Pass,
    /// Recovery completed with a violated invariant.
    Fail,
    /// Recovery observation remained unresolved.
    Unavailable,
}

/// One exact fault-injection/recovery cell.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FaultCell {
    /// Fault boundary.
    pub fault_point: FaultPoint,
    /// Baseline/candidate identity.
    pub baseline_id: OpaqueId,
    /// Zero-based repetition.
    pub repetition: u32,
    /// Terminal status.
    pub status: FaultCellStatus,
    /// Whether authoritative readback resolved possible mutation.
    pub authoritative_readback: bool,
    /// Whether replay remained idempotent.
    pub idempotent_replay: bool,
    /// Whether visible state linearized at most once.
    pub no_double_publish: bool,
    /// Whether source/root/membership identity remained exact.
    pub no_identity_widening: bool,
    /// Whether restrictive barriers survived restart.
    pub restrictive_barriers_preserved: bool,
    /// Immutable fault evidence.
    pub evidence_ref: ReceiptRef,
}

/// Complete A/B/C fault-recovery matrix.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FaultMatrixReport {
    /// Exact frozen run digest.
    pub run_digest: Blake3Digest32,
    /// Canonical cells.
    pub cells: Vec<FaultCell>,
    /// Hard blockers for failed/unavailable/incomplete cells.
    pub blockers: Vec<HardBlocker>,
    /// Whether every required cell passed.
    pub passed: bool,
    /// Deterministic matrix digest.
    pub matrix_digest: Blake3Digest32,
}

/// Validates every required fault point for every A/B/C identity and repetition.
pub fn audit_fault_matrix(
    run: &FrozenRunManifest,
    baseline_ids: BTreeSet<OpaqueId>,
    repetitions: u32,
    mut cells: Vec<FaultCell>,
    limits: EvalLimits,
) -> Result<FaultMatrixReport, EvalError> {
    let limits = limits.validate()?;
    if baseline_ids.len() != 3
        || repetitions == 0
        || repetitions > limits.max_repetitions
        || cells.len() > limits.max_audit_items
    {
        return Err(EvalError::FaultMatrixIncomplete);
    }
    cells.sort_by(|left, right| {
        (left.fault_point, &left.baseline_id, left.repetition).cmp(&(
            right.fault_point,
            &right.baseline_id,
            right.repetition,
        ))
    });
    let mut observed = BTreeSet::new();
    let mut blockers = Vec::new();
    for cell in &cells {
        let key = (cell.fault_point, cell.baseline_id.clone(), cell.repetition);
        if !baseline_ids.contains(&cell.baseline_id)
            || cell.repetition >= repetitions
            || cell.evidence_ref.as_str().is_empty()
            || !observed.insert(key)
        {
            return Err(EvalError::EvidenceBindingMismatch);
        }
        let invariant_pass = cell.status == FaultCellStatus::Pass
            && cell.authoritative_readback
            && cell.idempotent_replay
            && cell.no_double_publish
            && cell.no_identity_widening
            && cell.restrictive_barriers_preserved;
        if !invariant_pass {
            blockers.push(HardBlocker {
                class: HardBlockerClass::FaultRecovery,
                check_id: bounded_reason(&format!(
                    "fault-{}-{}",
                    fault_tag(cell.fault_point),
                    cell.repetition
                ))?,
                reason: bounded_reason(match cell.status {
                    FaultCellStatus::Pass => "FAULT_INVARIANT_VIOLATION",
                    FaultCellStatus::Fail => "FAULT_RECOVERY_FAILED",
                    FaultCellStatus::Unavailable => "FAULT_RECOVERY_UNAVAILABLE",
                })?,
                evidence_ref: cell.evidence_ref.clone(),
            });
        }
    }
    for fault_point in FaultPoint::MANDATORY {
        for baseline_id in &baseline_ids {
            for repetition in 0..repetitions {
                if !observed.contains(&(fault_point, baseline_id.clone(), repetition)) {
                    return Err(EvalError::FaultMatrixIncomplete);
                }
            }
        }
    }
    let mut fingerprint = FingerprintBuilder::new(b"eliot-search/eval/fault-matrix/v1");
    fingerprint.push_digest(run.run_digest());
    for cell in &cells {
        fingerprint.push_u64(fault_tag(cell.fault_point));
        fingerprint.push_text(cell.baseline_id.as_str());
        fingerprint.push_u64(u64::from(cell.repetition));
        fingerprint.push_u64(fault_status_tag(cell.status));
        fingerprint.push_bool(cell.authoritative_readback);
        fingerprint.push_bool(cell.idempotent_replay);
        fingerprint.push_bool(cell.no_double_publish);
        fingerprint.push_bool(cell.no_identity_widening);
        fingerprint.push_bool(cell.restrictive_barriers_preserved);
    }
    Ok(FaultMatrixReport {
        run_digest: run.run_digest(),
        cells,
        passed: blockers.is_empty(),
        blockers,
        matrix_digest: fingerprint.finish(),
    })
}

/// Immutable aggregate protocol stress evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtocolStressEvidence {
    /// Exact frozen run digest.
    pub run_digest: Blake3Digest32,
    /// Stable stress profile identity.
    pub profile_id: OpaqueId,
    /// Total frames attempted.
    pub attempted_frames: u64,
    /// Maximum admitted frame bytes.
    pub frame_limit_bytes: u64,
    /// Largest frame presented.
    pub largest_presented_frame_bytes: u64,
    /// Oversized frames presented.
    pub oversized_frames: u64,
    /// Oversized frames rejected before body allocation.
    pub oversized_frames_rejected: u64,
    /// Replay attempts presented.
    pub replay_attempts: u64,
    /// Replay attempts rejected.
    pub replay_attempts_rejected: u64,
    /// Sequence gaps/regressions presented.
    pub sequence_violations: u64,
    /// Sequence gaps/regressions rejected.
    pub sequence_violations_rejected: u64,
    /// Requests that emitted more than one terminal response.
    pub duplicate_terminal_requests: u64,
    /// Cancellation requests acknowledged at a bounded boundary.
    pub cancellations_acknowledged: u64,
    /// Cancellation requests presented.
    pub cancellations_presented: u64,
    /// Peak in-flight requests.
    pub peak_inflight: u64,
    /// Configured in-flight ceiling.
    pub max_inflight: u64,
    /// Sessions opened.
    pub sessions_opened: u64,
    /// Sessions closed without leaked request/pin state.
    pub sessions_cleanly_closed: u64,
    /// Process-local requests/pins retained after session cleanup.
    pub leaked_session_objects: u64,
    /// Whether exact raw stress evidence is complete.
    pub complete: bool,
    /// Immutable raw evidence.
    pub evidence_ref: ReceiptRef,
}

/// Protocol stress report and zero-tolerance blockers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtocolStressReport {
    /// Exact stress evidence.
    pub evidence: ProtocolStressEvidence,
    /// Hard blockers.
    pub blockers: Vec<HardBlocker>,
    /// Whether every protocol invariant passed.
    pub passed: bool,
    /// Deterministic report digest.
    pub report_digest: Blake3Digest32,
}

/// Validates framing, replay, cancellation, flow-control, terminal, and cleanup invariants.
pub fn audit_protocol_stress(
    run: &FrozenRunManifest,
    evidence: ProtocolStressEvidence,
) -> Result<ProtocolStressReport, EvalError> {
    if evidence.run_digest != run.run_digest()
        || evidence.attempted_frames == 0
        || evidence.frame_limit_bytes == 0
        || evidence.max_inflight == 0
        || evidence.sessions_opened == 0
        || evidence.evidence_ref.as_str().is_empty()
    {
        return Err(EvalError::ProtocolStressFailed);
    }
    let invariants = [
        (
            evidence.oversized_frames_rejected == evidence.oversized_frames,
            "OVERSIZE_FRAME_NOT_REJECTED",
        ),
        (
            evidence.replay_attempts_rejected == evidence.replay_attempts,
            "REPLAY_NOT_REJECTED",
        ),
        (
            evidence.sequence_violations_rejected == evidence.sequence_violations,
            "SEQUENCE_VIOLATION_NOT_REJECTED",
        ),
        (
            evidence.duplicate_terminal_requests == 0,
            "DUPLICATE_TERMINAL_RESPONSE",
        ),
        (
            evidence.cancellations_acknowledged == evidence.cancellations_presented,
            "CANCELLATION_NOT_ACKNOWLEDGED",
        ),
        (
            evidence.peak_inflight <= evidence.max_inflight,
            "INFLIGHT_LIMIT_EXCEEDED",
        ),
        (
            evidence.sessions_cleanly_closed == evidence.sessions_opened
                && evidence.leaked_session_objects == 0,
            "SESSION_STATE_LEAKED",
        ),
        (evidence.complete, "PROTOCOL_EVIDENCE_INCOMPLETE"),
    ];
    let mut blockers = Vec::new();
    for (index, (passed, reason)) in invariants.into_iter().enumerate() {
        if !passed {
            blockers.push(HardBlocker {
                class: HardBlockerClass::ProtocolSafety,
                check_id: bounded_reason(&format!("protocol-invariant-{index}"))?,
                reason: bounded_reason(reason)?,
                evidence_ref: evidence.evidence_ref.clone(),
            });
        }
    }
    let mut fingerprint = FingerprintBuilder::new(b"eliot-search/eval/protocol-stress/v1");
    fingerprint.push_digest(run.run_digest());
    fingerprint.push_text(evidence.profile_id.as_str());
    fingerprint.push_u64(evidence.attempted_frames);
    fingerprint.push_u64(evidence.frame_limit_bytes);
    fingerprint.push_u64(evidence.largest_presented_frame_bytes);
    fingerprint.push_u64(evidence.oversized_frames);
    fingerprint.push_u64(evidence.oversized_frames_rejected);
    fingerprint.push_u64(evidence.replay_attempts);
    fingerprint.push_u64(evidence.replay_attempts_rejected);
    fingerprint.push_u64(evidence.sequence_violations);
    fingerprint.push_u64(evidence.sequence_violations_rejected);
    fingerprint.push_u64(evidence.duplicate_terminal_requests);
    fingerprint.push_u64(evidence.cancellations_presented);
    fingerprint.push_u64(evidence.cancellations_acknowledged);
    fingerprint.push_u64(evidence.peak_inflight);
    fingerprint.push_u64(evidence.max_inflight);
    fingerprint.push_u64(evidence.sessions_opened);
    fingerprint.push_u64(evidence.sessions_cleanly_closed);
    fingerprint.push_u64(evidence.leaked_session_objects);
    fingerprint.push_bool(evidence.complete);
    Ok(ProtocolStressReport {
        evidence,
        passed: blockers.is_empty(),
        blockers,
        report_digest: fingerprint.finish(),
    })
}

/// Repeated-run digest observation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReproducibilityObservation {
    /// Exact baseline/candidate identity.
    pub baseline_id: OpaqueId,
    /// Repetition identity.
    pub repetition_id: OpaqueId,
    /// Frozen input digest.
    pub input_digest: Blake3Digest32,
    /// Deterministic output digest.
    pub output_digest: Blake3Digest32,
    /// Whether nondeterminism was explicitly expected by the registered profile.
    pub nondeterminism_expected: bool,
    /// Immutable raw evidence.
    pub evidence_ref: ReceiptRef,
}

/// Reproducibility report.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReproducibilityReport {
    /// Exact frozen run digest.
    pub run_digest: Blake3Digest32,
    /// Canonical observations.
    pub observations: Vec<ReproducibilityObservation>,
    /// Hard blockers for unexpected output drift.
    pub blockers: Vec<HardBlocker>,
    /// Whether every deterministic profile reproduced exactly.
    pub passed: bool,
    /// Deterministic report digest.
    pub report_digest: Blake3Digest32,
}

/// Audits repeated deterministic runs without hiding divergent output digests.
pub fn audit_reproducibility(
    run: &FrozenRunManifest,
    mut observations: Vec<ReproducibilityObservation>,
    limits: EvalLimits,
) -> Result<ReproducibilityReport, EvalError> {
    let limits = limits.validate()?;
    if observations.is_empty() || observations.len() > limits.max_audit_items {
        return Err(EvalError::ProductReportIncomplete);
    }
    observations.sort_by(|left, right| {
        (&left.baseline_id, &left.repetition_id).cmp(&(
            &right.baseline_id,
            &right.repetition_id,
        ))
    });
    let mut seen = BTreeSet::new();
    let mut expected: BTreeMap<(OpaqueId, Blake3Digest32), Blake3Digest32> = BTreeMap::new();
    let mut blockers = Vec::new();
    for observation in &observations {
        if observation.evidence_ref.as_str().is_empty()
            || !seen.insert((
                observation.baseline_id.clone(),
                observation.repetition_id.clone(),
            ))
        {
            return Err(EvalError::EvidenceBindingMismatch);
        }
        let key = (observation.baseline_id.clone(), observation.input_digest);
        match expected.get(&key) {
            Some(first)
                if first != &observation.output_digest
                    && !observation.nondeterminism_expected =>
            {
                blockers.push(HardBlocker {
                    class: HardBlockerClass::Reproducibility,
                    check_id: observation.repetition_id.clone(),
                    reason: bounded_reason("UNEXPECTED_OUTPUT_DRIFT")?,
                    evidence_ref: observation.evidence_ref.clone(),
                });
            }
            Some(_) => {}
            None => {
                expected.insert(key, observation.output_digest);
            }
        }
    }
    let mut fingerprint = FingerprintBuilder::new(b"eliot-search/eval/reproducibility/v1");
    fingerprint.push_digest(run.run_digest());
    for observation in &observations {
        fingerprint.push_text(observation.baseline_id.as_str());
        fingerprint.push_text(observation.repetition_id.as_str());
        fingerprint.push_digest(observation.input_digest);
        fingerprint.push_digest(observation.output_digest);
        fingerprint.push_bool(observation.nondeterminism_expected);
    }
    Ok(ReproducibilityReport {
        run_digest: run.run_digest(),
        observations,
        passed: blockers.is_empty(),
        blockers,
        report_digest: fingerprint.finish(),
    })
}

fn bounded_reason(value: &str) -> Result<OpaqueId, EvalError> {
    OpaqueId::new(value.to_owned()).map_err(|_| EvalError::ContractExhausted)
}

fn canary_tag(value: CanaryClass) -> u64 {
    match value {
        CanaryClass::SourceContent => 1,
        CanaryClass::QueryText => 2,
        CanaryClass::AbsolutePath => 3,
        CanaryClass::Secret => 4,
        CanaryClass::BearerToken => 5,
        CanaryClass::AuthorityRecord => 6,
        CanaryClass::Oracle => 7,
        CanaryClass::VendorMetadata => 8,
    }
}

fn surface_tag(value: LeakageSurface) -> u64 {
    match value {
        LeakageSurface::Logs => 1,
        LeakageSurface::ControlStore => 2,
        LeakageSurface::IndexPayload => 3,
        LeakageSurface::Telemetry => 4,
        LeakageSurface::Protocol => 5,
        LeakageSurface::TemporaryFiles => 6,
        LeakageSurface::CrashArtifacts => 7,
        LeakageSurface::ModelInput => 8,
        LeakageSurface::EvaluationFeedback => 9,
    }
}

fn admission_tag(value: AdmissionScenario) -> u64 {
    match value {
        AdmissionScenario::UnknownScope => 1,
        AdmissionScenario::SymlinkEscape => 2,
        AdmissionScenario::ReparseEscape => 3,
        AdmissionScenario::RemoteRoot => 4,
        AdmissionScenario::OversizedSource => 5,
        AdmissionScenario::RestrictedWithoutGrant => 6,
        AdmissionScenario::StaleOwnerEpoch => 7,
        AdmissionScenario::StalePolicyRevision => 8,
        AdmissionScenario::PurgeFenced => 9,
        AdmissionScenario::VirtualWithoutAttestation => 10,
    }
}

fn fault_tag(value: FaultPoint) -> u64 {
    match value {
        FaultPoint::BeforeOwnerAcquire => 1,
        FaultPoint::AfterOwnerLock => 2,
        FaultPoint::ControlCommit => 3,
        FaultPoint::RevisionCommit => 4,
        FaultPoint::IndexStage => 5,
        FaultPoint::IndexClose => 6,
        FaultPoint::VisibleEpochCommit => 7,
        FaultPoint::CapabilityMint => 8,
        FaultPoint::PurgeBarrier => 9,
        FaultPoint::PurgeDelete => 10,
        FaultPoint::Drain => 11,
        FaultPoint::OwnerRelease => 12,
    }
}

fn fault_status_tag(value: FaultCellStatus) -> u64 {
    match value {
        FaultCellStatus::Pass => 1,
        FaultCellStatus::Fail => 2,
        FaultCellStatus::Unavailable => 3,
    }
}

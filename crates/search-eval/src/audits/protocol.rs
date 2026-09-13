//! Protocol framing, replay, cancellation, flow-control and cleanup audit.

use search_contracts::{Blake3Digest32, OpaqueId, ReceiptRef};

use crate::{EvalError, FingerprintBuilder, FrozenRunManifest};

use super::common::{HardBlocker, HardBlockerClass, bounded_reason};

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

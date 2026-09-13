//! Shared zero-tolerance audit blocker model.

use search_contracts::{OpaqueId, ReceiptRef};

use crate::EvalError;

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

pub(super) fn bounded_reason(value: &str) -> Result<OpaqueId, EvalError> {
    OpaqueId::new(value.to_owned()).map_err(|_| EvalError::ContractExhausted)
}

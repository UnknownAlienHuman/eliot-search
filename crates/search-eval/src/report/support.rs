//! Shared closed tags and canonical hard-blocker helpers.

use search_contracts::{OpaqueId};

use crate::{AttemptStatus, EvalError, HardBlocker, HardBlockerClass};

use super::verdict::VerdictKind;

pub(super) fn canonicalize_blockers(blockers: &mut [HardBlocker]) -> Result<(), EvalError> {
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

pub(super) fn opaque_reason(value: &str) -> Result<OpaqueId, EvalError> {
    OpaqueId::new(value.to_owned()).map_err(|_| EvalError::ContractExhausted)
}

pub(super) const fn blocker_class_tag(value: HardBlockerClass) -> u64 {
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

pub(super) const fn verdict_tag(value: VerdictKind) -> u64 {
    match value {
        VerdictKind::Accepted => 1,
        VerdictKind::Rejected => 2,
        VerdictKind::Incomplete => 3,
    }
}

#[allow(dead_code)]
const fn _attempt_status_is_terminal(status: AttemptStatus) -> bool {
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

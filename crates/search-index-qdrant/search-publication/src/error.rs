//! Closed publication failures.

use core::fmt;

/// Failure returned by linearizable publication planning or recovery.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PublicationError {
    /// Another publication transaction is unresolved.
    PublicationBusy,
    /// Prepared input or exact manifest is malformed.
    InvalidPreparedPublication,
    /// Proposed visible epoch is not the exact next epoch.
    EpochMismatch,
    /// Owner/source/membership/access/shadow/purge/profile guard changed.
    GuardMismatch,
    /// Operation or transaction identity differs.
    OperationMismatch,
    /// Lifecycle transition is invalid.
    InvalidTransition,
    /// Staged exact IDs or readback differ from the manifest.
    StageReadbackMismatch,
    /// Closed exact IDs or readback differ from the retired manifest.
    ClosureReadbackMismatch,
    /// Unexpected point appeared during exact verification.
    UnexpectedPoint,
    /// Control compare-and-swap failed.
    ControlConflict,
    /// Immutable control snapshot was not published.
    SnapshotPublicationFailed,
    /// External mutation outcome is unresolved.
    OutcomeUnknown,
    /// Exact compensation is incomplete.
    CompensationIncomplete,
    /// Abandonment lacks a complete exclusion fence.
    AbandonFenceMissing,
    /// Recovery evidence is contradictory or incomplete.
    RecoveryBlocked,
    /// Finite point or transaction capacity was exceeded.
    BudgetExceeded,
    /// Shared epoch or revision space is exhausted.
    ContractExhausted,
}

impl PublicationError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::PublicationBusy => "PUBLICATION_BUSY",
            Self::InvalidPreparedPublication => "PUBLICATION_PREPARED_INVALID",
            Self::EpochMismatch => "PUBLICATION_EPOCH_MISMATCH",
            Self::GuardMismatch => "PUBLICATION_GUARD_MISMATCH",
            Self::OperationMismatch => "PUBLICATION_OPERATION_MISMATCH",
            Self::InvalidTransition => "PUBLICATION_TRANSITION_INVALID",
            Self::StageReadbackMismatch => "PUBLICATION_STAGE_READBACK_MISMATCH",
            Self::ClosureReadbackMismatch => "PUBLICATION_CLOSURE_READBACK_MISMATCH",
            Self::UnexpectedPoint => "PUBLICATION_UNEXPECTED_POINT",
            Self::ControlConflict => "PUBLICATION_CONTROL_CONFLICT",
            Self::SnapshotPublicationFailed => "PUBLICATION_SNAPSHOT_FAILED",
            Self::OutcomeUnknown => "PUBLICATION_OUTCOME_UNKNOWN",
            Self::CompensationIncomplete => "PUBLICATION_COMPENSATION_INCOMPLETE",
            Self::AbandonFenceMissing => "PUBLICATION_ABANDON_FENCE_MISSING",
            Self::RecoveryBlocked => "PUBLICATION_RECOVERY_BLOCKED",
            Self::BudgetExceeded => "PUBLICATION_BUDGET_EXCEEDED",
            Self::ContractExhausted => "PUBLICATION_CONTRACT_EXHAUSTED",
        }
    }
}

impl fmt::Display for PublicationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for PublicationError {}

//! Closed, content-free runtime-owner failures.

use core::fmt;

/// Typed failure returned by the runtime-owner state machine.
///
/// Variants deliberately carry no unrestricted path, process command line,
/// secret, or record body. Load-bearing identities are available through the
/// state snapshot and receipts returned by successful operations.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum OwnerError {
    /// The supplied data-root identity is invalid for local ownership.
    DataRootInvalid,
    /// The requested data root is remote, device-backed, or otherwise denied.
    DataRootRemoteDenied,
    /// Another verified live owner already holds the root.
    DataRootAlreadyOwned,
    /// Standalone and managed ownership modes conflict.
    OwnerModeConflict,
    /// The supplied owner epoch is stale, skipped, or otherwise mismatched.
    OwnerEpochMismatch,
    /// Process creation identity did not match the authoritative owner record.
    OwnerProcessIdentityMismatch,
    /// Executable identity did not match the authoritative owner record.
    OwnerExecutableIdentityMismatch,
    /// Ownership evidence is incomplete, contradictory, or ambiguous.
    OwnerIdentityAmbiguous,
    /// Acquisition may have crossed an external mutation boundary.
    OwnerAcquireOutcomeUnknown,
    /// Release may have crossed an external mutation boundary.
    OwnerReleaseOutcomeUnknown,
    /// Recovery evidence requires quarantine instead of attachment or cleanup.
    OwnerRecoveryQuarantined,
    /// A release was requested before the owner entered draining state.
    OwnerDrainRequired,
    /// Required dependency shutdown evidence is absent or mismatched.
    OwnerReleasePreconditionMissing,
    /// An operation identity was reused with another canonical request digest.
    OwnerOperationConflict,
    /// Cancellation was observed before any mutation began.
    OwnerCancelledBeforeMutation,
    /// A finite deadline was not supplied or was already expired.
    OwnerDeadlineInvalid,
    /// A lease window is internally inconsistent.
    OwnerLeaseInvalid,
    /// A heartbeat moved backwards or failed to extend the lease.
    OwnerHeartbeatRegression,
    /// A guard does not bind the current root, owner, epoch, or token digest.
    OwnerGuardMismatch,
    /// Durable owner-record readback did not match the expected digest.
    OwnerRecordDigestMismatch,
    /// A state transition is not legal from the current lifecycle state.
    OwnerInvalidTransition,
    /// The finite in-memory operation ledger reached its configured ceiling.
    OwnerOperationCapacityExceeded,
    /// A required exact readback or authorization receipt is missing.
    OwnerRecoveryEvidenceMissing,
    /// A proposed mode or root change requires drain and process restart.
    ModeTransitionRequiresRestart,
    /// A shared contract value could not advance without overflow.
    ContractExhausted,
}

impl OwnerError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::DataRootInvalid => "DATA_ROOT_INVALID",
            Self::DataRootRemoteDenied => "DATA_ROOT_REMOTE_DENIED",
            Self::DataRootAlreadyOwned => "DATA_ROOT_ALREADY_OWNED",
            Self::OwnerModeConflict => "OWNER_MODE_CONFLICT",
            Self::OwnerEpochMismatch => "OWNER_EPOCH_MISMATCH",
            Self::OwnerProcessIdentityMismatch => "OWNER_PROCESS_IDENTITY_MISMATCH",
            Self::OwnerExecutableIdentityMismatch => "OWNER_EXECUTABLE_IDENTITY_MISMATCH",
            Self::OwnerIdentityAmbiguous => "OWNER_IDENTITY_AMBIGUOUS",
            Self::OwnerAcquireOutcomeUnknown => "OWNER_ACQUIRE_OUTCOME_UNKNOWN",
            Self::OwnerReleaseOutcomeUnknown => "OWNER_RELEASE_OUTCOME_UNKNOWN",
            Self::OwnerRecoveryQuarantined => "OWNER_RECOVERY_QUARANTINED",
            Self::OwnerDrainRequired => "OWNER_DRAIN_REQUIRED",
            Self::OwnerReleasePreconditionMissing => "OWNER_RELEASE_PRECONDITION_MISSING",
            Self::OwnerOperationConflict => "OWNER_OPERATION_CONFLICT",
            Self::OwnerCancelledBeforeMutation => "OWNER_CANCELLED_BEFORE_MUTATION",
            Self::OwnerDeadlineInvalid => "OWNER_DEADLINE_INVALID",
            Self::OwnerLeaseInvalid => "OWNER_LEASE_INVALID",
            Self::OwnerHeartbeatRegression => "OWNER_HEARTBEAT_REGRESSION",
            Self::OwnerGuardMismatch => "OWNER_GUARD_MISMATCH",
            Self::OwnerRecordDigestMismatch => "OWNER_RECORD_DIGEST_MISMATCH",
            Self::OwnerInvalidTransition => "OWNER_INVALID_TRANSITION",
            Self::OwnerOperationCapacityExceeded => "OWNER_OPERATION_CAPACITY_EXCEEDED",
            Self::OwnerRecoveryEvidenceMissing => "OWNER_RECOVERY_EVIDENCE_MISSING",
            Self::ModeTransitionRequiresRestart => "MODE_TRANSITION_REQUIRES_RESTART",
            Self::ContractExhausted => "OWNER_CONTRACT_EXHAUSTED",
        }
    }
}

impl fmt::Display for OwnerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for OwnerError {}

//! Pure acquisition, recovery, renewal, drain, release, and health transitions.

use search_contracts::{Blake3Digest32, BoundedList, NonZeroRevision};

use crate::{OwnerBinding, OwnerOperation};

mod acquire;
mod health;
mod release;

pub use acquire::{
    AcquireCommitObservation, AcquirePlan, AcquireRecovery, AcquireRequest, AcquireResolution,
    LiveOwnerStatus, OwnerObservation, RecoveryDecision, RecoveryEvidence, RecoveryPolicy,
    RenewalReceipt, classify_abandoned_owner, complete_acquire, prepare_acquire,
    recover_acquisition, renew_verified, verify_owner_guard,
};
pub use health::{classify_owner_mutation_boundary, owner_health};
pub use release::{
    ModeChangeDecision, ReleaseCommitObservation, ReleasePlan, ReleaseResolution, begin_drain,
    complete_release, plan_mode_or_root_change, prepare_release, recover_release,
    verify_release_preconditions,
};

/// Maximum number of explicit effects in one owner mutation plan.
pub const MAX_OWNER_EFFECTS: usize = 8;

/// Explicit platform or durable effect emitted by a pure transition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OwnerEffect {
    /// Remove one exact stale record after verified absence and authorization.
    CleanStaleRecord {
        /// Expected stale record digest.
        expected_record_digest: Blake3Digest32,
        /// Digest-bound authorization reference.
        authorization_receipt: search_contracts::ReceiptRef,
    },
    /// Terminate one exact verified owned orphan.
    TerminateVerifiedOrphan {
        /// Exact orphan binding.
        binding: OwnerBinding,
        /// Explicit termination authorization.
        authorization_receipt: search_contracts::ReceiptRef,
        /// Exact process/executable observation receipt.
        process_identity_receipt: search_contracts::ReceiptRef,
    },
    /// Acquire the OS ownership primitive.
    AcquireOwnershipPrimitive {
        /// Exact new owner binding.
        binding: OwnerBinding,
        /// Immutable acquisition operation.
        operation: OwnerOperation,
    },
    /// Write the exact durable owner record.
    WriteOwnerRecord {
        /// Exact owner binding.
        binding: OwnerBinding,
        /// Expected durable record revision.
        record_revision: NonZeroRevision,
        /// Digest of exact record bytes.
        record_digest: Blake3Digest32,
        /// Immutable operation.
        operation: OwnerOperation,
    },
    /// Verify primitive and durable record by exact readback.
    VerifyOwnerReadback {
        /// Exact owner binding.
        binding: OwnerBinding,
        /// Expected record digest.
        record_digest: Blake3Digest32,
        /// Immutable operation.
        operation: OwnerOperation,
    },
    /// Persist an exact renewed or draining owner record.
    UpdateOwnerRecord {
        /// Exact owner binding.
        binding: OwnerBinding,
        /// Expected durable revision.
        record_revision: NonZeroRevision,
        /// Expected record digest.
        record_digest: Blake3Digest32,
        /// Immutable operation.
        operation: OwnerOperation,
    },
    /// Persist release intent before releasing the live primitive.
    WriteReleaseIntent {
        /// Exact owner binding.
        binding: OwnerBinding,
        /// Immutable release operation.
        operation: OwnerOperation,
    },
    /// Release the exact OS ownership primitive.
    ReleaseOwnershipPrimitive {
        /// Exact owner binding.
        binding: OwnerBinding,
        /// Immutable release operation.
        operation: OwnerOperation,
    },
    /// Verify exact released state after mutation.
    VerifyReleaseReadback {
        /// Exact owner binding.
        binding: OwnerBinding,
        /// Immutable release operation.
        operation: OwnerOperation,
    },
}

pub(crate) fn bounded_effects(
    effects: Vec<OwnerEffect>,
) -> Result<BoundedList<OwnerEffect, MAX_OWNER_EFFECTS>, crate::OwnerError> {
    BoundedList::new(effects).map_err(|_| crate::OwnerError::OwnerOperationCapacityExceeded)
}

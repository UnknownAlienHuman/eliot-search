//! Acquisition, abandoned-owner recovery, guard verification, and renewal.

use search_contracts::{Blake3Digest32, BoundedList, NonZeroRevision, OwnerEpoch, ReceiptRef};

use super::{MAX_OWNER_EFFECTS, OwnerEffect, bounded_effects};
use crate::{
    DataRootIdentity, OwnerBinding, OwnerError, OwnerGuard, OwnerIdentity, OwnerLifecycle,
    OwnerOperation, OwnerRecord, OwnerSnapshot, OwnerState, OwnerVerificationReceipt,
    PendingAcquire,
};

/// Policy applied only after a complete ownership observation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RecoveryPolicy {
    /// Never terminate a live process.
    Conservative,
    /// Permit termination only for an exact verified owned orphan.
    TerminateVerifiedOwnedOrphan,
}

/// Liveness classification for an exact matching owner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LiveOwnerStatus {
    /// Exact owner is live and retains authority.
    Active,
    /// Exact process is a verified owned orphan.
    VerifiedOwnedOrphan {
        /// Exact-process termination authorization.
        termination_authorization: ReceiptRef,
        /// Process and executable identity observation receipt.
        process_identity_receipt: ReceiptRef,
    },
}

/// Complete evidence allowing exact stale-record cleanup.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecoveryEvidence {
    /// Digest read back from the durable stale record.
    pub observed_record_digest: Blake3Digest32,
    /// Exact record readback receipt.
    pub record_readback_receipt: ReceiptRef,
    /// Reuse-resistant process-identity absence receipt.
    pub process_absence_receipt: ReceiptRef,
    /// OS ownership-primitive absence receipt.
    pub primitive_absence_receipt: ReceiptRef,
    /// Authorization to remove this exact record.
    pub cleanup_authorization_receipt: ReceiptRef,
}

/// Complete side-effect-free ownership observation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OwnerObservation {
    /// Neither a durable owner record nor ownership primitive exists.
    Absent {
        /// Observed root.
        root: DataRootIdentity,
        /// Highest epoch retained by independent history.
        highest_epoch: Option<OwnerEpoch>,
        /// Exact observation receipt.
        observation_receipt: ReceiptRef,
    },
    /// Durable record and live process match exactly.
    LiveMatchingOwner {
        /// Exact observed owner record.
        record: OwnerRecord,
        /// Active or verified-orphan state.
        status: LiveOwnerStatus,
        /// Exact observation receipt.
        observation_receipt: ReceiptRef,
    },
    /// A live owner conflicts with requested identity or mode.
    LiveConflictingOwner {
        /// Exact conflicting record.
        record: OwnerRecord,
        /// Exact observation receipt.
        observation_receipt: ReceiptRef,
    },
    /// Exact stale record exists and process/primitive identity is absent.
    StaleRecordIdentityAbsent {
        /// Exact stale record.
        record: OwnerRecord,
        /// Highest independently observed epoch.
        highest_epoch: OwnerEpoch,
        /// Complete cleanup evidence.
        evidence: RecoveryEvidence,
    },
    /// Ownership evidence is incomplete or contradictory.
    Ambiguous {
        /// Observed root.
        root: DataRootIdentity,
        /// Highest safely known epoch.
        highest_epoch: Option<OwnerEpoch>,
        /// Content-free observation receipt.
        observation_receipt: ReceiptRef,
    },
    /// Durable ownership data is malformed or unsupported.
    Corrupt {
        /// Observed root.
        root: DataRootIdentity,
        /// Highest safely known epoch.
        highest_epoch: Option<OwnerEpoch>,
        /// Content-free observation receipt.
        observation_receipt: ReceiptRef,
    },
}

impl OwnerObservation {
    /// Observed root identity.
    #[must_use]
    pub const fn root(&self) -> DataRootIdentity {
        match self {
            Self::Absent { root, .. }
            | Self::Ambiguous { root, .. }
            | Self::Corrupt { root, .. } => *root,
            Self::LiveMatchingOwner { record, .. }
            | Self::LiveConflictingOwner { record, .. }
            | Self::StaleRecordIdentityAbsent { record, .. } => record.binding().root(),
        }
    }

    /// Highest epoch represented by complete observation evidence.
    #[must_use]
    pub const fn highest_epoch(&self) -> Option<OwnerEpoch> {
        match self {
            Self::Absent { highest_epoch, .. }
            | Self::Ambiguous { highest_epoch, .. }
            | Self::Corrupt { highest_epoch, .. } => *highest_epoch,
            Self::LiveMatchingOwner { record, .. } | Self::LiveConflictingOwner { record, .. } => {
                Some(record.binding().epoch())
            }
            Self::StaleRecordIdentityAbsent { highest_epoch, .. } => Some(*highest_epoch),
        }
    }
}

/// Pure recovery classification.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RecoveryDecision {
    /// No owner evidence exists.
    StartFresh,
    /// A live owner retains authority.
    DenyLiveOwner,
    /// Exact stale record may be removed before retry.
    CleanStaleRecordAndRetry,
    /// Exact verified orphan may be terminated before retry.
    TerminateVerifiedOrphanThenRetry,
    /// Ambiguous or corrupt evidence requires quarantine.
    Quarantine,
}

/// Exact acquisition request supplied to the state machine.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcquireRequest {
    /// Canonical local root identity.
    pub root: DataRootIdentity,
    /// Exact candidate owner identity.
    pub owner: OwnerIdentity,
    /// Immutable operation and canonical request digest.
    pub operation: OwnerOperation,
    /// Digest of a process-local owner token.
    pub token_digest: Blake3Digest32,
    /// Finite initial lease.
    pub lease: crate::LeaseWindow,
    /// First durable record revision; must equal one.
    pub initial_record_revision: NonZeroRevision,
    /// Digest of exact planned record bytes.
    pub planned_record_digest: Blake3Digest32,
}

/// Prepared acquisition and ordered explicit effects.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcquirePlan {
    /// Exact pending acquisition.
    pub pending: PendingAcquire,
    /// Recovery classification used by the plan.
    pub recovery: RecoveryDecision,
    /// Finite ordered effects.
    pub effects: BoundedList<OwnerEffect, MAX_OWNER_EFFECTS>,
}

/// Observation after executing acquisition effects.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AcquireCommitObservation {
    /// Primitive and durable record match exact plan.
    VerifiedApplied {
        /// Exact read-back record.
        record: OwnerRecord,
        /// Whether the exact OS primitive is verified held.
        primitive_verified: bool,
        /// Post-mutation readback receipt.
        readback_receipt: ReceiptRef,
    },
    /// Platform rejected acquisition before mutation.
    RejectedBeforeMutation,
    /// Cancellation was observed before mutation.
    CancelledBeforeMutation,
    /// At least one mutation may have occurred.
    OutcomeUnknown,
    /// Readback is contradictory.
    Contradictory,
}

/// Terminal acquisition result.
pub enum AcquireResolution {
    /// Exact acquisition completed.
    Acquired {
        /// Process-local owner authority.
        guard: OwnerGuard,
        /// Exact verification receipt.
        receipt: OwnerVerificationReceipt,
    },
    /// No mutation occurred and acquisition was rejected.
    RejectedBeforeMutation,
    /// No mutation occurred and acquisition was cancelled.
    CancelledBeforeMutation,
    /// Exact outcome requires recovery.
    OutcomeUnknown,
    /// State was quarantined.
    Quarantined(OwnerError),
}

impl core::fmt::Debug for AcquireResolution {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Acquired { guard, receipt } => formatter
                .debug_struct("Acquired")
                .field("guard", guard)
                .field("receipt", receipt)
                .finish(),
            Self::RejectedBeforeMutation => formatter.write_str("RejectedBeforeMutation"),
            Self::CancelledBeforeMutation => formatter.write_str("CancelledBeforeMutation"),
            Self::OutcomeUnknown => formatter.write_str("OutcomeUnknown"),
            Self::Quarantined(error) => formatter.debug_tuple("Quarantined").field(error).finish(),
        }
    }
}

/// Acquisition recovery result.
pub enum AcquireRecovery {
    /// Exact live state reconstructed process-local authority.
    Reconstructed {
        /// Reconstructed owner guard.
        guard: OwnerGuard,
        /// Exact readback receipt.
        receipt: OwnerVerificationReceipt,
    },
    /// Exact readback proves acquisition did not apply.
    NotApplied,
    /// State was quarantined.
    Quarantined(OwnerError),
}

impl core::fmt::Debug for AcquireRecovery {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Reconstructed { guard, receipt } => formatter
                .debug_struct("Reconstructed")
                .field("guard", guard)
                .field("receipt", receipt)
                .finish(),
            Self::NotApplied => formatter.write_str("NotApplied"),
            Self::Quarantined(error) => formatter.debug_tuple("Quarantined").field(error).finish(),
        }
    }
}

/// Verified renewal receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenewalReceipt {
    /// Exact owner binding.
    pub binding: OwnerBinding,
    /// New durable revision.
    pub record_revision: NonZeroRevision,
    /// Digest of renewed record.
    pub record_digest: Blake3Digest32,
    /// Exact readback receipt.
    pub readback_receipt: ReceiptRef,
}

/// Classifies one complete observation without side effects.
#[must_use]
pub fn classify_abandoned_owner(
    observation: &OwnerObservation,
    policy: RecoveryPolicy,
) -> RecoveryDecision {
    match observation {
        OwnerObservation::Absent { .. } => RecoveryDecision::StartFresh,
        OwnerObservation::LiveMatchingOwner {
            status: LiveOwnerStatus::VerifiedOwnedOrphan { .. },
            ..
        } if policy == RecoveryPolicy::TerminateVerifiedOwnedOrphan => {
            RecoveryDecision::TerminateVerifiedOrphanThenRetry
        }
        OwnerObservation::LiveMatchingOwner { .. }
        | OwnerObservation::LiveConflictingOwner { .. } => RecoveryDecision::DenyLiveOwner,
        OwnerObservation::StaleRecordIdentityAbsent {
            record, evidence, ..
        } if evidence.observed_record_digest == record.record_digest() => {
            RecoveryDecision::CleanStaleRecordAndRetry
        }
        OwnerObservation::StaleRecordIdentityAbsent { .. }
        | OwnerObservation::Ambiguous { .. }
        | OwnerObservation::Corrupt { .. } => RecoveryDecision::Quarantine,
    }
}

/// Prepares exact acquisition from a vacant or released snapshot.
///
/// # Errors
///
/// Rejects live ownership, root/mode mismatch, incomplete recovery evidence,
/// ambiguous observations, and non-monotone epochs or revisions.
pub fn prepare_acquire(
    snapshot: &OwnerSnapshot,
    request: AcquireRequest,
    observation: &OwnerObservation,
    policy: RecoveryPolicy,
) -> Result<(OwnerSnapshot, AcquirePlan), OwnerError> {
    let previous_epoch = match snapshot.state() {
        OwnerState::Vacant { last_epoch, .. } => *last_epoch,
        OwnerState::Released { last_epoch, .. } => Some(*last_epoch),
        OwnerState::Active { record }
        | OwnerState::Draining { record, .. }
        | OwnerState::Releasing { record, .. }
        | OwnerState::ReleaseOutcomeUnknown { record, .. } => {
            if record.binding().owner().mode() != request.owner.mode() {
                return Err(OwnerError::OwnerModeConflict);
            }
            return Err(OwnerError::DataRootAlreadyOwned);
        }
        OwnerState::Acquiring { pending, .. }
        | OwnerState::AcquireOutcomeUnknown { pending, .. } => {
            pending
                .expected_record
                .acquire_operation()
                .verify_replay(&request.operation)?;
            return Err(OwnerError::OwnerAcquireOutcomeUnknown);
        }
        OwnerState::Quarantined { .. } => return Err(OwnerError::OwnerRecoveryQuarantined),
    };

    if snapshot.state().root() != request.root || observation.root() != request.root {
        return Err(OwnerError::DataRootInvalid);
    }
    if request.initial_record_revision.get() != 1 {
        return Err(OwnerError::OwnerEpochMismatch);
    }

    let recovery = classify_abandoned_owner(observation, policy);
    match recovery {
        RecoveryDecision::DenyLiveOwner => {
            if let OwnerObservation::LiveConflictingOwner { record, .. } = observation {
                if record.binding().owner().mode() != request.owner.mode() {
                    return Err(OwnerError::OwnerModeConflict);
                }
            }
            return Err(OwnerError::DataRootAlreadyOwned);
        }
        RecoveryDecision::Quarantine => return Err(OwnerError::OwnerRecoveryQuarantined),
        RecoveryDecision::StartFresh
        | RecoveryDecision::CleanStaleRecordAndRetry
        | RecoveryDecision::TerminateVerifiedOrphanThenRetry => {}
    }

    let highest_epoch = maximum_epoch(previous_epoch, observation.highest_epoch());
    let epoch = next_owner_epoch(highest_epoch)?;
    let binding = OwnerBinding::new(request.root, request.owner, epoch);
    let expected_record = OwnerRecord::new_active(
        binding,
        request.token_digest,
        request.lease,
        request.initial_record_revision,
        request.planned_record_digest,
        request.operation.clone(),
    );

    let mut effects = Vec::with_capacity(4);
    match (recovery, observation) {
        (
            RecoveryDecision::CleanStaleRecordAndRetry,
            OwnerObservation::StaleRecordIdentityAbsent {
                record, evidence, ..
            },
        ) => {
            if evidence.observed_record_digest != record.record_digest() {
                return Err(OwnerError::OwnerRecordDigestMismatch);
            }
            effects.push(OwnerEffect::CleanStaleRecord {
                expected_record_digest: record.record_digest(),
                authorization_receipt: evidence.cleanup_authorization_receipt.clone(),
            });
        }
        (
            RecoveryDecision::TerminateVerifiedOrphanThenRetry,
            OwnerObservation::LiveMatchingOwner {
                record,
                status:
                    LiveOwnerStatus::VerifiedOwnedOrphan {
                        termination_authorization,
                        process_identity_receipt,
                    },
                ..
            },
        ) => effects.push(OwnerEffect::TerminateVerifiedOrphan {
            binding: record.binding(),
            authorization_receipt: termination_authorization.clone(),
            process_identity_receipt: process_identity_receipt.clone(),
        }),
        (RecoveryDecision::StartFresh, OwnerObservation::Absent { .. }) => {}
        _ => return Err(OwnerError::OwnerRecoveryQuarantined),
    }
    effects.extend([
        OwnerEffect::AcquireOwnershipPrimitive {
            binding,
            operation: request.operation.clone(),
        },
        OwnerEffect::WriteOwnerRecord {
            binding,
            record_revision: request.initial_record_revision,
            record_digest: request.planned_record_digest,
            operation: request.operation.clone(),
        },
        OwnerEffect::VerifyOwnerReadback {
            binding,
            record_digest: request.planned_record_digest,
            operation: request.operation,
        },
    ]);

    let pending = PendingAcquire {
        expected_record,
        stale_cleanup_required: recovery == RecoveryDecision::CleanStaleRecordAndRetry,
    };
    let next = snapshot.advanced(OwnerState::Acquiring {
        previous_epoch,
        pending: pending.clone(),
    })?;
    Ok((
        next,
        AcquirePlan {
            pending,
            recovery,
            effects: bounded_effects(effects)?,
        },
    ))
}

/// Completes or fences a prepared acquisition.
///
/// # Errors
///
/// Snapshot must be in `Acquiring` state.
pub fn complete_acquire(
    snapshot: &OwnerSnapshot,
    observation: AcquireCommitObservation,
) -> Result<(OwnerSnapshot, AcquireResolution), OwnerError> {
    let OwnerState::Acquiring {
        previous_epoch,
        pending,
    } = snapshot.state()
    else {
        return Err(OwnerError::OwnerInvalidTransition);
    };

    match observation {
        AcquireCommitObservation::VerifiedApplied {
            record,
            primitive_verified: true,
            readback_receipt,
        } if record == pending.expected_record => {
            let receipt = verification_receipt(&record, readback_receipt);
            let guard = OwnerGuard::from_record(&record);
            let next = snapshot.advanced(OwnerState::Active { record })?;
            Ok((next, AcquireResolution::Acquired { guard, receipt }))
        }
        AcquireCommitObservation::RejectedBeforeMutation => {
            let next = snapshot.advanced(OwnerState::Vacant {
                root: pending.expected_record.binding().root(),
                last_epoch: *previous_epoch,
            })?;
            Ok((next, AcquireResolution::RejectedBeforeMutation))
        }
        AcquireCommitObservation::CancelledBeforeMutation => {
            let next = snapshot.advanced(OwnerState::Vacant {
                root: pending.expected_record.binding().root(),
                last_epoch: *previous_epoch,
            })?;
            Ok((next, AcquireResolution::CancelledBeforeMutation))
        }
        AcquireCommitObservation::OutcomeUnknown => {
            let next = snapshot.advanced(OwnerState::AcquireOutcomeUnknown {
                previous_epoch: *previous_epoch,
                pending: pending.clone(),
            })?;
            Ok((next, AcquireResolution::OutcomeUnknown))
        }
        AcquireCommitObservation::VerifiedApplied { .. }
        | AcquireCommitObservation::Contradictory => quarantine_acquire(snapshot, pending),
    }
}

/// Recovers an acquisition whose external outcome was unknown.
///
/// # Errors
///
/// Snapshot must be `AcquireOutcomeUnknown`; only exact readback may reconstruct
/// a guard or prove non-application.
pub fn recover_acquisition(
    snapshot: &OwnerSnapshot,
    observation: &OwnerObservation,
) -> Result<(OwnerSnapshot, AcquireRecovery), OwnerError> {
    let OwnerState::AcquireOutcomeUnknown {
        previous_epoch,
        pending,
    } = snapshot.state()
    else {
        return Err(OwnerError::OwnerInvalidTransition);
    };

    if observation.root() != pending.expected_record.binding().root() {
        return quarantine_acquire_recovery(snapshot, pending);
    }
    match observation {
        OwnerObservation::LiveMatchingOwner {
            record,
            status: LiveOwnerStatus::Active,
            observation_receipt,
        } if record == &pending.expected_record => {
            let receipt = verification_receipt(record, observation_receipt.clone());
            let guard = OwnerGuard::from_record(record);
            let next = snapshot.advanced(OwnerState::Active {
                record: record.clone(),
            })?;
            Ok((next, AcquireRecovery::Reconstructed { guard, receipt }))
        }
        OwnerObservation::Absent { highest_epoch, .. }
            if highest_epoch
                .is_none_or(|value| value < pending.expected_record.binding().epoch()) =>
        {
            let next = snapshot.advanced(OwnerState::Vacant {
                root: pending.expected_record.binding().root(),
                last_epoch: *previous_epoch,
            })?;
            Ok((next, AcquireRecovery::NotApplied))
        }
        _ => quarantine_acquire_recovery(snapshot, pending),
    }
}

/// Verifies a process-local guard against an exact owner observation.
///
/// # Errors
///
/// Any binding, token, process, executable, epoch, record, or liveness mismatch
/// invalidates the guard.
pub fn verify_owner_guard(
    snapshot: &OwnerSnapshot,
    guard: &OwnerGuard,
    observation: &OwnerObservation,
) -> Result<OwnerVerificationReceipt, OwnerError> {
    let record = snapshot
        .state()
        .record()
        .ok_or(OwnerError::OwnerGuardMismatch)?;
    if !guard.verifies(record) || observation.root() != record.binding().root() {
        return Err(OwnerError::OwnerGuardMismatch);
    }
    let OwnerObservation::LiveMatchingOwner {
        record: observed,
        status: LiveOwnerStatus::Active,
        observation_receipt,
    } = observation
    else {
        return Err(OwnerError::OwnerIdentityAmbiguous);
    };
    if observed.binding().owner().process() != record.binding().owner().process() {
        return Err(OwnerError::OwnerProcessIdentityMismatch);
    }
    if observed.binding().owner().executable() != record.binding().owner().executable() {
        return Err(OwnerError::OwnerExecutableIdentityMismatch);
    }
    if observed.binding().epoch() != record.binding().epoch() {
        return Err(OwnerError::OwnerEpochMismatch);
    }
    if observed != record {
        return Err(OwnerError::OwnerRecordDigestMismatch);
    }
    Ok(verification_receipt(record, observation_receipt.clone()))
}

/// Accepts an externally persisted exact heartbeat record.
///
/// # Errors
///
/// Requires active state, matching guard, exact next revision, monotone lease,
/// and exact operation/readback identity.
pub fn renew_verified(
    snapshot: &OwnerSnapshot,
    guard: &OwnerGuard,
    operation: &OwnerOperation,
    observed_record: OwnerRecord,
    readback_receipt: ReceiptRef,
) -> Result<(OwnerSnapshot, RenewalReceipt), OwnerError> {
    let OwnerState::Active { record } = snapshot.state() else {
        return Err(OwnerError::OwnerInvalidTransition);
    };
    operation.verify_replay(record.last_operation())?;
    if !guard.verifies(record)
        || observed_record.binding() != record.binding()
        || observed_record.token_digest() != record.token_digest()
        || observed_record.lifecycle() != OwnerLifecycle::Active
        || observed_record.acquire_operation() != record.acquire_operation()
        || observed_record.last_operation() != operation
    {
        return Err(OwnerError::OwnerGuardMismatch);
    }
    verify_record_revision_advanced(record, &observed_record)?;
    if observed_record.lease().heartbeat_at() < record.lease().heartbeat_at()
        || observed_record.lease().expires_at() <= record.lease().expires_at()
    {
        return Err(OwnerError::OwnerHeartbeatRegression);
    }
    let receipt = RenewalReceipt {
        binding: observed_record.binding(),
        record_revision: observed_record.record_revision(),
        record_digest: observed_record.record_digest(),
        readback_receipt,
    };
    let next = snapshot.advanced(OwnerState::Active {
        record: observed_record,
    })?;
    Ok((next, receipt))
}

fn verification_receipt(
    record: &OwnerRecord,
    observation_receipt: ReceiptRef,
) -> OwnerVerificationReceipt {
    OwnerVerificationReceipt {
        binding: record.binding(),
        record_revision: record.record_revision(),
        record_digest: record.record_digest(),
        observation_receipt,
    }
}

fn quarantine_acquire(
    snapshot: &OwnerSnapshot,
    pending: &PendingAcquire,
) -> Result<(OwnerSnapshot, AcquireResolution), OwnerError> {
    let next = snapshot.advanced(OwnerState::Quarantined {
        root: pending.expected_record.binding().root(),
        last_epoch: Some(pending.expected_record.binding().epoch()),
        reason: OwnerError::OwnerRecoveryQuarantined,
    })?;
    Ok((
        next,
        AcquireResolution::Quarantined(OwnerError::OwnerRecoveryQuarantined),
    ))
}

fn quarantine_acquire_recovery(
    snapshot: &OwnerSnapshot,
    pending: &PendingAcquire,
) -> Result<(OwnerSnapshot, AcquireRecovery), OwnerError> {
    let next = snapshot.advanced(OwnerState::Quarantined {
        root: pending.expected_record.binding().root(),
        last_epoch: Some(pending.expected_record.binding().epoch()),
        reason: OwnerError::OwnerRecoveryQuarantined,
    })?;
    Ok((
        next,
        AcquireRecovery::Quarantined(OwnerError::OwnerRecoveryQuarantined),
    ))
}

fn verify_record_revision_advanced(
    current: &OwnerRecord,
    observed: &OwnerRecord,
) -> Result<(), OwnerError> {
    let expected = current
        .record_revision()
        .checked_next()
        .map_err(|_| OwnerError::ContractExhausted)?;
    if observed.record_revision() == expected {
        Ok(())
    } else {
        Err(OwnerError::OwnerEpochMismatch)
    }
}

fn next_owner_epoch(previous: Option<OwnerEpoch>) -> Result<OwnerEpoch, OwnerError> {
    match previous {
        Some(epoch) => epoch
            .checked_next()
            .map_err(|_| OwnerError::ContractExhausted),
        None => OwnerEpoch::new(1).map_err(|_| OwnerError::ContractExhausted),
    }
}

const fn maximum_epoch(left: Option<OwnerEpoch>, right: Option<OwnerEpoch>) -> Option<OwnerEpoch> {
    match (left, right) {
        (Some(left), Some(right)) if left.get() >= right.get() => Some(left),
        (Some(_), Some(right)) => Some(right),
        (Some(left), None) => Some(left),
        (None, Some(right)) => Some(right),
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use search_contracts::{
        ArtifactDigest, Blake3Digest32, DataRootId, InstallationId, InstallationIncarnationId,
        NonZeroRevision, OpaqueId, OwnerEpoch, ReceiptRef,
    };
    use search_ports::{IdempotencyClass, MonotonicInstant, MutationIdentity};

    use super::{
        AcquireCommitObservation, AcquireRequest, AcquireResolution, LiveOwnerStatus,
        OwnerObservation, RecoveryDecision, RecoveryEvidence, RecoveryPolicy,
        classify_abandoned_owner, complete_acquire, prepare_acquire,
    };
    use crate::{
        DataRootIdentity, DataRootLocationClass, ExecutableIdentity, LeaseWindow, OwnerError,
        OwnerIdentity, OwnerOperation, OwnerSnapshot, OwnerState, ProcessCreationIdentity,
        RuntimeMode,
    };

    fn root() -> DataRootIdentity {
        DataRootIdentity::new(
            DataRootId::from_bytes([1; 16]),
            DataRootLocationClass::LocalFixed,
            Blake3Digest32::from_bytes([2; 32]),
            Blake3Digest32::from_bytes([3; 32]),
        )
    }

    fn owner() -> OwnerIdentity {
        OwnerIdentity::new(
            InstallationId::from_bytes([4; 16]),
            InstallationIncarnationId::from_bytes([5; 16]),
            ProcessCreationIdentity::new(6, 7, Blake3Digest32::from_bytes([8; 32]))
                .expect("process"),
            ExecutableIdentity::new(
                ArtifactDigest::from_bytes([9; 32]),
                Blake3Digest32::from_bytes([10; 32]),
            ),
            RuntimeMode::Standalone,
        )
    }

    fn operation() -> OwnerOperation {
        OwnerOperation::new(
            MutationIdentity::new(
                OpaqueId::new("owner-operation:acquire").expect("id"),
                IdempotencyClass::RetrySameIdentity,
            ),
            Blake3Digest32::from_bytes([11; 32]),
        )
    }

    fn request() -> AcquireRequest {
        AcquireRequest {
            root: root(),
            owner: owner(),
            operation: operation(),
            token_digest: Blake3Digest32::from_bytes([12; 32]),
            lease: LeaseWindow::new(
                MonotonicInstant::from_ticks(1),
                MonotonicInstant::from_ticks(1),
                MonotonicInstant::from_ticks(10),
            )
            .expect("lease"),
            initial_record_revision: NonZeroRevision::new(1).expect("revision"),
            planned_record_digest: Blake3Digest32::from_bytes([13; 32]),
        }
    }

    fn absent() -> OwnerObservation {
        OwnerObservation::Absent {
            root: root(),
            highest_epoch: None,
            observation_receipt: ReceiptRef::new("receipt:absent").expect("receipt"),
        }
    }

    #[test]
    fn fresh_acquisition_uses_epoch_one() {
        let snapshot = OwnerSnapshot::new(root()).expect("snapshot");
        let (next, plan) = prepare_acquire(
            &snapshot,
            request(),
            &absent(),
            RecoveryPolicy::Conservative,
        )
        .expect("plan");
        assert_eq!(plan.pending.expected_record.binding().epoch().get(), 1);
        assert!(matches!(next.state(), OwnerState::Acquiring { .. }));
    }

    #[test]
    fn unknown_acquisition_is_not_success() {
        let snapshot = OwnerSnapshot::new(root()).expect("snapshot");
        let (prepared, _) = prepare_acquire(
            &snapshot,
            request(),
            &absent(),
            RecoveryPolicy::Conservative,
        )
        .expect("plan");
        let (unknown, resolution) =
            complete_acquire(&prepared, AcquireCommitObservation::OutcomeUnknown)
                .expect("transition");
        assert!(matches!(resolution, AcquireResolution::OutcomeUnknown));
        assert!(matches!(
            unknown.state(),
            OwnerState::AcquireOutcomeUnknown { .. }
        ));
    }

    #[test]
    fn timeout_alone_does_not_authorize_cleanup() {
        let snapshot = OwnerSnapshot::new(root()).expect("snapshot");
        let (_, plan) = prepare_acquire(
            &snapshot,
            request(),
            &absent(),
            RecoveryPolicy::Conservative,
        )
        .expect("plan");
        let stale = plan.pending.expected_record;
        let observation = OwnerObservation::StaleRecordIdentityAbsent {
            record: stale,
            highest_epoch: OwnerEpoch::new(1).expect("epoch"),
            evidence: RecoveryEvidence {
                observed_record_digest: Blake3Digest32::from_bytes([99; 32]),
                record_readback_receipt: ReceiptRef::new("receipt:record").expect("receipt"),
                process_absence_receipt: ReceiptRef::new("receipt:process").expect("receipt"),
                primitive_absence_receipt: ReceiptRef::new("receipt:primitive").expect("receipt"),
                cleanup_authorization_receipt: ReceiptRef::new("receipt:auth").expect("receipt"),
            },
        };
        assert_eq!(
            classify_abandoned_owner(&observation, RecoveryPolicy::Conservative),
            RecoveryDecision::Quarantine
        );
    }

    #[test]
    fn live_owner_is_denied() {
        let snapshot = OwnerSnapshot::new(root()).expect("snapshot");
        let (prepared, plan) = prepare_acquire(
            &snapshot,
            request(),
            &absent(),
            RecoveryPolicy::Conservative,
        )
        .expect("plan");
        let record = plan.pending.expected_record;
        let (active, _) = complete_acquire(
            &prepared,
            AcquireCommitObservation::VerifiedApplied {
                record: record.clone(),
                primitive_verified: true,
                readback_receipt: ReceiptRef::new("receipt:readback").expect("receipt"),
            },
        )
        .expect("complete");
        let observation = OwnerObservation::LiveMatchingOwner {
            record,
            status: LiveOwnerStatus::Active,
            observation_receipt: ReceiptRef::new("receipt:live").expect("receipt"),
        };
        assert_eq!(
            prepare_acquire(
                &active,
                request(),
                &observation,
                RecoveryPolicy::Conservative,
            ),
            Err(OwnerError::DataRootAlreadyOwned)
        );
    }
}

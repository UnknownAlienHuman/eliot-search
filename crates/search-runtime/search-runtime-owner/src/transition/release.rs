//! Drain, release, release recovery, and mode-change transitions.

use search_contracts::{Blake3Digest32, BoundedList, ReceiptRef};

use super::{MAX_OWNER_EFFECTS, OwnerEffect, bounded_effects};
use crate::lease::validate_dependency_receipts;
use crate::{
    DataRootIdentity, DependencyComponent, DependencyShutdownReceipt, DrainFence, DrainReason,
    DrainToken, OwnerError, OwnerGuard, OwnerLifecycle, OwnerOperation, OwnerRecord,
    OwnerShutdownReceipt, OwnerSnapshot, OwnerState, OwnerVerificationReceipt, ReleaseFence,
    ReleasePermit, RuntimeMode,
};

/// Prepared release and explicit crash-recoverable effects.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReleasePlan {
    /// Exact owner binding.
    pub binding: crate::OwnerBinding,
    /// Immutable release operation.
    pub operation: OwnerOperation,
    /// Finite ordered effects.
    pub effects: BoundedList<OwnerEffect, MAX_OWNER_EFFECTS>,
}

/// Observation after executing release effects.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReleaseCommitObservation {
    /// Durable record no longer claims ownership and primitive is released.
    VerifiedReleased {
        /// Exact release readback receipt.
        readback_receipt: ReceiptRef,
    },
    /// Exact draining record and primitive remain held.
    VerifiedStillOwned {
        /// Exact read-back owner record.
        record: OwnerRecord,
        /// Exact readback receipt.
        readback_receipt: ReceiptRef,
    },
    /// Release was rejected before mutation.
    RejectedBeforeMutation,
    /// At least one release mutation may have occurred.
    OutcomeUnknown,
    /// Readback is contradictory or incomplete.
    Contradictory,
}

/// Terminal release resolution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReleaseResolution {
    /// Exact owner no longer claims the root.
    Released(OwnerShutdownReceipt),
    /// Exact draining owner still holds the root.
    StillOwned,
    /// Release was rejected before mutation.
    RejectedBeforeMutation,
    /// Exact outcome requires recovery.
    OutcomeUnknown,
    /// State was quarantined.
    Quarantined(OwnerError),
}

/// Mode/root change classification.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ModeChangeDecision {
    /// Root and mode are unchanged.
    NoRestartRequired,
    /// Current owner must finish drain/release before restart.
    DrainAndRestart,
}

/// Accepts an exact durable active-to-draining transition.
///
/// # Errors
///
/// Requires matching guard, operation, reason, binding, token, lease, and exact
/// next record revision.
pub fn begin_drain(
    snapshot: &OwnerSnapshot,
    guard: &OwnerGuard,
    reason: DrainReason,
    operation: OwnerOperation,
    observed_record: OwnerRecord,
    readback_receipt: ReceiptRef,
) -> Result<(OwnerSnapshot, DrainToken, OwnerVerificationReceipt), OwnerError> {
    if let OwnerState::Draining { record, drain } = snapshot.state() {
        operation.verify_replay(&drain.operation)?;
        if operation == drain.operation && reason == drain.reason && guard.verifies(record) {
            return Ok((
                snapshot.clone(),
                DrainToken::new(
                    record.binding(),
                    drain.operation.clone(),
                    drain.reason,
                    record.record_revision(),
                ),
                verification_receipt(record, readback_receipt),
            ));
        }
        return Err(OwnerError::OwnerOperationConflict);
    }

    let OwnerState::Active { record } = snapshot.state() else {
        return Err(OwnerError::OwnerInvalidTransition);
    };
    operation.verify_replay(record.last_operation())?;
    if !guard.verifies(record)
        || observed_record.binding() != record.binding()
        || observed_record.token_digest() != record.token_digest()
        || observed_record.lifecycle() != OwnerLifecycle::Draining
        || observed_record.acquire_operation() != record.acquire_operation()
        || observed_record.last_operation() != &operation
        || observed_record.lease() != record.lease()
    {
        return Err(OwnerError::OwnerGuardMismatch);
    }
    verify_record_revision_advanced(record, &observed_record)?;

    let drain = DrainFence {
        operation: operation.clone(),
        reason,
    };
    let token = DrainToken::new(
        observed_record.binding(),
        operation,
        reason,
        observed_record.record_revision(),
    );
    let receipt = verification_receipt(&observed_record, readback_receipt);
    let next = snapshot.advanced(OwnerState::Draining {
        record: observed_record,
        drain,
    })?;
    Ok((next, token, receipt))
}

/// Verifies exact drain and dependency shutdown evidence.
///
/// # Errors
///
/// Missing, duplicate, foreign-owner, or incomplete dependency receipts reject
/// release.
pub fn verify_release_preconditions(
    snapshot: &OwnerSnapshot,
    guard: &OwnerGuard,
    drain_token: &DrainToken,
    receipts: &[DependencyShutdownReceipt],
    dependency_receipt_digest: Blake3Digest32,
) -> Result<ReleasePermit, OwnerError> {
    let OwnerState::Draining { record, drain } = snapshot.state() else {
        return Err(OwnerError::OwnerDrainRequired);
    };
    if !guard.verifies(record)
        || drain_token.binding() != record.binding()
        || drain_token.record_revision() != record.record_revision()
        || drain_token.operation() != &drain.operation
        || drain_token.reason() != drain.reason
    {
        return Err(OwnerError::OwnerGuardMismatch);
    }
    validate_dependency_receipts(record.binding(), receipts)?;
    Ok(ReleasePermit::new(
        record.binding(),
        drain.operation.clone(),
        dependency_receipt_digest,
        receipts.len(),
    ))
}

/// Prepares crash-recoverable release effects.
///
/// # Errors
///
/// Requires exact draining authority, complete permit, and a non-conflicting
/// immutable operation.
pub fn prepare_release(
    snapshot: &OwnerSnapshot,
    guard: &OwnerGuard,
    drain_token: &DrainToken,
    permit: &ReleasePermit,
    operation: OwnerOperation,
) -> Result<(OwnerSnapshot, ReleasePlan), OwnerError> {
    let OwnerState::Draining { record, drain } = snapshot.state() else {
        return Err(OwnerError::OwnerDrainRequired);
    };
    operation.verify_replay(&drain.operation)?;
    if !guard.verifies(record)
        || drain_token.binding() != record.binding()
        || drain_token.operation() != &drain.operation
        || permit.binding() != record.binding()
        || permit.drain_operation() != &drain.operation
        || permit.receipt_count() != DependencyComponent::REQUIRED.len()
    {
        return Err(OwnerError::OwnerReleasePreconditionMissing);
    }

    let effects = bounded_effects(vec![
        OwnerEffect::WriteReleaseIntent {
            binding: record.binding(),
            operation: operation.clone(),
        },
        OwnerEffect::ReleaseOwnershipPrimitive {
            binding: record.binding(),
            operation: operation.clone(),
        },
        OwnerEffect::VerifyReleaseReadback {
            binding: record.binding(),
            operation: operation.clone(),
        },
    ])?;
    let permit_fence = ReleaseFence {
        dependency_receipt_digest: permit.dependency_receipt_digest(),
        receipt_count: permit.receipt_count(),
    };
    let next = snapshot.advanced(OwnerState::Releasing {
        record: record.clone(),
        drain: drain.clone(),
        permit: permit_fence,
        operation: operation.clone(),
    })?;
    Ok((
        next,
        ReleasePlan {
            binding: record.binding(),
            operation,
            effects,
        },
    ))
}

/// Completes or fences a prepared release.
///
/// # Errors
///
/// Snapshot must be in `Releasing` state.
pub fn complete_release(
    snapshot: &OwnerSnapshot,
    observation: ReleaseCommitObservation,
) -> Result<(OwnerSnapshot, ReleaseResolution), OwnerError> {
    let OwnerState::Releasing {
        record,
        drain,
        permit,
        operation,
    } = snapshot.state()
    else {
        return Err(OwnerError::OwnerInvalidTransition);
    };
    if permit.receipt_count != DependencyComponent::REQUIRED.len() {
        return quarantine_release(snapshot, record);
    }

    match observation {
        ReleaseCommitObservation::VerifiedReleased { readback_receipt } => {
            finish_release(snapshot, record, permit, operation, readback_receipt)
        }
        ReleaseCommitObservation::VerifiedStillOwned {
            record: observed, ..
        } if &observed == record => {
            let next = snapshot.advanced(OwnerState::Draining {
                record: record.clone(),
                drain: drain.clone(),
            })?;
            Ok((next, ReleaseResolution::StillOwned))
        }
        ReleaseCommitObservation::RejectedBeforeMutation => {
            let next = snapshot.advanced(OwnerState::Draining {
                record: record.clone(),
                drain: drain.clone(),
            })?;
            Ok((next, ReleaseResolution::RejectedBeforeMutation))
        }
        ReleaseCommitObservation::OutcomeUnknown => {
            let next = snapshot.advanced(OwnerState::ReleaseOutcomeUnknown {
                record: record.clone(),
                drain: drain.clone(),
                permit: permit.clone(),
                operation: operation.clone(),
            })?;
            Ok((next, ReleaseResolution::OutcomeUnknown))
        }
        ReleaseCommitObservation::VerifiedStillOwned { .. }
        | ReleaseCommitObservation::Contradictory => quarantine_release(snapshot, record),
    }
}

/// Recovers an unknown release by exact readback.
///
/// # Errors
///
/// Snapshot must be in `ReleaseOutcomeUnknown` state.
pub fn recover_release(
    snapshot: &OwnerSnapshot,
    observation: ReleaseCommitObservation,
) -> Result<(OwnerSnapshot, ReleaseResolution), OwnerError> {
    let OwnerState::ReleaseOutcomeUnknown {
        record,
        drain,
        permit,
        operation,
    } = snapshot.state()
    else {
        return Err(OwnerError::OwnerInvalidTransition);
    };
    if permit.receipt_count != DependencyComponent::REQUIRED.len() {
        return quarantine_release(snapshot, record);
    }

    match observation {
        ReleaseCommitObservation::VerifiedReleased { readback_receipt } => {
            finish_release(snapshot, record, permit, operation, readback_receipt)
        }
        ReleaseCommitObservation::VerifiedStillOwned {
            record: observed, ..
        } if &observed == record => {
            let next = snapshot.advanced(OwnerState::Draining {
                record: record.clone(),
                drain: drain.clone(),
            })?;
            Ok((next, ReleaseResolution::StillOwned))
        }
        _ => quarantine_release(snapshot, record),
    }
}

/// Classifies a requested live root or mode change.
///
/// # Errors
///
/// Active changes require drain and restart; unresolved or quarantined states
/// fail closed.
pub fn plan_mode_or_root_change(
    snapshot: &OwnerSnapshot,
    requested_root: DataRootIdentity,
    requested_mode: RuntimeMode,
) -> Result<ModeChangeDecision, OwnerError> {
    match snapshot.state() {
        OwnerState::Active { record } => {
            if record.binding().root() == requested_root
                && record.binding().owner().mode() == requested_mode
            {
                Ok(ModeChangeDecision::NoRestartRequired)
            } else {
                Err(OwnerError::ModeTransitionRequiresRestart)
            }
        }
        OwnerState::Draining { .. }
        | OwnerState::Releasing { .. }
        | OwnerState::ReleaseOutcomeUnknown { .. } => Ok(ModeChangeDecision::DrainAndRestart),
        OwnerState::Vacant { .. } | OwnerState::Released { .. } => {
            Ok(ModeChangeDecision::NoRestartRequired)
        }
        OwnerState::Acquiring { .. } | OwnerState::AcquireOutcomeUnknown { .. } => {
            Err(OwnerError::OwnerAcquireOutcomeUnknown)
        }
        OwnerState::Quarantined { .. } => Err(OwnerError::OwnerRecoveryQuarantined),
    }
}

fn finish_release(
    snapshot: &OwnerSnapshot,
    record: &OwnerRecord,
    permit: &ReleaseFence,
    operation: &OwnerOperation,
    readback_receipt: ReceiptRef,
) -> Result<(OwnerSnapshot, ReleaseResolution), OwnerError> {
    let receipt = OwnerShutdownReceipt {
        binding: record.binding(),
        operation: operation.clone(),
        final_record_revision: record.record_revision(),
        dependency_receipt_digest: permit.dependency_receipt_digest,
        release_readback_receipt: readback_receipt,
    };
    let next = snapshot.advanced(OwnerState::Released {
        root: record.binding().root(),
        last_epoch: record.binding().epoch(),
        receipt: receipt.clone(),
    })?;
    Ok((next, ReleaseResolution::Released(receipt)))
}

fn quarantine_release(
    snapshot: &OwnerSnapshot,
    record: &OwnerRecord,
) -> Result<(OwnerSnapshot, ReleaseResolution), OwnerError> {
    let next = snapshot.advanced(OwnerState::Quarantined {
        root: record.binding().root(),
        last_epoch: Some(record.binding().epoch()),
        reason: OwnerError::OwnerRecoveryQuarantined,
    })?;
    Ok((
        next,
        ReleaseResolution::Quarantined(OwnerError::OwnerRecoveryQuarantined),
    ))
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

#[cfg(test)]
mod tests {
    use search_contracts::{
        ArtifactDigest, Blake3Digest32, DataRootId, InstallationId, InstallationIncarnationId,
        NonZeroRevision, OpaqueId, OwnerEpoch, ReceiptRef,
    };
    use search_ports::{IdempotencyClass, MonotonicInstant, MutationIdentity};

    use super::{
        ModeChangeDecision, ReleaseCommitObservation, ReleaseResolution, complete_release,
    };
    use crate::{
        DataRootIdentity, DataRootLocationClass, DrainFence, DrainReason, ExecutableIdentity,
        LeaseWindow, OwnerBinding, OwnerIdentity, OwnerOperation, OwnerRecord, OwnerSnapshot,
        OwnerState, ProcessCreationIdentity, ReleaseFence, RuntimeMode,
    };

    fn root() -> DataRootIdentity {
        DataRootIdentity::new(
            DataRootId::from_bytes([1; 16]),
            DataRootLocationClass::LocalFixed,
            Blake3Digest32::from_bytes([2; 32]),
            Blake3Digest32::from_bytes([3; 32]),
        )
    }

    fn operation(name: &str, byte: u8) -> OwnerOperation {
        OwnerOperation::new(
            MutationIdentity::new(
                OpaqueId::new(name).expect("id"),
                IdempotencyClass::RetrySameIdentity,
            ),
            Blake3Digest32::from_bytes([byte; 32]),
        )
    }

    fn record() -> OwnerRecord {
        let owner = OwnerIdentity::new(
            InstallationId::from_bytes([4; 16]),
            InstallationIncarnationId::from_bytes([5; 16]),
            ProcessCreationIdentity::new(6, 7, Blake3Digest32::from_bytes([8; 32]))
                .expect("process"),
            ExecutableIdentity::new(
                ArtifactDigest::from_bytes([9; 32]),
                Blake3Digest32::from_bytes([10; 32]),
            ),
            RuntimeMode::Standalone,
        );
        OwnerRecord::new_active(
            OwnerBinding::new(root(), owner, OwnerEpoch::new(1).expect("epoch")),
            Blake3Digest32::from_bytes([11; 32]),
            LeaseWindow::new(
                MonotonicInstant::from_ticks(1),
                MonotonicInstant::from_ticks(1),
                MonotonicInstant::from_ticks(10),
            )
            .expect("lease"),
            NonZeroRevision::new(1).expect("revision"),
            Blake3Digest32::from_bytes([12; 32]),
            operation("owner-op:acquire", 13),
        )
    }

    #[test]
    fn unknown_release_is_not_declared_success() {
        let record = record();
        let snapshot = OwnerSnapshot::new(root())
            .expect("snapshot")
            .advanced(OwnerState::Releasing {
                drain: DrainFence {
                    operation: operation("owner-op:drain", 14),
                    reason: DrainReason::Shutdown,
                },
                permit: ReleaseFence {
                    dependency_receipt_digest: Blake3Digest32::from_bytes([15; 32]),
                    receipt_count: 4,
                },
                operation: operation("owner-op:release", 16),
                record,
            })
            .expect("releasing");
        let (next, result) = complete_release(&snapshot, ReleaseCommitObservation::OutcomeUnknown)
            .expect("transition");
        assert_eq!(result, ReleaseResolution::OutcomeUnknown);
        assert!(matches!(
            next.state(),
            OwnerState::ReleaseOutcomeUnknown { .. }
        ));
    }

    #[test]
    fn mode_decision_type_is_closed() {
        assert_ne!(
            ModeChangeDecision::NoRestartRequired,
            ModeChangeDecision::DrainAndRestart
        );
    }

    #[test]
    fn receipt_reference_is_bounded_by_contract() {
        let receipt = ReceiptRef::new("receipt:release").expect("receipt");
        assert_eq!(receipt.as_str(), "receipt:release");
    }
}

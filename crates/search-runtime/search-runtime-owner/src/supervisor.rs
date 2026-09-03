//! Stateful in-memory executor for the pure owner transition functions.
//!
//! The supervisor owns no clock, filesystem, process, database, or network
//! capability. Callers execute returned effects through qualified adapters and
//! feed exact observations back into this package.

use search_contracts::{Blake3Digest32, ReceiptRef};

use crate::{
    AcquireCommitObservation, AcquirePlan, AcquireRecovery, AcquireRequest, AcquireResolution,
    DataRootIdentity, DependencyShutdownReceipt, DrainReason, DrainToken, ModeChangeDecision,
    OwnerError, OwnerGuard, OwnerHealth, OwnerObservation, OwnerOperation, OwnerRecord,
    OwnerShutdownReceipt, OwnerSnapshot, OwnerVerificationReceipt, RecoveryPolicy,
    ReleaseCommitObservation, ReleasePermit, ReleasePlan, ReleaseResolution, RenewalReceipt,
    RuntimeMode, begin_drain, complete_acquire, complete_release, owner_health,
    plan_mode_or_root_change, prepare_acquire, prepare_release, recover_acquisition,
    recover_release, renew_verified, verify_owner_guard, verify_release_preconditions,
};

/// In-memory owner transition executor with an immutable revisioned snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OwnerSupervisor {
    snapshot: OwnerSnapshot,
}

impl OwnerSupervisor {
    /// Creates a vacant owner supervisor for one resolved root.
    ///
    /// # Errors
    ///
    /// Returns [`OwnerError::ContractExhausted`] if the shared revision type
    /// cannot represent the initial snapshot revision.
    pub fn new(root: DataRootIdentity) -> Result<Self, OwnerError> {
        Ok(Self {
            snapshot: OwnerSnapshot::new(root)?,
        })
    }

    /// Creates a supervisor from an exact persisted or recovered snapshot.
    #[must_use]
    pub const fn from_snapshot(snapshot: OwnerSnapshot) -> Self {
        Self { snapshot }
    }

    /// Returns the current immutable snapshot.
    #[must_use]
    pub const fn snapshot(&self) -> &OwnerSnapshot {
        &self.snapshot
    }

    /// Prepares acquisition and stores the pending snapshot.
    ///
    /// # Errors
    ///
    /// Propagates fail-closed acquisition and recovery errors.
    pub fn prepare_acquire(
        &mut self,
        request: AcquireRequest,
        observation: &OwnerObservation,
        policy: RecoveryPolicy,
    ) -> Result<AcquirePlan, OwnerError> {
        let (next, plan) = prepare_acquire(&self.snapshot, request, observation, policy)?;
        self.snapshot = next;
        Ok(plan)
    }

    /// Completes a prepared acquisition from exact external observations.
    ///
    /// # Errors
    ///
    /// Rejects invalid lifecycle state or inconsistent readback.
    pub fn complete_acquire(
        &mut self,
        observation: AcquireCommitObservation,
    ) -> Result<AcquireResolution, OwnerError> {
        let (next, resolution) = complete_acquire(&self.snapshot, observation)?;
        self.snapshot = next;
        Ok(resolution)
    }

    /// Resolves an unknown acquisition using authoritative readback only.
    ///
    /// # Errors
    ///
    /// Rejects invalid lifecycle state; ambiguous evidence is quarantined.
    pub fn recover_acquisition(
        &mut self,
        observation: &OwnerObservation,
    ) -> Result<AcquireRecovery, OwnerError> {
        let (next, recovery) = recover_acquisition(&self.snapshot, observation)?;
        self.snapshot = next;
        Ok(recovery)
    }

    /// Verifies a process-local guard against a complete observation.
    ///
    /// # Errors
    ///
    /// Any identity, epoch, token, record, or liveness mismatch is rejected.
    pub fn verify_guard(
        &self,
        guard: &OwnerGuard,
        observation: &OwnerObservation,
    ) -> Result<OwnerVerificationReceipt, OwnerError> {
        verify_owner_guard(&self.snapshot, guard, observation)
    }

    /// Accepts an externally persisted and read-back heartbeat record.
    ///
    /// # Errors
    ///
    /// Requires exact guard, operation, binding, revision, and monotone lease.
    pub fn renew_verified(
        &mut self,
        guard: &OwnerGuard,
        operation: &OwnerOperation,
        observed_record: OwnerRecord,
        readback_receipt: ReceiptRef,
    ) -> Result<RenewalReceipt, OwnerError> {
        let (next, receipt) = renew_verified(
            &self.snapshot,
            guard,
            operation,
            observed_record,
            readback_receipt,
        )?;
        self.snapshot = next;
        Ok(receipt)
    }

    /// Accepts an exact durable active-to-draining transition.
    ///
    /// # Errors
    ///
    /// Requires matching guard and exact next owner record.
    pub fn begin_drain(
        &mut self,
        guard: &OwnerGuard,
        reason: DrainReason,
        operation: OwnerOperation,
        observed_record: OwnerRecord,
        readback_receipt: ReceiptRef,
    ) -> Result<(DrainToken, OwnerVerificationReceipt), OwnerError> {
        let (next, token, receipt) = begin_drain(
            &self.snapshot,
            guard,
            reason,
            operation,
            observed_record,
            readback_receipt,
        )?;
        self.snapshot = next;
        Ok((token, receipt))
    }

    /// Validates drain identity and complete dependency-shutdown evidence.
    ///
    /// # Errors
    ///
    /// Missing, duplicate, or foreign dependency receipts are rejected.
    pub fn verify_release_preconditions(
        &self,
        guard: &OwnerGuard,
        drain_token: &DrainToken,
        receipts: &[DependencyShutdownReceipt],
        dependency_receipt_digest: Blake3Digest32,
    ) -> Result<ReleasePermit, OwnerError> {
        verify_release_preconditions(
            &self.snapshot,
            guard,
            drain_token,
            receipts,
            dependency_receipt_digest,
        )
    }

    /// Prepares the crash-recoverable release effect sequence.
    ///
    /// # Errors
    ///
    /// Requires exact draining authority, permit, and mutation identity.
    pub fn prepare_release(
        &mut self,
        guard: &OwnerGuard,
        drain_token: &DrainToken,
        permit: &ReleasePermit,
        operation: OwnerOperation,
    ) -> Result<ReleasePlan, OwnerError> {
        let (next, plan) = prepare_release(&self.snapshot, guard, drain_token, permit, operation)?;
        self.snapshot = next;
        Ok(plan)
    }

    /// Completes or fences a prepared release.
    ///
    /// # Errors
    ///
    /// Rejects invalid lifecycle state; contradictions quarantine the root.
    pub fn complete_release(
        &mut self,
        observation: ReleaseCommitObservation,
    ) -> Result<ReleaseResolution, OwnerError> {
        let (next, resolution) = complete_release(&self.snapshot, observation)?;
        self.snapshot = next;
        Ok(resolution)
    }

    /// Resolves an unknown release using exact readback.
    ///
    /// # Errors
    ///
    /// Rejects invalid lifecycle state; incomplete evidence quarantines.
    pub fn recover_release(
        &mut self,
        observation: ReleaseCommitObservation,
    ) -> Result<ReleaseResolution, OwnerError> {
        let (next, resolution) = recover_release(&self.snapshot, observation)?;
        self.snapshot = next;
        Ok(resolution)
    }

    /// Returns a bounded content-minimized health view.
    #[must_use]
    pub fn health(&self, observation: &OwnerObservation) -> OwnerHealth {
        owner_health(&self.snapshot, observation)
    }

    /// Classifies whether a root or mode change requires drain and restart.
    ///
    /// # Errors
    ///
    /// Active changes and unresolved or quarantined states fail closed.
    pub fn plan_mode_or_root_change(
        &self,
        root: DataRootIdentity,
        mode: RuntimeMode,
    ) -> Result<ModeChangeDecision, OwnerError> {
        plan_mode_or_root_change(&self.snapshot, root, mode)
    }

    /// Returns the last clean shutdown receipt when released.
    #[must_use]
    pub fn shutdown_receipt(&self) -> Option<&OwnerShutdownReceipt> {
        match self.snapshot.state() {
            crate::OwnerState::Released { receipt, .. } => Some(receipt),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use search_contracts::{Blake3Digest32, DataRootId};

    use super::OwnerSupervisor;
    use crate::{DataRootIdentity, DataRootLocationClass, OwnerState};

    fn root() -> DataRootIdentity {
        DataRootIdentity::new(
            DataRootId::from_bytes([1; 16]),
            DataRootLocationClass::LocalFixed,
            Blake3Digest32::from_bytes([2; 32]),
            Blake3Digest32::from_bytes([3; 32]),
        )
    }

    #[test]
    fn supervisor_starts_vacant_at_revision_one() {
        let supervisor = OwnerSupervisor::new(root()).expect("supervisor");
        assert_eq!(supervisor.snapshot().revision().get(), 1);
        assert!(matches!(
            supervisor.snapshot().state(),
            OwnerState::Vacant { .. }
        ));
    }
}

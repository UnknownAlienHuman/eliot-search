//! Same-owner registry calls with an original borrowed native meter.
use super::super::{
    Boundary,
    bootstrap::OriginalCheck,
    operation::{Check, Point},
};
use super::{BoundControlJournal, require_identity};
use crate::{
    CommitRecoveryDecision, ControlError, MigrationBootstrapFailure, MigrationBootstrapWork,
    RegistryCommitReadback, RegistryMutationRequest, RegistryOperationEnvelope,
    RegistryOperationLookup, RegistryReadSnapshot, RegistryStateKey, RegistryTransactionReadback,
};
use search_contracts::OpaqueId;
use search_ports::{CancellationProbe, MutationIdentity};
use std::time::Instant;

impl<C: CancellationProbe> BoundControlJournal<'_, C> {
    /// Account the original registry commit and suspend this same publisher after dispatch.
    /// # Errors
    /// Identity/context refusal precedes mutation; uncertain results require exact recovery.
    pub fn commit_registry_batch_with_original_context<F, E>(
        &mut self,
        request: &RegistryMutationRequest,
        mutation: &MutationIdentity,
        deadline: Instant,
        work: &F,
    ) -> Result<RegistryCommitReadback, MigrationBootstrapFailure<E>>
    where
        F: Fn(MigrationBootstrapWork) -> Result<(), E>,
    {
        let check = OriginalCheck::registry(deadline, work);
        let result = (|| {
            check.check(Point::Start)?;
            require_identity(request.control_operation_id(), mutation)?;
            self.journal
                .commit_registry_checked(request, Boundary::Normal, &check)
        })();
        self.finish_dispatch(result)
            .map_err(|error| check.failure(error))
    }

    /// Targeted operation lookup against the original native allowance.
    /// # Errors
    /// Refusal/unknown state never proves absence or publishes a snapshot.
    pub fn lookup_registry_operation_with_original_context<F, E>(
        &self,
        id: &OpaqueId,
        deadline: Instant,
        work: &F,
    ) -> Result<RegistryOperationLookup, MigrationBootstrapFailure<E>>
    where
        F: Fn(MigrationBootstrapWork) -> Result<(), E>,
    {
        self.journal
            .lookup_registry_operation_with_original_context(id, deadline, work)
    }

    /// One coherent exact-operation/targets read using original native accounting.
    /// # Errors
    /// Partial, missing, changed or interrupted targets refuse without partial output.
    pub fn read_registry_operation_targets_with_original_context<F, E>(
        &self,
        envelope: &RegistryOperationEnvelope,
        keys: &[RegistryStateKey],
        deadline: Instant,
        work: &F,
    ) -> Result<RegistryTransactionReadback, MigrationBootstrapFailure<E>>
    where
        F: Fn(MigrationBootstrapWork) -> Result<(), E>,
    {
        self.journal
            .read_registry_operation_targets_with_original_context(envelope, keys, deadline, work)
    }

    /// Full registry recovery metadata from this same journal, without publication.
    /// # Errors
    /// Refusal, pending, inert or corrupt state cannot become restore authority.
    pub fn read_registry_snapshot_with_original_context<F, E>(
        &self,
        deadline: Instant,
        work: &F,
    ) -> Result<RegistryReadSnapshot, MigrationBootstrapFailure<E>>
    where
        F: Fn(MigrationBootstrapWork) -> Result<(), E>,
    {
        self.journal
            .read_registry_snapshot_with_original_context(deadline, work)
    }

    /// Recover the exact original registry command; keep this publisher suspended.
    /// # Errors
    /// Foreign input cannot clear pending work; interrupted recovery stays unknown.
    pub fn recover_registry_batch_with_original_context<F, E>(
        &mut self,
        request: &RegistryMutationRequest,
        mutation: &MutationIdentity,
        deadline: Instant,
        work: &F,
    ) -> Result<CommitRecoveryDecision, MigrationBootstrapFailure<E>>
    where
        F: Fn(MigrationBootstrapWork) -> Result<(), E>,
    {
        let check = OriginalCheck::registry(deadline, work);
        let result = (|| {
            check.check(Point::Start)?;
            require_identity(request.control_operation_id(), mutation)?;
            self.journal.recover_registry_checked(request, &check)
        })();
        let result = self.finish_dispatch(result);
        result.map_err(|error| {
            let error = if check.interrupted() {
                ControlError::CommitOutcomeUnknown
            } else {
                error
            };
            check.failure(error)
        })
    }
}

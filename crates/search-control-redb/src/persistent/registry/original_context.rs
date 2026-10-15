//! Original absolute-deadline/native-meter entrypoints over the same registry engines.
use super::super::{Boundary, PersistentControlJournal, bootstrap::OriginalCheck};
use super::{
    RegistryCommitReadback, RegistryMutationRequest, RegistryOperationEnvelope,
    RegistryOperationLookup, RegistryReadSnapshot, RegistryStateKey, RegistryTransactionReadback,
};
use crate::{CommitRecoveryDecision, MigrationBootstrapFailure, MigrationBootstrapWork};
use search_contracts::OpaqueId;
use std::time::Instant;

impl PersistentControlJournal {
    /// Commit through the original borrowed native work meter and absolute deadline.
    /// # Errors
    /// First refusal is sticky; possible dispatch/readback failures stay unknown.
    pub fn commit_registry_batch_with_original_context<F, E>(
        &mut self,
        request: &RegistryMutationRequest,
        deadline: Instant,
        work: &F,
    ) -> Result<RegistryCommitReadback, MigrationBootstrapFailure<E>>
    where
        F: Fn(MigrationBootstrapWork) -> Result<(), E>,
    {
        let check = OriginalCheck::registry(deadline, work);
        self.commit_registry_checked(request, Boundary::Normal, &check)
            .map_err(|error| check.failure(error))
    }

    /// Target one immutable operation without renewing the original native allowance.
    /// # Errors
    /// Context refusal, unknown or corrupt state never proves absence.
    pub fn lookup_registry_operation_with_original_context<F, E>(
        &self,
        id: &OpaqueId,
        deadline: Instant,
        work: &F,
    ) -> Result<RegistryOperationLookup, MigrationBootstrapFailure<E>>
    where
        F: Fn(MigrationBootstrapWork) -> Result<(), E>,
    {
        let check = OriginalCheck::registry(deadline, work);
        self.lookup_registry_checked(id, &check)
            .map_err(|error| check.failure(error))
    }

    /// Read the exact operation and all requested targets in one accounted snapshot.
    /// # Errors
    /// Refusals and changed/missing/duplicate targets return no partial result.
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
        let check = OriginalCheck::registry(deadline, work);
        self.read_registry_targets_checked(envelope, keys, &check)
            .map_err(|error| check.failure(error))
    }

    /// Read full bounded restart metadata against the original native work allowance.
    /// This is recovery metadata, not source/native restoration authority.
    /// # Errors
    /// Partial, pending, inert, interrupted or oversized state refuses.
    pub fn read_registry_snapshot_with_original_context<F, E>(
        &self,
        deadline: Instant,
        work: &F,
    ) -> Result<RegistryReadSnapshot, MigrationBootstrapFailure<E>>
    where
        F: Fn(MigrationBootstrapWork) -> Result<(), E>,
    {
        let check = OriginalCheck::registry(deadline, work);
        self.registry_snapshot_checked(&check)
            .map_err(|error| check.failure(error))
    }

    /// Inspect only the original command; never redispatch it or renew native work.
    /// # Errors
    /// Changed input cannot clear pending state; interrupted recovery stays unknown.
    pub fn recover_registry_batch_with_original_context<F, E>(
        &mut self,
        request: &RegistryMutationRequest,
        deadline: Instant,
        work: &F,
    ) -> Result<CommitRecoveryDecision, MigrationBootstrapFailure<E>>
    where
        F: Fn(MigrationBootstrapWork) -> Result<(), E>,
    {
        let check = OriginalCheck::registry(deadline, work);
        let result = self.recover_registry_checked(request, &check);
        result.map_err(|error| {
            let error = if check.interrupted() {
                crate::ControlError::CommitOutcomeUnknown
            } else {
                error
            };
            check.failure(error)
        })
    }
}

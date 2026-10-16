//! Existing conditional records under an original borrowed native allowance.

use super::super::{
    Boundary,
    bootstrap::OriginalCheck,
    operation::{Check, Point},
};
use super::{BoundControlJournal, require_identity};
use crate::{
    CommitRecoveryDecision, ConditionalControlMutation, ControlCommitReceipt, ControlError,
    JournalReadSnapshot, MigrationBootstrapFailure, MigrationBootstrapWork,
};
use search_ports::{CancellationProbe, MutationIdentity};
use std::time::Instant;

impl<C: CancellationProbe> BoundControlJournal<'_, C> {
    /// Observes a historical selection-phase receipt, never backend/application proof.
    /// The original CAS generation is `before_generation`; exact command recovery
    /// must still verify the stored full request hash before acknowledgement.
    /// # Errors
    /// A pending/interrupted/corrupt read or a CID used outside the two selection
    /// keys is never absence. This read neither writes nor clears pending state.
    pub fn revision_dek_selection_operation_with_original_context<F, E>(
        &self, id: crate::MutationId, deadline: Instant, work: &F,
    ) -> Result<Option<ControlCommitReceipt>, MigrationBootstrapFailure<E>>
    where F: Fn(MigrationBootstrapWork) -> Result<(), E>,
    {
        let check = OriginalCheck::registry(deadline, work);
        let result = (|| {
            check.check(Point::Start)?;
            self.journal.ensure_available()?;
            check.check(Point::ReadHeader)?;
            let read = self.journal.database.begin_read().map_err(|_| ControlError::StoreUnavailable)?;
            let header = self.journal.header_from_checked(&read, &check)?;
            let operation = super::super::operation_from_checked(&read, id, &header, self.journal.limits, &check)?;
            let receipt = operation.map(|operation| {
                let receipt = operation.receipt;
                let [key] = receipt.changed_keys.as_slice() else { return Err(ControlError::OperationConflict); };
                if !matches!(key.as_bytes(), crate::REVISION_DEK_APPLICATION_INTENT_KEY | crate::REVISION_DEK_CURRENT_KEY) {
                    return Err(ControlError::OperationConflict);
                }
                Ok(receipt)
            }).transpose()?;
            check.check(Point::ReadComplete)?;
            Ok(receipt)
        })();
        result.map_err(|error| check.failure(error))
    }

    /// Commit one closed first-selection phase under the original allowance.
    /// # Errors
    /// Failed guards refuse; uncertain effects retain the exact command/fence.
    pub fn transact_revision_dek_selection_with_original_context<F, E>(
        &mut self, command: &crate::RevisionDekSelectionCommand, mutation: &MutationIdentity,
        deadline: Instant, work: &F,
    ) -> Result<ControlCommitReceipt, MigrationBootstrapFailure<E>>
    where F: Fn(MigrationBootstrapWork) -> Result<(), E>,
    {
        let check = OriginalCheck::registry(deadline, work);
        let result = (|| {
            check.check(Point::Start)?;
            require_identity(command.command().mutation().id(), mutation)?;
            self.validate_revision_dek_records(command.command(), &check)?;
            check.check(Point::Validated)?;
            super::super::transaction::execute_revision_dek(self.journal, command.command(), Boundary::Normal, &check)
        })();
        self.finish_dispatch(result).map_err(|error| check.failure(error))
    }

    /// Recover the identical selection phase; historical receipts confer no availability.
    /// # Errors
    /// Foreign/interrupted recovery cannot clear the original pending fence.
    pub fn recover_revision_dek_selection_with_original_context<F, E>(
        &mut self, command: &crate::RevisionDekSelectionCommand, mutation: &MutationIdentity,
        deadline: Instant, work: &F,
    ) -> Result<CommitRecoveryDecision, MigrationBootstrapFailure<E>>
    where F: Fn(MigrationBootstrapWork) -> Result<(), E>,
    {
        let check = OriginalCheck::registry(deadline, work);
        let result = (|| {
            check.check(Point::Start)?;
            require_identity(command.command().mutation().id(), mutation)?;
            self.validate_revision_dek_records(command.command(), &check)?;
            self.journal.recover_transaction_with_conditions_checked(
                command.command().mutation(), command.command().conditions(), &check)
        })();
        self.finish_dispatch(result).map_err(|error| {
            let error = if check.interrupted() { ControlError::CommitOutcomeUnknown } else { error };
            check.failure(error)
        })
    }

    /// True fences fresh entropy only; false is not preparation or key authority.
    /// # Errors
    /// Pending, corrupt or interrupted reads prove no absence and clear no fence.
    pub fn revision_dek_operation_exists_with_original_context<F, E>(
        &self, id: crate::MutationId, deadline: Instant, work: &F,
    ) -> Result<bool, MigrationBootstrapFailure<E>>
    where F: Fn(MigrationBootstrapWork) -> Result<(), E>,
    {
        let check = OriginalCheck::registry(deadline, work);
        let result = (|| {
            check.check(Point::Start)?;
            self.journal.ensure_available()?;
            check.check(Point::ReadHeader)?;
            let read = self.journal.database.begin_read().map_err(|_| ControlError::StoreUnavailable)?;
            let header = self.journal.header_from_checked(&read, &check)?;
            let observed = super::super::operation_from_checked(&read, id, &header, self.journal.limits, &check)?;
            check.check(Point::ReadComplete)?;
            Ok(observed.is_some())
        })();
        result.map_err(|error| check.failure(error))
    }

    /// Commit one closed revision-DEK command using the actual original allowance.
    /// # Errors
    /// Identity/bounds/context refusal precedes effects; uncertain writes remain
    /// pending and publisher admission stays suspended until exact recovery.
    pub fn transact_revision_dek_with_original_context<F, E>(
        &mut self, command: &crate::RevisionDekCommand, mutation: &MutationIdentity,
        deadline: Instant, work: &F,
    ) -> Result<ControlCommitReceipt, MigrationBootstrapFailure<E>>
    where F: Fn(MigrationBootstrapWork) -> Result<(), E>,
    {
        let check = OriginalCheck::registry(deadline, work);
        let result = (|| {
            check.check(Point::Start)?;
            require_identity(command.command().mutation().id(), mutation)?;
            self.validate_revision_dek_records(command.command(), &check)?;
            check.check(Point::Validated)?;
            super::super::transaction::execute_revision_dek(self.journal, command.command(), Boundary::Normal, &check)
        })();
        self.finish_dispatch(result).map_err(|error| check.failure(error))
    }

    /// Recover the retained exact revision-DEK phase without redispatch or entropy.
    /// # Errors
    /// Foreign/interrupted recovery cannot clear the original pending fence.
    pub fn recover_revision_dek_with_original_context<F, E>(
        &mut self, command: &crate::RevisionDekCommand, mutation: &MutationIdentity,
        deadline: Instant, work: &F,
    ) -> Result<CommitRecoveryDecision, MigrationBootstrapFailure<E>>
    where F: Fn(MigrationBootstrapWork) -> Result<(), E>,
    {
        let check = OriginalCheck::registry(deadline, work);
        let result = (|| {
            check.check(Point::Start)?;
            require_identity(command.command().mutation().id(), mutation)?;
            self.validate_revision_dek_records(command.command(), &check)?;
            self.journal.recover_transaction_with_conditions_checked(
                command.command().mutation(), command.command().conditions(), &check)
        })();
        self.finish_dispatch(result).map_err(|error| {
            let error = if check.interrupted() { ControlError::CommitOutcomeUnknown } else { error };
            check.failure(error)
        })
    }

    fn validate_revision_dek_records(&self, command: &ConditionalControlMutation, check: &dyn Check)
        -> Result<(), ControlError> {
        super::super::validate_mutation(command.mutation(), self.journal.limits)?;
        crate::conditions::validate_conditions(command.mutation(), command.conditions(), self.journal.limits,
            || check.check(Point::PlanRecord))
    }

    /// Commit the existing exact conditional command with the original context.
    ///
    /// This reuses protected-key validation, complete-byte fingerprinting and
    /// post-commit readback. It creates no source-policy or schema authority.
    ///
    /// # Errors
    /// Context/identity/precondition refusal precedes mutation. An uncertain
    /// dispatched write remains pending and requires exact recovery.
    pub fn transact_records_with_original_context<F, E>(
        &mut self,
        command: &ConditionalControlMutation,
        mutation: &MutationIdentity,
        deadline: Instant,
        work: &F,
    ) -> Result<ControlCommitReceipt, MigrationBootstrapFailure<E>>
    where
        F: Fn(MigrationBootstrapWork) -> Result<(), E>,
    {
        let check = OriginalCheck::registry(deadline, work);
        let result = (|| {
            check.check(Point::Start)?;
            require_identity(command.mutation().id(), mutation)?;
            self.validate_record_command(command, &check)?;
            check.check(Point::Validated)?;
            self.journal
                .transact_conditionally_checked(command.clone(), Boundary::Normal, &check)
        })();
        self.finish_dispatch(result)
            .map_err(|error| check.failure(error))
    }

    /// Read one coherent technical snapshot using the same original allowance.
    ///
    /// # Errors
    /// Interrupted or unavailable reads return no partial snapshot or grant.
    pub fn read_control_snapshot_with_original_context<F, E>(
        &self,
        deadline: Instant,
        work: &F,
    ) -> Result<JournalReadSnapshot, MigrationBootstrapFailure<E>>
    where
        F: Fn(MigrationBootstrapWork) -> Result<(), E>,
    {
        let check = OriginalCheck::registry(deadline, work);
        self.journal
            .read_snapshot_checked(&check)
            .map_err(|error| check.failure(error))
    }

    /// Recover the identical conditional command without redispatch or new identity.
    ///
    /// # Errors
    /// Foreign requests cannot clear pending work; interrupted recovery remains
    /// unknown and retains the original caller cause and publisher suspension.
    pub fn recover_records_with_original_context<F, E>(
        &mut self,
        command: &ConditionalControlMutation,
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
            require_identity(command.mutation().id(), mutation)?;
            self.validate_record_command(command, &check)?;
            self.journal.recover_transaction_with_conditions_checked(
                command.mutation(),
                command.conditions(),
                &check,
            )
        })();
        self.finish_dispatch(result).map_err(|error| {
            let error = if check.interrupted() {
                ControlError::CommitOutcomeUnknown
            } else {
                error
            };
            check.failure(error)
        })
    }
}

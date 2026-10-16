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
            self.validate_revision_dek_command(command, &check)?;
            check.check(Point::Validated)?;
            super::super::transaction::execute_revision_dek(self.journal, command, Boundary::Normal, &check)
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
            self.validate_revision_dek_command(command, &check)?;
            self.journal.recover_transaction_with_conditions_checked(
                command.command().mutation(), command.command().conditions(), &check)
        })();
        self.finish_dispatch(result).map_err(|error| {
            let error = if check.interrupted() { ControlError::CommitOutcomeUnknown } else { error };
            check.failure(error)
        })
    }

    fn validate_revision_dek_command(&self, command: &crate::RevisionDekCommand, check: &dyn Check)
        -> Result<(), ControlError> {
        let command = command.command();
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

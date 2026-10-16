//! Closed revision-use dispatch sharing the existing checked transaction engine.

use super::super::{Boundary, bootstrap::OriginalCheck, operation::{Check, Point}};
use super::{BoundControlJournal, require_identity};
use crate::{CommitRecoveryDecision, ControlCommitReceipt, ControlError,
    MigrationBootstrapFailure, MigrationBootstrapWork};
use crate::source_retention::{CommittedRevisionSealUse, RevisionUseCommand};
use search_ports::{CancellationProbe, MutationIdentity};
use std::time::Instant;

impl<C: CancellationProbe> BoundControlJournal<'_, C> {
    /// Commits exact genesis/material/use metadata under the original allowance.
    /// Only a fresh successfully acknowledged reservation returns a use permit.
    /// Replay returns its historical receipt and no permit.
    /// # Errors
    /// Unknown/post-commit refusal never issues a permit and retains the existing
    /// pending fence. Foreign journal or operation identity refuses before effects.
    pub fn transact_revision_use_with_original_context<F, E>(
        &mut self, command: &RevisionUseCommand, mutation: &MutationIdentity,
        deadline: Instant, work: &F,
    ) -> Result<(ControlCommitReceipt, Option<CommittedRevisionSealUse>), MigrationBootstrapFailure<E>>
    where F: Fn(MigrationBootstrapWork) -> Result<(), E>,
    {
        let check = OriginalCheck::registry(deadline, work);
        let result = (|| {
            check.check(Point::Start)?;
            if command.identity() != self.journal.identity { return Err(ControlError::IdentityMismatch); }
            require_identity(command.command().mutation().id(), mutation)?;
            // Prepay the single opaque handoff before any possible commit.
            check.reserve(MigrationBootstrapWork::Allocate(command.permit_allocation()))?;
            super::super::transaction::execute_revision_use(self.journal, command.command(), Boundary::Normal, &check)
        })();
        let receipt = self.finish_dispatch(result).map_err(|error| check.failure(error))?;
        let permit = if receipt.replayed { None } else { command.fresh_permit() };
        Ok((receipt, permit))
    }

    /// Recovers only the exact original command; a burned use is never reissued.
    /// # Errors
    /// Interrupted or foreign recovery cannot clear pending state or authorize AES.
    pub fn recover_revision_use_with_original_context<F, E>(
        &mut self, command: &RevisionUseCommand, mutation: &MutationIdentity,
        deadline: Instant, work: &F,
    ) -> Result<CommitRecoveryDecision, MigrationBootstrapFailure<E>>
    where F: Fn(MigrationBootstrapWork) -> Result<(), E>,
    {
        let check = OriginalCheck::registry(deadline, work);
        let result = (|| {
            check.check(Point::Start)?;
            if command.identity() != self.journal.identity { return Err(ControlError::IdentityMismatch); }
            require_identity(command.command().mutation().id(), mutation)?;
            self.journal.recover_transaction_with_conditions_checked(
                command.command().mutation(), command.command().conditions(), &check)
        })();
        self.finish_dispatch(result).map_err(|error| {
            let error = if check.interrupted() { ControlError::CommitOutcomeUnknown } else { error };
            check.failure(error)
        })
    }
}

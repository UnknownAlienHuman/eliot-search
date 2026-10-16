//! Derived-artifact reservations through the existing original-context journal port.

use super::super::{
    bootstrap::OriginalCheck,
    operation::{Check, Point},
};
use super::{BoundControlJournal, require_identity};
use crate::source_retention::{
    CommittedDerivedArtifactSealUse, DerivedArtifactObjectIntentCommand, DerivedArtifactUseCommand,
};
use crate::{
    CommitRecoveryDecision, ControlCommitReceipt, ControlError, MigrationBootstrapFailure,
    MigrationBootstrapWork,
};
use search_ports::{CancellationProbe, MutationIdentity};
use std::time::Instant;

impl<C: CancellationProbe> BoundControlJournal<'_, C> {
    /// Commits one derived-artifact reservation with the caller's original context.
    ///
    /// Only a fresh acknowledged commit returns a non-Clone permit. Replay returns
    /// its historical receipt with no permit; refusal or an unknown outcome returns
    /// no permit and retains the existing pending fence.
    ///
    /// # Errors
    /// Foreign journal/operation identity, interrupted work, stale conditions,
    /// cancellation, deadline or unknown commit outcome remain typed failures.
    pub fn transact_derived_artifact_use_with_original_context<F, E>(
        &mut self,
        command: &DerivedArtifactUseCommand,
        mutation: &MutationIdentity,
        deadline: Instant,
        work: &F,
    ) -> Result<
        (
            ControlCommitReceipt,
            Option<CommittedDerivedArtifactSealUse>,
        ),
        MigrationBootstrapFailure<E>,
    >
    where
        F: Fn(MigrationBootstrapWork) -> Result<(), E>,
    {
        let check = OriginalCheck::registry(deadline, work);
        let result = (|| {
            check.check(Point::Start)?;
            if command.identity() != self.journal.identity {
                return Err(ControlError::IdentityMismatch);
            }
            require_identity(command.command().mutation().id(), mutation)?;
            // Prepay the one opaque handoff before any possible commit.
            check.reserve(MigrationBootstrapWork::Allocate(
                command.permit_allocation(),
            ))?;
            super::super::transaction::execute_revision_use(
                self.journal,
                command.command(),
                super::Boundary::Normal,
                &check,
            )
        })();
        let receipt = self
            .finish_dispatch(result)
            .map_err(|error| check.failure(error))?;
        let permit = if receipt.replayed {
            None
        } else {
            Some(command.fresh_permit())
        };
        Ok((receipt, permit))
    }

    /// Recovers only the exact original derived-artifact reservation command.
    /// Recovery never reconstructs or reissues a permit.
    ///
    /// # Errors
    /// Interrupted/foreign recovery cannot clear pending state or authorize AES.
    pub fn recover_derived_artifact_use_with_original_context<F, E>(
        &mut self,
        command: &DerivedArtifactUseCommand,
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
            if command.identity() != self.journal.identity {
                return Err(ControlError::IdentityMismatch);
            }
            require_identity(command.command().mutation().id(), mutation)?;
            self.journal.recover_transaction_with_conditions_checked(
                command.command().mutation(),
                command.command().conditions(),
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

    /// Commits the exact post-AES object intent before the C07 filesystem publish.
    /// The command has no cryptographic permit and reuses the same original context.
    ///
    /// # Errors
    /// Changed Root/source/policy/purge guards, reservation/current mismatch,
    /// interruption, cancellation, deadline or unknown commit outcome refuse.
    pub fn transact_derived_artifact_object_intent_with_original_context<F, E>(
        &mut self,
        command: &DerivedArtifactObjectIntentCommand,
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
            if command.identity() != self.journal.identity {
                return Err(ControlError::IdentityMismatch);
            }
            require_identity(command.command().mutation().id(), mutation)?;
            super::super::transaction::execute_revision_use(
                self.journal,
                command.command(),
                super::Boundary::Normal,
                &check,
            )
        })();
        self.finish_dispatch(result)
            .map_err(|error| check.failure(error))
    }

    /// Recovers only the exact original derived-artifact object-intent command.
    /// This result is technical state and never grants a seal-use permit.
    ///
    /// # Errors
    /// Interrupted/foreign recovery cannot replace an intent or authorize publication.
    pub fn recover_derived_artifact_object_intent_with_original_context<F, E>(
        &mut self,
        command: &DerivedArtifactObjectIntentCommand,
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
            if command.identity() != self.journal.identity {
                return Err(ControlError::IdentityMismatch);
            }
            require_identity(command.command().mutation().id(), mutation)?;
            self.journal.recover_transaction_with_conditions_checked(
                command.command().mutation(),
                command.command().conditions(),
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

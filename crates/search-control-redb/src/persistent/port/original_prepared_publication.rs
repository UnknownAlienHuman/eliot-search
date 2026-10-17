//! Publication uses the existing engines with one borrowed original allowance.

use super::super::{
    Boundary,
    bootstrap::OriginalCheck,
    operation::{Check, Point},
};
use super::{BoundControlJournal, ControlPortCommand, require_identity};
use crate::persistent::publication::prepared_artifacts::reference_key;
use crate::{
    CommitRecoveryDecision, ControlCommitReceipt, ControlError, MigrationBootstrapFailure,
    MigrationBootstrapWork as Work, PreparedArtifactSetReadbackV1, PreparedArtifactSetRecordV1,
    VisibleEpochCommit,
};
use search_contracts::ReceiptRef;
use search_ports::{CancellationProbe, MutationIdentity};
use std::time::Instant;

impl<C: CancellationProbe> BoundControlJournal<'_, C> {
    /// Reads the existing intent head under the original cumulative allowance.
    /// # Errors
    /// Partial, pending, corrupt or interrupted observations return no head.
    pub fn read_publication_intent_with_original_context<F, E>(
        &self,
        deadline: Instant,
        work: &F,
    ) -> Result<crate::PublicationIntentHead, MigrationBootstrapFailure<E>>
    where
        F: Fn(Work) -> Result<(), E>,
    {
        let check = OriginalCheck::registry(deadline, work);
        self.journal
            .read_intent_checked(&check)
            .map_err(|error| check.failure(error))
    }

    /// Publishes an actual committed snapshot through the existing checked publisher.
    /// This performs no durable write and grants no external source authority.
    /// # Errors
    /// Stale receipts, unresolved work or original-context refusal retain suspension.
    pub fn publish_committed_snapshot_with_original_context<F, E>(
        &mut self,
        receipt: &ControlCommitReceipt,
        deadline: Instant,
        work: &F,
    ) -> Result<crate::SnapshotPublishReceipt, MigrationBootstrapFailure<E>>
    where
        F: Fn(Work) -> Result<(), E>,
    {
        let check = OriginalCheck::registry(deadline, work);
        self.journal
            .publish_snapshot_checked(receipt, self.publisher, &check)
            .map_err(|error| check.failure(error))
    }

    /// Reads one exact versioned C02 set record from the actual journal.
    /// No C07 I/O, query write, recovery or retention-root claim is performed.
    /// # Errors
    /// Missing/corrupt/foreign-profile bytes and original-context refusal return no readback.
    pub fn read_prepared_artifact_set_with_original_context<F, E>(
        &self,
        reference: &ReceiptRef,
        deadline: Instant,
        work: &F,
    ) -> Result<PreparedArtifactSetReadbackV1, MigrationBootstrapFailure<E>>
    where
        F: Fn(Work) -> Result<(), E>,
    {
        let check = OriginalCheck::registry(deadline, work);
        let result = (|| {
            check.check(Point::Start)?;
            if !crate::has_visibility_schema(self.journal.identity.schema_version) {
                return Err(ControlError::SchemaUnsupported);
            }
            check.reserve(Work::Allocate(256))?;
            check.reserve(Work::Process(256))?;
            let key = reference_key(reference, self.journal.limits)?;
            let (identity, generation, values) =
                self.journal.read_current_records_checked(&[&key], &check)?;
            let value = values
                .into_iter()
                .next()
                .flatten()
                .ok_or(ControlError::InvalidValue)?;
            let record = PreparedArtifactSetRecordV1::decode(&value, self.journal.limits, &check)?;
            if record.reference() != reference {
                return Err(ControlError::StoreCorrupt);
            }
            check.check(Point::ReadComplete)?;
            Ok(PreparedArtifactSetReadbackV1 {
                identity,
                generation,
                record,
            })
        })();
        result.map_err(|error| check.failure(error))
    }

    /// Persists the existing intent/successor command and optional v1 set record atomically.
    /// The original mutation identity, deadline and cumulative callback are retained.
    /// # Errors
    /// Refuses non-publication variants, stale guards, mismatched records or uncertain effects.
    pub fn transact_publication_with_original_context<F, E>(
        &mut self,
        command: &ControlPortCommand,
        mutation: &MutationIdentity,
        deadline: Instant,
        work: &F,
    ) -> Result<ControlCommitReceipt, MigrationBootstrapFailure<E>>
    where
        F: Fn(Work) -> Result<(), E>,
    {
        let check = OriginalCheck::registry(deadline, work);
        let result = (|| {
            check.check(Point::Start)?;
            require_identity(command.id(), mutation)?;
            match command {
                ControlPortCommand::PublicationIntent(update) => self
                    .journal
                    .persist_intent_checked(update, Boundary::Normal, &check),
                ControlPortCommand::PublicationSuccessor(request) => self
                    .journal
                    .reserve_successor_checked(request, self.publisher, Boundary::Normal, &check),
                _ => Err(ControlError::InvalidValue),
            }
        })();
        self.finish_dispatch(result)
            .map_err(|error| check.failure(error))
    }

    /// Recovers the exact retained publication command without re-executing it.
    /// # Errors
    /// Changed inputs cannot clear pending work; interruption remains outcome-unknown.
    pub fn recover_publication_with_original_context<F, E>(
        &mut self,
        command: &ControlPortCommand,
        mutation: &MutationIdentity,
        deadline: Instant,
        work: &F,
    ) -> Result<CommitRecoveryDecision, MigrationBootstrapFailure<E>>
    where
        F: Fn(Work) -> Result<(), E>,
    {
        let check = OriginalCheck::registry(deadline, work);
        let result = (|| {
            check.check(Point::Start)?;
            require_identity(command.id(), mutation)?;
            let exact = match command {
                ControlPortCommand::PublicationIntent(update) => {
                    update.command(self.journal.limits, &check)?
                }
                ControlPortCommand::PublicationSuccessor(request) => {
                    request.command(self.journal.limits, &check)?
                }
                _ => return Err(ControlError::InvalidValue),
            };
            self.journal.recover_transaction_with_conditions_checked(
                exact.mutation(),
                exact.conditions(),
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

    /// Atomically binds the existing current-manifest map and `VisibleEpoch` under
    /// the same original allowance; snapshot publication remains a separate step.
    /// # Errors
    /// Guard/record/prior-operation conflicts or uncertain commit never acknowledge visibility.
    pub fn commit_visible_epoch_with_original_context<F, E>(
        &mut self,
        request: &VisibleEpochCommit,
        mutation: &MutationIdentity,
        deadline: Instant,
        work: &F,
    ) -> Result<ControlCommitReceipt, MigrationBootstrapFailure<E>>
    where
        F: Fn(Work) -> Result<(), E>,
    {
        let check = OriginalCheck::registry(deadline, work);
        let result = (|| {
            check.check(Point::Start)?;
            require_identity(request.port_operation_id(), mutation)?;
            self.journal
                .commit_visibility_checked(request, Boundary::Normal, &check)
        })();
        self.finish_dispatch(result)
            .map_err(|error| check.failure(error))
    }

    /// Resolves only the original visibility command; no snapshot or C07 permission is issued.
    /// # Errors
    /// Interrupted or changed recovery retains the original pending fence.
    pub fn recover_visible_epoch_with_original_context<F, E>(
        &mut self,
        request: &VisibleEpochCommit,
        mutation: &MutationIdentity,
        deadline: Instant,
        work: &F,
    ) -> Result<CommitRecoveryDecision, MigrationBootstrapFailure<E>>
    where
        F: Fn(Work) -> Result<(), E>,
    {
        let check = OriginalCheck::registry(deadline, work);
        let result = (|| {
            check.check(Point::Start)?;
            require_identity(request.port_operation_id(), mutation)?;
            let exact = request.command(self.journal.limits, &check)?;
            self.journal.recover_transaction_with_conditions_checked(
                exact.mutation(),
                exact.conditions(),
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

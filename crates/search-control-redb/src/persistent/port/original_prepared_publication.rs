//! Publication uses the existing engines with one borrowed original allowance.

use super::super::{
    Boundary,
    bootstrap::OriginalCheck,
    operation::{Check, Point},
};
use super::{BoundControlJournal, ControlPortCommand, require_identity};
use crate::persistent::publication::prepared_artifacts::reference_key;
use crate::{
    CommitRecoveryDecision, ControlCommitReceipt, ControlError,
    MigrationBootstrapFailure, MutationId,
    MigrationBootstrapWork as Work, PreparationInputReadbackV1, PreparationInputRecordV1,
    PreparedArtifactSetReadbackV1, PreparedArtifactSetRecordV1, PublicationGuardImageReadbackV1,
    PublicationGuardImageRequestV1, PublicationRecoveryCheckpoint, RegistryReadSnapshot,
    VisibleEpochCommit,
};
use crate::security_restriction::SecurityRestrictionReadback;
use search_contracts::{Blake3Digest32, OpaqueId, PublicationGuards, ReceiptRef};
use search_ports::{CancellationProbe, MutationIdentity};
use redb::ReadableTable;
use std::time::Instant;

impl<C: CancellationProbe> BoundControlJournal<'_, C> {
    /// Reads the complete C02 publication recovery checkpoint under the original
    /// cumulative deadline/work allowance. This recovery-plane read may return
    /// unresolved intent and is not a serving projection snapshot.
    ///
    /// # Errors
    /// Pending, corrupt, foreign, unsupported or interrupted state is never
    /// returned as a partial checkpoint.
    pub fn read_publication_checkpoint_with_original_context<F, E>(
        &self,
        deadline: Instant,
        work: &F,
    ) -> Result<Option<crate::PublicationRecoveryCheckpoint>, MigrationBootstrapFailure<E>>
    where
        F: Fn(Work) -> Result<(), E>,
    {
        let check = OriginalCheck::registry(deadline, work);
        self.journal
            .read_checkpoint_checked(&check)
            .map_err(|error| check.failure(error))
    }

    /// Prepares a source-backed guard-image CAS from same-journal C02/C03/C18
    /// readbacks. Source and membership generations remain distinct values
    /// supplied by their semantic owner; this method never aliases them to the
    /// C02 journal generation or C03 registry revision.
    ///
    /// # Errors
    /// Refuses unresolved publication work, foreign/stale readbacks, incomplete
    /// security records, regressed guards, route changes, a receipt-bound shadow
    /// change after visibility is nonzero, or exhausted context.
    #[allow(clippy::too_many_arguments)] // Keeps each typed observation and original C02 identity explicit.
    pub fn prepare_publication_guard_image_with_original_context<F, E>(
        &self,
        checkpoint: &PublicationRecoveryCheckpoint,
        registry: &RegistryReadSnapshot,
        security: &[SecurityRestrictionReadback],
        guards: PublicationGuards,
        operation_id: MutationId,
        command_digest: Blake3Digest32,
        deadline: Instant,
        work: &F,
    ) -> Result<PublicationGuardImageRequestV1, MigrationBootstrapFailure<E>>
    where
        F: Fn(Work) -> Result<(), E>,
    {
        let check = OriginalCheck::registry(deadline, work);
        PublicationGuardImageRequestV1::prepare(
            checkpoint,
            registry,
            security,
            guards,
            operation_id,
            command_digest,
            self.journal.limits,
            &check,
        )
        .map_err(|error| check.failure(error))
    }

    /// Atomically replaces only the current visibility guard image while the
    /// exact C03 registry fence and all supplied C18 policy/restriction rows
    /// remain unchanged. This is not a publication epoch or manifest update.
    ///
    /// # Errors
    /// A stale source row, changed global generation, original-context refusal
    /// or uncertain write never acknowledges the new image.
    pub fn transact_publication_guard_image_with_original_context<F, E>(
        &mut self,
        request: &PublicationGuardImageRequestV1,
        mutation: &MutationIdentity,
        deadline: Instant,
        work: &F,
    ) -> Result<PublicationGuardImageReadbackV1, MigrationBootstrapFailure<E>>
    where
        F: Fn(Work) -> Result<(), E>,
    {
        let check = OriginalCheck::registry(deadline, work);
        let result = (|| {
            check.check(Point::Start)?;
            require_identity(request.operation_id(), mutation)?;
            self.journal.commit_publication_guard_image_checked(
                request,
                Boundary::Normal,
                &check,
            )
        })();
        self.finish_dispatch(result)
            .map_err(|error| check.failure(error))
    }

    /// Reconciles the exact original guard-image mutation without dispatching
    /// it again. The same private request must be retained across uncertainty.
    ///
    /// # Errors
    /// Changed request identity, interrupted readback and partial/corrupt state
    /// remain explicit recovery outcomes.
    pub fn recover_publication_guard_image_with_original_context<F, E>(
        &mut self,
        request: &PublicationGuardImageRequestV1,
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
            require_identity(request.operation_id(), mutation)?;
            let command = request.command(self.journal.limits, &check)?;
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

    /// Reads the actual operation receipt at the journal's current generation.
    ///
    /// Visibility CAS construction uses this as its exact predecessor even
    /// when unrelated source/control transactions advanced the global journal
    /// after the publication checkpoint. The bounded operation ledger retains
    /// one receipt for every committed generation; no generation is inferred
    /// from the publication intent.
    ///
    /// # Errors
    /// An absent current-generation receipt, corrupt ledger, pending mutation,
    /// unsupported schema or original-context refusal is not synthesized.
    pub fn read_current_control_commit_with_original_context<F, E>(
        &self,
        deadline: Instant,
        work: &F,
    ) -> Result<ControlCommitReceipt, MigrationBootstrapFailure<E>>
    where
        F: Fn(Work) -> Result<(), E>,
    {
        self.read_control_commit_at_generation_inner(None, deadline, work)
    }

    /// Reads the exact operation receipt that committed one known journal
    /// generation, even when later bounded control writes have advanced the
    /// global header.
    ///
    /// # Errors
    /// A generation without exactly one retained receipt is never synthesized.
    pub fn read_control_commit_at_generation_with_original_context<F, E>(
        &self,
        generation: u64,
        deadline: Instant,
        work: &F,
    ) -> Result<ControlCommitReceipt, MigrationBootstrapFailure<E>>
    where
        F: Fn(Work) -> Result<(), E>,
    {
        self.read_control_commit_at_generation_inner(Some(generation), deadline, work)
    }

    fn read_control_commit_at_generation_inner<F, E>(
        &self,
        requested_generation: Option<u64>,
        deadline: Instant,
        work: &F,
    ) -> Result<ControlCommitReceipt, MigrationBootstrapFailure<E>>
    where
        F: Fn(Work) -> Result<(), E>,
    {
        let check = OriginalCheck::registry(deadline, work);
        let result = (|| {
            check.check(Point::Start)?;
            self.journal.ensure_available()?;
            if !crate::has_visibility_schema(self.journal.identity.schema_version) {
                return Err(ControlError::SchemaUnsupported);
            }
            super::super::operating_profile::require_indexed_checked(self.journal, &check)?;
            check.check(Point::ReadHeader)?;
            let read = self
                .journal
                .database
                .begin_read()
                .map_err(|_| ControlError::StoreUnavailable)?;
            let header = self.journal.header_from_checked(&read, &check)?;
            let snapshot = self.journal.snapshot_from_checked(&read, &check)?;
            let target_generation = requested_generation.unwrap_or(header.generation);
            if target_generation == 0
                || target_generation > header.generation
                || snapshot.generation != header.generation
            {
                return Err(ControlError::StoreCorrupt);
            }
            let operations = read
                .open_table(super::super::OPERATIONS)
                .map_err(super::super::map_table_error)?;
            let mut current = None;
            for row in operations
                .iter()
                .map_err(|error| super::super::map_storage_error(&error))?
            {
                check.check(Point::ReadOperation)?;
                let (id, bytes) = row.map_err(|error| super::super::map_storage_error(&error))?;
                let id = MutationId(
                    id.value()
                        .try_into()
                        .map_err(|_| ControlError::StoreCorrupt)?,
                );
                let operation = super::super::StoredOperation::decode_checked(
                    bytes.value(),
                    id,
                    header.generation,
                    self.journal.limits,
                    &check,
                )?;
                if operation.receipt.after_generation == target_generation
                    && current.replace(operation.receipt).is_some()
                {
                    return Err(ControlError::StoreCorrupt);
                }
            }
            let receipt = current.ok_or(ControlError::StoreCorrupt)?;
            check.check(Point::ReadComplete)?;
            Ok(receipt)
        })();
        result.map_err(|error| check.failure(error))
    }

    /// Reads the actual retained receipt for one exact C02 operation identity.
    ///
    /// This is a ledger observation, not proof that the operation's semantic
    /// state is still current. Callers must also read and compare the complete
    /// owner state before using the receipt as a predecessor for another CAS.
    /// `None` is returned only after a coherent, bounded read proves that the
    /// operation identity has no retained receipt.
    ///
    /// # Errors
    /// A digest mismatch, corrupt/pending journal, unsupported schema or
    /// original-context refusal is never reported as absence.
    pub fn read_publication_operation_receipt_with_original_context<F, E>(
        &self,
        operation_id: MutationId,
        command_digest: Blake3Digest32,
        deadline: Instant,
        work: &F,
    ) -> Result<Option<ControlCommitReceipt>, MigrationBootstrapFailure<E>>
    where
        F: Fn(Work) -> Result<(), E>,
    {
        let check = OriginalCheck::registry(deadline, work);
        let result = (|| {
            check.check(Point::Start)?;
            self.journal.ensure_available()?;
            if !crate::has_visibility_schema(self.journal.identity.schema_version) {
                return Err(ControlError::SchemaUnsupported);
            }
            super::super::operating_profile::require_indexed_checked(self.journal, &check)?;
            check.check(Point::ReadHeader)?;
            let read = self
                .journal
                .database
                .begin_read()
                .map_err(|_| ControlError::StoreUnavailable)?;
            let header = self.journal.header_from_checked(&read, &check)?;
            // Validate the complete same-transaction journal snapshot before
            // exposing an operation receipt as an observation.
            self.journal.snapshot_from_checked(&read, &check)?;
            let operation = super::super::operation_from_checked(
                &read,
                operation_id,
                &header,
                self.journal.limits,
                &check,
            )?;
            let receipt = operation
                .map(|operation| {
                    let receipt = operation.receipt;
                    if receipt.operation_id != operation_id
                        || receipt.command_digest != command_digest
                        || receipt.before_generation.checked_add(1)
                            != Some(receipt.after_generation)
                    {
                        return Err(ControlError::OperationConflict);
                    }
                    Ok(receipt)
                })
                .transpose()?;
            check.check(Point::ReadComplete)?;
            Ok(receipt)
        })();
        result.map_err(|error| check.failure(error))
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

    /// Reads the immutable input by its original Root operation before any ID redraw.
    ///
    /// A verified absence means the atomic C02 input transaction did not commit.
    /// A returned row carries the original C02 mutation ID, generation and every
    /// input needed to replay or recover the same operation.
    ///
    /// # Errors
    /// Pending, corrupt, wrong-profile or interrupted reads return no result.
    pub fn read_preparation_input_with_original_context<F, E>(
        &self,
        original_operation_id: &OpaqueId,
        deadline: Instant,
        work: &F,
    ) -> Result<Option<PreparationInputReadbackV1>, MigrationBootstrapFailure<E>>
    where
        F: Fn(Work) -> Result<(), E>,
    {
        let check = OriginalCheck::registry(deadline, work);
        let result = (|| {
            check.check(Point::Start)?;
            if !crate::has_visibility_schema(self.journal.identity.schema_version) {
                return Err(ControlError::SchemaUnsupported);
            }
            super::super::operating_profile::require_indexed_checked(self.journal, &check)?;
            let key_len = crate::persistent::publication::preparation_input::operation_key_len(
                original_operation_id,
            );
            check.reserve(Work::Allocate(
                key_len
                    .checked_add(core::mem::size_of::<crate::ControlKey>())
                    .ok_or(ControlError::BudgetExceeded)?,
            ))?;
            check.reserve(Work::Process(key_len))?;
            let key = crate::persistent::publication::preparation_input::key_for_operation(
                original_operation_id,
                self.journal.limits,
            )?;
            let (identity, generation, values) =
                self.journal.read_current_records_checked(&[&key], &check)?;
            let value = values
                .into_iter()
                .next()
                .ok_or(ControlError::StoreCorrupt)?;
            let Some(value) = value else {
                check.check(Point::ReadComplete)?;
                return Ok(None);
            };
            let record = PreparationInputRecordV1::decode(
                &value,
                original_operation_id,
                self.journal.limits,
                &check,
            )?;
            if record.key(self.journal.limits)? != key {
                return Err(ControlError::StoreCorrupt);
            }
            check.check(Point::ReadComplete)?;
            Ok(Some(PreparationInputReadbackV1::new(
                identity,
                generation,
                record,
            )))
        })();
        result.map_err(|error| check.failure(error))
    }

    /// Commits the complete pre-effect input through the existing conditional redb journal.
    ///
    /// The exact operation-key absence check, global generation and C02 operation receipt
    /// commit atomically. Call this before any C07 effect. Unknown outcomes require the
    /// matching recovery method with the same record.
    ///
    /// # Errors
    /// Conflicting inputs, changed generation, invalid profile or unknown commit outcome
    /// remain explicit and fail closed.
    pub fn transact_preparation_input_with_original_context<F, E>(
        &mut self,
        record: &PreparationInputRecordV1,
        deadline: Instant,
        work: &F,
    ) -> Result<ControlCommitReceipt, MigrationBootstrapFailure<E>>
    where
        F: Fn(Work) -> Result<(), E>,
    {
        let check = OriginalCheck::registry(deadline, work);
        let result = (|| {
            check.check(Point::Start)?;
            if !crate::has_visibility_schema(self.journal.identity.schema_version) {
                return Err(ControlError::SchemaUnsupported);
            }
            super::super::operating_profile::require_indexed_checked(self.journal, &check)?;
            let command = record.command(self.journal.limits, &check)?;
            super::super::transaction::execute_publication(
                self.journal,
                &command,
                Boundary::Normal,
                &check,
            )
        })();
        self.finish_dispatch(result)
            .map_err(|error| check.failure(error))
    }

    /// Recovers the exact original pre-effect C02 input operation without replaying it.
    ///
    /// The retained record supplies the original C02 mutation ID, expected generation
    /// and complete input bytes. A conflicting record never clears the operation fence.
    ///
    /// # Errors
    /// Interrupted inspection remains outcome-unknown; changed inputs cannot recover success.
    pub fn recover_preparation_input_with_original_context<F, E>(
        &mut self,
        record: &PreparationInputRecordV1,
        deadline: Instant,
        work: &F,
    ) -> Result<CommitRecoveryDecision, MigrationBootstrapFailure<E>>
    where
        F: Fn(Work) -> Result<(), E>,
    {
        let check = OriginalCheck::registry(deadline, work);
        let result = (|| {
            check.check(Point::Start)?;
            if !crate::has_visibility_schema(self.journal.identity.schema_version) {
                return Err(ControlError::SchemaUnsupported);
            }
            super::super::operating_profile::require_indexed_checked(self.journal, &check)?;
            let command = record.command(self.journal.limits, &check)?;
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

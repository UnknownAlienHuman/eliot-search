//! Shared control ports over the existing disk owner and its sole publisher.
//! No database, snapshot pointer, operation ledger or cancellation state is copied.

use core::marker::PhantomData;
use std::sync::Arc;

use search_contracts::{OpaqueId, PublicationIntent};
use search_ports::{CancellationProbe, ControlJournalPort, ControlSnapshotPort,
    DisclosureClass, IdempotencyClass, MutationIdentity, OperationContext, Port,
    PortError, PortErrorKind, PortRetryability};

use crate::{CommitRecoveryDecision, ConditionalControlMutation, ControlCommitReceipt,
    ControlError, ControlSnapshot, ControlSnapshotPublisher, MutationId,
    PublicationIntentUpdate, PublicationSuccessor, SnapshotPublishReceipt, VisibleEpochCommit};
use crate::conditions::validate_conditions;
use super::operation::{Budget, Check, Point};
use super::{Boundary, ControlCallError, ControlQuarantineReceipt, ControlQuarantineRequest,
    JournalReadSnapshot, JournalWriteCounters, PersistentControlJournal,
    PUBLICATION_VISIBILITY_SCHEMA_VERSION, validate_mutation};

/// Redacted shared-port error retaining the existing closed journal reason.
/// Cross-port classification preserves cancellation, deadlines and unknown outcomes.
pub type ControlPortError = PortError<ControlError>;

/// Closed command routing; each variant uses its existing semantic validator.
///
/// Generic records cannot bypass publication's reserved keys. Initialization,
/// schema changes and owner succession remain explicit lifecycle operations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ControlPortCommand {
    /// Technical writes with exact value/absence and generation preconditions.
    Records(ConditionalControlMutation),
    /// First intent or one permitted existing-intent transition.
    PublicationIntent(PublicationIntentUpdate),
    /// Exact next reservation after a durably committed, published predecessor.
    PublicationSuccessor(PublicationSuccessor),
}

impl ControlPortCommand {
    const fn id(&self) -> MutationId {
        match self {
            Self::Records(command) => command.mutation().id(),
            Self::PublicationIntent(command) => command.port_operation_id(),
            Self::PublicationSuccessor(command) => command.port_operation_id(),
        }
    }
}

/// Produces the opaque port identity for an existing durable operation ID.
///
/// The complete 32-byte ID is encoded, never truncated, hashed into another ID or
/// parsed from caller text. Callers retain this value alongside the original command.
/// `RetrySameIdentity` is the only declaration supported by this transaction engine.
///
/// # Errors
/// Returns `InvalidValue` if the shared opaque-ID bound rejects the fixed encoding.
pub fn control_mutation_identity(id: MutationId) -> Result<MutationIdentity, ControlError> {
    let text = format!("eliot-control-v1:{}", encode_mutation_hex(&id.0));
    let operation_id = OpaqueId::new(text).map_err(|_| ControlError::InvalidValue)?;
    Ok(MutationIdentity::new(operation_id, IdempotencyClass::RetrySameIdentity))
}

/// Exclusive borrowed composition of the existing journal and snapshot publisher.
///
/// Dropping this adapter releases only its Rust borrows, not a root lock, journal,
/// quarantine or recovery fence. It is not Clone and introduces no second owner.
/// The cancellation type is supplied by the capability owner, as in existing calls.
#[derive(Debug)]
pub struct BoundControlJournal<'a, C: CancellationProbe> {
    journal: &'a mut PersistentControlJournal,
    publisher: &'a mut ControlSnapshotPublisher,
    cancellation: PhantomData<fn() -> C>,
}

impl PersistentControlJournal {
    /// Binds both shared ports to an already-open schema-3 journal and publisher.
    ///
    /// No file or database is opened. The caller must keep its verified external
    /// root-owner guard alive. Binding suspends publisher admission; recover or
    /// publish from this exact journal before requesting a current snapshot.
    ///
    /// # Errors
    /// Older schemas and foreign publisher identities fail without migration.
    pub fn bind_control_port<'a, C: CancellationProbe>(
        &'a mut self, publisher: &'a mut ControlSnapshotPublisher,
    ) -> Result<BoundControlJournal<'a, C>, ControlError> {
        if self.identity.schema_version != PUBLICATION_VISIBILITY_SCHEMA_VERSION {
            return Err(ControlError::SchemaUnsupported);
        }
        publisher.begin_disk_publication(self.identity)?;
        Ok(BoundControlJournal { journal: self, publisher, cancellation: PhantomData })
    }
}

impl<C: CancellationProbe> Port for BoundControlJournal<'_, C> {
    type Error = ControlPortError;
    type Cancellation = C;
}

impl<C: CancellationProbe> ControlJournalPort for BoundControlJournal<'_, C> {
    type ControlSnapshot = JournalReadSnapshot;
    type Command = ControlPortCommand;
    type VisibleEpochGuards = VisibleEpochCommit;
    type ControlCommit = ControlCommitReceipt;
    type QuarantineReason = ControlQuarantineRequest;
    type QuarantineReceipt = ControlQuarantineReceipt;
    type JournalWriteCounters = JournalWriteCounters;

    fn read_control_snapshot(&self, context: &OperationContext<C>)
        -> Result<JournalReadSnapshot, ControlPortError> {
        // Recovery-plane technical snapshot, not a new serving/admission snapshot.
        self.journal.read_snapshot_with_context(context).map_err(|error| failure(error, None))
    }

    fn transact(&mut self, command: &ControlPortCommand, context: &OperationContext<C>,
        mutation: &MutationIdentity,
    ) -> Result<ControlCommitReceipt, ControlPortError> {
        let budget = Budget::new(context);
        let id = command.id();
        let result = (|| {
            budget.check(Point::Start)?;
            require_identity(id, mutation)?;
            let result = match command {
                ControlPortCommand::Records(command) => {
                    // Validate finite dimensions and protected-key ownership before
                    // cloning. Generic writes/deletes cannot bypass the typed CAS.
                    self.validate_record_command(command, &budget)?;
                    budget.check(Point::Validated)?;
                    self.journal.transact_conditionally_checked(command.clone(), Boundary::Normal, &budget)
                }
                ControlPortCommand::PublicationIntent(command) => {
                    self.journal.persist_intent_checked(command, Boundary::Normal, &budget)
                }
                ControlPortCommand::PublicationSuccessor(command) => {
                    // The existing validator must see the published predecessor.
                    // Exclusive borrows prevent admission during this call; suspend
                    // the pointer after dispatch, before returning to the caller.
                    self.journal.reserve_successor_checked(command, self.publisher, Boundary::Normal, &budget)
                }
            };
            self.finish_dispatch(result)
        })();
        result.map_err(|error| failure(budget.failure(error, Some(id)), Some(mutation)))
    }

    fn compare_and_swap_visible_epoch(&mut self, guards: &VisibleEpochCommit,
        prior_commit: &ControlCommitReceipt, context: &OperationContext<C>, mutation: &MutationIdentity,
    ) -> Result<ControlCommitReceipt, ControlPortError> {
        let budget = Budget::new(context);
        let id = guards.port_operation_id();
        let result = (|| {
            budget.check(Point::Start)?;
            require_identity(id, mutation)?;
            if prior_commit.changed_keys.len() > self.journal.limits.max_mutation_items {
                return Err(ControlError::BudgetExceeded);
            }
            // Never ignore the separate prior_commit argument or silently replace
            // the original command's receipt with the latest journal receipt.
            if !guards.port_prior_matches(prior_commit) { return Err(ControlError::OperationConflict); }
            budget.check(Point::Validated)?;
            let result = self.journal.commit_visibility_checked(guards, Boundary::Normal, &budget);
            self.finish_dispatch(result)
        })();
        result.map_err(|error| failure(budget.failure(error, Some(id)), Some(mutation)))
    }

    fn load_unresolved_publication(&self, context: &OperationContext<C>)
        -> Result<Option<PublicationIntent>, ControlPortError> {
        self.journal.load_unresolved_publication(context).map_err(|error| failure(error, None))
    }

    fn quarantine(&mut self, request: &ControlQuarantineRequest,
        context: &OperationContext<C>, mutation: &MutationIdentity,
    ) -> Result<ControlQuarantineReceipt, ControlPortError> {
        let budget = Budget::new(context);
        let id = request.operation_id();
        let result = (|| {
            require_identity(id, mutation)?;
            // Do not preflight cancellation ahead of the existing quarantine
            // method: a matching request suspends admission even when cancelled.
            self.journal.quarantine_checked(request, self.publisher, Boundary::Normal, &budget)
        })();
        result.map_err(|error| failure(budget.failure(error, Some(id)), Some(mutation)))
    }

    fn write_counters(&self, context: &OperationContext<C>)
        -> Result<JournalWriteCounters, ControlPortError> {
        self.journal.write_counters_with_context(context).map_err(|error| failure(error, None))
    }
}

impl<C: CancellationProbe> ControlSnapshotPort for BoundControlJournal<'_, C> {
    type ControlSnapshot = Arc<ControlSnapshot>;

    fn current_snapshot(&self) -> Result<Arc<ControlSnapshot>, ControlPortError> {
        // Zero disk reads or writes. A prior Arc is historical, never a grant.
        let blocked = if self.journal.quarantined { Some(ControlError::StoreQuarantined) }
            else if self.journal.pending.is_some() { Some(ControlError::CommitOutcomeUnknown) }
            else { None };
        if let Some(error) = blocked { return Err(admission_failure(error)); }
        let snapshot = self.publisher.current()
            .ok_or_else(|| admission_failure(ControlError::SnapshotPublicationFailed))?;
        if self.publisher.diagnostic_disk_identity() != Some(self.journal.identity)
            || snapshot.identity != self.journal.identity {
            return Err(admission_failure(ControlError::IdentityMismatch));
        }
        Ok(snapshot)
    }
}

impl<C: CancellationProbe> BoundControlJournal<'_, C> {
    /// Recovers the original command, including exact record preconditions.
    /// No write is retried and no publisher fence is cleared by this operation.
    ///
    /// # Errors
    /// An interrupted inspection stays unknown; differing input cannot clear pending work.
    pub fn recover_command(&mut self, command: &ControlPortCommand,
        context: &OperationContext<C>, mutation: &MutationIdentity,
    ) -> Result<CommitRecoveryDecision, ControlPortError> {
        let budget = Budget::new(context);
        let id = command.id();
        let result = (|| {
            budget.check(Point::Start)?;
            require_identity(id, mutation)?;
            let generated;
            let exact = match command {
                ControlPortCommand::Records(command) => {
                    self.validate_record_command(command, &budget)?;
                    command
                }
                ControlPortCommand::PublicationIntent(command) => {
                    generated = command.command(self.journal.limits)?;
                    &generated
                }
                ControlPortCommand::PublicationSuccessor(command) => {
                    generated = command.command(self.journal.limits)?;
                    &generated
                }
            };
            self.journal.recover_transaction_with_conditions_checked(exact.mutation(), exact.conditions(), &budget)
        })();
        result.map_err(|error| failure(budget.failure(if budget.interrupted() {
            ControlError::CommitOutcomeUnknown
        } else { error }, Some(id)).for_recovery(), Some(mutation)))
    }

    /// Recovers the original guarded visibility commit, not a reconstructed request.
    ///
    /// # Errors
    /// Foreign identities, changed commands and interrupted readback remain explicit failures.
    pub fn recover_visible_epoch(&mut self, request: &VisibleEpochCommit,
        context: &OperationContext<C>, mutation: &MutationIdentity,
    ) -> Result<CommitRecoveryDecision, ControlPortError> {
        let budget = Budget::new(context);
        let id = request.port_operation_id();
        let result = (|| {
            budget.check(Point::Start)?;
            require_identity(id, mutation)?;
            let command = request.command(self.journal.limits, &budget)?;
            self.journal.recover_transaction_with_conditions_checked(command.mutation(), command.conditions(), &budget)
        })();
        result.map_err(|error| failure(budget.failure(if budget.interrupted() {
            ControlError::CommitOutcomeUnknown
        } else { error }, Some(id)).for_recovery(), Some(mutation)))
    }

    /// Publishes only the actual current commit after disk readback and reconstruction.
    ///
    /// # Errors
    /// Stale receipts, unresolved intent, quarantine and failed readback keep admission closed.
    pub fn publish_committed_snapshot(&mut self, commit: &ControlCommitReceipt,
        context: &OperationContext<C>,
    ) -> Result<SnapshotPublishReceipt, ControlPortError> {
        let identity = control_mutation_identity(commit.operation_id).map_err(admission_failure)?;
        self.journal.publish_committed_snapshot_with_context(commit, self.publisher, context)
            .map_err(|error| failure(error, Some(&identity)))
    }

    /// Rebuilds current snapshot state from the same journal without replaying writes.
    ///
    /// # Errors
    /// Pending transactions must be resolved first; no error unblocks admission.
    pub fn recover_snapshot(&mut self, context: &OperationContext<C>)
        -> Result<Option<SnapshotPublishReceipt>, ControlPortError> {
        self.journal.recover_snapshot_publication_with_context(self.publisher, context)
            .map_err(|error| failure(error, None))
    }

    fn validate_record_command(&self, command: &ConditionalControlMutation, check: &dyn Check)
        -> Result<(), ControlError> {
        let keys = validate_mutation(command.mutation(), self.journal.limits)?;
        validate_conditions(command.mutation(), command.conditions(), self.journal.limits,
            || check.check(Point::PlanRecord))?;
        for key in keys {
            check.check(Point::PlanRecord)?;
            if super::publication::port_reserved_key(key.as_bytes()) {
                return Err(ControlError::InvalidKey);
            }
        }
        Ok(())
    }

    fn finish_dispatch<T>(&mut self, result: Result<T, ControlError>) -> Result<T, ControlError> {
        // Also suspend after rejected/interrupted dispatch: only fresh disk
        // readback can establish which snapshot may subsequently be published.
        if let Err(error) = self.publisher.begin_disk_publication(self.journal.identity) {
            self.journal.quarantined = true;
            return Err(if result.is_ok() || self.journal.pending.is_some() {
                ControlError::CommitOutcomeUnknown
            } else { error });
        }
        result
    }
}

fn require_identity(id: MutationId, actual: &MutationIdentity) -> Result<(), ControlError> {
    if actual.idempotency != IdempotencyClass::RetrySameIdentity { return Err(ControlError::InvalidValue); }
    if control_mutation_identity(id)? != *actual { return Err(ControlError::OperationConflict); }
    Ok(())
}

/// Lowercase hex of the complete 32-byte mutation ID, never truncated.
/// Local to this crate so the shared `search-contracts` canonical helpers
/// stay `pub(crate)`; behavior matches the documented opaque port identity.
fn encode_mutation_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

fn failure(error: ControlCallError, identity: Option<&MutationIdentity>) -> ControlPortError {
    let operation = identity.map(|identity| identity.operation_id.clone());
    PortError::new(error.kind(), error.retryability(), DisclosureClass::Redacted, error.control_error(), operation)
}

fn admission_failure(error: ControlError) -> ControlPortError {
    let kind = if error == ControlError::CommitOutcomeUnknown { PortErrorKind::OutcomeUnknown }
        else { PortErrorKind::Quarantined };
    PortError::new(kind, PortRetryability::AfterReadback, DisclosureClass::Redacted, error, None)
}

#[cfg(test)]
mod tests;

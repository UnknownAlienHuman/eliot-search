//! Typed durable intent records; actual visibility commits remain separate.
//!
//! One reserved record retains the last intent, including terminal state. It is
//! never deleted here: absence after a recorded write is corruption, not an
//! invitation to start at an old epoch. This module records coordinator facts;
//! it neither performs Qdrant effects nor declares the index usable.

use core::fmt;
use search_contracts::{Blake3Digest32, PublicationIntent, PublicationIntentState};
use search_ports::{CancellationProbe, OperationContext};

use crate::{ConditionalControlMutation, ControlRecordCondition, ControlRecordClass,
    ControlValue, ControlWrite};
use super::operation::{Budget, Check, Point};
use super::{Boundary, CommitRecoveryDecision, ControlCallError, ControlCommitReceipt,
    ControlError, ControlKey, ControlMutation, JournalIdentity, JournalLimits, MutationId,
    OPERATIONS, PersistentControlJournal, RECORDS, ReadTransaction, ReadableTable,
    StoredOperation, decode_value, is_corruption, map_storage_error, map_table_error,
    operation_from};

mod codec;
pub(super) mod visibility;
pub use visibility::{PublicationManifestChange, PublicationReadbackEvidence, PublicationSourceShadow,
    PublicationVisibilityState, VisibleEpochCommit, PUBLICATION_VISIBILITY_SCHEMA_VERSION};
pub use visibility::PublicationSuccessor;
#[cfg(test)]
mod tests;

/// Explicit adapter schema for typed publication records. Older binaries only
/// accept schema 1 and therefore cannot silently ignore an unresolved intent.
/// No in-place schema-1 migration is implemented or implicitly performed.
pub const PUBLICATION_INTENT_SCHEMA_VERSION: u32 = 2;

const KEY: &[u8] = b"publication_intents/current/v1";

/// Immutable exact-input update for the durable publication intent.
///
/// The record stores only shared technical fields. Manifest references remain
/// references, never embedded point sets. This command cannot change VisibleEpoch,
/// write publication receipts or claim that a Qdrant effect was verified.
#[derive(Clone, Eq, PartialEq)]
pub struct PublicationIntentUpdate {
    operation_id: MutationId,
    command_digest: Blake3Digest32,
    expected_generation: u64,
    previous: Option<PublicationIntent>,
    next: PublicationIntent,
}

impl fmt::Debug for PublicationIntentUpdate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PublicationIntentUpdate")
            .field("expected_generation", &self.expected_generation)
            .field("state", &self.next.state)
            .finish_non_exhaustive()
    }
}

impl PublicationIntentUpdate {
    /// Prepares the first durable intent from a coordinator's PREPARED value.
    ///
    /// This does not allocate an epoch or prove that its guards match live state.
    /// The existing coordinator owns reservation/preparation; this command stores
    /// those exact inputs atomically. A retained prior intent is never overwritten
    /// by this method, even after abort. Successor reservation/finalization belongs
    /// to the complete guarded publication transaction, not this precommit writer.
    ///
    /// # Errors
    /// Rejects non-PREPARED input and epoch zero before storage dispatch.
    pub fn begin(
        operation_id: MutationId,
        command_digest: Blake3Digest32,
        expected_generation: u64,
        prepared: PublicationIntent,
    ) -> Result<Self, ControlError> {
        if prepared.state != PublicationIntentState::Prepared || prepared.target_epoch.get() == 0 {
            return Err(ControlError::InvalidValue);
        }
        let next = search_domain::transition_publication(
            &prepared, PublicationIntentState::IntentDurable,
        ).map_err(|_| ControlError::InvalidValue)?;
        Ok(Self { operation_id, command_digest, expected_generation, previous: None, next })
    }

    /// Records one existing domain transition while retaining all prepared fields.
    ///
    /// The coordinator must already have obtained the corresponding external
    /// acknowledgements/recovery evidence. Storage records the state; it does not
    /// create that evidence. The exact previous intent is an atomic precondition.
    /// A bare ABORTED value remains recovery-required until compensation/fencing
    /// has a complete separately verified durable finalization protocol.
    ///
    /// # Errors
    /// Rejects skipped/reversed edges and any visibility/finalization transition.
    /// CONTROL_COMMITTED, INVALIDATION_ONLY_COMMITTED and RECLAIMABLE require the
    /// separate guarded VisibleEpoch/finalization path and cannot be set here.
    pub fn advance(
        operation_id: MutationId,
        command_digest: Blake3Digest32,
        expected_generation: u64,
        previous: PublicationIntent,
        state: PublicationIntentState,
    ) -> Result<Self, ControlError> {
        if previous.state == PublicationIntentState::Prepared
            || matches!(state, PublicationIntentState::ControlCommitted
                | PublicationIntentState::InvalidationOnlyCommitted | PublicationIntentState::Reclaimable)
        { return Err(ControlError::InvalidValue); }
        let next = search_domain::transition_publication(&previous, state)
            .map_err(|_| ControlError::InvalidValue)?;
        Ok(Self { operation_id, command_digest, expected_generation, previous: Some(previous), next })
    }

    /// Exact proposed durable value, including unchanged preparation bindings.
    #[must_use]
    pub const fn intent(&self) -> &PublicationIntent { &self.next }

    fn command(&self, limits: JournalLimits) -> Result<ConditionalControlMutation, ControlError> {
        let key = ControlKey::new(KEY.to_vec(), limits)?;
        let expected = match &self.previous {
            Some(value) => ControlRecordCondition::exact(key.clone(), codec::encode(value, limits)?),
            None => ControlRecordCondition::absent(key.clone()),
        };
        let mutation = ControlMutation::new(self.operation_id, self.command_digest,
            self.expected_generation, vec![ControlWrite { key, value: codec::encode(&self.next, limits)? }], vec![]);
        Ok(ConditionalControlMutation::new(mutation, vec![expected]))
    }
}

/// Coherent control generation and the exact last durable intent, if initialized.
/// Terminal labels remain recorded; ABORTED alone does not resolve external effects.
#[derive(Clone, Eq, PartialEq)]
pub struct PublicationIntentHead {
    /// Exact journal/root/owner identity that supplied this read.
    pub identity: JournalIdentity,
    /// Control generation read in the same transaction as the intent.
    pub generation: u64,
    /// Actual durable value; no missing field is synthesized.
    pub intent: Option<PublicationIntent>,
}

impl fmt::Debug for PublicationIntentHead {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PublicationIntentHead")
            .field("generation", &self.generation)
            .field("state", &self.intent.as_ref().map(|intent| intent.state))
            .finish_non_exhaustive()
    }
}

impl PersistentControlJournal {
    /// Loads a typed intent and its control generation without writing or scanning
    /// source content. If the slot is absent, bounded receipt inspection distinguishes
    /// a never-initialized slot from a lost previously written intent.
    ///
    /// # Errors
    /// Corruption, pending mutation, quarantine or interruption returns no head.
    pub fn read_publication_intent<C: CancellationProbe>(
        &self, context: &OperationContext<C>,
    ) -> Result<PublicationIntentHead, ControlCallError> {
        let budget = Budget::new(context);
        self.read_intent_checked(&budget).map_err(|error| budget.failure(error, None))
    }

    /// Returns the exact recovery-required intent, including a bare ABORTED value.
    /// BLOCKED and COMPENSATING remain unresolved. CONTROL_COMMITTED resolves
    /// the durable mutation, not snapshot admission; its commit still needs publication.
    /// Resolved values are omitted here but retained by read_publication_intent.
    ///
    /// # Errors
    /// Missing-after-write, malformed bytes or interruption is not a successful None.
    pub fn load_unresolved_publication<C: CancellationProbe>(
        &self, context: &OperationContext<C>,
    ) -> Result<Option<PublicationIntent>, ControlCallError> {
        let budget = Budget::new(context);
        let result = (|| {
            let head = self.read_intent_checked(&budget)?;
            budget.check(Point::ReadComplete)?;
            Ok(head.intent.filter(|intent| unresolved(intent.state)))
        })();
        result.map_err(|error| budget.failure(error, None))
    }

    /// Reads route, visible epoch and consumed reservation history coherently.
    /// One read transaction validates typed relationships and the bounded ledger;
    /// it performs no writes, recovery, snapshot publication or source/index I/O.
    /// This recovery-plane read does not authorize a new coordinator or clear holds.
    ///
    /// # Errors
    /// Unsupported schema, missing/corrupt history, interruption or a pending
    /// mutation returns no checkpoint. `None` means an explicitly new empty
    /// schema-3 journal only, never a lost route or discarded aborted intent.
    pub fn read_publication_checkpoint<C: CancellationProbe>(
        &self, context: &OperationContext<C>,
    ) -> Result<Option<crate::PublicationRecoveryCheckpoint>, ControlCallError> {
        let budget = Budget::new(context);
        self.read_checkpoint_checked(&budget).map_err(|error| budget.failure(error, None))
    }

    fn read_checkpoint_checked(&self, check: &dyn Check)
        -> Result<Option<crate::PublicationRecoveryCheckpoint>, ControlError> {
        self.ensure_available()?;
        check.check(Point::Start)?;
        if self.identity.schema_version != PUBLICATION_VISIBILITY_SCHEMA_VERSION {
            return Err(ControlError::SchemaUnsupported);
        }
        let read = self.database.begin_read().map_err(|_| ControlError::StoreUnavailable)?;
        let snapshot = self.verify_from_checked(&read, check)?;
        // Reuse the existing relationship validator; all observations stay in
        // this same read transaction, with the original cooperative deadline.
        let visibility = visibility::validate_snapshot(&read, &snapshot, self.limits, check)?;
        let intent = snapshot.get(&ControlKey::new(KEY.to_vec(), self.limits)?)
            .map(codec::decode).transpose()?;
        check.check(Point::ReadComplete)?;
        Ok(visibility.map(|visibility| crate::PublicationRecoveryCheckpoint {
            identity: snapshot.identity, generation: snapshot.generation, visibility, intent,
        }))
    }

    /// Persists a bounded typed intent through the existing conditional transaction
    /// engine. Both its full pre-state and global generation are compared atomically.
    ///
    /// # Errors
    /// New forward progress requires this owner epoch. A successor may record
    /// compensation/block/abort without changing the original guards. Exact replay remains legal
    /// across owner handoff. Possible-write interruption needs exact recovery.
    pub fn persist_publication_intent<C: CancellationProbe>(
        &mut self, update: &PublicationIntentUpdate, context: &OperationContext<C>,
    ) -> Result<ControlCommitReceipt, ControlCallError> {
        let budget = Budget::new(context);
        self.persist_intent_checked(update, Boundary::Normal, &budget)
            .map_err(|error| budget.failure(error, Some(update.operation_id)))
    }

    /// Recovers the original typed command without re-executing it, weakening its
    /// guards or resetting its previous state to the current value.
    ///
    /// # Errors
    /// Interrupted/unavailable inspection preserves the pending operation fence.
    pub fn recover_publication_intent<C: CancellationProbe>(
        &mut self, update: &PublicationIntentUpdate, context: &OperationContext<C>,
    ) -> Result<CommitRecoveryDecision, ControlCallError> {
        let budget = Budget::new(context);
        let result = (|| {
            budget.check(Point::Start)?;
            self.require_intent_schema()?;
            let command = update.command(self.limits)?;
            self.recover_transaction_with_conditions_checked(command.mutation(), command.conditions(), &budget)
        })();
        result.map_err(|error| {
            let error = if budget.interrupted() { ControlError::CommitOutcomeUnknown } else { error };
            budget.failure(error, Some(update.operation_id)).for_recovery()
        })
    }

    fn require_intent_schema(&self) -> Result<(), ControlError> {
        if matches!(self.identity.schema_version, PUBLICATION_INTENT_SCHEMA_VERSION | PUBLICATION_VISIBILITY_SCHEMA_VERSION) { Ok(()) }
        else { Err(ControlError::SchemaUnsupported) }
    }

    fn read_intent_checked(&self, check: &dyn Check) -> Result<PublicationIntentHead, ControlError> {
        self.ensure_available()?;
        check.check(Point::Start)?;
        self.require_intent_schema()?;
        let read = self.database.begin_read().map_err(|_| ControlError::StoreUnavailable)?;
        let header = self.header_from(&read)?;
        check.check(Point::ReadHeader)?;
        if self.identity.schema_version == PUBLICATION_VISIBILITY_SCHEMA_VERSION {
            self.snapshot_from_checked(&read, check)?;
        }
        let records = read.open_table(RECORDS).map_err(map_table_error)?;
        let intent = match records.get(KEY).map_err(map_storage_error)? {
            Some(raw) => Some(codec::decode(&decode_value(raw.value(), self.limits)?)?),
            None => {
                verify_never_written(&read, header.generation, self.limits, check)?;
                None
            }
        };
        check.check(Point::ReadComplete)?;
        Ok(PublicationIntentHead { identity: self.identity, generation: header.generation, intent })
    }

    fn persist_intent_checked(
        &mut self, update: &PublicationIntentUpdate, boundary: Boundary, check: &dyn Check,
    ) -> Result<ControlCommitReceipt, ControlError> {
        let result = (|| {
            self.ensure_available()?;
            check.check(Point::Start)?;
            self.require_intent_schema()?;
            let command = update.command(self.limits)?;
            check.check(Point::Validated)?;
            {
                let read = self.database.begin_read().map_err(|_| ControlError::StoreUnavailable)?;
                let header = self.header_from(&read)?;
                let replay = operation_from(&read, update.operation_id, &header, self.limits)?.is_some();
                // Do not reject an already committed old-owner request before
                // the common replay engine verifies its exact fingerprint.
                let prepared_owner = update.next.owner_source_membership_access_guards.owner_epoch;
                let recovery_state = matches!(update.next.state, PublicationIntentState::Compensating
                    | PublicationIntentState::PublicationBlocked | PublicationIntentState::Aborted);
                if !replay && (prepared_owner > self.identity.owner_epoch
                    || (prepared_owner < self.identity.owner_epoch && !recovery_state)) {
                    return Err(ControlError::GenerationMismatch);
                }
                if self.identity.schema_version == PUBLICATION_VISIBILITY_SCHEMA_VERSION {
                    let snapshot = self.snapshot_from_checked(&read, check)?;
                    if !replay && update.previous.is_none() {
                        visibility::validate_initial_intent(&snapshot, &update.next, self.limits)?;
                    }
                }
                let table = read.open_table(RECORDS).map_err(map_table_error)?;
                match table.get(KEY).map_err(map_storage_error)? {
                    Some(raw) => { codec::decode(&decode_value(raw.value(), self.limits)?)?; }
                    None => verify_never_written(&read, header.generation, self.limits, check)?,
                }
            }
            self.transact_conditionally_checked(command, boundary, check)
        })();
        if matches!(self.identity.schema_version, PUBLICATION_INTENT_SCHEMA_VERSION | PUBLICATION_VISIBILITY_SCHEMA_VERSION)
            && result.as_ref().err().is_some_and(|error| is_corruption(*error)) {
            self.quarantined = true;
        }
        result
    }
}

pub(super) fn unresolved(state: PublicationIntentState) -> bool {
    // The stored terminal label alone proves neither two-sided compensation nor
    // effective membership-wide exclusion. Keep ABORTED recovery-visible.
    !matches!(state, PublicationIntentState::ControlCommitted
        | PublicationIntentState::Reclaimable | PublicationIntentState::InvalidationOnlyCommitted)
}

pub(super) fn validate_record(key: &ControlKey, value: &ControlValue) -> Result<(), ControlError> {
    if key.as_bytes() == KEY { codec::decode(value)?; }
    Ok(())
}

pub(super) fn require_resolved(records: &[(ControlKey, ControlValue)]) -> Result<(), ControlError> {
    if let Ok(index) = records.binary_search_by(|(key, _)| key.as_bytes().cmp(KEY)) {
        if unresolved(codec::decode(&records[index].1)?.state) { return Err(ControlError::SnapshotRebuildFailed); }
    }
    Ok(())
}

pub(super) fn verify_absence(
    read: &ReadTransaction, generation: u64, limits: JournalLimits,
    records: &[(ControlKey, ControlValue)], check: &dyn Check,
) -> Result<(), ControlError> {
    if records.binary_search_by(|(key, _)| key.as_bytes().cmp(KEY)).is_err() {
        verify_never_written(read, generation, limits, check)?;
    }
    Ok(())
}

fn verify_never_written(
    read: &ReadTransaction, generation: u64, limits: JournalLimits, check: &dyn Check,
) -> Result<(), ControlError> {
    if generation == 0 { return Ok(()); }
    let operations = read.open_table(OPERATIONS).map_err(map_table_error)?;
    for (count, row) in operations.iter().map_err(map_storage_error)?.enumerate() {
        check.check(Point::ReadOperation)?;
        if count >= limits.max_operation_records { return Err(ControlError::StoreCorrupt); }
        let (id, bytes) = row.map_err(map_storage_error)?;
        let id = MutationId(id.value().try_into().map_err(|_| ControlError::StoreCorrupt)?);
        let operation = StoredOperation::decode(bytes.value(), id, generation, limits)?;
        if operation.receipt.changed_keys.iter().any(|key| key.as_bytes() == KEY) {
            return Err(ControlError::StoreCorrupt);
        }
    }
    check.check(Point::ReadComplete)
}

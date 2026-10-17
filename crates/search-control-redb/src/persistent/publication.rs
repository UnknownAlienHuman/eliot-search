//! Typed durable intent records; actual visibility commits remain separate.
//!
//! One reserved record retains the last intent, including terminal state. It is
//! never deleted here: absence after a recorded write is corruption, not an
//! invitation to start at an old epoch. This module records coordinator facts;
//! it neither performs Qdrant effects nor declares the index usable.

use core::fmt;
use search_contracts::{Blake3Digest32, PublicationIntent, PublicationIntentState};
use search_ports::{CancellationProbe, OperationContext};
pub(in crate::persistent) fn read_intent_from_snapshot(
    snapshot: &super::JournalReadSnapshot,
    limits: JournalLimits,
) -> Result<Option<PublicationIntent>, ControlError> {
    snapshot
        .get(&ControlKey::new(KEY.to_vec(), limits)?)
        .map(codec::decode)
        .transpose()
}
pub(super) fn require_registry_base(journal: &PersistentControlJournal, read: &super::ReadTransaction, limits: super::JournalLimits)
    -> Result<(), super::ControlError> {
    match super::operating_profile::read_mode(read, journal.identity, limits)? {
        super::JournalOperatingProfile::Direct => Ok(()),
        super::JournalOperatingProfile::Indexed => visibility::require_initialized(read, limits),
        super::JournalOperatingProfile::MigrationInert => Err(super::ControlError::MigrationUnverified),
    }
}

use crate::{ConditionalControlMutation, ControlRecordCondition, ControlRecordClass,
    ControlValue, ControlWrite};
use super::operation::{Budget, Check, Point};
use super::{Boundary, CommitRecoveryDecision, ControlCallError, ControlCommitReceipt,
    ControlError, ControlKey, ControlMutation, JournalIdentity, JournalLimits, MutationId,
    OPERATIONS, PersistentControlJournal, RECORDS, ReadTransaction, ReadableTable,
    StoredOperation, is_corruption, map_storage_error, map_table_error};

mod codec;
pub(in crate::persistent) mod abort_finalization;
pub(in crate::persistent) mod abandonment;
pub(in crate::persistent) mod operation_binding;
pub use abort_finalization::{
    AbortResolutionMetadataV1, PublicationAbortFinalizationMetadataV1,
    PublicationAbortFinalizationRecordV1,
};
pub use abandonment::{
    PublicationAbandonFenceMembershipV1, PublicationAbandonFenceReadbackV1,
    PublicationAbandonFenceRecordV1, PublicationAbandonFenceRequestV1,
};
pub use operation_binding::{
    PublicationIntentBindingMetadataV1, PublicationIntentBindingRecordV1,
};
pub(in crate::persistent) mod prepared_artifacts;
pub(in crate::persistent) mod preparation_input;
pub use prepared_artifacts::{PreparedArtifactSetMetadataV1, PreparedArtifactSetRecordV1,
    PreparedArtifactSetReadbackV1, MAX_PREPARED_ARTIFACT_SET_REFERENCE_BYTES};
pub use preparation_input::{PreparationInputMetadataV1, PreparationInputReadbackV1,
    PreparationInputRecordV1, MAX_PREPARATION_INPUT_POLICY_BYTES,
    MAX_PREPARATION_INPUT_RECORD_BYTES, MAX_PREPARATION_INPUT_RESIDENCY_BYTES};
pub(super) mod visibility;
pub use visibility::{PublicationManifestChange, PublicationReadbackEvidence, PublicationSourceShadow,
    PublicationVisibilityState, VisibleEpochCommit, PUBLICATION_VISIBILITY_SCHEMA_VERSION};
pub use visibility::PublicationSuccessor;
pub use visibility::{PublicationProjectionSnapshot, PublicationRecoveryProjectionSnapshot};
#[cfg(test)]
mod tests;

/// Explicit adapter schema for typed publication records. Older binaries only
/// accept schema 1 and therefore cannot silently ignore an unresolved intent.
///
/// No in-place schema-1 migration is implemented or implicitly performed.
pub const PUBLICATION_INTENT_SCHEMA_VERSION: u32 = 2;

pub(in crate::persistent) const KEY: &[u8] = b"publication_intents/current/v1";

/// Immutable exact-input update for the durable publication intent.
///
/// The record stores only shared technical fields. Manifest references remain
/// references, never embedded point sets. This command cannot change `VisibleEpoch`,
/// write publication receipts or claim that a Qdrant effect was verified.
#[derive(Clone, Eq, PartialEq)]
pub struct PublicationIntentUpdate {
    operation_id: MutationId,
    command_digest: Blake3Digest32,
    expected_generation: u64,
    previous: Option<PublicationIntent>,
    next: PublicationIntent,
    artifact_set: Option<PreparedArtifactSetRecordV1>,
    original_binding: Option<Box<PublicationIntentBindingRecordV1>>,
    abort_finalization: Option<Box<PublicationAbortFinalizationRecordV1>>,
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
        prepared: &PublicationIntent,
    ) -> Result<Self, ControlError> {
        if prepared.state != PublicationIntentState::Prepared || prepared.target_epoch.get() == 0 {
            return Err(ControlError::InvalidValue);
        }
        let next = search_domain::transition_publication(
            prepared, PublicationIntentState::IntentDurable,
        ).map_err(|_| ControlError::InvalidValue)?;
        Ok(Self { operation_id, command_digest, expected_generation, previous: None, next, artifact_set: None, original_binding: None, abort_finalization: None })
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
    /// `CONTROL_COMMITTED`, `INVALIDATION_ONLY_COMMITTED` and `RECLAIMABLE` require the
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
        Ok(Self { operation_id, command_digest, expected_generation, previous: Some(previous), next, artifact_set: None, original_binding: None, abort_finalization: None })
    }

    /// Attaches the explicit v1 C02 record named by the unchanged prepared reference.
    /// Begin inserts it atomically with the intent; advance compares its full bytes.
    /// Root must supply metadata verified against actual C07 I/O and current owners.
    /// # Errors
    /// A different reference or a second attachment is refused, never relabeled.
    pub fn with_prepared_artifact_set_v1(mut self, record: PreparedArtifactSetRecordV1) -> Result<Self, ControlError> {
        if self.artifact_set.is_some() || self.next.prepared_manifest_ref != *record.reference() {
            return Err(ControlError::OperationConflict);
        }
        self.artifact_set = Some(record);
        Ok(self)
    }

    /// Attaches the original finite operation/context metadata to the first intent transaction.
    /// It is never reconstructed from a later abort attempt's recovery context.
    /// # Errors
    /// Refuses a non-initial intent or metadata that does not name the exact C02 command.
    pub fn with_original_binding_v1(
        mut self,
        record: PublicationIntentBindingRecordV1,
    ) -> Result<Self, ControlError> {
        let metadata = record.metadata();
        if self.original_binding.is_some() || self.previous.is_some() {
            return Err(ControlError::OperationConflict);
        }
        if metadata.c02_mutation_id != self.operation_id {
            return Err(ControlError::OperationConflict);
        }
        if metadata.c02_command_digest != self.command_digest {
            return Err(ControlError::OperationConflict);
        }
        if metadata.expected_control_generation != self.expected_generation {
            return Err(ControlError::OperationConflict);
        }
        if !record.matches_intent(&self.next) {
            return Err(ControlError::OperationConflict);
        }
        self.original_binding = Some(Box::new(record));
        Ok(self)
    }

    /// Atomically retains compact original abort metadata with the ABORTED intent.
    /// Full bindings, manifests, diffs and points remain source-backed and are
    /// rehydrated by Root from the retained C07/C13 records.
    ///
    /// # Errors
    /// Refuses a non-aborted transition, mismatched C02 identity/generation,
    /// changed intent identity/reference/guards, or duplicate attachment.
    pub fn with_abort_finalization_v1(
        mut self,
        record: PublicationAbortFinalizationRecordV1,
    ) -> Result<Self, ControlError> {
        let metadata = record.metadata();
        if self.abort_finalization.is_some()
            || self.next.state != PublicationIntentState::Aborted
            || metadata.c02_mutation_id != self.operation_id
            || metadata.c02_command_digest != self.command_digest
            || metadata.expected_control_generation != self.expected_generation
            || metadata.publication_intent_id != self.next.publication_intent_id
            || metadata.target_epoch != self.next.target_epoch
            || metadata.original_guards != self.next.owner_source_membership_access_guards
            || metadata.new_manifest_reference != self.next.prepared_manifest_ref
            || match &metadata.resolution {
                AbortResolutionMetadataV1::Compensated { .. } => {
                    record.abandonment_fence().is_some()
                }
                AbortResolutionMetadataV1::Excluded { .. } => record
                    .abandonment_fence()
                    .is_none_or(|fence| !fence.matches_abort(metadata)),
            }
        {
            return Err(ControlError::OperationConflict);
        }
        self.abort_finalization = Some(Box::new(record));
        Ok(self)
    }
    /// Exact proposed durable value, including unchanged preparation bindings.
    #[must_use]
    pub const fn intent(&self) -> &PublicationIntent { &self.next }

    pub(in crate::persistent) const fn port_operation_id(&self) -> MutationId { self.operation_id }

    pub(in crate::persistent) fn command(&self, limits: JournalLimits, check: &dyn Check) -> Result<ConditionalControlMutation, ControlError> {
        if (self.next.state == PublicationIntentState::Aborted) != self.abort_finalization.is_some() {
            return Err(ControlError::InvalidValue);
        }
        if self.original_binding.is_some() && self.previous.is_some() {
            return Err(ControlError::InvalidValue);
        }
        let key = ControlKey::new(KEY.to_vec(), limits)?;
        let expected = match &self.previous {
            Some(value) => ControlRecordCondition::exact(key.clone(), codec::encode(value, limits)?),
            None => ControlRecordCondition::absent(key.clone()),
        };
        let mut writes = vec![ControlWrite {
            key,
            value: codec::encode(&self.next, limits)?,
        }];
        let mut conditions = vec![expected];
        if self.previous.is_none() {
            let binding_key = operation_binding::key(limits)?;
            conditions.push(ControlRecordCondition::absent(binding_key.clone()));
            if let Some(binding) = &self.original_binding {
                writes.push(ControlWrite {
                    key: binding_key,
                    value: binding.value(limits, check)?,
                });
            }
        }
        if let Some(abort) = &self.abort_finalization {
            let abort_key = abort_finalization::key(limits)?;
            let abort_value = abort.value(limits, check)?;
            if let Some(fence) = abort.abandonment_fence() {
                conditions.push(ControlRecordCondition::exact(
                    abort_key.clone(),
                    fence.value(limits, check)?,
                ));
                conditions.push(ControlRecordCondition::exact(
                    ControlKey::new(visibility::STATE.to_vec(), limits)?,
                    visibility::codec::state(fence.fenced_visibility(), limits)?,
                ));
                for (membership_id, _, _, fenced_shadow) in fence.memberships() {
                    check.check(Point::PlanRecord)?;
                    conditions.push(ControlRecordCondition::exact(
                        visibility::id_key(
                            visibility::SHADOWS,
                            membership_id.as_bytes(),
                            limits,
                        )?,
                        visibility::codec::shadow(&fenced_shadow, limits)?,
                    ));
                }
            } else {
                conditions.push(ControlRecordCondition::absent(abort_key.clone()));
            }
            writes.push(ControlWrite { key: abort_key, value: abort_value });
        }
        let mutation = ControlMutation::new(
            self.operation_id,
            self.command_digest,
            self.expected_generation,
            writes,
            vec![],
        );
        prepared_artifacts::attach(
            &self.next.prepared_manifest_ref,
            self.artifact_set.as_ref(),
            self.previous.is_none(),
            limits,
            check,
            ConditionalControlMutation::new(mutation, conditions),
        )
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

    /// Returns the exact recovery-required intent, including a bare `ABORTED` value.
    /// `BLOCKED` and `COMPENSATING` remain unresolved. `CONTROL_COMMITTED` resolves
    /// the durable mutation, not snapshot admission; its commit still needs publication.
    /// Resolved values are omitted here but retained by `read_publication_intent`.
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

    pub(super) fn read_checkpoint_checked(&self, check: &dyn Check)
        -> Result<Option<crate::PublicationRecoveryCheckpoint>, ControlError> {
        self.ensure_available()?;
        check.check(Point::Start)?;
        if !crate::has_visibility_schema(self.identity.schema_version) {
            return Err(ControlError::SchemaUnsupported);
        }
        super::operating_profile::require_indexed_checked(self, check)?;
        let read = self.database.begin_read().map_err(|_| ControlError::StoreUnavailable)?;
        let snapshot = self.verify_from_checked(&read, check)?;
        // Reuse the existing relationship validator; all observations stay in
        // this same read transaction, with the original cooperative deadline.
        let visibility = visibility::validate_snapshot(&read, &snapshot, self.limits, check)?;
        let intent = snapshot.get(&ControlKey::new(KEY.to_vec(), self.limits)?)
            .map(codec::decode).transpose()?;
        let original_binding =
            operation_binding::read_snapshot(&snapshot, self.limits, check)?;
        if original_binding.as_ref().is_some_and(|record| {
            intent.as_ref().is_none_or(|intent| !record.matches_intent(intent))
        }) {
            return Err(ControlError::StoreCorrupt);
        }
        let abort_finalization =
            abort_finalization::read_snapshot(&snapshot, self.limits, check)?;
        let abandonment_fence = abandonment::read_snapshot(&snapshot, self.limits, check)?;
        if abort_finalization.as_ref().is_some_and(|record| {
            intent.as_ref().is_none_or(|intent| !record.matches_intent(intent))
                || original_binding.is_none()
        }) {
            return Err(ControlError::StoreCorrupt);
        }
        if abandonment_fence.as_ref().is_some_and(|record| {
            intent.as_ref().is_none_or(|intent| !record.matches_intent(intent))
                || original_binding.as_ref() != Some(record.original_binding())
        }) {
            return Err(ControlError::StoreCorrupt);
        }
        let publication_receipt = visibility
            .as_ref()
            .map(|state| {
                visibility::read_checkpoint_receipt(
                    &snapshot,
                    state,
                    abandonment_fence.as_ref(),
                    self.limits,
                    check,
                )
            })
            .transpose()?
            .flatten();
        check.check(Point::ReadComplete)?;
        Ok(visibility.map(|visibility| crate::PublicationRecoveryCheckpoint {
            identity: snapshot.identity,
            generation: snapshot.generation,
            visibility,
            intent,
            original_binding,
            abort_finalization,
            abandonment_fence,
            publication_receipt,
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
            self.require_intent_schema(&budget)?;
            let command = update.command(self.limits, &budget)?;
            self.recover_transaction_with_conditions_checked(command.mutation(), command.conditions(), &budget)
        })();
        result.map_err(|error| {
            let error = if budget.interrupted() { ControlError::CommitOutcomeUnknown } else { error };
            budget.failure(error, Some(update.operation_id)).for_recovery()
        })
    }

    fn require_intent_schema(&self, check: &dyn Check) -> Result<(), ControlError> {
        if crate::has_intent_schema(self.identity.schema_version) {
            super::operating_profile::require_indexed_checked(self, check)
        }
        else { Err(ControlError::SchemaUnsupported) }
    }

    pub(in crate::persistent) fn read_intent_checked(&self, check: &dyn Check) -> Result<PublicationIntentHead, ControlError> {
        self.ensure_available()?;
        check.check(Point::Start)?;
        self.require_intent_schema(check)?;
        let read = self.database.begin_read().map_err(|_| ControlError::StoreUnavailable)?;
        let header = self.header_from_checked(&read, check)?;
        check.check(Point::ReadHeader)?;
        if crate::has_visibility_schema(self.identity.schema_version) {
            self.snapshot_from_checked(&read, check)?;
        }
        let records = read.open_table(RECORDS).map_err(map_table_error)?;
        let intent = if let Some(raw) = records.get(KEY).map_err(|error| map_storage_error(&error))? { Some(codec::decode(&super::decode_value_checked(raw.value(), self.limits, check)?)?) } else {
            verify_never_written(&read, header.generation, self.limits, check)?;
            None
        };
        check.check(Point::ReadComplete)?;
        Ok(PublicationIntentHead { identity: self.identity, generation: header.generation, intent })
    }

    pub(super) fn persist_intent_checked(
        &mut self, update: &PublicationIntentUpdate, boundary: Boundary, check: &dyn Check,
    ) -> Result<ControlCommitReceipt, ControlError> {
        let result = (|| {
            self.ensure_available()?;
            check.check(Point::Start)?;
            self.require_intent_schema(check)?;
            if update.artifact_set.is_some() && !crate::has_visibility_schema(self.identity.schema_version) {
                return Err(ControlError::SchemaUnsupported);
            }
            let command = update.command(self.limits, check)?;
            check.check(Point::Validated)?;
            {
                let read = self.database.begin_read().map_err(|_| ControlError::StoreUnavailable)?;
                let header = self.header_from_checked(&read, check)?;
                let replay = super::operation_from_checked(&read, update.operation_id, &header, self.limits, check)?.is_some();
                // Do not reject an already committed old-owner request before
                // the common replay engine verifies its exact fingerprint.
                let prepared_owner = update.next.owner_source_membership_access_guards.owner_epoch;
                let recovery_state = matches!(update.next.state, PublicationIntentState::Compensating
                    | PublicationIntentState::PublicationBlocked | PublicationIntentState::Aborted);
                if !replay && (prepared_owner > self.identity.owner_epoch
                    || (prepared_owner < self.identity.owner_epoch && !recovery_state)) {
                    return Err(ControlError::GenerationMismatch);
                }
                if crate::has_visibility_schema(self.identity.schema_version) {
                    let snapshot = self.snapshot_from_checked(&read, check)?;
                    let pending_fence = abandonment::read_snapshot(&snapshot, self.limits, check)?;
                    if !replay && pending_fence.as_ref().is_some_and(|fence| {
                        update.next.state != PublicationIntentState::Aborted
                            || update.abort_finalization.as_ref().is_none_or(|record| {
                                record.abandonment_fence() != Some(fence)
                            })
                    }) {
                        return Err(ControlError::OperationConflict);
                    }
                    if !replay && update.previous.is_none() {
                        visibility::validate_initial_intent(&snapshot, &update.next, self.limits)?;
                    }
                }
                let table = read.open_table(RECORDS).map_err(map_table_error)?;
                match table.get(KEY).map_err(|error| map_storage_error(&error))? {
                    Some(raw) => { codec::decode(&super::decode_value_checked(raw.value(), self.limits, check)?)?; }
                    None => verify_never_written(&read, header.generation, self.limits, check)?,
                }
            }
            super::transaction::execute_publication(self, &command, boundary, check)
        })();
        if crate::has_intent_schema(self.identity.schema_version)
            && result.as_ref().err().is_some_and(|error| is_corruption(*error)) {
            self.quarantined = true;
        }
        result
    }
}

// The shared port routes these keys through typed publication commands only.
// Reuse the owning modules' key definitions instead of a second reserved-key list.
pub(super) fn port_reserved_key(key: &[u8]) -> bool {
    key == KEY || abort_finalization::reserved(key) || operation_binding::reserved(key)
        || visibility::port_reserved_key(key) || prepared_artifacts::reserved(key)
}

pub(super) const fn unresolved(state: PublicationIntentState) -> bool {
    // The stored terminal label alone proves neither two-sided compensation nor
    // effective membership-wide exclusion. Keep ABORTED recovery-visible.
    !matches!(state, PublicationIntentState::ControlCommitted
        | PublicationIntentState::Reclaimable | PublicationIntentState::InvalidationOnlyCommitted)
}

pub(super) fn validate_record(key: &ControlKey, value: &ControlValue, limits: JournalLimits, check: &dyn Check) -> Result<(), ControlError> {
    if key.as_bytes() == KEY {
        codec::decode(value)?;
    } else if abort_finalization::reserved(key.as_bytes()) {
        abandonment::validate_record(value, limits, check)?;
    } else if operation_binding::reserved(key.as_bytes()) {
        PublicationIntentBindingRecordV1::decode(value, limits, check)?;
    }
    Ok(())
}

pub(super) fn require_resolved(records: &[(ControlKey, ControlValue)]) -> Result<(), ControlError> {
    if let Ok(index) = records.binary_search_by(|(key, _)| key.as_bytes().cmp(KEY))
        && unresolved(codec::decode(&records[index].1)?.state)
    {
        return Err(ControlError::SnapshotRebuildFailed);
    }
    Ok(())
}

/// Allows an internal current-control snapshot after an exact abort commit while
/// keeping the serving projection snapshot unresolved and unavailable.
pub(super) fn require_abort_snapshot(
    records: &[(ControlKey, ControlValue)],
    limits: JournalLimits,
    check: &dyn Check,
) -> Result<(), ControlError> {
    let intent = records
        .binary_search_by(|(key, _)| key.as_bytes().cmp(KEY))
        .ok()
        .map(|index| codec::decode(&records[index].1))
        .transpose()?;
    let original_binding_key = operation_binding::key(limits)?;
    let original_binding = records
        .binary_search_by(|(key, _)| key.as_bytes().cmp(original_binding_key.as_bytes()))
        .ok()
        .map(|index| PublicationIntentBindingRecordV1::decode(&records[index].1, limits, check))
        .transpose()?;
    let abort_key = abort_finalization::key(limits)?;
    let abort = records
        .binary_search_by(|(key, _)| key.as_bytes().cmp(abort_key.as_bytes()))
        .ok()
        .map(|index| PublicationAbortFinalizationRecordV1::decode(
            &records[index].1,
            limits,
            check,
        ))
        .transpose()?;
    match (intent.as_ref(), abort.as_ref()) {
        (Some(intent), Some(record))
            if intent.state == PublicationIntentState::Aborted
                && record.matches_intent(intent)
                && original_binding
                    .as_ref()
                    .is_some_and(|binding| binding.matches_intent(intent)) =>
        {
            Ok(())
        }
        _ => Err(ControlError::SnapshotRebuildFailed),
    }
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
    for (count, row) in operations.iter().map_err(|error| map_storage_error(&error))?.enumerate() {
        check.check(Point::ReadOperation)?;
        if count >= limits.max_operation_records { return Err(ControlError::StoreCorrupt); }
        let (id, bytes) = row.map_err(|error| map_storage_error(&error))?;
        let id = MutationId(id.value().try_into().map_err(|_| ControlError::StoreCorrupt)?);
        let operation = StoredOperation::decode_checked(bytes.value(), id, generation, limits, check)?;
        if operation.receipt.changed_keys.iter().any(|key| key.as_bytes() == KEY) {
            return Err(ControlError::StoreCorrupt);
        }
    }
    check.check(Point::ReadComplete)
}

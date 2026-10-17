//! Normal committed-epoch succession through the existing conditional journal.
//! Aborted, skipped and invalidation-only epochs require separate recovery.

use super::{MutationId, Blake3Digest32, PublicationIntent, PublicationVisibilityState, fmt,
    ControlError, PublicationIntentState, JournalLimits, ConditionalControlMutation, key,
    ControlWrite, intent_codec, ControlRecordCondition, STATE, codec, ControlMutation,
    PersistentControlJournal, CancellationProbe, OperationContext, ControlCommitReceipt,
    ControlCallError, Budget, Boundary, CommitRecoveryDecision, Check, Point, require_schema,
    RECEIPTS, is_corruption,
    JournalReadSnapshot};
use crate::ControlSnapshotPublisher;

/// Exact command reserving the next intent after a fully committed publication.
///
/// This contains expected state, not authorization or external readback evidence.
/// The journal compares it with coherent durable state and requires the current
/// disk-bound snapshot to have been published before new forward progress.
#[derive(Clone, Eq, PartialEq)]
pub struct PublicationSuccessor {
    operation_id: MutationId,
    command_digest: Blake3Digest32,
    expected_generation: u64,
    previous: PublicationIntent,
    visibility: PublicationVisibilityState,
    next: PublicationIntent,
    artifact_set: Option<super::super::PreparedArtifactSetRecordV1>,
    original_binding: Option<Box<super::super::PublicationIntentBindingRecordV1>>,
    previous_original_binding: Option<Box<super::super::PublicationIntentBindingRecordV1>>,
}

impl fmt::Debug for PublicationSuccessor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PublicationSuccessor")
            .field("expected_generation", &self.expected_generation)
            .field("target_epoch", &self.next.target_epoch)
            .finish_non_exhaustive()
    }
}

impl PublicationSuccessor {
    /// Reserves the exact next epoch of an ordinary `CONTROL_COMMITTED` intent.
    /// All inputs are revalidated against storage by the executing journal.
    ///
    /// # Errors
    /// Rejects unresolved/aborted predecessors, reused identity, missing latest
    /// receipt, stale guards, skipped epochs and exhausted generation counters.
    pub fn new(
        operation_id: MutationId,
        command_digest: Blake3Digest32,
        expected_generation: u64,
        previous: PublicationIntent,
        visibility: PublicationVisibilityState,
        prepared: &PublicationIntent,
    ) -> Result<Self, ControlError> {
        expected_generation.checked_add(1).ok_or(ControlError::GenerationExhausted)?;
        let next_epoch = previous.target_epoch.checked_next()
            .map_err(|_| ControlError::GenerationExhausted)?;
        if expected_generation == 0
            || previous.state != PublicationIntentState::ControlCommitted
            || previous.target_epoch.get() == 0
            || previous.target_epoch != visibility.visible_epoch
            || visibility.last_receipt.is_none()
            || prepared.state != PublicationIntentState::Prepared
            || prepared.target_epoch != next_epoch
            || prepared.publication_intent_id == previous.publication_intent_id
            || prepared.owner_source_membership_access_guards != visibility.guards
        {
            return Err(ControlError::InvalidValue);
        }
        let next = search_domain::transition_publication(
            prepared, PublicationIntentState::IntentDurable,
        ).map_err(|_| ControlError::InvalidValue)?;
        Ok(Self { operation_id, command_digest, expected_generation, previous, visibility, next, artifact_set: None, original_binding: None, previous_original_binding: None })
    }

    /// Atomically retains a v1 C02 set record with this new durable intent.
    /// The existing prepared reference must already name the exact full record.
    /// # Errors
    /// Refuses reference substitution or duplicate attachment; grants no C07 proof.
    pub fn with_prepared_artifact_set_v1(mut self, record: super::super::PreparedArtifactSetRecordV1) -> Result<Self, ControlError> {
        if self.artifact_set.is_some() || self.next.prepared_manifest_ref != *record.reference() {
            return Err(ControlError::OperationConflict);
        }
        self.artifact_set = Some(record);
        Ok(self)
    }

    /// Replaces the finite original-binding sidecar with the new intent in the same CAS.
    /// Root supplies the prior actual record from the coherent recovery checkpoint.
    /// # Errors
    /// Refuses metadata that does not bind the exact previous and next C02 intents.
    pub fn with_original_binding_v1(
        mut self,
        next: super::super::PublicationIntentBindingRecordV1,
        previous: super::super::PublicationIntentBindingRecordV1,
    ) -> Result<Self, ControlError> {
        let next_metadata = next.metadata();
        if self.original_binding.is_some() || self.previous_original_binding.is_some() {
            return Err(ControlError::OperationConflict);
        }
        if next_metadata.c02_mutation_id != self.operation_id {
            return Err(ControlError::OperationConflict);
        }
        if next_metadata.c02_command_digest != self.command_digest {
            return Err(ControlError::OperationConflict);
        }
        if next_metadata.expected_control_generation != self.expected_generation {
            return Err(ControlError::OperationConflict);
        }
        if !next.matches_intent(&self.next) {
            return Err(ControlError::OperationConflict);
        }
        if !previous.matches_intent(&self.previous) {
            return Err(ControlError::OperationConflict);
        }
        self.original_binding = Some(Box::new(next));
        self.previous_original_binding = Some(Box::new(previous));
        Ok(self)
    }

    /// Exact intent to be made durable, without advancing `VisibleEpoch`.
    #[must_use]
    pub const fn intent(&self) -> &PublicationIntent { &self.next }

    pub(in crate::persistent) const fn port_operation_id(&self) -> MutationId { self.operation_id }

    pub(in crate::persistent) fn command(&self, limits: JournalLimits, check: &dyn Check) -> Result<ConditionalControlMutation, ControlError> {
        let intent_key = key(super::super::KEY, limits)?;
        let mut writes = vec![ControlWrite {
            key: intent_key.clone(), value: intent_codec::encode(&self.next, limits)?,
        }];
        let mut conditions = vec![
            ControlRecordCondition::exact(intent_key, intent_codec::encode(&self.previous, limits)?),
            ControlRecordCondition::exact(key(STATE, limits)?, codec::state(&self.visibility, limits)?),
        ];
        let binding_key = super::super::operation_binding::key(limits)?;
        match (&self.original_binding, &self.previous_original_binding) {
            (Some(next), Some(previous)) => {
                conditions.push(ControlRecordCondition::exact(
                    binding_key.clone(),
                    previous.value(limits, check)?,
                ));
                writes.push(ControlWrite {
                    key: binding_key,
                    value: next.value(limits, check)?,
                });
            }
            (None, None) => conditions.push(ControlRecordCondition::absent(binding_key)),
            _ => return Err(ControlError::InvalidValue),
        }
        let command = ConditionalControlMutation::new(ControlMutation::new(
            self.operation_id, self.command_digest, self.expected_generation, writes, vec![],
        ), conditions);
        super::super::prepared_artifacts::attach(&self.next.prepared_manifest_ref, self.artifact_set.as_ref(),
            true, limits, check, command)
    }
}

impl PersistentControlJournal {
    /// Durably replaces one completed intent with its exact next-epoch successor.
    ///
    /// The previous visibility, receipt and published snapshot are checked first.
    /// The existing CAS then compares the exact previous intent/visibility and
    /// control generation inside the write transaction. Only the intent changes;
    /// `VisibleEpoch`, manifests, shadows and old receipts are retained unchanged.
    ///
    /// # Errors
    /// An unpublished, suspended, foreign or model-backed snapshot cannot permit
    /// a new reservation. A pending predecessor, reused ID or stale command is
    /// refused. Interruption after dispatch requires the exact request's recovery.
    pub fn reserve_next_publication<C: CancellationProbe>(
        &mut self,
        request: &PublicationSuccessor,
        publisher: &ControlSnapshotPublisher,
        context: &OperationContext<C>,
    ) -> Result<ControlCommitReceipt, ControlCallError> {
        let budget = Budget::new(context);
        self.reserve_successor_checked(request, publisher, Boundary::Normal, &budget)
            .map_err(|error| budget.failure(error, Some(request.operation_id)))
    }

    /// Recovers the original reservation without re-running pre-state checks or
    /// reserving another epoch. Recovery never publishes or acknowledges a snapshot.
    ///
    /// # Errors
    /// Changed inputs cannot clear a pending operation. Interrupted inspection
    /// preserves its unknown outcome; corrupt current state remains quarantined.
    pub fn recover_publication_successor<C: CancellationProbe>(
        &mut self,
        request: &PublicationSuccessor,
        context: &OperationContext<C>,
    ) -> Result<CommitRecoveryDecision, ControlCallError> {
        let budget = Budget::new(context);
        let result = (|| {
            budget.check(Point::Start)?;
            require_schema(self, &budget)?;
            let command = request.command(self.limits, &budget)?;
            self.recover_transaction_with_conditions_checked(
                command.mutation(), command.conditions(), &budget,
            )
        })();
        result.map_err(|error| budget.failure(if budget.interrupted() {
            ControlError::CommitOutcomeUnknown
        } else { error }, Some(request.operation_id)).for_recovery())
    }

    pub(in crate::persistent) fn reserve_successor_checked(
        &mut self,
        request: &PublicationSuccessor,
        publisher: &ControlSnapshotPublisher,
        boundary: Boundary,
        check: &dyn Check,
    ) -> Result<ControlCommitReceipt, ControlError> {
        let result = (|| {
            self.ensure_available()?;
            check.check(Point::Start)?;
            require_schema(self, check)?;
            let command = request.command(self.limits, check)?;
            check.check(Point::Validated)?;
            {
                let read = self.database.begin_read().map_err(|_| ControlError::StoreUnavailable)?;
                let header = self.header_from_checked(&read, check)?;
                let replay = super::super::super::operation_from_checked(&read, request.operation_id, &header, self.limits, check)?.is_some();
                let snapshot = self.snapshot_from_checked(&read, check)?;
                // Existing receipts remain replayable after later progress or
                // owner handoff. The common engine checks the full fingerprint
                // before returning a historical receipt; it never reapplies it.
                if !replay {
                    if header.generation != request.expected_generation {
                        return Err(ControlError::TransactionConflict);
                    }
                    if request.next.owner_source_membership_access_guards.owner_epoch != self.identity.owner_epoch {
                        return Err(ControlError::GenerationMismatch);
                    }
                    require_published_predecessor(self, &snapshot, publisher, check)?;
                    for (key, value) in &snapshot.records {
                        check.check(Point::PlanRecord)?;
                        if key.as_bytes().starts_with(RECEIPTS)
                            && codec::read_receipt(value)?.intent.publication_intent_id
                                == request.next.publication_intent_id
                        {
                            return Err(ControlError::OperationConflict);
                        }
                    }
                }
            }
            super::super::super::transaction::execute_publication(self, &command, boundary, check)
        })();
        if crate::has_visibility_schema(self.identity.schema_version)
            && result.as_ref().err().is_some_and(|error| is_corruption(*error))
        {
            self.quarantined = true;
        }
        result
    }
}

fn require_published_predecessor(
    journal: &PersistentControlJournal,
    snapshot: &JournalReadSnapshot,
    publisher: &ControlSnapshotPublisher,
    check: &dyn Check,
) -> Result<(), ControlError> {
    if publisher.diagnostic_disk_identity() != Some(journal.identity) {
        return Err(ControlError::SnapshotPublicationFailed);
    }
    let current = publisher.current().ok_or(ControlError::SnapshotPublicationFailed)?;
    if current.identity != snapshot.identity || current.generation != snapshot.generation
        || current.records.len() != snapshot.records.len()
    {
        return Err(ControlError::SnapshotPublicationFailed);
    }
    for (published, durable) in current.records.iter().zip(&snapshot.records) {
        check.check(Point::SnapshotRebuild)?;
        check.reserve(crate::MigrationBootstrapWork::Process(published.0.as_bytes().len() + published.1.len()))?;
        if published != durable { return Err(ControlError::SnapshotPublicationFailed); }
    }
    check.check(Point::SnapshotPrepared)
}

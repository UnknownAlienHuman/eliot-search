//! Concrete binding from source-owner canonical metadata to the single redb owner.
//!
//! This adapter opens no files and mints no native, policy or serving authority.
//! Its caller retains the admitted root/journal custody and original operation
//! context. Persisted admission inputs prove historical equality only.

#[cfg(test)]
#[path = "registry_control/tests.rs"]
mod tests;

mod original_inputs;
mod v4_restore;
mod v4_readback;
mod source_ids;


use search_contracts::OpaqueId;
use search_control_redb::{
    BoundControlJournal, ControlError, ControlPortCommand, ControlPortError,
    ControlSnapshotPublisher, JournalIdentity, JournalLimits, PersistentControlJournal,
    RegistryMutationRequest, RegistryOperationEnvelope, RegistryReadSnapshot, RegistryStateChange,
    RegistryStateFamily as StoredFamily, RegistryStateKey as StoredKey, RegistryStoredState,
    RegistryTransactionReadback, control_mutation_identity,
};
use search_ports::{
    CancellationProbe, ControlJournalPort, MutationIdentity, OperationContext, Port, PortErrorKind,
    PortFailure, PortReceipt, PortRetryability,
};
use search_source_registry::error::{
    RegistryControlPort, RegistryError, RegistryJournalEntry, RegistryLimits, RegistryPortError,
    registry_mutation, registry_port_error,
};
use search_source_admission::{AcceptedCapabilities, SectionPlatform};
use search_source_registry::recovery::{
    DecodedBatch, MAX_REGISTRY_RESTORE_BYTES, ObservedRegistryRestore, PreparedBatch,
    RegistryBatchLookup, RegistryBatchReadback, RegistryRestoreOperation, RegistryRestoreState,
    RegistryStateFamily, RegistryStateKey, RegistryStateRecord, decode_batch_record,
    decode_registry_state_entry, decode_registry_state_key, decode_registry_state_key_v4,
    restore_registry_state,
};

/// One exclusive borrowed adapter; no journal, operation ledger or snapshot copy.
type OriginalNativeWork<'a> = &'a dyn Fn(search_control_redb::MigrationBootstrapWork) -> Result<(), search_runtime_owner::OwnerError>;
pub struct RegistryJournal<'journal, C: CancellationProbe> {
    original: Option<(std::time::Instant, OriginalNativeWork<'journal>)>,
    original_v4: Option<v4_readback::OriginalV4Context<'journal>>,
    journal: BoundControlJournal<'journal, C>,
    identity: JournalIdentity,
    registry_limits: RegistryLimits,
    pending: Option<RegistryMutationRequest>,
    original_uncertain: bool,
    cancellation: core::marker::PhantomData<fn() -> C>,
}

impl<'journal, C: CancellationProbe> RegistryJournal<'journal, C> {
    pub(crate) const fn requires_recovery(&self) -> bool { self.pending.is_some() || self.original_uncertain }
    /// The enclosing native owner keeps its verified journal/root alive.
    pub(crate) fn bind(
        journal: &'journal mut PersistentControlJournal,
        publisher: &'journal mut ControlSnapshotPublisher,
        expected: JournalIdentity,
        registry_limits: RegistryLimits,
    ) -> Result<Self, RegistryPortError> {
        registry_limits.validate().map_err(owner_failure)?;
        if journal.identity() != expected || journal.requires_recovery() {
            return Err(owner_failure(RegistryError::MutationOutcomeUnknown));
        }
        let journal = journal
            .bind_control_port(publisher)
            .map_err(|error| storage_failure(error, None))?;
        Ok(Self {
            original: None,
            original_v4: None,
            journal,
            identity: expected,
            registry_limits,
            pending: None,
            original_uncertain: false,
            cancellation: core::marker::PhantomData,
        })
    }

    pub(crate) fn bind_original(
        journal: &'journal mut PersistentControlJournal, publisher: &'journal mut ControlSnapshotPublisher,
        expected: JournalIdentity, limits: RegistryLimits, deadline: std::time::Instant,
        work: &'journal dyn Fn(search_control_redb::MigrationBootstrapWork) -> Result<(), search_runtime_owner::OwnerError>,
    ) -> Result<Self, RegistryPortError> {
        let mut result = Self::bind(journal, publisher, expected, limits)?;
        result.original = Some((deadline, work));
        Ok(result)
    }

    /// Binds normal v4 readback to the actual caller platform/capabilities and
    /// the same retained original deadline/work pair.
    pub(crate) fn bind_original_v4(
        journal: &'journal mut PersistentControlJournal,
        publisher: &'journal mut ControlSnapshotPublisher,
        expected: JournalIdentity,
        limits: RegistryLimits,
        deadline: std::time::Instant,
        work: &'journal dyn Fn(
            search_control_redb::MigrationBootstrapWork,
        ) -> Result<(), search_runtime_owner::OwnerError>,
        platform: SectionPlatform,
        capabilities: AcceptedCapabilities,
    ) -> Result<Self, RegistryPortError> {
        let mut result = Self::bind(journal, publisher, expected, limits)?;
        result.original = Some((deadline, work));
        result.original_v4 = Some(v4_readback::OriginalV4Context {
            deadline,
            work,
            platform,
            capabilities,
            storage_limits: result.journal.limits(),
            registry_limits: result.registry_limits,
        });
        Ok(result)
    }
}

impl<C: CancellationProbe> Port for RegistryJournal<'_, C> {
    type Error = RegistryPortError;
    type Cancellation = C;
}

impl<C: CancellationProbe> RegistryControlPort for RegistryJournal<'_, C> {
    fn commit_batch(
        &mut self,
        batch: &PreparedBatch,
        context: &OperationContext<C>,
        mutation: &MutationIdentity,
    ) -> Result<RegistryBatchReadback, RegistryPortError> {
        context
            .preflight()
            .map_err(|_| owner_failure(RegistryError::CancelledBeforeCommit))?;
        let id = &batch.record().entry().operation_id;
        if mutation != &registry_mutation(id) || self.pending.is_some() {
            return Err(owner_failure(RegistryError::MutationOutcomeUnknown));
        }
        let lookup = self.lookup(id, context)?;
        let generation = match lookup {
            search_control_redb::RegistryOperationLookup::Absent {
                identity,
                journal_generation,
                registry_revision,
            } => {
                if identity != self.identity
                    || registry_revision != batch.expected_registry_revision()
                {
                    return Err(owner_failure(RegistryError::RegistryRevisionConflict));
                }
                journal_generation
            }
            search_control_redb::RegistryOperationLookup::Committed(observed) => {
                if observed.identity() != self.identity
                    || observed.envelope().owner_bytes() != batch.encoded()
                {
                    return Err(owner_failure(RegistryError::OperationConflict));
                }
                // The existing operation owns its original backend generation.
                observed.envelope().journal_generation_before()
            }
        };
        let request = prepare(
            batch,
            generation,
            self.journal.limits(),
            self.registry_limits,
        )?;
        context
            .preflight()
            .map_err(|_| owner_failure(RegistryError::CancelledBeforeCommit))?;
        self.pending = Some(request.clone());
        let control_identity = control_mutation_identity(request.control_operation_id())
            .map_err(|error| storage_failure(error, Some(id)))?;
        let dispatched = if let Some((deadline, callback)) = self.original {
            self.journal.commit_registry_batch_with_original_context(&request, &control_identity, deadline, &callback)
                .map(|_| ()).map_err(|error| storage_failure(error.control, Some(id)))
        } else { self.journal.transact(
            &ControlPortCommand::Registry(request.clone()),
            context,
            &control_identity,
        ).map(|_| ()).map_err(|error| call_failure(&error, Some(id))) };
        match dispatched {
            Ok(()) => {}
            Err(error) => {
                if error.kind() != PortErrorKind::OutcomeUnknown {
                    self.pending = None;
                }
                return Err(error);
            }
        }
        let keys = request
            .changes()
            .iter()
            .map(|change| change.key().clone())
            .collect::<Vec<_>>();
        let observed = self.targets(request.envelope(), &keys, context).map_err(|_| {
                registry_port_error(
                    PortErrorKind::OutcomeUnknown,
                    PortRetryability::AfterReadback,
                    RegistryError::MutationOutcomeUnknown,
                    Some(id.clone()),
                )
            })?;
        let result = self.readback(&observed, context).map_err(|_| {
            registry_port_error(
                PortErrorKind::OutcomeUnknown,
                PortRetryability::AfterReadback,
                RegistryError::MutationOutcomeUnknown,
                Some(id.clone()),
            )
        })?;
        self.pending = None;
        Ok(result)
    }

    fn load_batch(
        &self,
        id: &OpaqueId,
        context: &OperationContext<C>,
    ) -> Result<RegistryBatchLookup, RegistryPortError> {
        context
            .preflight()
            .map_err(|_| owner_failure(RegistryError::CancelledBeforeCommit))?;
        let lookup = self.lookup(id, context)?;
        let observed = match lookup {
            search_control_redb::RegistryOperationLookup::Absent {
                identity,
                registry_revision,
                ..
            } => {
                if identity != self.identity {
                    return Err(owner_failure(RegistryError::SnapshotInvalid));
                }
                return Ok(RegistryBatchLookup::Absent {
                    revision: registry_revision,
                });
            }
            search_control_redb::RegistryOperationLookup::Committed(observed) => observed,
        };
        if observed.identity() != self.identity {
            return Err(owner_failure(RegistryError::SnapshotInvalid));
        }
        let decoded = self.decode_operation_for_load(
            observed.envelope(),
            observed.journal_generation(),
            context,
        )?;
        let limits = self.journal.limits();
        let keys = decoded
            .deltas()
            .iter()
            .map(|delta| {
                let encoded = delta.key().encode().map_err(owner_failure)?;
                state_key(delta.key(), &encoded, limits)
            })
            .collect::<Result<Vec<_>, RegistryPortError>>()?;
        // Recheck the immutable operation alongside all targets in one read.
        let targets = self.targets(observed.envelope(), &keys, context)?;
        self.readback(&targets, context)
            .map(RegistryBatchLookup::Committed)
    }

    fn persist_entry(
        &mut self,
        _entry: &RegistryJournalEntry,
        _context: &OperationContext<C>,
        _mutation: &MutationIdentity,
    ) -> Result<PortReceipt, RegistryPortError> {
        // Receipt-only effects cannot enter this canonical durable authority.
        Err(owner_failure(RegistryError::DurabilityRejected))
    }

    fn load_entry(
        &self,
        id: &OpaqueId,
        context: &OperationContext<C>,
    ) -> Result<Option<RegistryJournalEntry>, RegistryPortError> {
        match self.load_batch(id, context)? {
            RegistryBatchLookup::Absent { .. } => Ok(None),
            RegistryBatchLookup::Committed(observed) => Ok(Some(observed.record().entry().clone())),
        }
    }
}

impl<C: CancellationProbe> RegistryJournal<'_, C> {
    fn lookup(&self, id: &OpaqueId, context: &OperationContext<C>) -> Result<search_control_redb::RegistryOperationLookup, RegistryPortError> {
        if let Some((deadline, callback)) = self.original {
            self.journal.lookup_registry_operation_with_original_context(id, deadline, &callback)
                .map_err(|e| storage_failure(e.control, Some(id)))
        } else { self.journal.lookup_registry_operation(id, context).map_err(|e| call_failure(&e, Some(id))) }
    }
    fn targets(&self, envelope: &RegistryOperationEnvelope, keys: &[StoredKey], context: &OperationContext<C>) -> Result<RegistryTransactionReadback, RegistryPortError> {
        if let Some((deadline, callback)) = self.original {
            self.journal.read_registry_operation_targets_with_original_context(envelope, keys, deadline, &callback)
                .map_err(|e| storage_failure(e.control, Some(envelope.operation_id())))
        } else { self.journal.read_registry_operation_targets(envelope, keys, context).map_err(|e| call_failure(&e, Some(envelope.operation_id()))) }
    }

    fn decode_operation_for_load(
        &self,
        envelope: &RegistryOperationEnvelope,
        observed_generation: u64,
        context: &OperationContext<C>,
    ) -> Result<DecodedBatch, RegistryPortError> {
        match decode_operation(envelope, self.registry_limits) {
            Ok(decoded) => Ok(decoded),
            Err(legacy_error) => {
                let Some(original_v4) = self.original_v4.as_ref() else {
                    return Err(legacy_error);
                };
                if !v4_restore::is_v4_batch(envelope.owner_bytes()) {
                    return Err(legacy_error);
                }
                let shape = v4_readback::collect_transaction_refs(envelope, &[], original_v4)
                    .map_err(|error| v4_failure(error, Some(envelope.operation_id())))?;
                if !shape.operation_v4() {
                    return Err(legacy_error);
                }
                let references = shape
                    .references()
                    .iter()
                    .map(Vec::as_slice)
                    .collect::<Vec<_>>();
                if references.is_empty() {
                    return Err(owner_failure(RegistryError::AdmissionRestorationUnavailable));
                }
                let originals = self
                    .original_inputs_snapshot(&references, context)
                    .map_err(original_inputs_failure)?;
                if originals.identity() != self.identity
                    || originals.generation() != observed_generation
                {
                    return Err(owner_failure(RegistryError::SnapshotInvalid));
                }
                v4_readback::decode_transaction(
                    &shape,
                    envelope,
                    &[],
                    &originals,
                    original_v4,
                )
                .map(|(decoded, _)| decoded)
                .map_err(|error| v4_failure(error, Some(envelope.operation_id())))
            }
        }
    }

    fn readback(
        &self,
        observed: &RegistryTransactionReadback,
        context: &OperationContext<C>,
    ) -> Result<RegistryBatchReadback, RegistryPortError> {
        let Some(original_v4) = self.original_v4.as_ref() else {
            return readback(observed, self.identity, self.registry_limits);
        };
        let operation_v4 = v4_restore::is_v4_batch(observed.operation().owner_bytes());
        let state_v4 = observed
            .states()
            .iter()
            .any(|state| v4_restore::is_v4_state(state.canonical_bytes()));
        if !operation_v4 && !state_v4 {
            return readback(observed, self.identity, self.registry_limits);
        }
        if !operation_v4 {
            decode_batch_record(observed.operation().owner_bytes(), self.registry_limits)
                .map_err(owner_failure)?;
        }
        for state in observed.states() {
            if !v4_restore::is_v4_state(state.canonical_bytes()) {
                decode_registry_state_entry(
                    state.key().canonical_bytes(),
                    state.canonical_bytes(),
                )
                .map_err(owner_failure)?;
            }
        }
        let shape = v4_readback::collect_transaction_refs(
            observed.operation(),
            observed.states(),
            original_v4,
        )
        .map_err(|error| v4_failure(error, Some(observed.operation().operation_id())))?;
        if !shape.has_v4() {
            return readback(observed, self.identity, self.registry_limits);
        }
        let references = shape
            .references()
            .iter()
            .map(Vec::as_slice)
            .collect::<Vec<_>>();
        if references.is_empty() {
            return Err(owner_failure(RegistryError::AdmissionRestorationUnavailable));
        }
        let originals = self
            .original_inputs_snapshot(&references, context)
            .map_err(original_inputs_failure)?;
        if originals.identity() != self.identity
            || originals.generation() != observed.journal_generation()
        {
            return Err(owner_failure(RegistryError::SnapshotInvalid));
        }
        let (decoded, states) = v4_readback::decode_transaction(
            &shape,
            observed.operation(),
            observed.states(),
            &originals,
            original_v4,
        )
        .map_err(|error| v4_failure(error, Some(observed.operation().operation_id())))?;
        let expected_keys = decoded
            .deltas()
            .iter()
            .map(search_source_registry::recovery::RegistryStateDelta::key)
            .collect::<std::collections::BTreeSet<_>>();
        let actual_keys = states
            .iter()
            .map(RegistryStateRecord::key)
            .collect::<std::collections::BTreeSet<_>>();
        if states.len() != decoded.deltas().len()
            || actual_keys.len() != states.len()
            || !actual_keys.iter().all(|key| expected_keys.contains(key))
        {
            return Err(owner_failure(RegistryError::SnapshotInvalid));
        }
        RegistryBatchReadback::new(decoded, observed.registry_revision(), states, self.registry_limits)
            .map_err(owner_failure)
    }
    /// Explicitly rebuild the current technical snapshot after owner installation.
    /// This publishes verified disk metadata only; native/current-policy/grant
    /// admission remains a separate owning runtime obligation. Historical replay
    /// uses the current disk generation, never an obsolete operation receipt.
    #[cfg(test)]
    pub(crate) fn finish_metadata_publication(
        &mut self,
        context: &OperationContext<C>,
    ) -> Result<Option<search_control_redb::SnapshotPublishReceipt>, RegistryPortError> {
        if self.pending.is_some() {
            return Err(owner_failure(RegistryError::MutationOutcomeUnknown));
        }
        self.journal
            .recover_snapshot(context)
            .map_err(|error| call_failure(&error, None))
    }

    /// Reconstruct observed metadata and immutable history from one bounded disk read.
    /// Root/policy/security revalidation is a distinct subsequent native owner step.
    pub(crate) fn restore(
        &self,
        context: &OperationContext<C>,
    ) -> Result<ObservedRegistryRestore, RegistryPortError> {
        self.restore_for_root(None, context).map(|v| v.0)
    }

    /// Select a historical original only from the same fully validated startup read.
    pub(crate) fn restore_for_root(&self, root: Option<search_contracts::RootBindingId>, context: &OperationContext<C>)
        -> Result<(ObservedRegistryRestore, Option<OpaqueId>), RegistryPortError> {
        context
            .preflight()
            .map_err(|_| owner_failure(RegistryError::CancelledBeforeCommit))?;
        let snapshot = if let Some((deadline, callback)) = self.original {
            self.journal.read_registry_snapshot_with_original_context(deadline, &callback).map_err(|e| storage_failure(e.control, None))?
        } else { self.journal.read_registry_snapshot(context).map_err(|error| call_failure(&error, None))? };
        let mut originals: Option<original_inputs::OriginalInputsReadback> = None;
        let restored = if let Some(original_v4) = self.original_v4.as_ref() {
            let shape = v4_restore::collect_snapshot_refs(&snapshot, original_v4)
                .map_err(|error| v4_failure(error, None))?;
            if !shape.has_v4() {
                restore_observed_snapshot(
                    &snapshot,
                    self.identity,
                    self.journal.limits(),
                    self.registry_limits,
                    context,
                )?
            } else {
                let references = shape
                    .references()
                    .iter()
                    .map(Vec::as_slice)
                    .collect::<Vec<_>>();
                if !references.is_empty() {
                    let readback = self
                        .original_inputs_snapshot(&references, context)
                        .map_err(original_inputs_failure)?;
                    if readback.identity() != self.identity
                        || readback.generation() != snapshot.journal_generation()
                    {
                        return Err(owner_failure(RegistryError::SnapshotInvalid));
                    }
                    originals = Some(readback);
                }
                let max_restore_bytes = self
                    .journal
                    .limits()
                    .max_total_value_bytes
                    .min(MAX_REGISTRY_RESTORE_BYTES);
                v4_restore::restore_snapshot_v4(
                    &snapshot,
                    originals.as_ref(),
                    original_v4,
                    max_restore_bytes,
                )
                .map_err(|error| v4_failure(error, None))?
            }
        } else {
            restore_observed_snapshot(
                &snapshot,
                self.identity,
                self.journal.limits(),
                self.registry_limits,
                context,
            )?
        };
        let mut original = None;
        if let Some(root) = root {
            for envelope in snapshot.operations() {
                let record = if let Some(original_v4) = self.original_v4.as_ref() {
                    if v4_restore::is_v4_batch(envelope.owner_bytes()) {
                        v4_restore::decode_v4_operation_for_root(
                            envelope,
                            originals.as_ref(),
                            original_v4,
                        )
                        .map_err(|error| v4_failure(error, Some(envelope.operation_id())))?
                    } else {
                        decode_operation(envelope, self.registry_limits)?
                    }
                } else {
                    decode_operation(envelope, self.registry_limits)?
                };
                if record.entry().kind == search_source_registry::error::JournalEntryKind::BatchApply
                    && record.change_count() == 1 && record.deltas().len() == 1
                    && record.deltas().iter().any(|delta| delta.before().is_none()
                        && delta.key() == &RegistryStateKey::Root(root)
                        && matches!(delta.after(), RegistryStateRecord::Root(value)
                            if value.record_revision.get() == 1 && value.registry_revision == record.registry_revision_after()
                            && value.last_receipt == record.entry().receipt))
                    && original.replace(envelope.operation_id().clone()).is_some() {
                    return Err(owner_failure(RegistryError::OperationConflict));
                }
                context.preflight().map_err(|_| owner_failure(RegistryError::CancelledBeforeCommit))?;
            }
        }
        Ok((restored, original))
    }

    /// Explicit authoritative readback of a retained same-process uncertain command.
    /// This issues no replacement mutation and never guesses the original generation.
    pub(crate) fn recover_pending(
        &mut self,
        context: &OperationContext<C>,
    ) -> Result<search_control_redb::CommitRecoveryDecision, RegistryPortError> {
        let request = self
            .pending
            .as_ref()
            .ok_or_else(|| owner_failure(RegistryError::MutationOutcomeUnknown))?;
        let control_identity = control_mutation_identity(request.control_operation_id())
            .map_err(|error| storage_failure(error, Some(request.envelope().operation_id())))?;
        let decision = if let Some((deadline, callback)) = self.original {
            self.journal.recover_registry_batch_with_original_context(request, &control_identity, deadline, &callback)
                .map_err(|e| storage_failure(e.control, Some(request.envelope().operation_id())))?
        } else { self.journal.recover_command(
                &ControlPortCommand::Registry(request.clone()),
                context,
                &control_identity,
            )
            .map_err(|error| call_failure(&error, Some(request.envelope().operation_id())))? };
        if matches!(
            decision,
            search_control_redb::CommitRecoveryDecision::Committed(_)
                | search_control_redb::CommitRecoveryDecision::NotCommittedRetrySameOperation
        ) {
            self.pending = None;
        }
        Ok(decision)
    }
}

fn restore_observed_snapshot<C: CancellationProbe>(
    snapshot: &RegistryReadSnapshot,
    identity: JournalIdentity,
    storage_limits: JournalLimits,
    registry_limits: RegistryLimits,
    context: &OperationContext<C>,
) -> Result<ObservedRegistryRestore, RegistryPortError> {
    context
        .preflight()
        .map_err(|_| owner_failure(RegistryError::CancelledBeforeCommit))?;
    if snapshot.identity() != identity {
        return Err(owner_failure(RegistryError::SnapshotInvalid));
    }
    let max_restore_bytes = storage_limits
        .max_total_value_bytes
        .min(MAX_REGISTRY_RESTORE_BYTES);
    search_source_registry::recovery::preflight_registry_restore_inputs(
        registry_limits,
        snapshot.states().len(),
        snapshot.operations().len(),
        0,
        max_restore_bytes,
    )
    .map_err(owner_failure)?;
    let encoded_bytes = snapshot
        .states()
        .iter()
        .flat_map(|state| {
            [
                state.key().canonical_bytes().len(),
                state.canonical_bytes().len(),
            ]
        })
        .chain(snapshot.operations().iter().flat_map(|operation| {
            [
                operation.operation_id().as_str().len(),
                operation.owner_bytes().len(),
            ]
        }))
        .try_fold(0_usize, |total, bytes| {
            total
                .checked_add(bytes)
                .ok_or_else(|| owner_failure(RegistryError::CapacityExceeded))
        })?;
    search_source_registry::recovery::preflight_registry_restore_inputs(
        registry_limits,
        snapshot.states().len(),
        snapshot.operations().len(),
        encoded_bytes,
        max_restore_bytes,
    )
    .map_err(owner_failure)?;
    for stored in snapshot.states() {
        let key =
            decode_registry_state_key(stored.key().canonical_bytes()).map_err(owner_failure)?;
        if family(key.family()) != stored.key().family() {
            return Err(owner_failure(RegistryError::SnapshotInvalid));
        }
        context
            .preflight()
            .map_err(|_| owner_failure(RegistryError::CancelledBeforeCommit))?;
    }
    for operation in snapshot.operations() {
        decode_operation(operation, registry_limits)?;
        context
            .preflight()
            .map_err(|_| owner_failure(RegistryError::CancelledBeforeCommit))?;
    }
    let states = snapshot
        .states()
        .iter()
        .map(|stored| RegistryRestoreState {
            key: stored.key().canonical_bytes(),
            value: stored.canonical_bytes(),
        })
        .collect::<Vec<_>>();
    let operations = snapshot
        .operations()
        .iter()
        .map(|stored| RegistryRestoreOperation {
            operation_id: stored.operation_id(),
            value: stored.owner_bytes(),
        })
        .collect::<Vec<_>>();
    let restored = restore_registry_state(
        snapshot.registry_revision(),
        &states,
        &operations,
        registry_limits,
        max_restore_bytes,
    )
    .map_err(owner_failure)?;
    context
        .preflight()
        .map_err(|_| owner_failure(RegistryError::CancelledBeforeCommit))?;
    Ok(restored)
}

fn owner_failure(reason: RegistryError) -> RegistryPortError {
    registry_port_error(
        PortErrorKind::InvalidInput,
        PortRetryability::Never,
        reason,
        None,
    )
}

fn storage_failure(error: ControlError, operation: Option<&OpaqueId>) -> RegistryPortError {
    let (kind, retry, reason) = match error {
        ControlError::CommitOutcomeUnknown => (
            PortErrorKind::OutcomeUnknown,
            PortRetryability::AfterReadback,
            RegistryError::MutationOutcomeUnknown,
        ),
        ControlError::BudgetExceeded => (
            PortErrorKind::ResourceExhausted,
            PortRetryability::Never,
            RegistryError::DurabilityExhausted,
        ),
        ControlError::TransactionConflict | ControlError::GenerationMismatch => (
            PortErrorKind::CompareAndSwapRejected,
            PortRetryability::AfterRefresh,
            RegistryError::RegistryRevisionConflict,
        ),
        ControlError::OperationConflict => (
            PortErrorKind::Conflict,
            PortRetryability::Never,
            RegistryError::OperationConflict,
        ),
        ControlError::StoreCorrupt
        | ControlError::StoreQuarantined
        | ControlError::IdentityMismatch
        | ControlError::SchemaUnsupported
        | ControlError::SchemaMismatch => (
            PortErrorKind::Quarantined,
            PortRetryability::AfterReadback,
            RegistryError::SnapshotInvalid,
        ),
        _ => (
            PortErrorKind::DependencyUnavailable,
            PortRetryability::Never,
            RegistryError::DurabilityRejected,
        ),
    };
    registry_port_error(kind, retry, reason, operation.cloned())
}

fn call_failure(error: &ControlPortError, operation: Option<&OpaqueId>) -> RegistryPortError {
    let reason = match error.kind() {
        PortErrorKind::OutcomeUnknown => RegistryError::MutationOutcomeUnknown,
        PortErrorKind::CancelledBeforeSideEffect => RegistryError::CancelledBeforeCommit,
        PortErrorKind::ResourceExhausted => RegistryError::DurabilityExhausted,
        _ => RegistryError::DurabilityRejected,
    };
    registry_port_error(
        error.kind(),
        error.retryability(),
        reason,
        operation.cloned(),
    )
}

fn original_inputs_failure(error: original_inputs::OriginalInputsError) -> RegistryPortError {
    match error {
        original_inputs::OriginalInputsError::Control(error) => storage_failure(error, None),
        original_inputs::OriginalInputsError::OriginalControl(error) => {
            storage_failure(error.control, None)
        }
        original_inputs::OriginalInputsError::Context(_)
        | original_inputs::OriginalInputsError::Owner(_)
        | original_inputs::OriginalInputsError::ContextUnavailable => {
            owner_failure(RegistryError::CancelledBeforeCommit)
        }
        original_inputs::OriginalInputsError::InvalidMetadata => {
            owner_failure(RegistryError::SnapshotInvalid)
        }
    }
}

fn v4_failure(
    error: v4_readback::V4ReadbackError,
    _operation: Option<&OpaqueId>,
) -> RegistryPortError {
    match error {
        v4_readback::V4ReadbackError::Owner(_) => {
            owner_failure(RegistryError::CancelledBeforeCommit)
        }
        v4_readback::V4ReadbackError::Batch(
            search_source_registry::recovery::BatchCodecV4Error::Registry(error),
        ) => owner_failure(error),
        v4_readback::V4ReadbackError::Batch(
            search_source_registry::recovery::BatchCodecV4Error::Admission(error),
        ) => match error {
            search_source_admission::AdmissionError::Cancelled => {
                owner_failure(RegistryError::CancelledBeforeCommit)
            }
            search_source_admission::AdmissionError::BudgetExhausted => {
                owner_failure(RegistryError::DurabilityExhausted)
            }
            _ => owner_failure(RegistryError::AdmissionRestorationUnavailable),
        },
    }
}

const fn family(value: RegistryStateFamily) -> StoredFamily {
    match value {
        RegistryStateFamily::Root => StoredFamily::Root,
        RegistryStateFamily::Source => StoredFamily::Source,
        RegistryStateFamily::Membership => StoredFamily::Membership,
        RegistryStateFamily::Generation => StoredFamily::Generation,
        RegistryStateFamily::Namespace => StoredFamily::Namespace,
    }
}

fn decode_operation(
    envelope: &RegistryOperationEnvelope,
    limits: RegistryLimits,
) -> Result<DecodedBatch, RegistryPortError> {
    let decoded = decode_batch_record(envelope.owner_bytes(), limits).map_err(owner_failure)?;
    let entry = decoded.entry();
    if &entry.operation_id != envelope.operation_id()
        || entry.before_revision != envelope.registry_revision_before()
        || decoded.request_fingerprint() != envelope.command_digest()
    {
        return Err(owner_failure(RegistryError::SnapshotInvalid));
    }
    Ok(decoded)
}

fn state_key(
    key: &RegistryStateKey,
    bytes: &[u8],
    limits: JournalLimits,
) -> Result<StoredKey, RegistryPortError> {
    let decoded = decode_registry_state_key(bytes)
        .or_else(|_| decode_registry_state_key_v4(bytes))
        .map_err(owner_failure)?;
    if decoded != *key {
        return Err(owner_failure(RegistryError::SnapshotInvalid));
    }
    StoredKey::new(family(key.family()), bytes.to_vec(), limits)
        .map_err(|error| storage_failure(error, None))
}

fn prepare(
    batch: &PreparedBatch,
    journal_generation: u64,
    storage_limits: JournalLimits,
    source_limits: RegistryLimits,
) -> Result<RegistryMutationRequest, RegistryPortError> {
    let wrapped_operation_bytes = search_control_redb::registry_operation_value_len(
        &batch.record().entry().operation_id,
        batch.encoded().len(),
    )
    .map_err(|error| storage_failure(error, Some(&batch.record().entry().operation_id)))?;
    if wrapped_operation_bytes > storage_limits.max_value_bytes
        || batch
            .states()
            .len()
            .checked_mul(2)
            .and_then(|n| n.checked_add(4))
            .is_none_or(|n| n > storage_limits.max_mutation_items)
    {
        return Err(storage_failure(
            ControlError::BudgetExceeded,
            Some(&batch.record().entry().operation_id),
        ));
    }
    let decoded = if batch.record().encoding_profile() == "eliot.registry-batch.v4" {
        let encoded = batch.record().encode().map_err(owner_failure)?;
        if encoded != batch.encoded() {
            return Err(owner_failure(RegistryError::SnapshotInvalid));
        }
        batch.record().clone()
    } else {
        decode_batch_record(batch.encoded(), source_limits).map_err(owner_failure)?
    };
    if decoded != *batch.record() || batch.states().len() != decoded.deltas().len() {
        return Err(owner_failure(RegistryError::SnapshotInvalid));
    }
    let mut changes = Vec::with_capacity(batch.states().len());
    for (bytes, delta) in batch.states().iter().zip(decoded.deltas()) {
        let key_matches = bytes.key() == delta.key()
            && if decoded.state_encoding_profile() == "eliot.registry-state.v4" {
                delta.key().encode().map_err(owner_failure)? == bytes.key_bytes()
                    && delta
                        .after()
                        .encode_with_profile(decoded.state_encoding_profile())
                        .map_err(owner_failure)?
                        == bytes.after_bytes()
            } else {
                decode_registry_state_entry(bytes.key_bytes(), bytes.after_bytes())
                    .map_err(owner_failure)?
                    == *delta.after()
            };
        if !key_matches {
            return Err(owner_failure(RegistryError::SnapshotInvalid));
        }
        match (bytes.before_bytes(), delta.before()) {
            (None, None) => {}
            (Some(encoded), Some(expected))
                if decoded.state_encoding_profile() == "eliot.registry-state.v4"
                    && v4_restore::is_v4_state(encoded)
                    && expected
                        .encode_with_profile("eliot.registry-state.v4")
                        .map_err(owner_failure)?
                        == encoded => {}
            (Some(encoded), Some(expected))
                if decode_registry_state_entry(bytes.key_bytes(), encoded)
                    .map_err(owner_failure)?
                    == *expected => {}
            _ => return Err(owner_failure(RegistryError::SnapshotInvalid)),
        }
        changes.push(
            RegistryStateChange::new(
                state_key(bytes.key(), bytes.key_bytes(), storage_limits)?,
                bytes.before_bytes().map(<[u8]>::to_vec),
                bytes.after_bytes().to_vec(),
                storage_limits,
            )
            .map_err(|error| storage_failure(error, Some(&decoded.entry().operation_id)))?,
        );
    }
    RegistryMutationRequest::new(
        decoded.entry().operation_id.clone(),
        journal_generation,
        decoded.expected_registry_revision(),
        decoded.request_fingerprint(),
        batch.encoded().to_vec(),
        changes,
        storage_limits,
    )
    .map_err(|error| storage_failure(error, Some(&decoded.entry().operation_id)))
}

fn decode_states(
    states: &[RegistryStoredState],
) -> Result<Vec<RegistryStateRecord>, RegistryPortError> {
    states
        .iter()
        .map(|stored| {
            let key =
                decode_registry_state_key(stored.key().canonical_bytes()).map_err(owner_failure)?;
            if family(key.family()) != stored.key().family() {
                return Err(owner_failure(RegistryError::SnapshotInvalid));
            }
            decode_registry_state_entry(stored.key().canonical_bytes(), stored.canonical_bytes())
                .map_err(owner_failure)
        })
        .collect()
}

fn readback(
    observed: &RegistryTransactionReadback,
    expected: JournalIdentity,
    limits: RegistryLimits,
) -> Result<RegistryBatchReadback, RegistryPortError> {
    if observed.identity() != expected {
        return Err(owner_failure(RegistryError::SnapshotInvalid));
    }
    let decoded = decode_operation(observed.operation(), limits)?;
    let expected_keys = decoded
        .deltas()
        .iter()
        .map(search_source_registry::recovery::RegistryStateDelta::key)
        .collect::<std::collections::BTreeSet<_>>();
    let states = decode_states(observed.states())?;
    let actual_keys = states
        .iter()
        .map(RegistryStateRecord::key)
        .collect::<std::collections::BTreeSet<_>>();
    if states.len() != decoded.deltas().len()
        || actual_keys.len() != states.len()
        || !actual_keys.iter().all(|key| expected_keys.contains(key))
    {
        return Err(owner_failure(RegistryError::SnapshotInvalid));
    }
    RegistryBatchReadback::new(decoded, observed.registry_revision(), states, limits)
        .map_err(owner_failure)
}

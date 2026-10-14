use super::super::operation::{Budget, Check, Point};
use super::super::{
    Boundary, ControlCallError, PersistentControlJournal, ReadTransaction, operation_from,
    transaction,
};
use super::{
    ControlError, ControlKey, JournalLimits, REVISION, RegistryCommitReadback,
    RegistryMutationRequest, RegistryReadSnapshot, RegistryTransactionReadback, codec, reserved,
};
use crate::{
    CommitRecoveryDecision, ConditionalControlMutation, ControlCommitReceipt, ControlMutation,
    ControlRecordCondition, ControlWrite, JournalReadSnapshot, REGISTRY_JOURNAL_SCHEMA_VERSION,
};
use search_ports::{CancellationProbe, OperationContext};
use std::collections::BTreeSet;

impl RegistryMutationRequest {
    pub(in crate::persistent) fn command(
        &self,
        limits: JournalLimits,
    ) -> Result<ConditionalControlMutation, ControlError> {
        if self
            .changes
            .len()
            .checked_mul(2)
            .and_then(|n| n.checked_add(4))
            .is_none_or(|n| n > limits.max_mutation_items)
        {
            return Err(ControlError::BudgetExceeded);
        }
        let revision_key = ControlKey::new(REVISION.to_vec(), limits)?;
        let operation_key = codec::operation_key(&self.envelope.operation_id, limits)?;
        let mut conditions = vec![ControlRecordCondition::absent(operation_key.clone())];
        let mut writes = vec![ControlWrite {
            key: operation_key,
            value: codec::operation_value(&self.envelope, limits)?,
        }];
        conditions.push(if self.envelope.registry_revision == 0 {
            ControlRecordCondition::absent(revision_key.clone())
        } else {
            ControlRecordCondition::exact(
                revision_key.clone(),
                codec::revision_value(self.envelope.registry_revision, limits)?,
            )
        });
        writes.push(ControlWrite {
            key: revision_key,
            value: codec::revision_value(
                self.envelope
                    .registry_revision
                    .checked_add(1)
                    .ok_or(ControlError::GenerationExhausted)?,
                limits,
            )?,
        });
        for change in &self.changes {
            let key = codec::state_key(&change.key, limits)?;
            conditions.push(match &change.before {
                None => ControlRecordCondition::absent(key.clone()),
                Some(value) => ControlRecordCondition::exact(
                    key.clone(),
                    codec::state_value(change.key.family, value, limits)?,
                ),
            });
            writes.push(ControlWrite {
                key,
                value: codec::state_value(change.key.family, &change.after, limits)?,
            });
        }
        let command = ConditionalControlMutation::new(
            ControlMutation::new(
                codec::mutation_id(&self.envelope.operation_id),
                self.envelope.command_digest,
                self.envelope.journal_generation,
                writes,
                vec![],
            ),
            conditions,
        );
        super::super::validate_mutation(command.mutation(), limits)?;
        crate::conditions::validate_conditions(
            command.mutation(),
            command.conditions(),
            limits,
            || Ok(()),
        )?;
        Ok(command)
    }
}

impl PersistentControlJournal {
    /// Commit complete registry metadata, its immutable operation and revision fence atomically.
    ///
    /// Owner payloads must already be semantically validated. This method proves
    /// storage atomicity only and retains the original journal generation on replay.
    ///
    /// # Errors
    /// Exact before-value/revision conflicts and active bounds refuse before commit.
    /// Possible-write failures remain unknown until exact same-command recovery.
    pub fn commit_registry_batch<C: CancellationProbe>(
        &mut self,
        request: &RegistryMutationRequest,
        context: &OperationContext<C>,
    ) -> Result<RegistryCommitReadback, ControlCallError> {
        let budget = Budget::new(context);
        self.commit_registry_checked(request, Boundary::Normal, &budget)
            .map_err(|error| {
                budget.failure(
                    error,
                    Some(codec::mutation_id(&request.envelope.operation_id)),
                )
            })
    }

    /// Recover the original registry command without issuing another mutation.
    ///
    /// # Errors
    /// Changed input/generation conflicts; interruption preserves the pending fence.
    pub fn recover_registry_batch<C: CancellationProbe>(
        &mut self,
        request: &RegistryMutationRequest,
        context: &OperationContext<C>,
    ) -> Result<CommitRecoveryDecision, ControlCallError> {
        let budget = Budget::new(context);
        let result = (|| {
            self.require_registry_schema()?;
            budget.check(Point::Start)?;
            let command = request.command(self.limits)?;
            self.recover_transaction_with_conditions_checked(
                command.mutation(),
                command.conditions(),
                &budget,
            )
        })();
        result.map_err(|error| {
            budget
                .failure(
                    if budget.interrupted() {
                        ControlError::CommitOutcomeUnknown
                    } else {
                        error
                    },
                    Some(codec::mutation_id(&request.envelope.operation_id)),
                )
                .for_recovery()
        })
    }

    /// Read complete coherent registry metadata without writes or admission publication.
    ///
    /// Source-owner decoding and a real migration/cutover decision remain required.
    ///
    /// # Errors
    /// Old schemas, unavailable/pending/corrupt state and exhausted budgets refuse.
    pub fn read_registry_snapshot<C: CancellationProbe>(
        &self,
        context: &OperationContext<C>,
    ) -> Result<RegistryReadSnapshot, ControlCallError> {
        let budget = Budget::new(context);
        self.registry_snapshot_checked(&budget)
            .map_err(|error| budget.failure(error, None))
    }

    pub(in crate::persistent) fn commit_registry_checked(
        &mut self,
        request: &RegistryMutationRequest,
        boundary: Boundary,
        check: &dyn Check,
    ) -> Result<RegistryCommitReadback, ControlError> {
        self.require_registry_schema()?;
        check.check(Point::Start)?;
        self.ensure_available()?;
        let read = self
            .database
            .begin_read()
            .map_err(|_| ControlError::StoreUnavailable)?;
        self.header_from(&read)?;
        check.check(Point::ReadRecord)?;
        super::super::publication::require_registry_base(&read, self.limits)?;
        drop(read);
        let command = request.command(self.limits)?;
        let result = transaction::execute_registry(
            self,
            command.mutation(),
            command.conditions(),
            boundary,
            check,
        );
        if result
            .as_ref()
            .err()
            .is_some_and(|error| super::super::is_corruption(*error))
        {
            self.quarantined = true;
        }
        let (receipt, fingerprint) = result?;
        // Any failure after dispatch needs recovery, including this final readback.
        let readback = self
            .registry_readback_checked(request, &receipt, check)
            .map_err(|_| {
                self.pending = Some((command.mutation().id(), fingerprint));
                ControlError::CommitOutcomeUnknown
            })?;
        Ok(RegistryCommitReadback { receipt, readback })
    }

    fn registry_readback_checked(
        &self,
        request: &RegistryMutationRequest,
        receipt: &ControlCommitReceipt,
        check: &dyn Check,
    ) -> Result<RegistryTransactionReadback, ControlError> {
        self.ensure_available()?;
        check.check(Point::Start)?;
        let read = self
            .database
            .begin_read()
            .map_err(|_| ControlError::StoreUnavailable)?;
        let header = self.header_from(&read)?;
        let table = read
            .open_table(super::super::RECORDS)
            .map_err(super::super::map_table_error)?;
        let revision_key = ControlKey::new(REVISION.to_vec(), self.limits)?;
        let operation_key = codec::operation_key(&request.envelope.operation_id, self.limits)?;
        let get = |key: &ControlKey| {
            check.check(Point::Readback)?;
            let raw = table
                .get(key.as_bytes())
                .map_err(|e| super::super::map_storage_error(&e))?
                .ok_or(ControlError::StoreCorrupt)?;
            let value = super::super::decode_value(raw.value(), self.limits)?;
            codec::decode_record(key, &value, self.limits)
        };
        let codec::Record::Revision(registry_revision) = get(&revision_key)? else {
            return Err(ControlError::StoreCorrupt);
        };
        let codec::Record::Operation(operation) = get(&operation_key)? else {
            return Err(ControlError::StoreCorrupt);
        };
        check.check(Point::ReadOperation)?;
        let stored = operation_from(&read, receipt.operation_id, &header, self.limits)?
            .ok_or(ControlError::StoreCorrupt)?;
        let mut original_receipt = stored.receipt;
        original_receipt.replayed = receipt.replayed;
        if operation != request.envelope
            || original_receipt != *receipt
            || header.generation < receipt.after_generation
            || registry_revision <= operation.registry_revision
        {
            return Err(ControlError::StoreCorrupt);
        }
        let mut states = Vec::with_capacity(request.changes.len());
        for change in &request.changes {
            let key = codec::state_key(&change.key, self.limits)?;
            let codec::Record::State(state) = get(&key)? else {
                return Err(ControlError::StoreCorrupt);
            };
            if header.generation == receipt.after_generation && state.bytes != change.after {
                return Err(ControlError::StoreCorrupt);
            }
            states.push(state);
        }
        check.check(Point::ReadComplete)?;
        Ok(RegistryTransactionReadback {
            identity: self.identity,
            journal_generation: header.generation,
            registry_revision,
            operation,
            states,
        })
    }

    fn registry_snapshot_checked(
        &self,
        check: &dyn Check,
    ) -> Result<RegistryReadSnapshot, ControlError> {
        self.require_registry_schema()?;
        self.ensure_available()?;
        check.check(Point::Start)?;
        let read = self
            .database
            .begin_read()
            .map_err(|_| ControlError::StoreUnavailable)?;
        let snapshot = self.verify_from_checked(&read, check)?;
        extract(&snapshot, self.limits, check)
    }

    const fn require_registry_schema(&self) -> Result<(), ControlError> {
        if self.identity.schema_version == REGISTRY_JOURNAL_SCHEMA_VERSION {
            Ok(())
        } else {
            Err(ControlError::SchemaUnsupported)
        }
    }
}

fn extract(
    snapshot: &JournalReadSnapshot,
    limits: JournalLimits,
    check: &dyn Check,
) -> Result<RegistryReadSnapshot, ControlError> {
    let mut value = RegistryReadSnapshot {
        identity: snapshot.identity,
        journal_generation: snapshot.generation,
        registry_revision: 0,
        states: vec![],
        operations: vec![],
    };
    let mut revision = None;
    for (key, record) in &snapshot.records {
        if !reserved(key.as_bytes()) {
            continue;
        }
        check.check(Point::ReadRecord)?;
        match codec::decode_record(key, record, limits)? {
            codec::Record::Revision(n) => revision = Some(n),
            codec::Record::State(state) => value.states.push(state),
            codec::Record::Operation(operation) => value.operations.push(operation),
        }
    }
    if revision.is_none() && (!value.states.is_empty() || !value.operations.is_empty()) {
        return Err(ControlError::StoreCorrupt);
    }
    value.registry_revision = revision.unwrap_or(0);
    if value.registry_revision
        != u64::try_from(value.operations.len()).map_err(|_| ControlError::StoreCorrupt)?
    {
        return Err(ControlError::StoreCorrupt);
    }
    let mut revisions = BTreeSet::new();
    for operation in &value.operations {
        if operation.registry_revision >= value.registry_revision
            || !revisions.insert(operation.registry_revision)
        {
            return Err(ControlError::StoreCorrupt);
        }
    }
    Ok(value)
}

pub(in crate::persistent) fn validate_snapshot(
    read: &ReadTransaction,
    snapshot: &JournalReadSnapshot,
    limits: JournalLimits,
    check: &dyn Check,
) -> Result<(), ControlError> {
    if snapshot.identity.schema_version != REGISTRY_JOURNAL_SCHEMA_VERSION {
        return Ok(());
    }
    let registry = extract(snapshot, limits, check)?;
    // Join each immutable raw-ID envelope to the existing backend receipt ledger.
    // Neither a class tag nor an independently stored receipt proves this join.
    let table = read
        .open_table(super::super::META)
        .map_err(super::super::map_table_error)?;
    let header = table
        .get("header")
        .map_err(|e| super::super::map_storage_error(&e))?
        .ok_or(ControlError::StoreCorrupt)?;
    let header = super::super::Header::decode(header.value(), snapshot.identity, limits)?;
    for envelope in &registry.operations {
        check.check(Point::ReadOperation)?;
        let operation = operation_from(
            read,
            codec::mutation_id(&envelope.operation_id),
            &header,
            limits,
        )?
        .ok_or(ControlError::StoreCorrupt)?;
        if operation.receipt.before_generation != envelope.journal_generation
            || operation.receipt.command_digest != envelope.command_digest
            || !operation
                .receipt
                .changed_keys
                .contains(&codec::operation_key(&envelope.operation_id, limits)?)
            || !operation
                .receipt
                .changed_keys
                .contains(&ControlKey::new(REVISION.to_vec(), limits)?)
            || operation
                .receipt
                .changed_keys
                .iter()
                .any(|k| !reserved(k.as_bytes()))
        {
            return Err(ControlError::StoreCorrupt);
        }
    }
    Ok(())
}

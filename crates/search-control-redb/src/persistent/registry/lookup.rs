//! Targeted normal-path reads. No catalog/history replay or semantic callbacks.
use super::super::operation::{Budget, Check, Point};
use super::super::{
    ControlCallError, Header, PersistentControlJournal, RECORDS, ReadTransaction,
    map_storage_error, map_table_error, operation_from_checked, decode_value_checked,
};
use super::{
    PREFIX, REVISION, RegistryOperationEnvelope, RegistryOperationLookup,
    RegistryOperationObservation, RegistryStateKey, RegistryTransactionReadback, codec,
};
use crate::{ControlError, ControlKey, JournalLimits, REGISTRY_JOURNAL_SCHEMA_VERSION};
use search_contracts::OpaqueId;
use search_ports::{CancellationProbe, OperationContext};
use std::collections::BTreeSet;

impl PersistentControlJournal {
    /// Look up one immutable full-ID operation and current observation coordinates.
    ///
    /// Proven absence checks both the operation record and existing receipt ledger.
    /// A missing revision fence is zero only after an empty-prefix range proof.
    ///
    /// # Errors
    /// Pending/unknown, partial, corrupt, foreign, unavailable or interrupted reads
    /// return errors, never absence. This grants no source/restore authority.
    pub fn lookup_registry_operation<C: CancellationProbe>(
        &self,
        id: &OpaqueId,
        context: &OperationContext<C>,
    ) -> Result<RegistryOperationLookup, ControlCallError> {
        let budget = Budget::new(context);
        self.lookup_registry_checked(id, &budget)
            .map_err(|error| budget.failure(error, None).for_recovery())
    }

    /// Recheck the exact immutable envelope and requested targets in one read.
    ///
    /// Root decodes owner bytes outside this call to derive canonical target keys.
    /// Returned state is current coherent metadata, including on historical replay.
    ///
    /// # Errors
    /// Changed envelopes, duplicate/oversized keys, missing targets, unknown/pending
    /// state and interrupted reads refuse the whole result without partial output.
    pub fn read_registry_operation_targets<C: CancellationProbe>(
        &self,
        expected: &RegistryOperationEnvelope,
        keys: &[RegistryStateKey],
        context: &OperationContext<C>,
    ) -> Result<RegistryTransactionReadback, ControlCallError> {
        let budget = Budget::new(context);
        self.read_registry_targets_checked(expected, keys, &budget)
            .map_err(|error| budget.failure(error, None).for_recovery())
    }

    pub(super) fn lookup_registry_checked(
        &self,
        id: &OpaqueId,
        check: &dyn Check,
    ) -> Result<RegistryOperationLookup, ControlError> {
        let (read, header) = self.registry_read_head(check)?;
        let registry_revision = current_revision(&read, self.limits, check)?;
        let operation = read_operation(&read, &header, id, registry_revision, self.limits, check)?;
        check.check(Point::ReadComplete)?;
        Ok(operation.map_or(
            RegistryOperationLookup::Absent {
                identity: self.identity,
                journal_generation: header.generation,
                registry_revision,
            },
            |envelope| {
                RegistryOperationLookup::Committed(RegistryOperationObservation {
                    identity: self.identity,
                    journal_generation: header.generation,
                    registry_revision,
                    envelope,
                })
            },
        ))
    }

    pub(super) fn read_registry_targets_checked(
        &self,
        expected: &RegistryOperationEnvelope,
        keys: &[RegistryStateKey],
        check: &dyn Check,
    ) -> Result<RegistryTransactionReadback, ControlError> {
        check.check(Point::Start)?;
        if keys.is_empty() || keys.len() > self.limits.max_mutation_items {
            return Err(ControlError::BudgetExceeded);
        }
        let mut seen = BTreeSet::new();
        check.reserve(super::super::MigrationBootstrapWork::Allocate(keys.len()
            .checked_mul(std::mem::size_of::<ControlKey>() + 64).ok_or(ControlError::BudgetExceeded)?))?;
        let mut physical = Vec::with_capacity(keys.len());
        for key in keys {
            check.check(Point::PlanRecord)?;
            check.reserve(super::super::MigrationBootstrapWork::Allocate(codec::key_size(key)?.checked_mul(2).ok_or(ControlError::BudgetExceeded)?))?;
            if !seen.insert(key) {
                return Err(ControlError::DuplicateMutationKey);
            }
            physical.push(codec::state_key(key, self.limits)?);
        }
        let (read, header) = self.registry_read_head(check)?;
        let registry_revision = current_revision(&read, self.limits, check)?;
        let operation = read_operation(
            &read,
            &header,
            expected.operation_id(),
            registry_revision,
            self.limits,
            check,
        )?
        .ok_or(ControlError::OperationConflict)?;
        if operation != *expected {
            return Err(ControlError::OperationConflict);
        }
        let table = read.open_table(RECORDS).map_err(map_table_error)?;
        let mut total = codec::operation_size(&operation)?;
        check.reserve(super::super::MigrationBootstrapWork::Allocate(keys.len()
            .checked_mul(std::mem::size_of::<super::RegistryStoredState>()).ok_or(ControlError::BudgetExceeded)?))?;
        let mut states = Vec::with_capacity(keys.len());
        for key in &physical {
            check.check(Point::ReadRecord)?;
            let raw = table
                .get(key.as_bytes())
                .map_err(|e| map_storage_error(&e))?
                .ok_or(ControlError::StoreCorrupt)?;
            let value = decode_value_checked(raw.value(), self.limits, check)?;
            total = total
                .checked_add(value.len())
                .ok_or(ControlError::BudgetExceeded)?;
            if total > self.limits.max_total_value_bytes {
                return Err(ControlError::BudgetExceeded);
            }
            let codec::Record::State(state) = codec::decode_record_checked(key, &value, self.limits, check)?
            else {
                return Err(ControlError::StoreCorrupt);
            };
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

    fn registry_read_head(
        &self,
        check: &dyn Check,
    ) -> Result<(ReadTransaction, Header), ControlError> {
        if self.requires_recovery() && !self.quarantined {
            return Err(ControlError::CommitOutcomeUnknown);
        }
        self.ensure_available()?;
        check.check(Point::Start)?;
        if self.identity.schema_version != REGISTRY_JOURNAL_SCHEMA_VERSION {
            return Err(ControlError::SchemaUnsupported);
        }
        let read = self
            .database
            .begin_read()
            .map_err(|_| ControlError::StoreUnavailable)?;
        let header = self.header_from_checked(&read, check)?;
        super::super::operating_profile::normal(super::super::registry_mode_checked(
            &read, self.identity, self.limits, check,
        )?)?;
        check.check(Point::ReadHeader)?;
        Ok((read, header))
    }
}

fn current_revision(
    read: &ReadTransaction,
    limits: JournalLimits,
    check: &dyn Check,
) -> Result<u64, ControlError> {
    check.check(Point::ReadRecord)?;
    let table = read.open_table(RECORDS).map_err(map_table_error)?;
    check.reserve(super::super::MigrationBootstrapWork::Allocate(REVISION.len()))?;
    let key = ControlKey::new(REVISION.to_vec(), limits)?;
    if let Some(raw) = table.get(REVISION).map_err(|e| map_storage_error(&e))? {
        let codec::Record::Revision(revision) =
            codec::decode_record_checked(&key, &decode_value_checked(raw.value(), limits, check)?, limits, check)?
        else {
            return Err(ControlError::StoreCorrupt);
        };
        if revision == 0 {
            return Err(ControlError::StoreCorrupt);
        }
        return Ok(revision);
    }
    // One bounded range observation, not a scan of the catalog. Missing fence
    // plus any reserved row is partial/corrupt state, never an empty registry.
    let first = table
        .range::<&[u8]>(PREFIX..)
        .map_err(|e| map_storage_error(&e))?
        .next();
    if let Some(row) = first {
        let (key, _) = row.map_err(|e| map_storage_error(&e))?;
        if key.value().starts_with(PREFIX) {
            return Err(ControlError::StoreCorrupt);
        }
    }
    Ok(0)
}

fn read_operation(
    read: &ReadTransaction,
    header: &Header,
    id: &OpaqueId,
    registry_revision: u64,
    limits: JournalLimits,
    check: &dyn Check,
) -> Result<Option<RegistryOperationEnvelope>, ControlError> {
    check.reserve(super::super::MigrationBootstrapWork::Allocate(limits.max_key_bytes))?;
    check.reserve(super::super::MigrationBootstrapWork::Process(id.as_str().len()))?;
    let key = codec::operation_key(id, limits)?;
    check.check(Point::ReadOperation)?;
    let table = read.open_table(RECORDS).map_err(map_table_error)?;
    let raw = table
        .get(key.as_bytes())
        .map_err(|e| map_storage_error(&e))?;
    let receipt = operation_from_checked(read, codec::mutation_id(id), header, limits, check)?;
    let (raw, receipt) = match (raw, receipt) {
        (None, None) => return Ok(None),
        (Some(raw), Some(receipt)) => (raw, receipt),
        _ => return Err(ControlError::StoreCorrupt),
    };
    let codec::Record::Operation(envelope) =
        codec::decode_record_checked(&key, &decode_value_checked(raw.value(), limits, check)?, limits, check)?
    else {
        return Err(ControlError::StoreCorrupt);
    };
    if envelope.operation_id() != id {
        return Err(ControlError::OperationConflict);
    }
    if receipt.receipt.before_generation != envelope.journal_generation_before()
        || receipt.receipt.command_digest != envelope.command_digest()
        || registry_revision <= envelope.registry_revision_before()
        || !receipt.receipt.changed_keys.contains(&key)
        || !receipt
            .receipt
            .changed_keys
            .contains(&ControlKey::new(REVISION.to_vec(), limits)?)
        || receipt
            .receipt
            .changed_keys
            .iter()
            .any(|key| !super::reserved(key.as_bytes()))
    {
        return Err(ControlError::StoreCorrupt);
    }
    Ok(Some(envelope))
}

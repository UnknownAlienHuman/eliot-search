//! Touched-key transaction planning and exact post-commit readback.
//!
//! The enclosing journal is the sole owner of its private redb handle. Open and
//! explicit verification check the whole store. Normal mutations use those
//! checked counters plus exact old values of only the distinct touched keys;
//! they do not build a second in-memory catalog or scan unrelated records.

use crate::conditions::{bind_conditions, validate_conditions};
use crate::ControlRecordCondition;
#[cfg(test)]
use super::operation_from;

use super::{
    Boundary, ControlCommitReceipt, ControlError, ControlKey, ControlMutation, Durability,
    Header, OPERATIONS, META, PersistentControlJournal, RECORDS, ReadTransaction,
    ReadableTable, StoredOperation, as_u64, decode_value_checked, encode_value, operation_from_checked,
    request_fingerprint, validate_mutation, map_storage_error, map_table_error,
    Check, Point,
};

pub(super) fn execute(
    journal: &mut PersistentControlJournal,
    mutation: &ControlMutation,
    boundary: Boundary,
    check: &dyn Check,
) -> Result<ControlCommitReceipt, ControlError> {
    execute_with_conditions(journal, mutation, &[], boundary, check)
}

// Pure planning outcome: either an idempotent replay receipt or a fully
// validated staging plan. Staging, commit and readback stay in the entry
// below so the single atomic fence keeps one reviewable commit point.
enum PlanOutcome {
    Replay(ControlCommitReceipt),
    Stage(Box<StagePlan>),
}

struct StagePlan {
    before: Header,
    after: Header,
    receipt: ControlCommitReceipt,
    operation: StoredOperation,
    operation_bytes: Vec<u8>,
    fingerprint: [u8; 32],
}

pub(super) fn execute_with_conditions(
    journal: &mut PersistentControlJournal,
    mutation: &ControlMutation,
    conditions: &[ControlRecordCondition],
    boundary: Boundary,
    check: &dyn Check,
) -> Result<ControlCommitReceipt, ControlError> {
    execute_scoped(journal, mutation, conditions, boundary, check, Scope::General).map(|(receipt, _)| receipt)
}

pub(super) fn execute_registry(
    journal: &mut PersistentControlJournal,
    mutation: &ControlMutation,
    conditions: &[ControlRecordCondition],
    boundary: Boundary,
    check: &dyn Check,
) -> Result<(ControlCommitReceipt, [u8; 32]), ControlError> {
    execute_scoped(journal, mutation, conditions, boundary, check, Scope::Registry)
}

pub(super) fn execute_profile(journal: &mut PersistentControlJournal, mutation: &ControlMutation,
    conditions: &[ControlRecordCondition], boundary: Boundary, check: &dyn Check,
) -> Result<ControlCommitReceipt, ControlError> {
    execute_scoped(journal, mutation, conditions, boundary, check, Scope::Profile).map(|(receipt, _)| receipt)
}

pub(super) fn execute_revision_dek(journal: &mut PersistentControlJournal,
    command: &crate::ConditionalControlMutation, boundary: Boundary, check: &dyn Check,
) -> Result<ControlCommitReceipt, ControlError> {
    let result = execute_scoped(journal, command.mutation(), command.conditions(), boundary, check, Scope::RevisionDek)
        .map(|(receipt, _)| receipt);
    if result.as_ref().err().is_some_and(|error| super::is_corruption(*error)) { journal.quarantined = true; }
    result
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Scope { General, Registry, Profile, RevisionDek }

fn execute_scoped(
    journal: &mut PersistentControlJournal,
    mutation: &ControlMutation,
    conditions: &[ControlRecordCondition],
    boundary: Boundary,
    check: &dyn Check,
    scope: Scope,
) -> Result<(ControlCommitReceipt, [u8; 32]), ControlError> {
    // Production fences are inert by construction; the test helper below
    // consumes the boundary to exercise every fault-injection arm.
    #[cfg(not(test))]
    let _ = boundary;
    journal.ensure_available()?;
    check.check(Point::Start)?;
    reserve_mutation_work(mutation, conditions, journal.limits, check)?;
    let changed_keys = validate_mutation(mutation, journal.limits)?;
    validate_conditions(mutation, conditions, journal.limits, || check.check(Point::PlanRecord))?;
    let fingerprint = bind_conditions(request_fingerprint(journal.identity, mutation)?, conditions,
        || check.check(Point::PlanRecord))?;
    check.check(Point::Validated)?;
    let read = journal.database.begin_read().map_err(|_| ControlError::StoreUnavailable)?;
    validate_scope(journal, &read, mutation, &changed_keys, scope, check)?;
    let plan = match plan_commit(journal, &read, mutation, conditions, changed_keys, fingerprint, check)? {
        PlanOutcome::Replay(receipt) => return Ok((receipt, fingerprint)),
        PlanOutcome::Stage(plan) => plan,
    };
    let StagePlan { before, after, receipt, operation, operation_bytes, fingerprint } = *plan;
    drop(read);

    let write = begin_write(journal, check)?;
    let staging = (|| -> Result<(), ControlError> {
        check.check(Point::BeforeWrite)?;
        let mut meta = write.open_table(META).map_err(map_table_error)?;
        {
            let stored = meta.get("header").map_err(|error| map_storage_error(&error))?
                .ok_or(ControlError::StoreCorrupt)?;
            check.reserve(super::MigrationBootstrapWork::Read(stored.value().len()))?;
            if Header::decode(stored.value(), journal.identity, journal.limits)? != before {
                return Err(ControlError::TransactionConflict);
            }
        }
        let mut records = write.open_table(RECORDS).map_err(map_table_error)?;
        let mut operations = write.open_table(OPERATIONS).map_err(map_table_error)?;
        if operations.get(mutation.id().0.as_slice())
            .map_err(|error| map_storage_error(&error))?.is_some()
        {
            return Err(ControlError::OperationConflict);
        }
        // Validate pre-state again under the actual write transaction, before
        // staging any change. These checks and the header comparison cannot
        // interleave with another conforming writer. A condition may concern a
        // key that this same command intentionally changes afterward.
        verify_conditions(journal, &records, conditions, check, Point::StageRecord)?;
        for key in mutation.deletes() {
            check.check(Point::StageRecord)?;
            records.remove(key.as_bytes()).map_err(|error| map_storage_error(&error))?;
        }
        for change in mutation.writes() {
            check.check(Point::StageRecord)?;
            reserve_encoded_value(&change.value, check)?;
            check.reserve(super::MigrationBootstrapWork::Write(change.key.as_bytes().len() + change.value.len() + 1))?;
            let value = encode_value(&change.value);
            records.insert(change.key.as_bytes(), value.as_slice())
                .map_err(|error| map_storage_error(&error))?;
        }
        check.reserve(super::MigrationBootstrapWork::Write(operation_bytes.len() + 32))?;
        operations.insert(mutation.id().0.as_slice(), operation_bytes.as_slice())
            .map_err(|error| map_storage_error(&error))?;
        check.reserve(super::MigrationBootstrapWork::Allocate(256))?;
        let header = after.encode();
        check.reserve(super::MigrationBootstrapWork::Write(header.len()))?;
        meta.insert("header", header.as_slice()).map_err(|error| map_storage_error(&error))?;
        #[cfg(not(test))]
        Boundary::before_commit();
        #[cfg(test)]
        boundary.before_commit()?;
        check.check(Point::BeforeCommit)?;
        check.reserve(super::MigrationBootstrapWork::Write(0))?;
        Ok(())
    })();
    if let Err(error) = staging {
        // Explicit abort observes errors hidden by Drop. The function contract
        // still requires authoritative recovery for interruption after dispatch,
        // even when local rollback succeeds. Do not clear this fence here.
        let abort_failed = write.abort().is_err();
        if abort_failed || matches!(error, ControlError::ReadCancelled | ControlError::BudgetExceeded) {
            journal.pending = Some((mutation.id(), fingerprint));
            return Err(ControlError::CommitOutcomeUnknown);
        }
        return Err(error);
    }
    // A possible commit cannot be described as rollback. Only authoritative
    // recovery of this exact request may clear a pending fence.
    journal.pending = Some((mutation.id(), fingerprint));
    write.commit().map_err(|_| ControlError::CommitOutcomeUnknown)?;
    #[cfg(not(test))]
    Boundary::after_commit();
    #[cfg(test)]
    boundary.after_commit()?;
    check.check(Point::AfterCommit).map_err(|_| ControlError::CommitOutcomeUnknown)?;

    let observed = journal.database.begin_read().map_err(|_| ControlError::CommitOutcomeUnknown)?;
    let observed_header = journal.header_from_checked(&observed, check)
        .map_err(|_| ControlError::CommitOutcomeUnknown)?;
    let observed_operation = operation_from_checked(&observed, mutation.id(), &observed_header, journal.limits, check)
        .map_err(|_| ControlError::CommitOutcomeUnknown)?;
    if observed_header != after || observed_operation.as_ref() != Some(&operation) {
        return Err(ControlError::CommitOutcomeUnknown);
    }
    #[cfg(not(test))]
    verify_touched(&observed, mutation, check)
        .map_err(|_| ControlError::CommitOutcomeUnknown)?;
    #[cfg(test)]
    verify_touched(journal, &observed, mutation, check)
        .map_err(|_| ControlError::CommitOutcomeUnknown)?;
    verify_conditional_poststate(journal, &observed, conditions, &receipt.changed_keys, check)
        .map_err(|_| ControlError::CommitOutcomeUnknown)?;
    check.check(Point::MutationComplete).map_err(|_| ControlError::CommitOutcomeUnknown)?;
    journal.pending = None;
    journal.committed_writes = journal.committed_writes.saturating_add(1);
    Ok((receipt, fingerprint))
}

fn begin_write(journal: &PersistentControlJournal, check: &dyn Check)
    -> Result<redb::WriteTransaction, ControlError> {
    check.check(Point::BeforeWrite)?;
    check.reserve(super::MigrationBootstrapWork::Write(0))?;
    let mut write = journal.database.begin_write().map_err(|_| ControlError::StoreUnavailable)?;
    write.set_durability(Durability::Immediate);
    Ok(write)
}

fn validate_scope(journal: &PersistentControlJournal, read: &ReadTransaction, mutation: &ControlMutation,
    changed_keys: &[ControlKey], scope: Scope, check: &dyn Check,
) -> Result<(), ControlError> {
    if scope == Scope::RevisionDek && journal.identity.schema_version != crate::REGISTRY_JOURNAL_SCHEMA_VERSION {
        return Err(ControlError::SchemaUnsupported);
    }
    for key in changed_keys {
        if crate::revision_dek::reserved(key.as_bytes()) != (scope == Scope::RevisionDek) {
            return Err(ControlError::InvalidKey);
        }
        if super::registry::reserved(key.as_bytes()) != (scope == Scope::Registry) {
            // The dedicated registry path changes only its namespace. Generic
            // calls may neither replace nor delete a reserved registry record.
            return Err(ControlError::InvalidKey);
        }
        if journal.identity.schema_version == crate::REGISTRY_JOURNAL_SCHEMA_VERSION
            && ((key.as_bytes() == super::operating_profile::KEY && scope != Scope::Profile)
                || (scope == Scope::Profile && key.as_bytes() != super::operating_profile::KEY
                    && key.as_bytes() != super::publication::visibility::STATE)) {
            return Err(ControlError::InvalidKey);
        }
    }
    if journal.identity.schema_version == crate::REGISTRY_JOURNAL_SCHEMA_VERSION && scope != Scope::Profile {
        use super::operating_profile::JournalOperatingProfile;
        let mode = super::registry_mode_checked(read, journal.identity, journal.limits, check)?;
        if matches!(scope, Scope::Registry | Scope::RevisionDek) { super::operating_profile::normal(mode)?; }
        if mode != JournalOperatingProfile::Indexed
            && changed_keys.iter().any(|key| super::publication::port_reserved_key(key.as_bytes())) {
            return Err(ControlError::MigrationUnverified);
        }
        if mode == JournalOperatingProfile::MigrationInert && mutation.writes().iter()
            .any(|write| write.value.class() != crate::ControlRecordClass::Migration) {
            return Err(ControlError::MigrationUnverified);
        }
    }
    Ok(())
}

pub(super) fn reserve_mutation_work(mutation: &ControlMutation, conditions: &[ControlRecordCondition],
    limits: super::JournalLimits, check: &dyn Check,
) -> Result<(), ControlError> {
    use super::MigrationBootstrapWork as Work;
    let items = mutation.writes().len().checked_add(mutation.deletes().len())
        .and_then(|n| n.checked_add(conditions.len())).ok_or(ControlError::BudgetExceeded)?;
    if items > limits.max_mutation_items { return Err(ControlError::BudgetExceeded); }
    let mut bytes = 256_usize;
    let mut key_bytes = 0_usize;
    for (key, value) in mutation.writes().iter().map(|w| (&w.key, Some(&w.value)))
        .chain(mutation.deletes().iter().map(|k| (k, None)))
        .chain(conditions.iter().map(|c| (c.key(), c.expected()))) {
        key_bytes = key_bytes.checked_add(key.as_bytes().len()).ok_or(ControlError::BudgetExceeded)?;
        bytes = bytes.checked_add(key.as_bytes().len()).and_then(|n| n.checked_add(value.map_or(0, crate::ControlValue::len)))
            .ok_or(ControlError::BudgetExceeded)?;
    }
    let capacity = items.checked_mul(4 * std::mem::size_of::<ControlKey>() + 64)
        .and_then(|n| key_bytes.checked_mul(4).and_then(|k| n.checked_add(k))).ok_or(ControlError::BudgetExceeded)?;
    check.reserve(Work::Allocate(capacity))?;
    check.reserve(Work::Process(bytes))
}

fn reserve_encoded_value(value: &crate::ControlValue, check: &dyn Check) -> Result<(), ControlError> {
    check.reserve(super::MigrationBootstrapWork::Allocate(value.len() + 1))?;
    check.reserve(super::MigrationBootstrapWork::Process(value.len() + 1))
}

fn plan_commit(
    journal: &PersistentControlJournal,
    read: &ReadTransaction,
    mutation: &ControlMutation,
    conditions: &[ControlRecordCondition],
    changed_keys: Vec<ControlKey>,
    fingerprint: [u8; 32],
    check: &dyn Check,
) -> Result<PlanOutcome, ControlError> {
    let before = journal.header_from_checked(read, check)?;
    check.check(Point::ReadHeader)?;
    if let Some(previous) = operation_from_checked(read, mutation.id(), &before, journal.limits, check)? {
        if previous.request_sha256 != fingerprint {
            return Err(ControlError::OperationConflict);
        }
        verify_replay(journal, read, &before, &previous.receipt, mutation, &changed_keys, check)?;
        if previous.receipt.after_generation == before.generation {
            verify_conditional_poststate(journal, read, conditions, &changed_keys, check)?;
        }
        let mut receipt = previous.receipt;
        receipt.replayed = true;
        return Ok(PlanOutcome::Replay(receipt));
    }
    if mutation.expected_generation() != before.generation {
        return Err(ControlError::TransactionConflict);
    }
    if before.operations >= as_u64(journal.limits.max_operation_records)? {
        return Err(ControlError::IdempotencyCapacityExceeded);
    }
    {
        let records = read.open_table(RECORDS).map_err(map_table_error)?;
        verify_conditions(journal, &records, conditions, check, Point::PlanRecord)?;
    }
    let mut after = plan_records(journal, read, &before, mutation, &changed_keys, check)?;
    after.generation = before.generation.checked_add(1)
        .ok_or(ControlError::GenerationExhausted)?;
    after.operations = after.generation;
    let receipt = ControlCommitReceipt {
        operation_id: mutation.id(),
        command_digest: mutation.command_digest(),
        before_generation: before.generation,
        after_generation: after.generation,
        changed_keys,
        replayed: false,
    };
    let operation = StoredOperation { request_sha256: fingerprint, receipt: receipt.clone() };
    let operation_bytes = operation.encode_checked(journal.limits, check)?;
    after.operation_bytes = before.operation_bytes.checked_add(as_u64(operation_bytes.len())?)
        .ok_or(ControlError::BudgetExceeded)?;
    if after.operation_bytes > as_u64(journal.limits.max_total_value_bytes)? {
        return Err(ControlError::IdempotencyCapacityExceeded);
    }
    Ok(PlanOutcome::Stage(Box::new(StagePlan { before, after, receipt, operation, operation_bytes, fingerprint })))
}

fn verify_conditions(
    journal: &PersistentControlJournal,
    records: &impl ReadableTable<&'static [u8], &'static [u8]>,
    conditions: &[ControlRecordCondition],
    check: &dyn Check,
    point: Point,
) -> Result<(), ControlError> {
    for condition in conditions {
        check.check(point)?;
        #[cfg(not(test))]
        note_lookup();
        #[cfg(test)]
        note_lookup(journal);
        let observed = records.get(condition.key().as_bytes()).map_err(|error| map_storage_error(&error))?;
        // Bad stored classes/lengths are corruption, not an ordinary failed CAS.
        // At most one temporary value is decoded per lookup.
        let value = observed.map(|bytes| decode_value_checked(bytes.value(), journal.limits, check)).transpose()?;
        if value.as_ref() != condition.expected() { return Err(ControlError::GenerationMismatch); }
    }
    Ok(())
}

fn plan_records(
    journal: &PersistentControlJournal,
    read: &ReadTransaction,
    before: &Header,
    mutation: &ControlMutation,
    changed_keys: &[ControlKey],
    check: &dyn Check,
) -> Result<Header, ControlError> {
    // Distinct keys are validated by the caller. Every written value remains
    // in the final state, so their aggregate alone may not exceed the ceiling.
    let maximum = as_u64(journal.limits.max_total_value_bytes)?;
    let written_bytes = mutation.writes().iter().try_fold(0_u64, |total, change| {
        check.check(Point::PlanRecord)?;
        total.checked_add(as_u64(change.value.len())?)
            .filter(|bytes| *bytes <= maximum).ok_or(ControlError::BudgetExceeded)
    })?;
    let table = read.open_table(RECORDS).map_err(map_table_error)?;
    let mut replaced_records = 0_u64;
    let mut replaced_bytes = 0_u64;
    for key in changed_keys {
        check.check(Point::PlanRecord)?;
        #[cfg(not(test))]
        note_lookup();
        #[cfg(test)]
        note_lookup(journal);
        if let Some(previous) = table.get(key.as_bytes()).map_err(|error| map_storage_error(&error))? {
            // Validate class and size before using the old length. The one
            // temporary decoded value is released on each iteration.
            let value = decode_value_checked(previous.value(), journal.limits, check)?;
            replaced_records = replaced_records.checked_add(1).ok_or(ControlError::StoreCorrupt)?;
            replaced_bytes = replaced_bytes.checked_add(as_u64(value.len())?)
                .ok_or(ControlError::StoreCorrupt)?;
        }
    }
    let mut after = before.clone();
    // Subtract all replaced/deleted values before adding new ones. A full
    // database can therefore replace a key or atomically shrink one and grow
    // another, independently of the command's write order.
    after.records = before.records.checked_sub(replaced_records)
        .ok_or(ControlError::StoreCorrupt)?
        .checked_add(as_u64(mutation.writes().len())?).ok_or(ControlError::BudgetExceeded)?;
    after.value_bytes = before.value_bytes.checked_sub(replaced_bytes)
        .ok_or(ControlError::StoreCorrupt)?
        .checked_add(written_bytes).ok_or(ControlError::BudgetExceeded)?;
    if after.records > as_u64(journal.limits.max_records)? || after.value_bytes > maximum {
        return Err(ControlError::BudgetExceeded);
    }
    Ok(after)
}

/// Validate receipt semantics for both historical replay and current recovery.
/// Historical receipts describe a past commit, not values to reapply today.
#[allow(clippy::too_many_arguments)] // Exact receipt/request bindings plus one shared call budget.
pub(super) fn verify_replay(
    journal: &PersistentControlJournal,
    read: &ReadTransaction,
    current: &Header,
    receipt: &ControlCommitReceipt,
    mutation: &ControlMutation,
    changed_keys: &[ControlKey],
    check: &dyn Check,
) -> Result<(), ControlError> {
    // Production readback needs no journal handle; the test twin of
    // `verify_touched` below consumes it for admission accounting.
    #[cfg(not(test))]
    let _ = journal;
    if receipt.operation_id != mutation.id()
        || receipt.command_digest != mutation.command_digest()
        || receipt.before_generation != mutation.expected_generation()
        || receipt.before_generation.checked_add(1) != Some(receipt.after_generation)
        || receipt.changed_keys.as_slice() != changed_keys
        || receipt.after_generation > current.generation
    {
        return Err(ControlError::StoreCorrupt);
    }
    if receipt.after_generation == current.generation {
        #[cfg(not(test))]
        verify_touched(read, mutation, check)?;
        #[cfg(test)]
        verify_touched(journal, read, mutation, check)?;
    }
    check.check(Point::ReplayComplete)?;
    Ok(())
}

// Production readback carries no point-read accounting, so it takes no
// journal handle. The test twin below counts lookups for admission evidence.
#[cfg(not(test))]
fn verify_touched(
    read: &ReadTransaction,
    mutation: &ControlMutation,
    check: &dyn Check,
) -> Result<(), ControlError> {
    let table = read.open_table(RECORDS).map_err(map_table_error)?;
    for change in mutation.writes() {
        check.check(Point::Readback)?;
        note_lookup();
        let observed = table.get(change.key.as_bytes()).map_err(|error| map_storage_error(&error))?
            .ok_or(ControlError::StoreCorrupt)?;
        // Compare exact class and bytes, not just length/hash/header presence.
        check.reserve(super::MigrationBootstrapWork::Read(observed.value().len()))?;
        reserve_encoded_value(&change.value, check)?;
        let expected = encode_value(&change.value);
        if observed.value() != expected.as_slice() {
            return Err(ControlError::StoreCorrupt);
        }
    }
    for key in mutation.deletes() {
        check.check(Point::Readback)?;
        note_lookup();
        if table.get(key.as_bytes()).map_err(|error| map_storage_error(&error))?.is_some() {
            return Err(ControlError::StoreCorrupt);
        }
    }
    Ok(())
}

#[cfg(test)]
fn verify_touched(
    journal: &PersistentControlJournal,
    read: &ReadTransaction,
    mutation: &ControlMutation,
    check: &dyn Check,
) -> Result<(), ControlError> {
    let table = read.open_table(RECORDS).map_err(map_table_error)?;
    for change in mutation.writes() {
        check.check(Point::Readback)?;
        note_lookup(journal);
        let observed = table.get(change.key.as_bytes()).map_err(|error| map_storage_error(&error))?
            .ok_or(ControlError::StoreCorrupt)?;
        // Compare exact class and bytes, not just length/hash/header presence.
        check.reserve(super::MigrationBootstrapWork::Read(observed.value().len()))?;
        reserve_encoded_value(&change.value, check)?;
        let expected = encode_value(&change.value);
        if observed.value() != expected.as_slice() {
            return Err(ControlError::StoreCorrupt);
        }
    }
    for key in mutation.deletes() {
        check.check(Point::Readback)?;
        note_lookup(journal);
        if table.get(key.as_bytes()).map_err(|error| map_storage_error(&error))?.is_some() {
            return Err(ControlError::StoreCorrupt);
        }
    }
    Ok(())
}

/// Conditions on keys not changed by this command must still hold at its
/// current generation. Changed keys are instead checked against their exact
/// intended post-state by `verify_touched`. Never use this for historical replay.
pub(super) fn verify_conditional_poststate(
    journal: &PersistentControlJournal,
    read: &ReadTransaction,
    conditions: &[ControlRecordCondition],
    changed_keys: &[ControlKey],
    check: &dyn Check,
) -> Result<(), ControlError> {
    if conditions.is_empty() { return Ok(()); }
    let records = read.open_table(RECORDS).map_err(map_table_error)?;
    for condition in conditions {
        check.check(Point::Readback)?;
        if changed_keys.binary_search(condition.key()).is_ok() { continue; }
        #[cfg(not(test))]
        note_lookup();
        #[cfg(test)]
        note_lookup(journal);
        let observed = records.get(condition.key().as_bytes()).map_err(|error| map_storage_error(&error))?;
        let value = observed.map(|bytes| decode_value_checked(bytes.value(), journal.limits, check)).transpose()?;
        if value.as_ref() != condition.expected() { return Err(ControlError::StoreCorrupt); }
    }
    check.check(Point::Readback)?;
    Ok(())
}

// Test builds count point reads for the 10k-admission evidence; production
// builds carry no accounting, so the probe is an infallible `const` no-op.
#[cfg(not(test))]
const fn note_lookup() {}

#[cfg(test)]
fn note_lookup(journal: &PersistentControlJournal) {
    journal.work.point_reads.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod condition_tests;

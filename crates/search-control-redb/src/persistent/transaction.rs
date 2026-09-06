//! Touched-key transaction planning and exact post-commit readback.
//!
//! The enclosing journal is the sole owner of its private redb handle. Open and
//! explicit verification check the whole store. Normal mutations use those
//! checked counters plus exact old values of only the distinct touched keys;
//! they do not build a second in-memory catalog or scan unrelated records.

use super::{
    Boundary, ControlCommitReceipt, ControlError, ControlKey, ControlMutation, Durability,
    Header, OPERATIONS, META, PersistentControlJournal, RECORDS, ReadTransaction,
    ReadableTable, StoredOperation, as_u64, decode_value, encode_value, operation_from,
    request_fingerprint, validate_mutation, map_storage_error, map_table_error,
    Check, Point,
};

pub(super) fn execute(
    journal: &mut PersistentControlJournal,
    mutation: ControlMutation,
    boundary: Boundary,
    check: &dyn Check,
) -> Result<ControlCommitReceipt, ControlError> {
    journal.ensure_available()?;
    check.check(Point::Start)?;
    let changed_keys = validate_mutation(&mutation, journal.limits)?;
    let fingerprint = request_fingerprint(journal.identity, &mutation)?;
    check.check(Point::Validated)?;
    let read = journal.database.begin_read().map_err(|_| ControlError::StoreUnavailable)?;
    let before = journal.header_from(&read)?;
    check.check(Point::ReadHeader)?;
    if let Some(previous) = operation_from(&read, mutation.id(), &before, journal.limits)? {
        if previous.request_sha256 != fingerprint {
            return Err(ControlError::OperationConflict);
        }
        verify_replay(journal, &read, &before, &previous.receipt, &mutation, &changed_keys, check)?;
        let mut receipt = previous.receipt;
        receipt.replayed = true;
        return Ok(receipt);
    }
    if mutation.expected_generation() != before.generation {
        return Err(ControlError::TransactionConflict);
    }
    if before.operations >= as_u64(journal.limits.max_operation_records)? {
        return Err(ControlError::IdempotencyCapacityExceeded);
    }
    let mut after = plan_records(journal, &read, &before, &mutation, &changed_keys, check)?;
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
    let operation_bytes = operation.encode(journal.limits)?;
    after.operation_bytes = before.operation_bytes.checked_add(as_u64(operation_bytes.len())?)
        .ok_or(ControlError::BudgetExceeded)?;
    if after.operation_bytes > as_u64(journal.limits.max_total_value_bytes)? {
        return Err(ControlError::IdempotencyCapacityExceeded);
    }
    drop(read);

    check.check(Point::BeforeWrite)?;
    let mut write = journal.database.begin_write().map_err(|_| ControlError::StoreUnavailable)?;
    write.set_durability(Durability::Immediate);
    let staging = (|| -> Result<(), ControlError> {
        check.check(Point::BeforeWrite)?;
        let mut meta = write.open_table(META).map_err(map_table_error)?;
        {
            let stored = meta.get("header").map_err(map_storage_error)?
                .ok_or(ControlError::StoreCorrupt)?;
            if Header::decode(stored.value(), journal.identity, journal.limits)? != before {
                return Err(ControlError::TransactionConflict);
            }
        }
        let mut records = write.open_table(RECORDS).map_err(map_table_error)?;
        let mut operations = write.open_table(OPERATIONS).map_err(map_table_error)?;
        if operations.get(mutation.id().0.as_slice())
            .map_err(map_storage_error)?.is_some()
        {
            return Err(ControlError::OperationConflict);
        }
        for key in mutation.deletes() {
            check.check(Point::StageRecord)?;
            records.remove(key.as_bytes()).map_err(map_storage_error)?;
        }
        for change in mutation.writes() {
            check.check(Point::StageRecord)?;
            let value = encode_value(&change.value);
            records.insert(change.key.as_bytes(), value.as_slice())
                .map_err(map_storage_error)?;
        }
        operations.insert(mutation.id().0.as_slice(), operation_bytes.as_slice())
            .map_err(map_storage_error)?;
        let header = after.encode();
        meta.insert("header", header.as_slice()).map_err(map_storage_error)?;
        boundary.before_commit()?;
        check.check(Point::BeforeCommit)?;
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
    boundary.after_commit()?;
    check.check(Point::AfterCommit).map_err(|_| ControlError::CommitOutcomeUnknown)?;

    let observed = journal.database.begin_read().map_err(|_| ControlError::CommitOutcomeUnknown)?;
    let observed_header = journal.header_from(&observed)
        .map_err(|_| ControlError::CommitOutcomeUnknown)?;
    let observed_operation = operation_from(&observed, mutation.id(), &observed_header, journal.limits)
        .map_err(|_| ControlError::CommitOutcomeUnknown)?;
    if observed_header != after || observed_operation.as_ref() != Some(&operation) {
        return Err(ControlError::CommitOutcomeUnknown);
    }
    verify_touched(journal, &observed, &mutation, check)
        .map_err(|_| ControlError::CommitOutcomeUnknown)?;
    check.check(Point::MutationComplete).map_err(|_| ControlError::CommitOutcomeUnknown)?;
    journal.pending = None;
    journal.committed_writes = journal.committed_writes.saturating_add(1);
    Ok(receipt)
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
        note_lookup(journal);
        if let Some(previous) = table.get(key.as_bytes()).map_err(map_storage_error)? {
            // Validate class and size before using the old length. The one
            // temporary decoded value is released on each iteration.
            let value = decode_value(previous.value(), journal.limits)?;
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
        verify_touched(journal, read, mutation, check)?;
    }
    check.check(Point::ReplayComplete)?;
    Ok(())
}

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
        let observed = table.get(change.key.as_bytes()).map_err(map_storage_error)?
            .ok_or(ControlError::StoreCorrupt)?;
        // Compare exact class and bytes, not just length/hash/header presence.
        let expected = encode_value(&change.value);
        if observed.value() != expected.as_slice() {
            return Err(ControlError::StoreCorrupt);
        }
    }
    for key in mutation.deletes() {
        check.check(Point::Readback)?;
        note_lookup(journal);
        if table.get(key.as_bytes()).map_err(map_storage_error)?.is_some() {
            return Err(ControlError::StoreCorrupt);
        }
    }
    Ok(())
}

fn note_lookup(_journal: &PersistentControlJournal) {
    #[cfg(test)]
    _journal.work.point_reads.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

#[cfg(test)]
mod tests;

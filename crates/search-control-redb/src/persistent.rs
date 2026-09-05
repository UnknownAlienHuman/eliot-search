//! Disk-backed, identity-bound technical control transactions.
//!
//! The caller must hold the verified data-root owner guard for this handle's
//! entire lifetime and supply a verified final regular-file handle. Creation
//! and reopening are separate: an empty existing file is corruption, not an
//! invitation to initialize a new installation. Native path admission and owner
//! succession are composition responsibilities; neither is inferred here.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs::File;

use redb::{Database, DatabaseError, Durability, ReadTransaction, ReadableTable,
    ReadableTableMetadata, StorageError, TableDefinition, TableHandle};

use crate::{CommitRecoveryDecision, ControlCommitReceipt, ControlError, ControlKey,
    ControlMutation, ControlSnapshot, ControlSnapshotPublisher, JournalIdentity, JournalLimits,
    JournalReadSnapshot, MutationId, SnapshotPublishReceipt, rebuild_control_snapshot};

mod codec;
use codec::{Header, StoredOperation, as_u64, decode_value, encode_value,
    request_fingerprint, validate_mutation};

const META: TableDefinition<&str, &[u8]> = TableDefinition::new("eliot.control.meta.v1");
const RECORDS: TableDefinition<&[u8], &[u8]> = TableDefinition::new("eliot.control.records.v1");
const OPERATIONS: TableDefinition<&[u8], &[u8]> = TableDefinition::new("eliot.control.operations.v1");
const CACHE_BYTES: usize = 8 * 1024 * 1024;

/// Concrete redb owner. Vendor values, guards, paths and errors stay private.
///
/// This is not a search database. Record classes describe technical metadata;
/// the capability owner must validate the semantic payload before constructing
/// `ControlValue`. A class tag alone cannot prove that arbitrary bytes are safe.
/// No schema migration, owner-epoch rebinding or receipt pruning is implicit.
pub struct PersistentControlJournal {
    database: Database,
    identity: JournalIdentity,
    limits: JournalLimits,
    pending: Option<(MutationId, [u8; 32])>,
    quarantined: bool,
    committed_writes: u64,
}

impl fmt::Debug for PersistentControlJournal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("PersistentControlJournal")
            .field("identity", &self.identity)
            .field("requires_recovery", &self.pending.is_some())
            .field("quarantined", &self.quarantined)
            .field("committed_writes", &self.committed_writes)
            .finish_non_exhaustive()
    }
}

impl PersistentControlJournal {
    /// Initializes an explicitly newly created, empty regular file.
    ///
    /// The caller creates the file exclusively and keeps the external owner
    /// guard alive. This method does not replace an existing database. Failure
    /// after initialization may have started requires inspection/reopening.
    pub fn create(file: File, identity: JournalIdentity, limits: JournalLimits) -> Result<Self, ControlError> {
        let identity = validate_identity(identity)?;
        let limits = limits.validate()?;
        let metadata = file.metadata().map_err(|_| ControlError::StoreUnavailable)?;
        if !metadata.is_file() || metadata.len() != 0 { return Err(ControlError::StoreCorrupt); }
        let database = Database::builder().set_cache_size(CACHE_BYTES).create_file(file)
            .map_err(|_| ControlError::CommitOutcomeUnknown)?;
        let mut write = database.begin_write().map_err(|_| ControlError::CommitOutcomeUnknown)?;
        write.set_durability(Durability::Immediate);
        {
            let mut meta = write.open_table(META).map_err(|_| ControlError::CommitOutcomeUnknown)?;
            let header = Header::empty(identity).encode();
            meta.insert("header", header.as_slice()).map_err(|_| ControlError::CommitOutcomeUnknown)?;
            drop(write.open_table(RECORDS).map_err(|_| ControlError::CommitOutcomeUnknown)?);
            drop(write.open_table(OPERATIONS).map_err(|_| ControlError::CommitOutcomeUnknown)?);
        }
        write.commit().map_err(|_| ControlError::CommitOutcomeUnknown)?;
        let result = Self { database, identity, limits, pending: None, quarantined: false, committed_writes: 1 };
        result.verify().map_err(|_| ControlError::CommitOutcomeUnknown)?;
        Ok(result)
    }

    /// Reopens an existing non-empty database without creating missing tables.
    ///
    /// Every identity field, including owner epoch, must match. A successor
    /// owner uses the explicit `advance_owner` handoff, not a guessed epoch.
    /// redb may recover its own unclean transaction state; this adapter never
    /// invokes forced integrity repair or invents missing application records.
    pub fn open(file: File, identity: JournalIdentity, limits: JournalLimits) -> Result<Self, ControlError> {
        let identity = validate_identity(identity)?;
        let limits = limits.validate()?;
        let metadata = file.metadata().map_err(|_| ControlError::StoreUnavailable)?;
        if !metadata.is_file() || metadata.len() == 0 { return Err(ControlError::StoreCorrupt); }
        let database = Database::builder().set_cache_size(CACHE_BYTES).create_file(file).map_err(map_database_error)?;
        let result = Self { database, identity, limits, pending: None, quarantined: false, committed_writes: 0 };
        result.verify()?;
        Ok(result)
    }

    /// Performs an explicit, consuming handoff to the next verified owner epoch.
    ///
    /// The caller must hold the new live root-owner guard. Immutable root,
    /// incarnation, path and schema bindings cannot change. Data generations
    /// and operation receipts are preserved. An already-current target is an
    /// idempotent readback. On failure this handle is consumed; after a possible
    /// write, reopen using the exact intended identity to resolve the outcome.
    pub fn advance_owner(mut self, next: JournalIdentity) -> Result<Self, ControlError> {
        self.verify()?;
        validate_identity(next)?;
        if next == self.identity { return Ok(self); }
        let stable = JournalIdentity { owner_epoch: self.identity.owner_epoch, ..next };
        if stable != self.identity
            || self.identity.owner_epoch.checked_next().map_err(|_| ControlError::GenerationExhausted)? != next.owner_epoch
        { return Err(ControlError::IdentityMismatch); }
        let read = self.database.begin_read().map_err(|_| ControlError::StoreUnavailable)?;
        let before = self.header_from(&read)?;
        drop(read);
        let mut write = self.database.begin_write().map_err(|_| ControlError::StoreUnavailable)?;
        write.set_durability(Durability::Immediate);
        {
            let mut meta = write.open_table(META).map_err(|_| ControlError::SchemaMismatch)?;
            {
                let bytes = meta.get("header").map_err(|_| ControlError::StoreUnavailable)?.ok_or(ControlError::StoreCorrupt)?;
                if Header::decode(bytes.value(), self.identity, self.limits)? != before {
                    return Err(ControlError::TransactionConflict);
                }
            }
            let after = Header { identity: next, ..before }.encode();
            meta.insert("header", after.as_slice()).map_err(|_| ControlError::StoreUnavailable)?;
        }
        write.commit().map_err(|_| ControlError::CommitOutcomeUnknown)?;
        self.identity = next;
        self.verify().map_err(|_| ControlError::CommitOutcomeUnknown)?;
        self.committed_writes = self.committed_writes.saturating_add(1);
        Ok(self)
    }

    /// Verified exact installation/root/owner/path/schema binding.
    #[must_use]
    pub const fn identity(&self) -> JournalIdentity { self.identity }

    /// Diagnostic count of successful initialization, transaction and owner-handoff calls.
    /// Reads, recovery and idempotent replay do not increment it; this is not an I/O metric.
    #[must_use]
    pub const fn committed_writes(&self) -> u64 { self.committed_writes }

    /// Whether a possible commit still requires authoritative readback.
    #[must_use]
    pub const fn requires_recovery(&self) -> bool { self.pending.is_some() }

    /// Reads a coherent bounded snapshot using a read-only database transaction.
    pub fn read_snapshot(&self) -> Result<JournalReadSnapshot, ControlError> {
        self.ensure_available()?;
        let read = self.database.begin_read().map_err(|_| ControlError::StoreUnavailable)?;
        self.snapshot_from(&read)
    }

    /// Rebuilds the shared immutable snapshot from committed disk state.
    pub fn control_snapshot(&self) -> Result<ControlSnapshot, ControlError> {
        rebuild_control_snapshot(self.read_snapshot()?, self.identity, self.limits)
    }

    /// Publishes a snapshot only after exact readback of its current durable receipt.
    /// A fabricated or historical receipt cannot publish an unrelated generation.
    pub fn publish_committed_snapshot(
        &self,
        receipt: &ControlCommitReceipt,
        publisher: &mut ControlSnapshotPublisher,
    ) -> Result<SnapshotPublishReceipt, ControlError> {
        self.ensure_available()?;
        let read = self.database.begin_read().map_err(|_| ControlError::StoreUnavailable)?;
        let header = self.header_from(&read)?;
        let stored = operation_from(&read, receipt.operation_id, &header, self.limits)?
            .ok_or(ControlError::SnapshotPublicationFailed)?;
        let mut expected = receipt.clone();
        expected.replayed = false;
        if stored.receipt != expected || receipt.after_generation != header.generation {
            return Err(ControlError::SnapshotPublicationFailed);
        }
        let snapshot = rebuild_control_snapshot(self.snapshot_from(&read)?, self.identity, self.limits)?;
        publisher.publish_snapshot_after_commit(receipt, snapshot)
    }

    /// Republishes the latest committed generation without replaying a mutation.
    /// An empty initialized journal has no commit receipt and returns `None`.
    pub fn recover_snapshot_publication(
        &self,
        publisher: &mut ControlSnapshotPublisher,
    ) -> Result<Option<SnapshotPublishReceipt>, ControlError> {
        self.ensure_available()?;
        let read = self.database.begin_read().map_err(|_| ControlError::StoreUnavailable)?;
        let snapshot = self.verify_from(&read)?;
        if snapshot.generation == 0 { return Ok(None); }
        let table = read.open_table(OPERATIONS).map_err(|_| ControlError::SchemaMismatch)?;
        for row in table.iter().map_err(|_| ControlError::StoreUnavailable)? {
            let (id, bytes) = row.map_err(|_| ControlError::StoreUnavailable)?;
            let id = MutationId(id.value().try_into().map_err(|_| ControlError::StoreCorrupt)?);
            let operation = StoredOperation::decode(bytes.value(), id, snapshot.generation, self.limits)?;
            if operation.receipt.after_generation == snapshot.generation {
                let state = rebuild_control_snapshot(snapshot, self.identity, self.limits)?;
                return publisher.publish_snapshot_after_commit(&operation.receipt, state).map(Some);
            }
        }
        Err(ControlError::SnapshotPublicationFailed)
    }

    /// Verifies exact tables, metadata, records and the complete bounded receipt ledger.
    /// This is an application consistency check, not physical-media qualification.
    pub fn verify(&self) -> Result<JournalReadSnapshot, ControlError> {
        self.ensure_available()?;
        let read = self.database.begin_read().map_err(|_| ControlError::StoreUnavailable)?;
        self.verify_from(&read)
    }

    /// Commits metadata, all record changes and the receipt in one immediate transaction.
    ///
    /// Replay validates the complete canonical request, not merely a digest
    /// supplied by a caller. A commit or post-commit readback error blocks normal
    /// operations until `recover_transaction` resolves the exact request.
    pub fn transact(&mut self, mutation: ControlMutation) -> Result<ControlCommitReceipt, ControlError> {
        self.transact_inner(mutation, Boundary::Normal)
    }

    /// Resolves the exact request against the disk ledger without retrying any write.
    ///
    /// A historical committed receipt is not a claim that its values remain
    /// current after later transactions. The ledger is not pruned by this adapter.
    pub fn recover_transaction(&mut self, mutation: &ControlMutation) -> Result<CommitRecoveryDecision, ControlError> {
        validate_mutation(mutation, self.limits)?;
        let fingerprint = request_fingerprint(self.identity, mutation)?;
        if self.quarantined { return Ok(CommitRecoveryDecision::PartialOrCorruptQuarantine); }
        if self.pending.is_some_and(|pending| pending != (mutation.id(), fingerprint)) {
            return Ok(CommitRecoveryDecision::ConflictingInput);
        }
        let read = self.database.begin_read().map_err(|_| ControlError::StoreUnavailable)?;
        if self.verify_from(&read).is_err() {
            self.quarantined = true;
            return Ok(CommitRecoveryDecision::PartialOrCorruptQuarantine);
        }
        let header = self.header_from(&read)?;
        let result = match operation_from(&read, mutation.id(), &header, self.limits)? {
            Some(operation) if operation.request_sha256 == fingerprint => {
                let mut receipt = operation.receipt;
                receipt.replayed = true;
                CommitRecoveryDecision::Committed(receipt)
            }
            Some(_) => return Ok(CommitRecoveryDecision::ConflictingInput),
            None => CommitRecoveryDecision::NotCommittedRetrySameOperation,
        };
        self.pending = None;
        Ok(result)
    }

    fn ensure_available(&self) -> Result<(), ControlError> {
        if self.quarantined || self.pending.is_some() { Err(ControlError::StoreQuarantined) } else { Ok(()) }
    }

    fn header_from(&self, read: &ReadTransaction) -> Result<Header, ControlError> {
        verify_tables(read)?;
        let meta = read.open_table(META).map_err(|_| ControlError::SchemaMismatch)?;
        if meta.len().map_err(|_| ControlError::StoreUnavailable)? != 1 { return Err(ControlError::StoreCorrupt); }
        let bytes = meta.get("header").map_err(|_| ControlError::StoreUnavailable)?.ok_or(ControlError::StoreCorrupt)?;
        let header = Header::decode(bytes.value(), self.identity, self.limits)?;
        let records = read.open_table(RECORDS).map_err(|_| ControlError::SchemaMismatch)?;
        let operations = read.open_table(OPERATIONS).map_err(|_| ControlError::SchemaMismatch)?;
        if records.len().map_err(|_| ControlError::StoreUnavailable)? != header.records
            || operations.len().map_err(|_| ControlError::StoreUnavailable)? != header.operations
        { return Err(ControlError::StoreCorrupt); }
        Ok(header)
    }

    fn snapshot_from(&self, read: &ReadTransaction) -> Result<JournalReadSnapshot, ControlError> {
        let header = self.header_from(read)?;
        let table = read.open_table(RECORDS).map_err(|_| ControlError::SchemaMismatch)?;
        let mut records = Vec::new();
        let mut total = 0_u64;
        for row in table.iter().map_err(|_| ControlError::StoreUnavailable)? {
            let (key, value) = row.map_err(|_| ControlError::StoreUnavailable)?;
            if records.len() >= self.limits.max_records || key.value().len() > self.limits.max_key_bytes {
                return Err(ControlError::StoreCorrupt);
            }
            let key = ControlKey::new(key.value().to_vec(), self.limits).map_err(|_| ControlError::StoreCorrupt)?;
            let value = decode_value(value.value(), self.limits)?;
            total = total.checked_add(as_u64(value.len())?).ok_or(ControlError::StoreCorrupt)?;
            if total > as_u64(self.limits.max_total_value_bytes)? { return Err(ControlError::StoreCorrupt); }
            records.push((key, value));
        }
        if total != header.value_bytes { return Err(ControlError::StoreCorrupt); }
        Ok(JournalReadSnapshot { identity: self.identity, generation: header.generation, records })
    }

    fn verify_from(&self, read: &ReadTransaction) -> Result<JournalReadSnapshot, ControlError> {
        let snapshot = self.snapshot_from(read)?;
        let header = self.header_from(read)?;
        let table = read.open_table(OPERATIONS).map_err(|_| ControlError::SchemaMismatch)?;
        let mut generations = BTreeSet::new();
        let mut total = 0_u64;
        for row in table.iter().map_err(|_| ControlError::StoreUnavailable)? {
            let (id, bytes) = row.map_err(|_| ControlError::StoreUnavailable)?;
            if generations.len() >= self.limits.max_operation_records { return Err(ControlError::StoreCorrupt); }
            let id = MutationId(id.value().try_into().map_err(|_| ControlError::StoreCorrupt)?);
            let operation = StoredOperation::decode(bytes.value(), id, header.generation, self.limits)?;
            if !generations.insert(operation.receipt.after_generation) { return Err(ControlError::StoreCorrupt); }
            total = total.checked_add(as_u64(bytes.value().len())?).ok_or(ControlError::StoreCorrupt)?;
            if total > as_u64(self.limits.max_total_value_bytes)? { return Err(ControlError::StoreCorrupt); }
        }
        if total != header.operation_bytes || as_u64(generations.len())? != header.generation {
            return Err(ControlError::StoreCorrupt);
        }
        Ok(snapshot)
    }

    fn transact_inner(&mut self, mutation: ControlMutation, boundary: Boundary) -> Result<ControlCommitReceipt, ControlError> {
        self.ensure_available()?;
        let changed_keys = validate_mutation(&mutation, self.limits)?;
        let fingerprint = request_fingerprint(self.identity, &mutation)?;
        let read = self.database.begin_read().map_err(|_| ControlError::StoreUnavailable)?;
        let before = self.header_from(&read)?;
        if let Some(previous) = operation_from(&read, mutation.id(), &before, self.limits)? {
            if previous.request_sha256 != fingerprint { return Err(ControlError::OperationConflict); }
            let mut receipt = previous.receipt;
            receipt.replayed = true;
            return Ok(receipt);
        }
        if mutation.expected_generation() != before.generation { return Err(ControlError::TransactionConflict); }
        if before.operations >= as_u64(self.limits.max_operation_records)? { return Err(ControlError::IdempotencyCapacityExceeded); }
        let snapshot = self.snapshot_from(&read)?;
        let mut staged = snapshot.records.into_iter().collect::<BTreeMap<_, _>>();
        for key in mutation.deletes() { staged.remove(key); }
        for write in mutation.writes() { staged.insert(write.key.clone(), write.value.clone()); }
        let mut after = before.clone();
        after.records = as_u64(staged.len())?;
        after.value_bytes = staged.values().try_fold(0_u64, |total, value| {
            total.checked_add(as_u64(value.len())?).ok_or(ControlError::BudgetExceeded)
        })?;
        if after.records > as_u64(self.limits.max_records)? || after.value_bytes > as_u64(self.limits.max_total_value_bytes)? {
            return Err(ControlError::BudgetExceeded);
        }
        after.generation = before.generation.checked_add(1).ok_or(ControlError::GenerationExhausted)?;
        after.operations = after.generation;
        let receipt = ControlCommitReceipt {
            operation_id: mutation.id(), command_digest: mutation.command_digest(),
            before_generation: before.generation, after_generation: after.generation,
            changed_keys, replayed: false,
        };
        let operation = StoredOperation { request_sha256: fingerprint, receipt: receipt.clone() };
        let operation_bytes = operation.encode(self.limits)?;
        after.operation_bytes = before.operation_bytes.checked_add(as_u64(operation_bytes.len())?).ok_or(ControlError::BudgetExceeded)?;
        if after.operation_bytes > as_u64(self.limits.max_total_value_bytes)? { return Err(ControlError::IdempotencyCapacityExceeded); }
        drop(read);
        let mut write = self.database.begin_write().map_err(|_| ControlError::StoreUnavailable)?;
        write.set_durability(Durability::Immediate);
        {
            let mut meta = write.open_table(META).map_err(|_| ControlError::SchemaMismatch)?;
            {
                let stored = meta.get("header").map_err(|_| ControlError::StoreUnavailable)?.ok_or(ControlError::StoreCorrupt)?;
                if Header::decode(stored.value(), self.identity, self.limits)? != before { return Err(ControlError::TransactionConflict); }
            }
            let mut records = write.open_table(RECORDS).map_err(|_| ControlError::SchemaMismatch)?;
            let mut operations = write.open_table(OPERATIONS).map_err(|_| ControlError::SchemaMismatch)?;
            if operations.get(mutation.id().0.as_slice()).map_err(|_| ControlError::StoreUnavailable)?.is_some() {
                return Err(ControlError::OperationConflict);
            }
            for key in mutation.deletes() { records.remove(key.as_bytes()).map_err(|_| ControlError::StoreUnavailable)?; }
            for change in mutation.writes() {
                let value = encode_value(&change.value);
                records.insert(change.key.as_bytes(), value.as_slice()).map_err(|_| ControlError::StoreUnavailable)?;
            }
            operations.insert(mutation.id().0.as_slice(), operation_bytes.as_slice()).map_err(|_| ControlError::StoreUnavailable)?;
            let header = after.encode();
            meta.insert("header", header.as_slice()).map_err(|_| ControlError::StoreUnavailable)?;
        }
        boundary.before_commit()?;
        // From this point a failure may hide a durable effect. Never report it
        // as a safe-to-repeat pre-dispatch failure or expose a guessed snapshot.
        self.pending = Some((mutation.id(), fingerprint));
        write.commit().map_err(|_| ControlError::CommitOutcomeUnknown)?;
        boundary.after_commit()?;
        let observed = self.database.begin_read().map_err(|_| ControlError::CommitOutcomeUnknown)?;
        let observed_header = self.header_from(&observed).map_err(|_| ControlError::CommitOutcomeUnknown)?;
        let observed_operation = operation_from(&observed, mutation.id(), &observed_header, self.limits)
            .map_err(|_| ControlError::CommitOutcomeUnknown)?;
        let actual = self.snapshot_from(&observed).map_err(|_| ControlError::CommitOutcomeUnknown)?;
        if observed_header != after || observed_operation.as_ref() != Some(&operation)
            || actual.records != staged.into_iter().collect::<Vec<_>>()
        { return Err(ControlError::CommitOutcomeUnknown); }
        self.pending = None;
        self.committed_writes = self.committed_writes.saturating_add(1);
        Ok(receipt)
    }
}

fn validate_identity(identity: JournalIdentity) -> Result<JournalIdentity, ControlError> {
    identity.validate()?;
    if identity.schema_version != 1 { return Err(ControlError::SchemaUnsupported); }
    Ok(identity)
}

fn verify_tables(read: &ReadTransaction) -> Result<(), ControlError> {
    let mut names = read.list_tables().map_err(|_| ControlError::StoreUnavailable)?
        .take(4).map(|table| table.name().to_owned()).collect::<Vec<_>>();
    names.sort();
    if !names.iter().map(String::as_str).eq(["eliot.control.meta.v1", "eliot.control.operations.v1", "eliot.control.records.v1"])
        || read.list_multimap_tables().map_err(|_| ControlError::StoreUnavailable)?.next().is_some()
    { return Err(ControlError::SchemaMismatch); }
    Ok(())
}

fn operation_from(read: &ReadTransaction, id: MutationId, header: &Header, limits: JournalLimits) -> Result<Option<StoredOperation>, ControlError> {
    let table = read.open_table(OPERATIONS).map_err(|_| ControlError::SchemaMismatch)?;
    table.get(id.0.as_slice()).map_err(|_| ControlError::StoreUnavailable)?
        .map(|bytes| StoredOperation::decode(bytes.value(), id, header.generation, limits)).transpose()
}

fn map_database_error(error: DatabaseError) -> ControlError {
    match error {
        DatabaseError::DatabaseAlreadyOpen | DatabaseError::Storage(StorageError::Io(_)) => ControlError::StoreUnavailable,
        DatabaseError::UpgradeRequired(_) => ControlError::MigrationUnverified,
        _ => ControlError::StoreCorrupt,
    }
}

#[derive(Clone, Copy)]
enum Boundary {
    Normal,
    #[cfg(test)] BeforeCommit,
    #[cfg(test)] LostAcknowledgement,
    #[cfg(test)] ExitBeforeCommit,
    #[cfg(test)] ExitAfterCommit,
}
impl Boundary {
    fn before_commit(self) -> Result<(), ControlError> {
        match self {
            #[cfg(test)] Self::BeforeCommit => Err(ControlError::StoreUnavailable),
            #[cfg(test)] Self::ExitBeforeCommit => std::process::exit(73),
            _ => Ok(()),
        }
    }
    fn after_commit(self) -> Result<(), ControlError> {
        match self {
            #[cfg(test)] Self::LostAcknowledgement => Err(ControlError::CommitOutcomeUnknown),
            #[cfg(test)] Self::ExitAfterCommit => std::process::exit(74),
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests;

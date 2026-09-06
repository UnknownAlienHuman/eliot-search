//! Context-controlled creation, reopening and consuming owner handoff.
//!
//! Every old and new entrypoint uses these same engines. The database is never
//! attached after a partial inspection. Opening redb can recover native metadata;
//! it is not the contract's side-effect-free inspect operation.

use std::fs::File;

use redb::{Database, DatabaseError, Durability};
use search_ports::{CancellationProbe, OperationContext};

use super::operation::{Budget, Check, Point};
use super::{
    CACHE_BYTES, ControlCallError, ControlError, Header, JournalIdentity, JournalLimits,
    META, MutationId, OPERATIONS, PersistentControlJournal, RECORDS, ReadableTable,
    is_corruption, map_storage_error, map_table_error, validate_identity,
};

impl PersistentControlJournal {
    /// Initializes only a caller-created empty file under one cooperative budget.
    ///
    /// The caller retains the verified root-owner guard and a stable operation
    /// ID. That ID is error correlation, not a new durable mutation-ledger row.
    /// The file and any possibly initialized state are never deleted on failure.
    ///
    /// # Errors
    /// Before native database dispatch, interruption has no initialization effect.
    /// Afterwards all failures are unknown until exact inspect/reopen. No partial
    /// journal guard is returned. Synchronous OS/redb I/O cannot be preempted.
    pub fn create_with_context<C: CancellationProbe>(
        file: File,
        identity: JournalIdentity,
        limits: JournalLimits,
        operation_id: MutationId,
        context: &OperationContext<C>,
    ) -> Result<Self, ControlCallError> {
        let budget = Budget::new(context);
        Self::create_checked(file, identity, limits, &budget)
            .map_err(|error| budget.failure(error, Some(operation_id)))
    }

    /// Reopens an existing non-empty journal under one cooperative call budget.
    ///
    /// Exact identity, schema, records and receipt ledger are checked before the
    /// handle is returned. Missing tables are not created. The supplied operation
    /// ID correlates an error; opening does not append an idempotency record.
    ///
    /// # Errors
    /// Cancellation or transient failure after native open requires inspect/reopen,
    /// because redb may already have recovered internal metadata. Actual schema or
    /// identity contradictions stay typed quarantine failures, not an empty store.
    /// This is not side-effect-free inspection or a hard wall-clock I/O guarantee.
    pub fn open_with_context<C: CancellationProbe>(
        file: File,
        identity: JournalIdentity,
        limits: JournalLimits,
        operation_id: MutationId,
        context: &OperationContext<C>,
    ) -> Result<Self, ControlCallError> {
        let budget = Budget::new(context);
        Self::open_checked(file, identity, limits, &budget)
            .map_err(|error| budget.failure(error, Some(operation_id)))
    }

    /// Consumes the journal and explicitly advances its exact owner binding.
    ///
    /// The caller must retain the newly verified root-owner guard. Only the next
    /// epoch is accepted; immutable identity fields and mutation receipts remain
    /// unchanged. An already-current target is verified without another write.
    /// The supplied operation ID is correlation only, not a fabricated receipt.
    ///
    /// # Errors
    /// No usable handle escapes a failed handoff. After write dispatch, failure
    /// is unknown even if staged abort succeeds. Inspect the exact trusted prior
    /// and intended identities before reopening/retrying; do not guess an epoch
    /// or acquire root authority from a matching database header.
    pub fn advance_owner_with_context<C: CancellationProbe>(
        self,
        next: JournalIdentity,
        operation_id: MutationId,
        context: &OperationContext<C>,
    ) -> Result<Self, ControlCallError> {
        let budget = Budget::new(context);
        self.advance_owner_checked(next, &budget)
            .map_err(|error| budget.failure(error, Some(operation_id)))
    }

    pub(super) fn create_checked(
        file: File,
        identity: JournalIdentity,
        limits: JournalLimits,
        check: &dyn Check,
    ) -> Result<Self, ControlError> {
        preflight(&file, identity, limits, true, check)?;
        check.check(Point::BeforeOpen)?;
        // From here even native initialization may have changed the file. No
        // cancellation or later failed inspection can assert "not created".
        let database = Database::builder().set_cache_size(CACHE_BYTES).create_file(file)
            .map_err(|_| ControlError::CommitOutcomeUnknown)?;
        (|| {
            check.check(Point::AfterOpen)?;
            let mut write = database.begin_write().map_err(|_| ControlError::StoreUnavailable)?;
            write.set_durability(Durability::Immediate);
            let stage = (|| {
                check.check(Point::StageRecord)?;
                let mut meta = write.open_table(META).map_err(map_table_error)?;
                let header = Header::empty(identity).encode();
                meta.insert("header", header.as_slice()).map_err(map_storage_error)?;
                drop(meta);
                check.check(Point::StageRecord)?;
                drop(write.open_table(RECORDS).map_err(map_table_error)?);
                check.check(Point::StageRecord)?;
                drop(write.open_table(OPERATIONS).map_err(map_table_error)?);
                check.check(Point::BeforeCommit)
            })();
            if stage.is_err() {
                // redb's explicit abort closes staged state. Native initialization
                // preceded it, so even a successful abort cannot undo that fact.
                let _ = write.abort();
                return Err(ControlError::CommitOutcomeUnknown);
            }
            write.commit().map_err(|_| ControlError::CommitOutcomeUnknown)?;
            check.check(Point::AfterCommit)?;
            let journal = from_database(database, identity, limits, 1);
            verify_ready(&journal, check)?;
            Ok(journal)
        })().map_err(|_| ControlError::CommitOutcomeUnknown)
    }

    pub(super) fn open_checked(
        file: File,
        identity: JournalIdentity,
        limits: JournalLimits,
        check: &dyn Check,
    ) -> Result<Self, ControlError> {
        let database = open_existing_database_checked(file, identity, limits, check)?;
        let journal = from_database(database, identity, limits, 0);
        let verification = (|| {
            check.check(Point::AfterOpen)?;
            verify_ready(&journal, check)
        })();
        match verification {
            Ok(()) => Ok(journal),
            Err(error) if is_corruption(error) || error == ControlError::StoreQuarantined => Err(error),
            // Dropping the unreturned guard releases only its own native handle;
            // it neither deletes the file nor rewrites missing application state.
            Err(_) => Err(ControlError::CommitOutcomeUnknown),
        }
    }

    pub(super) fn advance_owner_checked(
        mut self,
        next: JournalIdentity,
        check: &dyn Check,
    ) -> Result<Self, ControlError> {
        self.ensure_available()?;
        check.check(Point::Start)?;
        validate_identity(next)?;
        let stable = JournalIdentity { owner_epoch: self.identity.owner_epoch, ..next };
        if stable != self.identity || (next != self.identity
            && self.identity.owner_epoch.checked_next()
                .map_err(|_| ControlError::GenerationExhausted)? != next.owner_epoch)
        {
            return Err(ControlError::IdentityMismatch);
        }
        let read = self.database.begin_read().map_err(|_| ControlError::StoreUnavailable)?;
        self.verify_from_checked(&read, check)?;
        let before = self.header_from(&read)?;
        check.check(Point::Validated)?;
        drop(read);
        if next == self.identity {
            check.check(Point::LifecycleComplete)?;
            return Ok(self);
        }
        check.check(Point::BeforeWrite)?;
        let mut write = self.database.begin_write().map_err(|_| ControlError::StoreUnavailable)?;
        write.set_durability(Durability::Immediate);
        let stage = (|| {
            check.check(Point::StageRecord)?;
            let mut meta = write.open_table(META).map_err(map_table_error)?;
            {
                let bytes = meta.get("header").map_err(map_storage_error)?
                    .ok_or(ControlError::StoreCorrupt)?;
                if Header::decode(bytes.value(), self.identity, self.limits)? != before {
                    return Err(ControlError::TransactionConflict);
                }
            }
            let after = Header { identity: next, ..before }.encode();
            meta.insert("header", after.as_slice()).map_err(map_storage_error)?;
            drop(meta);
            check.check(Point::BeforeCommit)
        })();
        if stage.is_err() {
            let _ = write.abort();
            return Err(ControlError::CommitOutcomeUnknown);
        }
        write.commit().map_err(|_| ControlError::CommitOutcomeUnknown)?;
        self.identity = next;
        check.check(Point::AfterCommit).map_err(|_| ControlError::CommitOutcomeUnknown)?;
        verify_ready(&self, check).map_err(|_| ControlError::CommitOutcomeUnknown)?;
        self.committed_writes = self.committed_writes.saturating_add(1);
        Ok(self)
    }
}

// Shared only with diagnostic quarantine recovery. Does not return a usable
// journal until the caller completes its own specific admission/inspection.
pub(super) fn open_existing_database_checked(
    file: File, identity: JournalIdentity, limits: JournalLimits, check: &dyn Check,
) -> Result<Database, ControlError> {
    preflight(&file, identity, limits, false, check)?;
    check.check(Point::BeforeOpen)?;
    Database::builder().set_cache_size(CACHE_BYTES).create_file(file).map_err(open_error)
}

fn preflight(
    file: &File,
    identity: JournalIdentity,
    limits: JournalLimits,
    creating: bool,
    check: &dyn Check,
) -> Result<(), ControlError> {
    check.check(Point::Start)?;
    validate_identity(identity)?;
    limits.validate()?;
    let metadata = file.metadata().map_err(|_| ControlError::StoreUnavailable)?;
    if !metadata.is_file() || ((metadata.len() == 0) != creating) {
        return Err(ControlError::StoreCorrupt);
    }
    check.check(Point::Validated)
}

fn from_database(
    database: Database,
    identity: JournalIdentity,
    limits: JournalLimits,
    committed_writes: u64,
) -> PersistentControlJournal {
    PersistentControlJournal {
        database, identity, limits, committed_writes, pending: None, quarantined: false,
        #[cfg(test)]
        work: super::TestWork::default(),
    }
}

fn verify_ready(journal: &PersistentControlJournal, check: &dyn Check) -> Result<(), ControlError> {
    let read = journal.database.begin_read().map_err(|_| ControlError::StoreUnavailable)?;
    journal.verify_from_checked(&read, check)?;
    // Includes the empty freshly-created journal: no loop is needed to notice
    // an interruption after open/commit and before returning its usable guard.
    check.check(Point::LifecycleComplete)
}

fn open_error(error: DatabaseError) -> ControlError {
    match error {
        DatabaseError::DatabaseAlreadyOpen => ControlError::StoreUnavailable,
        DatabaseError::UpgradeRequired(_) => ControlError::MigrationUnverified,
        DatabaseError::Storage(redb::StorageError::Corrupted(_)) => ControlError::StoreCorrupt,
        // Open can repair native metadata. I/O/aborted-repair/internal failure is
        // not a proven no-effect failure and must not offer a blind retry.
        _ => ControlError::CommitOutcomeUnknown,
    }
}

#[cfg(test)]
mod tests;

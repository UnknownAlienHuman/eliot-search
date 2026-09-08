//! Administrative quarantine uses one immutable marker in the existing META
//! table. It never repairs a damaged record or deletes application state.

use std::fs::File;
use search_ports::{CancellationProbe, OperationContext};
use super::{Boundary, Check, ControlCallError, ControlError, ControlSnapshotPublisher,
    Database, Durability, Header, JournalIdentity, JournalLimits, META, MutationId, OPERATIONS,
    PersistentControlJournal, Point, ReadableTable, ReadableTableMetadata,
    is_corruption, lifecycle, map_storage_error, map_table_error};
use super::operation::Budget;

mod record;
use record::Marker;
pub use record::{ControlQuarantineReason, ControlQuarantineReceipt, ControlQuarantineRequest};

const MARKER_KEY: &str = "quarantine";

impl PersistentControlJournal {
    /// Durably suspends the journal and its owning snapshot publisher.
    ///
    /// The caller must retain the verified external root-owner guard and pass
    /// the publisher used for request admission. A matching call suspends both
    /// immediately, even if it is already cancelled. A foreign publisher is
    /// rejected before either is changed. Previously returned Arc snapshots are
    /// not live grants; composition must still serialize admission/revocation.
    ///
    /// One fixed-size META record binds the exact request and journal header.
    /// Data generation, records, operation history and pending data mutations
    /// are preserved. Existing holds can only be read back, never replaced.
    /// There is no automatic unquarantine, repair or deletion API.
    ///
    /// # Errors
    /// A stale generation or conflicting request returns no durable receipt.
    /// Every failure after write dispatch is outcome-unknown until exact
    /// `recover_quarantine_with_context`. An unreadable META/header cannot be
    /// durably marked here; the caller must keep external admission blocked.
    /// Deadlines are cooperative and cannot preempt a blocked redb/OS call.
    pub fn quarantine_with_context<C: CancellationProbe>(
        &mut self,
        request: &ControlQuarantineRequest,
        publisher: &mut ControlSnapshotPublisher,
        context: &OperationContext<C>,
    ) -> Result<ControlQuarantineReceipt, ControlCallError> {
        let budget = Budget::new(context);
        self.quarantine_checked(request, publisher, Boundary::Normal, &budget)
            .map_err(|error| budget.failure(error, Some(request.operation_id())))
    }

    /// Reopens solely to recover the exact quarantine marker; no usable journal
    /// guard or normal snapshot escapes. Never creates a missing/empty database.
    ///
    /// The caller supplies an admitted existing file and holds root exclusion.
    /// `Some` proves the requested hold is present. `None` means only that this
    /// marker is absent: it does NOT prove the data sound, resolve another
    /// mutation or authorize serving. A contradictory marker fails closed.
    /// This call never clears or rewrites a hold or any application record.
    ///
    /// # Errors
    /// Reopening redb may recover native transaction metadata; this is NOT the
    /// side-effect-free `inspect_journal` port. Interrupted/transient post-open
    /// inspection is outcome-unknown; retry only with exact readback. Corruption
    /// and identity mismatch remain explicit. Full journal inspection and
    /// external owner quarantine for an unreadable database remain separate.
    pub fn recover_quarantine_with_context<C: CancellationProbe>(
        file: File,
        identity: JournalIdentity,
        limits: JournalLimits,
        request: &ControlQuarantineRequest,
        context: &OperationContext<C>,
    ) -> Result<Option<ControlQuarantineReceipt>, ControlCallError> {
        let budget = Budget::new(context);
        recover_checked(file, identity, limits, request, &budget)
            .map_err(|error| budget.failure(error, Some(request.operation_id())).for_recovery())
    }

    pub(super) fn quarantine_checked(
        &mut self,
        request: &ControlQuarantineRequest,
        publisher: &mut ControlSnapshotPublisher,
        boundary: Boundary,
        check: &dyn Check,
    ) -> Result<ControlQuarantineReceipt, ControlError> {
        publisher.begin_disk_publication(self.identity)?;
        self.quarantined = true;
        // The in-memory admission fence is intentionally not undone by failure.
        // self.pending remains the exact earlier data mutation, if any.
        check.check(Point::Start)?;
        if self.pending.is_some_and(|(id, _)| id == request.operation_id()) {
            return Err(ControlError::OperationConflict);
        }
        let read = self.database.begin_read().map_err(|_| ControlError::StoreUnavailable)?;
        let meta = read.open_table(META).map_err(map_table_error)?;
        let (header, header_bytes, existing) = read_metadata(&meta, self.identity, self.limits, check)?;
        publisher.observe_disk_generation(header.generation)?;
        if let Some(marker) = existing {
            require_request(&marker, request)?;
            check.check(Point::ReplayComplete)?;
            return Ok(marker.receipt(self.identity));
        }
        if request.expected_generation() != header.generation {
            return Err(ControlError::GenerationMismatch);
        }
        {
            let operations = read.open_table(OPERATIONS).map_err(map_table_error)?;
            require_unused_operation_id(&operations, request.operation_id())?;
        }
        let marker = Marker::new(*request, &header_bytes);
        let encoded = marker.encode();
        drop(meta);
        drop(read);
        check.check(Point::BeforeWrite)?;
        let mut write = self.database.begin_write().map_err(|_| ControlError::StoreUnavailable)?;
        write.set_durability(Durability::Immediate);
        let staged = (|| {
            check.check(Point::StageRecord)?;
            let mut meta = write.open_table(META).map_err(map_table_error)?;
            let (_, actual_header, existing) = read_metadata(&meta, self.identity, self.limits, check)?;
            if actual_header != header_bytes || existing.is_some() {
                return Err(ControlError::TransactionConflict);
            }
            {
                let operations = write.open_table(OPERATIONS).map_err(map_table_error)?;
                require_unused_operation_id(&operations, request.operation_id())?;
            }
            meta.insert(MARKER_KEY, encoded.as_slice()).map_err(map_storage_error)?;
            drop(meta);
            boundary.before_commit()?;
            check.check(Point::BeforeCommit)
        })();
        if staged.is_err() {
            let _ = write.abort();
            return Err(ControlError::CommitOutcomeUnknown);
        }
        write.commit().map_err(|_| ControlError::CommitOutcomeUnknown)?;
        // No new pending data-mutation ID is fabricated for an administrative
        // hold. All normal work is already blocked and the marker has its own
        // exact readback identity, including across process restart.
        let readback = (|| {
            boundary.after_commit()?;
            check.check(Point::AfterCommit)?;
            let observed = inspect_database(&self.database, self.identity, self.limits, request, check)?
                .ok_or(ControlError::StoreCorrupt)?;
            check.check(Point::MutationComplete)?;
            Ok(observed)
        })();
        let receipt = readback.map_err(|_: ControlError| ControlError::CommitOutcomeUnknown)?;
        self.committed_writes = self.committed_writes.saturating_add(1);
        Ok(receipt)
    }
}

// Normal open/reads do not try to interpret or fix a marker. Even an empty or
// malformed marker blocks admission. Diagnostic recovery performs strict decode.
pub(super) fn require_unquarantined(
    meta: &impl ReadableTable<&'static str, &'static [u8]>,
) -> Result<(), ControlError> {
    if meta.get(MARKER_KEY).map_err(map_storage_error)?.is_some() {
        Err(ControlError::StoreQuarantined)
    } else {
        Ok(())
    }
}

fn read_metadata(
    meta: &(impl ReadableTable<&'static str, &'static [u8]> + ReadableTableMetadata),
    identity: JournalIdentity,
    limits: JournalLimits,
    check: &dyn Check,
) -> Result<(Header, Vec<u8>, Option<Marker>), ControlError> {
    check.check(Point::ReadHeader)?;
    let raw = meta.get("header").map_err(map_storage_error)?.ok_or(ControlError::StoreCorrupt)?;
    // Decode BEFORE copying: Header::decode requires the exact bounded layout.
    let header = Header::decode(raw.value(), identity, limits)?;
    let stored = meta.get(MARKER_KEY).map_err(map_storage_error)?;
    let expected_count = if stored.is_some() { 2 } else { 1 };
    if meta.len().map_err(map_storage_error)? != expected_count {
        return Err(ControlError::StoreCorrupt);
    }
    let marker = stored.map(|bytes| Marker::decode(bytes.value(), raw.value(), header.generation)).transpose()?;
    check.check(Point::ReadComplete)?;
    Ok((header, raw.value().to_vec(), marker))
}

fn require_unused_operation_id(
    operations: &impl ReadableTable<&'static [u8], &'static [u8]>,
    id: MutationId,
) -> Result<(), ControlError> {
    if operations.get(id.0.as_slice()).map_err(map_storage_error)?.is_some() {
        Err(ControlError::OperationConflict)
    } else { Ok(()) }
}

fn require_request(marker: &Marker, request: &ControlQuarantineRequest) -> Result<(), ControlError> {
    if marker.request == *request { Ok(()) } else { Err(ControlError::OperationConflict) }
}

fn inspect_database(
    database: &Database,
    identity: JournalIdentity,
    limits: JournalLimits,
    request: &ControlQuarantineRequest,
    check: &dyn Check,
) -> Result<Option<ControlQuarantineReceipt>, ControlError> {
    let read = database.begin_read().map_err(|_| ControlError::StoreUnavailable)?;
    let meta = read.open_table(META).map_err(map_table_error)?;
    let (_, _, marker) = read_metadata(&meta, identity, limits, check)?;
    let receipt = match marker {
        Some(marker) => {
            require_request(&marker, request)?;
            Some(marker.receipt(identity))
        }
        None => None,
    };
    check.check(Point::RecoveryComplete)?;
    Ok(receipt)
}

fn recover_checked(
    file: File, identity: JournalIdentity, limits: JournalLimits,
    request: &ControlQuarantineRequest, check: &dyn Check,
) -> Result<Option<ControlQuarantineReceipt>, ControlError> {
    // Reuse the existing admitted-file preflight and native reopening path.
    let database = lifecycle::open_existing_database_checked(file, identity, limits, check)?;
    let observed = (|| {
        check.check(Point::AfterOpen)?;
        inspect_database(&database, identity, limits, request, check)
    })();
    match observed {
        Err(error) if is_corruption(error) || error == ControlError::OperationConflict => Err(error),
        Err(_) => Err(ControlError::CommitOutcomeUnknown),
        Ok(receipt) => Ok(receipt),
    }
}

#[cfg(test)]
mod tests;
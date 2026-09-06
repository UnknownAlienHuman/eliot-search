//! One-read-transaction snapshot publication with cooperative admission fencing.

use search_ports::{CancellationProbe, OperationContext};

use super::operation::{Budget, Check, Point};
use super::{
    ControlCallError, ControlCommitReceipt, ControlError, ControlSnapshot,
    ControlSnapshotPublisher, JournalReadSnapshot, MutationId, OPERATIONS,
    PersistentControlJournal, ReadableTable, SnapshotPublishReceipt, StoredOperation,
    map_storage_error, map_table_error, operation_from, rebuild_control_snapshot,
};

impl PersistentControlJournal {
    /// Reconstructs a complete immutable snapshot under one cooperative call budget.
    /// This reads committed state only; it does not open snapshot admission.
    ///
    /// # Errors
    /// Cancellation, deadline or failed validation returns no partial snapshot.
    pub fn control_snapshot_with_context<C: CancellationProbe>(
        &self,
        context: &OperationContext<C>,
    ) -> Result<ControlSnapshot, ControlCallError> {
        let budget = Budget::new(context);
        self.control_snapshot_checked(&budget).map_err(|error| budget.failure(error, None))
    }

    /// Reads back a real current receipt and publishes its exact snapshot.
    ///
    /// A matching publisher is suspended before inspection, including when the
    /// call is already cancelled. Failure leaves admission closed and preserves
    /// the prior pointer privately. Recovery reads disk; it never replays commit.
    /// The final cancellation checkpoint precedes the pointer swap. Once that
    /// publication linearizes, a later cancellation does not undo its success.
    ///
    /// # Errors
    /// Returns typed readback, identity, publication or interruption failures.
    /// No new durable write is dispatched, so cancellation is not evidence of a
    /// new unknown commit. A pending journal mutation must be recovered separately.
    pub fn publish_committed_snapshot_with_context<C: CancellationProbe>(
        &self,
        receipt: &ControlCommitReceipt,
        publisher: &mut ControlSnapshotPublisher,
        context: &OperationContext<C>,
    ) -> Result<SnapshotPublishReceipt, ControlCallError> {
        let budget = Budget::new(context);
        self.publish_snapshot_checked(receipt, publisher, &budget)
            .map_err(|error| budget.failure(error, Some(receipt.operation_id)).for_recovery())
    }

    /// Verifies and republishes current disk state without executing any mutation.
    ///
    /// Generation zero returns `None`, not a fabricated commit receipt. Readback,
    /// reconstruction and the final pointer swap share one cooperative deadline.
    /// Existing synchronous OS/redb calls cannot be preempted by this budget.
    ///
    /// # Errors
    /// Failed or interrupted recovery keeps admission closed. Foreign identities
    /// cannot attach to or unblock a publisher belonging to another journal.
    pub fn recover_snapshot_publication_with_context<C: CancellationProbe>(
        &self,
        publisher: &mut ControlSnapshotPublisher,
        context: &OperationContext<C>,
    ) -> Result<Option<SnapshotPublishReceipt>, ControlCallError> {
        let budget = Budget::new(context);
        self.recover_publication_checked(publisher, &budget)
            .map_err(|error| budget.failure(error, None).for_recovery())
    }

    pub(super) fn control_snapshot_checked(&self, check: &dyn Check) -> Result<ControlSnapshot, ControlError> {
        let readback = self.read_snapshot_checked(check)?;
        self.rebuild_snapshot_checked(readback, check)
    }

    fn rebuild_snapshot_checked(
        &self,
        readback: JournalReadSnapshot,
        check: &dyn Check,
    ) -> Result<ControlSnapshot, ControlError> {
        check.check(Point::SnapshotRebuild)?;
        let snapshot = rebuild_control_snapshot(readback, self.identity, self.limits)?;
        check.check(Point::SnapshotPrepared)?;
        Ok(snapshot)
    }

    pub(super) fn publish_snapshot_checked(
        &self,
        receipt: &ControlCommitReceipt,
        publisher: &mut ControlSnapshotPublisher,
        check: &dyn Check,
    ) -> Result<SnapshotPublishReceipt, ControlError> {
        publisher.begin_disk_publication(self.identity)?;
        self.ensure_available()?;
        check.check(Point::Start)?;
        // Bound untrusted receipt comparison without cloning its key inventory.
        if receipt.changed_keys.is_empty() || receipt.changed_keys.len() > self.limits.max_mutation_items {
            return Err(ControlError::SnapshotPublicationFailed);
        }
        let read = self.database.begin_read().map_err(|_| ControlError::StoreUnavailable)?;
        let header = self.header_from(&read)?;
        publisher.observe_disk_generation(header.generation)?;
        check.check(Point::ReadHeader)?;
        let stored = operation_from(&read, receipt.operation_id, &header, self.limits)?
            .ok_or(ControlError::SnapshotPublicationFailed)?;
        if !same_commit(&stored.receipt, receipt) || receipt.after_generation != header.generation {
            return Err(ControlError::SnapshotPublicationFailed);
        }
        check.check(Point::ReadOperation)?;
        let snapshot = self.rebuild_snapshot_checked(self.snapshot_from_checked(&read, check)?, check)?;
        publisher.publish_verified_disk_snapshot(receipt, snapshot, || check.check(Point::BeforePublish))
    }

    pub(super) fn recover_publication_checked(
        &self,
        publisher: &mut ControlSnapshotPublisher,
        check: &dyn Check,
    ) -> Result<Option<SnapshotPublishReceipt>, ControlError> {
        publisher.begin_disk_publication(self.identity)?;
        self.ensure_available()?;
        check.check(Point::Start)?;
        let read = self.database.begin_read().map_err(|_| ControlError::StoreUnavailable)?;
        // Retain the highest actually observed generation even if a later record
        // or ledger read fails; never lower the fence to a caller-supplied value.
        let header = self.header_from(&read)?;
        publisher.observe_disk_generation(header.generation)?;
        check.check(Point::ReadHeader)?;
        let snapshot = self.verify_from_checked(&read, check)?;
        if snapshot.generation == 0 {
            publisher.finish_verified_empty_disk(self.identity, || check.check(Point::BeforePublish))?;
            return Ok(None);
        }
        let table = read.open_table(OPERATIONS).map_err(map_table_error)?;
        for row in table.iter().map_err(map_storage_error)? {
            check.check(Point::ReadOperation)?;
            let (id, bytes) = row.map_err(map_storage_error)?;
            let id = MutationId(id.value().try_into().map_err(|_| ControlError::StoreCorrupt)?);
            let operation = StoredOperation::decode(bytes.value(), id, snapshot.generation, self.limits)?;
            if operation.receipt.after_generation == snapshot.generation {
                let state = self.rebuild_snapshot_checked(snapshot, check)?;
                return publisher.publish_verified_disk_snapshot(
                    &operation.receipt, state, || check.check(Point::BeforePublish),
                ).map(Some);
            }
        }
        Err(ControlError::SnapshotPublicationFailed)
    }
}

fn same_commit(stored: &ControlCommitReceipt, supplied: &ControlCommitReceipt) -> bool {
    // Replay is invocation-local, not part of the persisted commit identity.
    stored.operation_id == supplied.operation_id
        && stored.command_digest == supplied.command_digest
        && stored.before_generation == supplied.before_generation
        && stored.after_generation == supplied.after_generation
        && stored.changed_keys == supplied.changed_keys
}

#[cfg(test)]
mod tests;

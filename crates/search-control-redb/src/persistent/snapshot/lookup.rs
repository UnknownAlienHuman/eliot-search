//! Current published metadata lookup, without reconstructing the whole journal.

use search_ports::{CancellationProbe, OperationContext};

use crate::{ControlCallError, ControlError, ControlKey, ControlSnapshotPublisher,
    ControlValue, PersistentControlJournal};
use super::super::operation::{Budget, Check, Point};
use super::super::{OPERATIONS, RECORDS, ReadableTableMetadata, map_storage_error, map_table_error};

impl PersistentControlJournal {
    /// Read one bounded value from a disk-published snapshot after checking the
    /// actual current journal header in a read-only transaction.
    ///
    /// A retained historical Arc, an unbound reference-model publisher, a pending
    /// commit or publication, and a snapshot older than an unpublished commit
    /// cannot satisfy this read. No record/receipt scan, write, publication or
    /// recovery is performed. Absence requires a verified current snapshot or
    /// completed native generation-zero recovery with a still-empty disk journal.
    /// A newly constructed or suspended publisher is never sufficient. This read
    /// does not initialize a row, fabricate a receipt or provide default authority.
    ///
    /// The caller must hold the live root/domain owner lock. Shared borrows keep
    /// this journal and publisher unchanged during the call, not after return.
    /// The returned metadata is not a source-access permit.
    ///
    /// # Errors
    /// Returns the original context interruption or identity/publication/store
    /// failure. A failed read never repairs or republishes an old snapshot.
    pub fn read_published_record<C: CancellationProbe>(
        &self,
        publisher: &ControlSnapshotPublisher,
        key: &ControlKey,
        context: &OperationContext<C>,
    ) -> Result<Option<ControlValue>, ControlCallError> {
        let (_, [value]) = self.read_published_fixed(publisher, [key], context)?;
        Ok(value)
    }

    /// Read two distinct technical records and their common journal generation.
    ///
    /// Both lookups use one verified disk header and one published snapshot (or
    /// verified empty generation zero); neither row is fetched from another
    /// generation. Values retain input key order, including explicit absence.
    /// Count, key, per-value and aggregate
    /// byte limits apply before results escape. No table or receipt scan occurs.
    /// The caller still holds the actual owner lock through any subsequent use.
    /// A successful read is metadata, not publication, a lease or source authority.
    ///
    /// # Errors
    /// Preserves native read errors; duplicate keys and exhausted bounds refuse
    /// the whole read without returning either partial value or a generation.
    pub fn read_published_record_pair<C: CancellationProbe>(
        &self,
        publisher: &ControlSnapshotPublisher,
        keys: [&ControlKey; 2],
        context: &OperationContext<C>,
    ) -> Result<(u64, [Option<ControlValue>; 2]), ControlCallError> {
        self.read_published_fixed(publisher, keys, context)
    }

    // Only the one- and two-record entrypoints instantiate this bounded reader.
    fn read_published_fixed<C: CancellationProbe, const N: usize>(
        &self,
        publisher: &ControlSnapshotPublisher,
        keys: [&ControlKey; N],
        context: &OperationContext<C>,
    ) -> Result<(u64, [Option<ControlValue>; N]), ControlCallError> {
        let budget = Budget::new(context);
        let read = || {
            budget.check(Point::Start)?;
            self.ensure_available()?;
            if N == 0 || N > self.limits.max_mutation_items {
                return Err(ControlError::BudgetExceeded);
            }
            for (index, &key) in keys.iter().enumerate() {
                budget.check(Point::ReadRecord)?;
                if key.as_bytes().is_empty() || key.as_bytes().len() > self.limits.max_key_bytes {
                    return Err(ControlError::InvalidKey);
                }
                if keys[..index].contains(&key) {
                    return Err(ControlError::DuplicateMutationKey);
                }
            }
            if publisher.diagnostic_disk_identity() != Some(self.identity) {
                return Err(ControlError::IdentityMismatch);
            }
            let snapshot = publisher.current();
            if let Some(snapshot) = &snapshot {
                if snapshot.identity != self.identity {
                    return Err(ControlError::IdentityMismatch);
                }
            } else if !publisher.has_verified_empty_disk(self.identity) {
                return Err(ControlError::SnapshotPublicationFailed);
            }
            let transaction = self.database.begin_read().map_err(|_| ControlError::StoreUnavailable)?;
            let header = self.header_from(&transaction)?;
            budget.check(Point::ReadHeader)?;
            let mut values: [Option<ControlValue>; N] = std::array::from_fn(|_| None);
            let Some(snapshot) = snapshot else {
                // A first registration has no mutation receipt or Arc snapshot.
                // Never mistake an unpublished first commit for empty state.
                if header.generation != 0 {
                    return Err(ControlError::SnapshotPublicationFailed);
                }
                // Header decoding requires zero counts/bytes at generation zero.
                // Verify actual table cardinalities too, in THIS read transaction,
                // so hidden/corrupt rows cannot be reported as verified absence.
                let records = transaction.open_table(RECORDS).map_err(map_table_error)?;
                if records.len().map_err(|error| map_storage_error(&error))? != 0 {
                    return Err(ControlError::StoreCorrupt);
                }
                budget.check(Point::ReadRecord)?;
                let operations = transaction.open_table(OPERATIONS).map_err(map_table_error)?;
                if operations.len().map_err(|error| map_storage_error(&error))? != 0 {
                    return Err(ControlError::StoreCorrupt);
                }
                budget.check(Point::ReadComplete)?;
                return Ok((0, values));
            };
            if header.generation != snapshot.generation {
                return Err(ControlError::SnapshotPublicationFailed);
            }
            let mut total_bytes = 0_usize;
            for (slot, key) in values.iter_mut().zip(keys) {
                budget.check(Point::ReadRecord)?;
                if let Ok(index) = snapshot.records.binary_search_by(|(found, _)| found.cmp(key)) {
                    let value = &snapshot.records[index].1;
                    if value.is_empty() || value.len() > self.limits.max_value_bytes {
                        return Err(ControlError::StoreCorrupt);
                    }
                    total_bytes = total_bytes.checked_add(value.len()).ok_or(ControlError::BudgetExceeded)?;
                    if total_bytes > self.limits.max_total_value_bytes {
                        return Err(ControlError::BudgetExceeded);
                    }
                    *slot = Some(value.clone());
                }
            }
            budget.check(Point::ReadComplete)?;
            Ok((header.generation, values))
        };
        read().map_err(|error| budget.failure(error, None))
    }
}

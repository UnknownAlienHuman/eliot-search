//! Current published metadata lookup, without reconstructing the whole journal.

use search_ports::{CancellationProbe, OperationContext};

use crate::{ControlCallError, ControlError, ControlKey, ControlSnapshotPublisher,
    ControlValue, PersistentControlJournal};
use super::super::operation::{Budget, Check, Point};
use super::super::{
    OPERATIONS, RECORDS, ReadableTable, ReadableTableMetadata, decode_value,
    map_storage_error, map_table_error,
};

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
        let (_, [value]) = self.read_published_records(publisher, [key], context)?;
        Ok(value)
    }

    /// Read two distinct technical records and their common journal generation.
    ///
    /// Both lookups use one verified disk header and one published snapshot (or
    /// verified empty generation zero); neither row is fetched from another
    /// generation. Values retain input key order, including explicit absence.
    /// Count, key, per-value and aggregate byte limits apply before results escape.
    /// No table or receipt scan occurs. A successful read is metadata, not
    /// publication, a lease or source authority.
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
        self.read_published_records(publisher, keys, context)
    }

    /// Read one finite set from the current durable journal head for recovery.
    ///
    /// Unlike [`PersistentControlJournal::read_published_records`], this method
    /// does not consult or advance the admission snapshot. It verifies the native
    /// journal header and reads exact keys from one read-only transaction so an
    /// interrupted operation can be reconstructed before guarded publication.
    /// Returned values are recovery metadata only and MUST NOT authorize serving.
    ///
    /// The journal must have no in-process pending transaction; an existing owner
    /// resolves that through its exact command first. After process restart there
    /// is no volatile pending flag, so the durable operation ledger and records
    /// remain authoritative. No table scan, write, repair or snapshot swap occurs.
    ///
    /// # Errors
    /// Invalid/duplicate keys, unavailable/quarantined state, corruption,
    /// cancellation or aggregate-byte exhaustion return no partial observations.
    pub fn read_current_records_for_recovery<C: CancellationProbe, const N: usize>(
        &self,
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
            let transaction = self.database.begin_read()
                .map_err(|_| ControlError::StoreUnavailable)?;
            let header = self.header_from(&transaction)?;
            budget.check(Point::ReadHeader)?;
            let records = transaction.open_table(RECORDS).map_err(map_table_error)?;
            let mut values: [Option<ControlValue>; N] = std::array::from_fn(|_| None);
            let mut total_bytes = 0_usize;
            for (slot, key) in values.iter_mut().zip(keys) {
                budget.check(Point::ReadRecord)?;
                if let Some(raw) = records.get(key.as_bytes())
                    .map_err(|error| map_storage_error(&error))?
                {
                    let value = decode_value(raw.value(), self.limits)?;
                    total_bytes = total_bytes.checked_add(value.len())
                        .ok_or(ControlError::BudgetExceeded)?;
                    if total_bytes > self.limits.max_total_value_bytes {
                        return Err(ControlError::BudgetExceeded);
                    }
                    *slot = Some(value);
                }
            }
            budget.check(Point::ReadComplete)?;
            Ok((header.generation, values))
        };
        read().map_err(|error| budget.failure(error, None).for_recovery())
    }

    /// Read one finite fixed-size set of distinct technical records and the exact
    /// disk-published generation that supplied all of them.
    ///
    /// This is the shared bounded primitive behind the one- and two-record APIs.
    /// Every key is validated before the read. One verified native header and one
    /// immutable snapshot supply all values in caller order; explicit absence is
    /// retained. The total cloned value bytes may not exceed the journal aggregate
    /// ceiling. Generation-zero absence still requires completed empty recovery
    /// and actual empty record/operation tables in this same native transaction.
    ///
    /// The const count is part of the caller's code, not untrusted input. `N == 0`,
    /// duplicate keys and counts above the mutation-item ceiling fail closed. The
    /// caller must retain the actual owner lock through subsequent interpretation.
    ///
    /// # Errors
    /// Returns the original bounded read/publication/store failure without partial
    /// values. This operation never repairs, republishes, writes or scans a table.
    pub fn read_published_records<C: CancellationProbe, const N: usize>(
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
                if header.generation != 0 {
                    return Err(ControlError::SnapshotPublicationFailed);
                }
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

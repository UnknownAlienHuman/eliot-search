//! Current published metadata lookup, without reconstructing the whole journal.

use search_ports::{CancellationProbe, OperationContext};

use crate::{ControlCallError, ControlError, ControlKey, ControlSnapshotPublisher,
    ControlValue, PersistentControlJournal};
use super::super::operation::{Budget, Check, Point};

impl PersistentControlJournal {
    /// Read one bounded value from a disk-published snapshot after checking the
    /// actual current journal header in a read-only transaction.
    ///
    /// A retained historical Arc, an unbound reference-model publisher, a pending
    /// commit or publication, and a snapshot older than an unpublished commit
    /// cannot satisfy this read. No record/receipt scan, write, publication or
    /// recovery is performed. Absence is returned only from a verified current
    /// snapshot; it does not initialize a row or provide default authority.
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
    /// Both lookups use one verified disk header and one published snapshot;
    /// neither row is fetched from another generation. Values retain input key
    /// order, including explicit absence. Count, key, per-value and aggregate
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
            let snapshot = publisher.current().ok_or(ControlError::SnapshotPublicationFailed)?;
            if snapshot.identity != self.identity {
                return Err(ControlError::IdentityMismatch);
            }
            let transaction = self.database.begin_read().map_err(|_| ControlError::StoreUnavailable)?;
            let header = self.header_from(&transaction)?;
            budget.check(Point::ReadHeader)?;
            if header.generation != snapshot.generation {
                return Err(ControlError::SnapshotPublicationFailed);
            }
            let mut values: [Option<ControlValue>; N] = std::array::from_fn(|_| None);
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

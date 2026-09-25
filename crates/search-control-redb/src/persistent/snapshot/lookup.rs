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
        let budget = Budget::new(context);
        let read = || {
            budget.check(Point::Start)?;
            self.ensure_available()?;
            if key.as_bytes().is_empty() || key.as_bytes().len() > self.limits.max_key_bytes {
                return Err(ControlError::InvalidKey);
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
            let value = match snapshot.records.binary_search_by(|(found, _)| found.cmp(key)) {
                Ok(index) => {
                    let value = &snapshot.records[index].1;
                    if value.is_empty() || value.len() > self.limits.max_value_bytes {
                        return Err(ControlError::StoreCorrupt);
                    }
                    Some(value.clone())
                }
                Err(_) => None,
            };
            budget.check(Point::ReadComplete)?;
            Ok(value)
        };
        read().map_err(|error| budget.failure(error, None))
    }
}

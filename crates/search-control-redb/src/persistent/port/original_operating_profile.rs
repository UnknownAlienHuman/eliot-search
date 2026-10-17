//! Read the persisted operating profile under the original native allowance.

use super::super::{bootstrap::OriginalCheck, operation::Check};
use super::BoundControlJournal;
use crate::{JournalOperatingProfile, MigrationBootstrapFailure, MigrationBootstrapWork};
use search_ports::CancellationProbe;
use std::time::Instant;

impl<C: CancellationProbe> BoundControlJournal<'_, C> {
    /// Reads the immutable operating profile using the caller's original deadline and work callback.
    ///
    /// This is a read-only observation of the existing journal. It neither creates a new budget nor
    /// changes the operation ledger.
    ///
    /// # Errors
    /// Unavailable, uninitialized, corrupt, interrupted or over-budget state refuses.
    pub fn read_operating_profile_with_original_context<F, E>(
        &self,
        original_deadline: Instant,
        work: &F,
    ) -> Result<JournalOperatingProfile, MigrationBootstrapFailure<E>>
    where
        F: Fn(MigrationBootstrapWork) -> Result<(), E>,
    {
        let check = OriginalCheck::registry(original_deadline, work);
        let result = self
            .journal
            .ensure_available()
            .and_then(|()| self.journal.profile_checked(&check as &dyn Check));
        result.map_err(|error| check.failure(error))
    }
}

//! Read-only publication references under the caller's original allowance.

use super::super::{bootstrap::OriginalCheck, operation::Check};
use super::BoundControlJournal;
use crate::{MigrationBootstrapFailure, MigrationBootstrapWork, PublicationProjectionSnapshot};
use search_ports::CancellationProbe;
use std::time::Instant;

impl<C: CancellationProbe> BoundControlJournal<'_, C> {
    /// Reads committed visibility, manifest references and shadows coherently.
    ///
    /// All nested work uses the original deadline and work callback. No durable
    /// query job, snapshot publication, recovery or write is performed. References
    /// still require C07/source verification and live C18 authorization.
    /// # Errors
    /// Pending, unresolved, corrupt, interrupted or foreign-owner state returns
    /// no partial snapshot. Only a genuinely empty journal can return `None`.
    pub fn read_publication_projection_snapshot_with_original_context<F, E>(
        &self,
        deadline: Instant,
        work: &F,
    ) -> Result<Option<PublicationProjectionSnapshot>, MigrationBootstrapFailure<E>>
    where
        F: Fn(MigrationBootstrapWork) -> Result<(), E>,
    {
        let check = OriginalCheck::registry(deadline, work);
        self.journal
            .read_projection_snapshot_checked(&check as &dyn Check)
            .map_err(|error| check.failure(error))
    }
}

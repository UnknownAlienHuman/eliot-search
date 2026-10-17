//! Read-only recovery projection through the existing borrowed control owner.

use super::super::{bootstrap::OriginalCheck, operation::Check};
use super::BoundControlJournal;
use crate::{
    MigrationBootstrapFailure, MigrationBootstrapWork, PublicationRecoveryProjectionSnapshot,
};
use search_ports::CancellationProbe;
use std::time::Instant;

impl<C: CancellationProbe> BoundControlJournal<'_, C> {
    /// Reads unresolved publication metadata and exact projection references from one redb snapshot.
    ///
    /// The call uses this already-bound journal and the original deadline/work allowance. Its
    /// result is technical recovery input only; it does not make unresolved work serveable or
    /// grant publication, source, C07, or query authority. Only a genuinely empty journal returns
    /// `None`.
    ///
    /// # Errors
    /// Missing-after-write, corrupt, foreign-owner, unsupported-profile and interrupted reads fail.
    pub fn read_publication_recovery_projection_snapshot_with_original_context<F, E>(
        &self,
        deadline: Instant,
        work: &F,
    ) -> Result<Option<PublicationRecoveryProjectionSnapshot>, MigrationBootstrapFailure<E>>
    where
        F: Fn(MigrationBootstrapWork) -> Result<(), E>,
    {
        let check = OriginalCheck::registry(deadline, work);
        self.journal
            .read_recovery_projection_snapshot_checked(&check as &dyn Check)
            .map_err(|error| check.failure(error))
    }
}

//! Original-context C02 membership-exclusion fence operations.

use super::super::{
    bootstrap::OriginalCheck,
    operation::{Check, Point},
};
use super::BoundControlJournal;
use crate::{
    CommitRecoveryDecision, ControlError, MigrationBootstrapFailure,
    MigrationBootstrapWork as Work, PublicationAbandonFenceReadbackV1,
    PublicationAbandonFenceRecordV1, PublicationAbandonFenceRequestV1,
};
use search_ports::CancellationProbe;
use std::time::Instant;

impl<C: CancellationProbe> BoundControlJournal<'_, C> {
    /// Commits the complete affected-membership shadow set under the supplied
    /// original deadline/work allowance and returns exact same-transaction
    /// recovery-plane readback. It leaves visibility epoch, route, receipt and
    /// reservation history unchanged.
    ///
    /// # Errors
    /// Conflicting intent/guards/shadows, incomplete bounds, interruption and
    /// ambiguous commit remain explicit; callers must reconcile the same request.
    pub fn transact_publication_abandon_fence_with_original_context<F, E>(
        &mut self,
        request: &PublicationAbandonFenceRequestV1,
        deadline: Instant,
        work: &F,
    ) -> Result<PublicationAbandonFenceReadbackV1, MigrationBootstrapFailure<E>>
    where
        F: Fn(Work) -> Result<(), E>,
    {
        let check = OriginalCheck::registry(deadline, work);
        self.journal
            .abandon_publication_checked(request, super::super::Boundary::Normal, &check)
            .map_err(|error| check.failure(error))
    }

    /// Resolves the exact original C02 fence request without building a new ID
    /// or replaying a changed membership set.
    ///
    /// # Errors
    /// Changed command, corrupt state and interrupted recovery remain explicit.
    pub fn recover_publication_abandon_fence_with_original_context<F, E>(
        &mut self,
        request: &PublicationAbandonFenceRequestV1,
        deadline: Instant,
        work: &F,
    ) -> Result<CommitRecoveryDecision, MigrationBootstrapFailure<E>>
    where
        F: Fn(Work) -> Result<(), E>,
    {
        let check = OriginalCheck::registry(deadline, work);
        let result = self
            .journal
            .recover_abandon_publication_checked(request, &check);
        match result {
            Ok(decision) => Ok(decision),
            Err(error) => {
                let error = if check.interrupted() {
                    ControlError::CommitOutcomeUnknown
                } else {
                    error
                };
                Err(check.failure(error))
            }
        }
    }

    /// Re-reads the exact retained fence and actual current shadow map through
    /// the original allowance after a lost acknowledgement or restart.
    ///
    /// # Errors
    /// Missing/replaced fence, a changed member shadow or corrupt operation
    /// receipt never returns a successful readback.
    pub fn read_publication_abandon_fence_with_original_context<F, E>(
        &self,
        record: &PublicationAbandonFenceRecordV1,
        deadline: Instant,
        work: &F,
    ) -> Result<PublicationAbandonFenceReadbackV1, MigrationBootstrapFailure<E>>
    where
        F: Fn(Work) -> Result<(), E>,
    {
        let check = OriginalCheck::registry(deadline, work);
        if let Err(error) = check.check(Point::Start) {
            return Err(check.failure(error));
        }
        self.journal
            .read_abandonment_fence_checked(record, &check)
            .map_err(|error| check.failure(error))
    }
}

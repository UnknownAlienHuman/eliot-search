//! Read the persisted operating profile under the original native allowance.

use super::super::{Boundary, bootstrap::OriginalCheck, operation::Check};
use super::BoundControlJournal;
use crate::{
    CommitRecoveryDecision, ControlCommitReceipt, ControlError, IndexedProfileAdoption,
    JournalOperatingProfile, MigrationBootstrapFailure, MigrationBootstrapWork, MutationId,
    PublicationVisibilityState,
};
use search_contracts::Blake3Digest32;
use search_ports::CancellationProbe;
use std::time::Instant;

impl<C: CancellationProbe> BoundControlJournal<'_, C> {
    /// Reads the complete retained original adoption before Root draws any new
    /// operation or collection-generation identity. `Some` retains the original
    /// operation, digest, DIRECT predecessor, generation and initial visibility.
    /// Only verified `None` permits preparing a fresh adoption. A pending unknown
    /// must first recover its retained request; it is never reported as absence.
    ///
    /// # Errors
    /// Non-DIRECT genesis, pending/corrupt state, or original-context refusal
    /// returns no adoption or absence observation. No durable write is performed.
    pub fn read_indexed_profile_adoption_with_original_context<F, E>(
        &self,
        original_deadline: Instant,
        work: &F,
    ) -> Result<Option<IndexedProfileAdoption>, MigrationBootstrapFailure<E>>
    where
        F: Fn(MigrationBootstrapWork) -> Result<(), E>,
    {
        let check = OriginalCheck::registry(original_deadline, work);
        self.journal
            .read_indexed_profile_adoption_checked(&check)
            .map_err(|error| check.failure(error))
    }

    /// Prepares an exact adoption using the original absolute deadline and
    /// cumulative work/cancellation callback. Existing adoption must match the
    /// entire original input; its original predecessor is never recaptured.
    ///
    /// # Errors
    /// Substituted identity/seed, non-DIRECT genesis and interrupted reads refuse.
    pub fn prepare_indexed_profile_adoption_with_original_context<F, E>(
        &self,
        initial: PublicationVisibilityState,
        operation: MutationId,
        digest: Blake3Digest32,
        original_deadline: Instant,
        work: &F,
    ) -> Result<IndexedProfileAdoption, MigrationBootstrapFailure<E>>
    where
        F: Fn(MigrationBootstrapWork) -> Result<(), E>,
    {
        let check = OriginalCheck::registry(original_deadline, work);
        self.journal
            .prepare_indexed_profile_adoption_checked(initial, operation, digest, &check)
            .map_err(|error| check.failure(error))
    }

    /// Applies the retained original adoption through the existing conditional
    /// transaction engine, with no new deadline, operation or store. Snapshot
    /// admission remains suspended until separate exact snapshot publication.
    ///
    /// # Errors
    /// Stale/conflicting input refuses; any possible commit requires recovery of
    /// this same request. No fresh request is substituted after an unknown result.
    pub fn adopt_indexed_profile_with_original_context<F, E>(
        &mut self,
        request: &IndexedProfileAdoption,
        original_deadline: Instant,
        work: &F,
    ) -> Result<ControlCommitReceipt, MigrationBootstrapFailure<E>>
    where
        F: Fn(MigrationBootstrapWork) -> Result<(), E>,
    {
        let check = OriginalCheck::registry(original_deadline, work);
        let result = self
            .journal
            .adopt_indexed_checked(request, Boundary::Normal, &check);
        self.finish_dispatch(result)
            .map_err(|error| check.failure(error))
    }

    /// Resolves only the retained original adoption using its exact mutation,
    /// digest, predecessor and conditions, including on the same pending handle.
    /// This inspects disk and never replays a write or allocates replacement IDs.
    ///
    /// # Errors
    /// Changed input cannot clear pending work. Interrupted recovery remains
    /// outcome-unknown and cannot reopen snapshot admission.
    pub fn recover_indexed_profile_adoption_with_original_context<F, E>(
        &mut self,
        request: &IndexedProfileAdoption,
        original_deadline: Instant,
        work: &F,
    ) -> Result<CommitRecoveryDecision, MigrationBootstrapFailure<E>>
    where
        F: Fn(MigrationBootstrapWork) -> Result<(), E>,
    {
        let check = OriginalCheck::registry(original_deadline, work);
        let result = self
            .journal
            .recover_indexed_profile_adoption_checked(request, &check);
        self.finish_dispatch(result).map_err(|error| {
            let error = if check.interrupted() {
                ControlError::CommitOutcomeUnknown
            } else {
                error
            };
            check.failure(error)
        })
    }

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

//! Minimal borrowed original-context seam for capability-owned record paths.
//!
//! A capability module (for example `security_restriction`) owns its record
//! semantics, but the checked read/commit/recovery engines and their `Check`
//! budget trait stay private to this module tree. This seam exposes only those
//! three existing checked engine calls plus explicit checkpoints and
//! byte-proportional reservations, so an additive original-context variant can
//! meter its own key construction, codec and clone work without widening engine
//! visibility and without adding a second journal, record map or context.
//!
//! The borrowed allowance is the caller's single original absolute deadline and
//! cumulative work callback. It is never renewed, cloned or reset here, and the
//! first callback refusal is preserved for the returned failure value.

use std::time::Instant;

use super::{
    Boundary, CommitRecoveryDecision, ControlCommitReceipt, ControlError, JournalReadSnapshot,
    PersistentControlJournal, bootstrap::OriginalCheck, operation::{Check, Point},
};
use crate::{
    ConditionalControlMutation, MigrationBootstrapFailure, MigrationBootstrapWork,
};

/// Borrowed original-context allowance over one capability-owned record path.
///
/// Every checkpoint and reservation consumes the caller's single cumulative
/// allowance. The first callback refusal is retained for the returned
/// [`MigrationBootstrapFailure`] and is never replaced by a later cause.
pub struct OriginalSeam<'a, F, E> {
    check: OriginalCheck<'a, F, E>,
}

impl<'a, F, E> OriginalSeam<'a, F, E> {
    /// Binds the caller's original absolute deadline and work callback.
    pub const fn new(deadline: Instant, work: &'a F) -> Self {
        Self { check: OriginalCheck::registry(deadline, work) }
    }

    /// Consumes the seam into its first-refusal-preserving failure value.
    pub fn failure(self, control: ControlError) -> MigrationBootstrapFailure<E> {
        self.check.failure(control)
    }

    /// Whether the allowance already stopped on a refusal or elapsed deadline.
    #[must_use]
    pub const fn interrupted(&self) -> bool { self.check.interrupted() }
}

impl<F, E> OriginalSeam<'_, F, E>
where
    F: Fn(MigrationBootstrapWork) -> Result<(), E>,
{
    /// Entry checkpoint taken before any capability-owned preparation.
    ///
    /// # Errors
    /// Returns the original callback refusal or elapsed-deadline classification.
    pub fn start(&self) -> Result<(), ControlError> {
        Check::check(&self.check, Point::Start)
    }

    /// Completion checkpoint taken after a bounded read and its codec work.
    ///
    /// # Errors
    /// Returns the original callback refusal or elapsed-deadline classification.
    pub fn complete(&self) -> Result<(), ControlError> {
        Check::check(&self.check, Point::ReadComplete)
    }

    /// Reserves concrete finite work before the caller performs it.
    ///
    /// # Errors
    /// Returns the original callback refusal or elapsed-deadline classification.
    pub fn reserve(&self, work: MigrationBootstrapWork) -> Result<(), ControlError> {
        Check::reserve(&self.check, work)
    }

    /// One coherent checked read snapshot on this exact journal.
    ///
    /// # Errors
    /// Returns the existing checked read, identity or quarantine classification.
    pub fn read_snapshot(
        &self,
        journal: &PersistentControlJournal,
    ) -> Result<JournalReadSnapshot, ControlError> {
        journal.read_snapshot_checked(&self.check)
    }

    /// The existing conditional transaction engine under this allowance.
    ///
    /// # Errors
    /// Returns the existing checked engine classification. A possible write
    /// stays outcome-unknown and requires the exact same request's recovery.
    pub fn transact(
        &self,
        journal: &mut PersistentControlJournal,
        command: ConditionalControlMutation,
    ) -> Result<ControlCommitReceipt, ControlError> {
        journal.transact_conditionally_checked(command, Boundary::Normal, &self.check)
    }

    /// Resolves the identical conditional request without replaying any write.
    ///
    /// # Errors
    /// Returns the existing checked recovery classification. A foreign request
    /// stays conflicting and an interrupted inspection stays unresolved.
    pub fn recover(
        &self,
        journal: &mut PersistentControlJournal,
        command: &ConditionalControlMutation,
    ) -> Result<CommitRecoveryDecision, ControlError> {
        journal.recover_transaction_with_conditions_checked(
            command.mutation(),
            command.conditions(),
            &self.check,
        )
    }
}

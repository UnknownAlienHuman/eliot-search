//! Operation-context entrypoints share the exact existing transaction engine.

use search_ports::{CancellationProbe, OperationContext};

use super::operation::{Budget, Check, Point};
use super::{
    Boundary, CommitRecoveryDecision, ControlCallError, ControlCommitReceipt, ControlError,
    ControlMutation, JournalReadSnapshot, PersistentControlJournal, is_corruption,
    operation_from, request_fingerprint, transaction, validate_mutation,
};

impl PersistentControlJournal {
    /// Reads a snapshot with the supplied cancellation and relative deadline.
    ///
    /// The deadline starts once at entry and is not reset for table scans. No
    /// partial snapshot is returned. Checks are cooperative: synchronous redb
    /// or OS I/O cannot be preempted; expiration is detected when control returns.
    /// Existing journal byte/item limits still apply; `budget_ref` is not decoded.
    ///
    /// # Errors
    /// Returns cancellation, deadline, quarantine or a typed storage failure.
    pub fn read_snapshot_with_context<C: CancellationProbe>(
        &self,
        context: &OperationContext<C>,
    ) -> Result<JournalReadSnapshot, ControlCallError> {
        let budget = Budget::new(context);
        self.read_snapshot_checked(&budget).map_err(|error| budget.failure(error, None))
    }

    /// Verifies the full bounded record/receipt inventory under one call deadline.
    ///
    /// # Errors
    /// Cancellation, expiration or failed reads return no partial verification.
    /// Cooperative checks cannot interrupt a synchronous OS call in progress.
    pub fn verify_with_context<C: CancellationProbe>(
        &self,
        context: &OperationContext<C>,
    ) -> Result<JournalReadSnapshot, ControlCallError> {
        let budget = Budget::new(context);
        self.verify_checked(&budget).map_err(|error| budget.failure(error, None))
    }

    /// Applies the exact mutation under a cooperative cancellation/deadline budget.
    ///
    /// Before write dispatch, interruption has no committed effect. Afterwards,
    /// staged state is explicitly aborted where possible, but the exact request
    /// stays pending until fresh authoritative recovery, even if abort succeeds.
    /// Interruption after commit is also an unknown outcome. No background
    /// worker, hidden retry, new operation ID or alternate transaction path exists.
    ///
    /// # Errors
    /// An abort failure or interruption after write-transaction dispatch returns
    /// `CommitOutcomeUnknown` and requires readback before retry. A blocking redb
    /// or OS operation is not forcibly terminated at the cooperative deadline.
    pub fn transact_with_context<C: CancellationProbe>(
        &mut self,
        mutation: ControlMutation,
        context: &OperationContext<C>,
    ) -> Result<ControlCommitReceipt, ControlCallError> {
        let budget = Budget::new(context);
        let id = mutation.id();
        self.transact_checked(mutation, Boundary::Normal, &budget)
            .map_err(|error| budget.failure(error, Some(id)))
    }

    /// Resolves the exact request with a fresh budget, never re-executing its write.
    ///
    /// A cancelled, timed-out or transiently failed inspection cannot establish
    /// corruption or clear the pending fence. Actual structural corruption still
    /// quarantines. Every positive/negative recovery decision requires a complete
    /// readback; an interrupted inspection remains an unknown mutation outcome.
    ///
    /// # Errors
    /// Failed inspection returns a typed error and leaves pending state intact.
    /// Interruption is `CommitOutcomeUnknown`, not proof of rollback or damage.
    pub fn recover_transaction_with_context<C: CancellationProbe>(
        &mut self,
        mutation: &ControlMutation,
        context: &OperationContext<C>,
    ) -> Result<CommitRecoveryDecision, ControlCallError> {
        let budget = Budget::new(context);
        self.recover_transaction_checked(mutation, &budget).map_err(|error| {
            let error = if budget.interrupted() { ControlError::CommitOutcomeUnknown } else { error };
            budget.failure(error, Some(mutation.id())).for_recovery()
        })
    }

    pub(super) fn read_snapshot_checked(&self, check: &dyn Check) -> Result<JournalReadSnapshot, ControlError> {
        self.ensure_available()?;
        check.check(Point::Start)?;
        let read = self.database.begin_read().map_err(|_| ControlError::StoreUnavailable)?;
        self.snapshot_from_checked(&read, check)
    }

    pub(super) fn verify_checked(&self, check: &dyn Check) -> Result<JournalReadSnapshot, ControlError> {
        self.ensure_available()?;
        check.check(Point::Start)?;
        let read = self.database.begin_read().map_err(|_| ControlError::StoreUnavailable)?;
        self.verify_from_checked(&read, check)
    }

    pub(super) fn recover_transaction_checked(
        &mut self,
        mutation: &ControlMutation,
        check: &dyn Check,
    ) -> Result<CommitRecoveryDecision, ControlError> {
        if self.quarantined { return Ok(CommitRecoveryDecision::PartialOrCorruptQuarantine); }
        check.check(Point::Start)?;
        let changed_keys = validate_mutation(mutation, self.limits)?;
        let fingerprint = request_fingerprint(self.identity, mutation)?;
        check.check(Point::Validated)?;
        if self.pending.is_some_and(|pending| pending != (mutation.id(), fingerprint)) {
            return Ok(CommitRecoveryDecision::ConflictingInput);
        }
        let result: Result<CommitRecoveryDecision, ControlError> = (|| {
            let read = self.database.begin_read().map_err(|_| ControlError::StoreUnavailable)?;
            self.verify_from_checked(&read, check)?;
            let header = self.header_from(&read)?;
            check.check(Point::ReadHeader)?;
            let result = match operation_from(&read, mutation.id(), &header, self.limits)? {
                Some(operation) if operation.request_sha256 == fingerprint => {
                    transaction::verify_replay(
                        self, &read, &header, &operation.receipt, mutation, &changed_keys, check,
                    )?;
                    let mut receipt = operation.receipt;
                    receipt.replayed = true;
                    CommitRecoveryDecision::Committed(receipt)
                }
                Some(_) => CommitRecoveryDecision::ConflictingInput,
                None => CommitRecoveryDecision::NotCommittedRetrySameOperation,
            };
            // Covers empty inventories and historical/no-op recovery too.
            check.check(Point::RecoveryComplete)?;
            Ok(result)
        })();
        match result {
            Err(error) if is_corruption(error) => {
                self.quarantined = true;
                Ok(CommitRecoveryDecision::PartialOrCorruptQuarantine)
            }
            Err(error) => Err(error), // Preserve the pending fence, not a corruption verdict.
            Ok(CommitRecoveryDecision::ConflictingInput) => Ok(CommitRecoveryDecision::ConflictingInput),
            Ok(result) => {
                self.pending = None;
                Ok(result)
            }
        }
    }
}

#[cfg(test)]
mod tests;

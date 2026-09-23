//! One cooperative deadline across native reads, commit, publication and owners.

use std::time::{Duration, Instant};
use search_ports::{CancellationProbe, OperationContext};
use super::NativeSecurityError;

pub(super) struct MutationBudget<'a, C: CancellationProbe> {
    original: &'a OperationContext<C>,
    deadline: Instant,
}

impl<'a, C: CancellationProbe + Clone> MutationBudget<'a, C> {
    pub(super) fn new(original: &'a OperationContext<C>) -> Result<Self, NativeSecurityError> {
        let deadline = Instant::now()
            .checked_add(Duration::from_millis(original.relative_deadline_ms().get()))
            .ok_or(NativeSecurityError::DeadlineElapsed)?;
        let budget = Self { original, deadline };
        budget.check()?;
        Ok(budget)
    }

    pub(super) fn check(&self) -> Result<(), NativeSecurityError> {
        if self.original.cancellation().is_cancelled() {
            return Err(NativeSecurityError::Cancelled);
        }
        if Instant::now() >= self.deadline { return Err(NativeSecurityError::DeadlineElapsed); }
        Ok(())
    }

    pub(super) fn context(&self) -> Result<OperationContext<C>, NativeSecurityError> {
        self.check()?;
        // Round down, never give each stage a fresh original timeout. Less
        // than one millisecond is refused rather than widening the deadline.
        let left = self.deadline.saturating_duration_since(Instant::now()).as_millis();
        let left = u64::try_from(left).map_err(|_| NativeSecurityError::DeadlineElapsed)?;
        OperationContext::new(
            self.original.request_id(), left, self.original.cancellation().clone(),
            self.original.budget_ref().clone(),
        ).map_err(|_| NativeSecurityError::DeadlineElapsed)
    }
}

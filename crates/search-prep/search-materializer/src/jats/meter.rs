//! One cumulative original operation meter; logical admission is not CPU/RSS.
use super::model::{JatsError, JatsLimits};
use super::profile::JatsOperationLimits;
use crate::request::CancellationToken;
use std::mem::size_of;
use std::time::Instant;

pub(super) struct Meter<'a> {
    pub(super) limits: JatsOperationLimits,
    pub(super) operation: Option<(CancellationToken<'a>, Instant)>,
    pub(super) work: usize,
    pub(super) owned: usize,
    pub(super) canonical: usize,
}
impl Meter<'_> {
    pub(super) fn poll(&self) -> Result<(), JatsError> {
        if let Some((cancel, deadline)) = self.operation {
            if cancel.is_cancelled() {
                return Err(JatsError::Cancelled);
            }
            if Instant::now() >= deadline {
                return Err(JatsError::DeadlineExceeded);
            }
        }
        Ok(())
    }
    pub(super) fn work(&mut self, amount: usize) -> Result<(), JatsError> {
        self.poll()?;
        self.work = debit(self.work, amount, self.limits.parser.work_steps)?;
        Ok(())
    }
    pub(super) fn own(&mut self, amount: usize) -> Result<(), JatsError> {
        self.poll()?;
        self.owned = debit(self.owned, amount, self.limits.parser.owned_bytes)?;
        Ok(())
    }
    pub(super) fn push<T>(
        &mut self,
        out: &mut Vec<T>,
        item: T,
        cap: usize,
    ) -> Result<(), JatsError> {
        if out.len() >= cap {
            return Err(JatsError::LimitExceeded);
        }
        self.own(size_of::<T>())?;
        self.work(1)?;
        out.try_reserve_exact(1)
            .map_err(|_| JatsError::AllocationFailed)?;
        out.push(item);
        Ok(())
    }
    pub(super) fn copy(&mut self, value: &str) -> Result<String, JatsError> {
        self.own(value.len())?;
        self.work(value.len())?;
        let mut out = String::new();
        out.try_reserve_exact(value.len())
            .map_err(|_| JatsError::AllocationFailed)?;
        out.push_str(value);
        Ok(out)
    }
    pub(super) fn complete(&mut self, actual: usize) -> Result<(), JatsError> {
        self.work(actual)?;
        self.canonical = debit(self.canonical, actual, self.limits.total_canonical_bytes)?;
        Ok(())
    }
    pub(super) fn canonical_cap(&self) -> usize {
        self.limits
            .canonical_preimage_bytes
            .min(self.limits.total_canonical_bytes - self.canonical)
            .min(self.limits.parser.work_steps - self.work)
    }
    pub(super) const fn parser(&self) -> JatsLimits {
        self.limits.parser
    }
}
pub(super) fn debit(used: usize, amount: usize, cap: usize) -> Result<usize, JatsError> {
    let result = used.checked_add(amount).ok_or(JatsError::Overflow)?;
    if result > cap {
        return Err(JatsError::LimitExceeded);
    }
    Ok(result)
}

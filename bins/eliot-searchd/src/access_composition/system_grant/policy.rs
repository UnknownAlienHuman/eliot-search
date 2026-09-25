//! Policy lifetime checks share the existing native UTC rollback fence.

use std::time::SystemTime;

use search_contracts::UtcTimestamp;

use search_provider_protocol::MonotonicMillis;

use super::{GrantUseError, SystemGrantClock, monotonic_millis,
    system_time_from_timestamp, timestamp_from_system_time};

impl SystemGrantClock {
    pub(in crate::access_composition) fn check_policy_window(
        &mut self,
        issued_at: &UtcTimestamp,
        expires_at: Option<&UtcTimestamp>,
    ) -> Result<Option<MonotonicMillis>, GrantUseError> {
        let started = monotonic_millis();
        let now = SystemTime::now();
        if self.last_observed.as_ref().is_some_and(|previous| &now < previous) {
            return Err(GrantUseError::ClockUnavailable);
        }
        let observed = timestamp_from_system_time(&now)
            .map_err(|_| GrantUseError::ClockUnavailable)?;
        // Remember even a refused forward observation; expiry cannot be undone
        // by subsequently moving the wall clock backwards.
        self.last_observed = Some(now);
        if &observed < issued_at || expires_at.is_some_and(|end| &observed >= end) {
            return Err(GrantUseError::Expired);
        }
        let Some(expires_at) = expires_at else { return Ok(None); };
        let left = system_time_from_timestamp(expires_at)?.duration_since(now)
            .map_err(|_| GrantUseError::Expired)?;
        let millis = u64::try_from(left.as_millis()).map_err(|_| GrantUseError::ClockUnavailable)?;
        let until = started.get().checked_add(millis).map(MonotonicMillis::new)
            .ok_or(GrantUseError::ClockUnavailable)?;
        if monotonic_millis() >= until { return Err(GrantUseError::Expired); }
        Ok(Some(until))
    }
}

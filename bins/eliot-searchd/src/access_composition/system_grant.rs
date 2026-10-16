//! Production entropy and canonical wall-clock adapters for standalone grants.
//!
//! This module supplies only identity bytes and one finite UTC window. It owns
//! no binding, policy, connection or grant authority; those remain mandatory
//! inputs to `mint_standalone_grant`.

#![allow(clippy::module_name_repetitions)]

mod policy;

use std::time::{Duration, SystemTime};

use search_contracts::UtcTimestamp;
use search_provider_protocol::MonotonicMillis;

use eliot_searchd::native_time::{
    NativeTimeError, civil_from_unix_days as civil_days,
    system_time_from_timestamp as system_time_from_canonical,
    timestamp_from_system_time as timestamp_from_instant,
};
use crate::provider_composition::monotonic_millis;

use super::grant::{
    BoundedStandaloneGrantIssuer, GrantEntropySource, GrantIssuerError,
    GrantTimeSource, GrantTimeWindow, GrantUseError, GrantValidationClock,
};

/// Daemon-qualified operating-system entropy adapter.
///
/// Every call is a fresh exact OS CSPRNG read through the daemon's single
/// native entropy owner. Empty output buffers fail closed.
#[derive(Clone, Copy, Debug, Default)]
pub struct QualifiedGrantEntropy;

impl GrantEntropySource for QualifiedGrantEntropy {
    fn fill_random(
        &mut self,
        output: &mut [u8],
    ) -> Result<(), GrantIssuerError> {
        crate::qualified_entropy::fill_qualified_entropy(output)
            .map_err(|_| GrantIssuerError::Unavailable)
    }
}

/// Canonical UTC wall-clock adapter.
///
/// One `SystemTime` observation determines both issue and expiry timestamps.
/// Sub-microsecond precision is truncated consistently because the shared
/// contract admits exactly six fractional digits. A process-local rollback
/// fence rejects a wall clock that moves behind the preceding observation.
#[derive(Clone, Debug, Default)]
pub struct SystemGrantClock {
    last_observed: Option<SystemTime>,
}

impl SystemGrantClock {
    fn issue_window_from(
        &mut self,
        now: SystemTime,
        requested_ttl_ms: u64,
    ) -> Result<GrantTimeWindow, GrantIssuerError> {
        if self
            .last_observed
            .as_ref()
            .is_some_and(|previous| &now < previous)
        {
            return Err(GrantIssuerError::Unavailable);
        }
        let window = issue_window_at(&now, requested_ttl_ms)?;
        self.last_observed = Some(now);
        Ok(window)
    }
}

impl GrantTimeSource for SystemGrantClock {
    fn issue_window(
        &mut self,
        requested_ttl_ms: u64,
    ) -> Result<GrantTimeWindow, GrantIssuerError> {
        let started = monotonic_millis();
        self.issue_window_from(SystemTime::now(), requested_ttl_ms)?
            .with_monotonic_origin(started)
    }
}

impl GrantValidationClock for SystemGrantClock {
    fn check_grant_window(
        &mut self,
        issued_at: &UtcTimestamp,
        expires_at: &UtcTimestamp,
        monotonic_expiry: MonotonicMillis,
    ) -> Result<MonotonicMillis, GrantUseError> {
        let started = monotonic_millis();
        if started >= monotonic_expiry { return Err(GrantUseError::Expired); }
        let now = SystemTime::now();
        if self.last_observed.as_ref().is_some_and(|previous| &now < previous) {
            return Err(GrantUseError::ClockUnavailable);
        }
        let timestamp = timestamp_from_system_time(&now)
            .map_err(|_| GrantUseError::ClockUnavailable)?;
        // Retain even a refused forward observation: clock rollback must not
        // resurrect an expired grant. Never modify its original native expiry.
        self.last_observed = Some(now);
        if &timestamp < issued_at || &timestamp >= expires_at {
            return Err(GrantUseError::Expired);
        }
        let wall_remaining = system_time_from_timestamp(expires_at)?
            .duration_since(now).map_err(|_| GrantUseError::Expired)?;
        let millis = u64::try_from(wall_remaining.as_millis())
            .map_err(|_| GrantUseError::ClockUnavailable)?;
        let wall_deadline = started.get().checked_add(millis)
            .map(MonotonicMillis::new).ok_or(GrantUseError::ClockUnavailable)?;
        let valid_until = monotonic_expiry.min(wall_deadline);
        if monotonic_millis() >= valid_until { return Err(GrantUseError::Expired); }
        Ok(valid_until)
    }
}

// The date algorithm lives in the one shared `crate::native_time` module. These
// three wrappers keep this adapter's original refusals and call shapes exactly:
// a refused conversion is still an unavailable issuer or an unavailable clock.

const fn issuer_unavailable(_: NativeTimeError) -> GrantIssuerError {
    GrantIssuerError::Unavailable
}

const fn clock_unavailable(_: NativeTimeError) -> GrantUseError {
    GrantUseError::ClockUnavailable
}

// Inverse of the shared formatter, only for an already validated canonical UTC
// value. No second public timestamp parser is introduced and nothing outside
// the shared UtcTimestamp contract is accepted. Production timestamps are
// post-epoch.
fn system_time_from_timestamp(value: &UtcTimestamp) -> Result<SystemTime, GrantUseError> {
    system_time_from_canonical(value).map_err(clock_unavailable)
}

/// Concrete boot-local issuer using the daemon-qualified OS CSPRNG and clock.
pub type ProductionStandaloneGrantIssuer =
    BoundedStandaloneGrantIssuer<QualifiedGrantEntropy, SystemGrantClock>;

/// Builds an empty finite production standalone-grant issuer.
///
/// This does not authorize a request or expose a wire operation. The caller
/// must still supply one authenticated binding/policy capture to
/// `mint_standalone_grant`.
///
/// # Errors
///
/// Returns [`GrantIssuerError::CapacityExceeded`] when either finite issuer
/// bound is zero.
pub fn production_standalone_grant_issuer(
    max_records: usize,
    max_entropy_attempts: usize,
) -> Result<ProductionStandaloneGrantIssuer, GrantIssuerError> {
    BoundedStandaloneGrantIssuer::new(
        QualifiedGrantEntropy,
        SystemGrantClock::default(),
        max_records,
        max_entropy_attempts,
    )
}

fn issue_window_at(
    now: &SystemTime,
    requested_ttl_ms: u64,
) -> Result<GrantTimeWindow, GrantIssuerError> {
    if requested_ttl_ms == 0 {
        return Err(GrantIssuerError::Unavailable);
    }
    let expires = now
        .checked_add(Duration::from_millis(requested_ttl_ms))
        .ok_or(GrantIssuerError::Unavailable)?;
    GrantTimeWindow::new(
        timestamp_from_system_time(now)?,
        timestamp_from_system_time(&expires)?,
        requested_ttl_ms,
    )
}

fn timestamp_from_system_time(
    instant: &SystemTime,
) -> Result<UtcTimestamp, GrantIssuerError> {
    timestamp_from_instant(*instant).map_err(issuer_unavailable)
}

fn civil_from_unix_days(
    days: u64,
) -> Result<(u32, u32, u32), GrantIssuerError> {
    civil_days(days).map_err(issuer_unavailable)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::UNIX_EPOCH;

    #[test]
    fn canonical_clock_formats_epoch_and_leap_day() {
        let epoch = timestamp_from_system_time(&UNIX_EPOCH).expect("epoch");
        assert_eq!(epoch.as_str(), "1970-01-01T00:00:00.000000Z");

        let leap = UNIX_EPOCH
            .checked_add(Duration::from_micros(951_827_696_789_123))
            .expect("leap instant")
            .checked_add(Duration::from_nanos(999))
            .expect("sub-microsecond precision");
        let leap = timestamp_from_system_time(&leap).expect("leap timestamp");
        assert_eq!(leap.as_str(), "2000-02-29T12:34:56.789123Z");
    }

    #[test]
    fn one_observation_produces_exact_finite_window() {
        let now = UNIX_EPOCH
            .checked_add(Duration::from_micros(951_827_696_789_123))
            .expect("fixed instant");
        let window = issue_window_at(&now, 1_500).expect("window");
        assert_eq!(
            window.issued_at().as_str(),
            "2000-02-29T12:34:56.789123Z"
        );
        assert_eq!(
            window.expires_at().as_str(),
            "2000-02-29T12:34:58.289123Z"
        );
        assert_eq!(window.effective_ttl_ms(), 1_500);
    }

    #[test]
    fn zero_ttl_and_out_of_contract_year_fail_closed() {
        assert_eq!(
            issue_window_at(&UNIX_EPOCH, 0),
            Err(GrantIssuerError::Unavailable)
        );
        assert_eq!(
            civil_from_unix_days(2_932_897),
            Err(GrantIssuerError::Unavailable)
        );
    }

    #[test]
    fn clock_rollback_fails_closed() {
        let later = UNIX_EPOCH
            .checked_add(Duration::from_secs(2))
            .expect("later instant");
        let earlier = UNIX_EPOCH
            .checked_add(Duration::from_secs(1))
            .expect("earlier instant");
        let mut clock = SystemGrantClock::default();
        clock
            .issue_window_from(later, 1_000)
            .expect("first observation");
        assert_eq!(
            clock.issue_window_from(earlier, 1_000),
            Err(GrantIssuerError::Unavailable)
        );
    }

    #[test]
    fn production_factory_retains_declared_capacity() {
        let issuer =
            production_standalone_grant_issuer(2, 4).expect("issuer");
        assert_eq!(issuer.retained_operations(), 0);
        assert!(matches!(
            production_standalone_grant_issuer(0, 4),
            Err(GrantIssuerError::CapacityExceeded)
        ));
    }
}

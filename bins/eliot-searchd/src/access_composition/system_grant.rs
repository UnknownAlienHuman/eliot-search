//! Production entropy and canonical wall-clock adapters for standalone grants.
//!
//! This module supplies only identity bytes and one finite UTC window. It owns
//! no binding, policy, connection or grant authority; those remain mandatory
//! inputs to `mint_standalone_grant`.

#![allow(clippy::module_name_repetitions)]

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use search_contracts::UtcTimestamp;
use search_provider_protocol::MonotonicMillis;

use crate::provider_composition::monotonic_millis;

use super::grant::{
    BoundedStandaloneGrantIssuer, GrantEntropySource, GrantIssuerError,
    GrantTimeSource, GrantTimeWindow, GrantUseError, GrantValidationClock,
};

const SECONDS_PER_DAY: u64 = 86_400;
const SECONDS_PER_HOUR: u64 = 3_600;
const SECONDS_PER_MINUTE: u64 = 60;
const CIVIL_EPOCH_OFFSET_DAYS: u128 = 719_468;
const DAYS_PER_ERA: u128 = 146_097;

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

// Inverse of this module's formatter, only for an already validated canonical
// UTC value. Do not introduce a second public timestamp parser or accept input
// outside the shared UtcTimestamp contract. Production timestamps are post-epoch.
fn system_time_from_timestamp(value: &UtcTimestamp) -> Result<SystemTime, GrantUseError> {
    fn decimal(bytes: &[u8], start: usize, end: usize) -> Result<u64, GrantUseError> {
        bytes.get(start..end).ok_or(GrantUseError::ClockUnavailable)?.iter()
            .try_fold(0_u64, |number, byte| {
                let digit = byte.checked_sub(b'0').filter(|digit| *digit <= 9)
                    .ok_or(GrantUseError::ClockUnavailable)?;
                number.checked_mul(10).and_then(|n| n.checked_add(u64::from(digit)))
                    .ok_or(GrantUseError::ClockUnavailable)
            })
    }
    let bytes = value.as_str().as_bytes();
    let year = decimal(bytes, 0, 4)?;
    let month = decimal(bytes, 5, 7)?;
    let day = decimal(bytes, 8, 10)?;
    let prior_year = year.checked_sub(1).ok_or(GrantUseError::ClockUnavailable)?;
    const MONTH_OFFSETS: [u64; 12] = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];
    const UNIX_DAYS: u64 = 365 * 1969 + 1969 / 4 - 1969 / 100 + 1969 / 400;
    let month_index = usize::try_from(month.checked_sub(1).ok_or(GrantUseError::ClockUnavailable)?)
        .map_err(|_| GrantUseError::ClockUnavailable)?;
    let offset = *MONTH_OFFSETS.get(month_index).ok_or(GrantUseError::ClockUnavailable)?;
    let leap_day = u64::from(month > 2 && year % 4 == 0 && (year % 100 != 0 || year % 400 == 0));
    let days = (365 * prior_year + prior_year / 4 - prior_year / 100 + prior_year / 400
        + offset + leap_day + day.checked_sub(1).ok_or(GrantUseError::ClockUnavailable)?)
        .checked_sub(UNIX_DAYS).ok_or(GrantUseError::ClockUnavailable)?;
    // The shared four-digit year bound keeps these products within u64.
    let seconds = days * SECONDS_PER_DAY + decimal(bytes, 11, 13)? * SECONDS_PER_HOUR
        + decimal(bytes, 14, 16)? * SECONDS_PER_MINUTE + decimal(bytes, 17, 19)?;
    UNIX_EPOCH.checked_add(Duration::from_secs(seconds))
        .and_then(|time| time.checked_add(Duration::from_micros(decimal(bytes, 20, 26).ok()?)))
        .ok_or(GrantUseError::ClockUnavailable)
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
    let since_epoch = instant
        .duration_since(UNIX_EPOCH)
        .map_err(|_| GrantIssuerError::Unavailable)?;
    let seconds = since_epoch.as_secs();
    let days = seconds / SECONDS_PER_DAY;
    let second_of_day = seconds % SECONDS_PER_DAY;
    let (year, month, day) = civil_from_unix_days(days)?;
    let hour = second_of_day / SECONDS_PER_HOUR;
    let minute =
        (second_of_day % SECONDS_PER_HOUR) / SECONDS_PER_MINUTE;
    let second = second_of_day % SECONDS_PER_MINUTE;
    let micros = since_epoch.subsec_micros();
    UtcTimestamp::parse(format!(
        "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{micros:06}Z"
    ))
    .map_err(|_| GrantIssuerError::Unavailable)
}

fn civil_from_unix_days(
    days: u64,
) -> Result<(u32, u32, u32), GrantIssuerError> {
    let shifted = u128::from(days)
        .checked_add(CIVIL_EPOCH_OFFSET_DAYS)
        .ok_or(GrantIssuerError::Unavailable)?;
    let era = shifted / DAYS_PER_ERA;
    let day_of_era = shifted % DAYS_PER_ERA;
    let year_of_era = (
        day_of_era
            - day_of_era / 1_460
            + day_of_era / 36_524
            - day_of_era / 146_096
    ) / 365;
    let mut year = year_of_era
        .checked_add(
            era.checked_mul(400)
                .ok_or(GrantIssuerError::Unavailable)?,
        )
        .ok_or(GrantIssuerError::Unavailable)?;
    let day_of_year = day_of_era
        - (
            365 * year_of_era
                + year_of_era / 4
                - year_of_era / 100
        );
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = if month_prime < 10 {
        month_prime + 3
    } else {
        month_prime - 9
    };
    if month <= 2 {
        year = year
            .checked_add(1)
            .ok_or(GrantIssuerError::Unavailable)?;
    }
    if !(1..=9_999).contains(&year) {
        return Err(GrantIssuerError::Unavailable);
    }
    Ok((
        u32::try_from(year).map_err(|_| GrantIssuerError::Unavailable)?,
        u32::try_from(month).map_err(|_| GrantIssuerError::Unavailable)?,
        u32::try_from(day).map_err(|_| GrantIssuerError::Unavailable)?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

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

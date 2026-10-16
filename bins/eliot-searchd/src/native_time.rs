//! One pure UTC/civil conversion shared by native wall-clock observations.
//!
//! The standalone grant window and the Native retention observation must not
//! drift into two date algorithms. This module owns only the conversion between
//! `SystemTime` and the shared canonical `UtcTimestamp` contract. It owns no
//! clock, authority, grant, lease, deadline, identity or persisted state, and
//! every caller keeps its own observation and refusal semantics.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use search_contracts::UtcTimestamp;

const SECONDS_PER_DAY: u64 = 86_400;
const SECONDS_PER_HOUR: u64 = 3_600;
const SECONDS_PER_MINUTE: u64 = 60;
const CIVIL_EPOCH_OFFSET_DAYS: u128 = 719_468;
const DAYS_PER_ERA: u128 = 146_097;

/// Closed failure of one pure UTC conversion.
///
/// The value is deliberately empty: it carries no instant, path, credential or
/// caller refusal. Each caller maps it to its own existing closed refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NativeTimeError {
    /// The instant or value is outside the shared canonical UTC contract.
    OutOfContract,
}

/// Formats one post-epoch instant as the shared canonical UTC timestamp.
///
/// Sub-microsecond precision is truncated consistently because the shared
/// contract admits exactly six fractional digits.
///
/// # Errors
///
/// Returns [`NativeTimeError::OutOfContract`] for a pre-epoch instant, an
/// overflowing civil computation, or a year outside the four-digit contract.
pub(crate) fn timestamp_from_system_time(instant: SystemTime) -> Result<UtcTimestamp, NativeTimeError> {
    let since_epoch = instant
        .duration_since(UNIX_EPOCH)
        .map_err(|_| NativeTimeError::OutOfContract)?;
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
    .map_err(|_| NativeTimeError::OutOfContract)
}

/// Civil `(year, month, day)` for whole days elapsed since the Unix epoch.
///
/// Howard Hinnant's `civil_from_days` algorithm over a 400-year era, retained
/// inside the shared four-digit year contract.
///
/// # Errors
///
/// Returns [`NativeTimeError::OutOfContract`] on any checked-arithmetic
/// overflow or a civil year outside `1..=9999`.
pub(crate) fn civil_from_unix_days(days: u64) -> Result<(u32, u32, u32), NativeTimeError> {
    let shifted = u128::from(days)
        .checked_add(CIVIL_EPOCH_OFFSET_DAYS)
        .ok_or(NativeTimeError::OutOfContract)?;
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
                .ok_or(NativeTimeError::OutOfContract)?,
        )
        .ok_or(NativeTimeError::OutOfContract)?;
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
            .ok_or(NativeTimeError::OutOfContract)?;
    }
    if !(1..=9_999).contains(&year) {
        return Err(NativeTimeError::OutOfContract);
    }
    Ok((
        u32::try_from(year).map_err(|_| NativeTimeError::OutOfContract)?,
        u32::try_from(month).map_err(|_| NativeTimeError::OutOfContract)?,
        u32::try_from(day).map_err(|_| NativeTimeError::OutOfContract)?,
    ))
}

/// Inverse formatter for an already validated canonical UTC value.
///
/// Only for a value already accepted by the shared `UtcTimestamp` contract.
/// This introduces no second public timestamp parser and accepts nothing
/// outside that contract; production timestamps are post-epoch.
///
/// # Errors
///
/// Returns [`NativeTimeError::OutOfContract`] for a non-decimal field or an
/// instant that cannot be represented after the Unix epoch.
pub(crate) fn system_time_from_timestamp(value: &UtcTimestamp) -> Result<SystemTime, NativeTimeError> {
    const MONTH_OFFSETS: [u64; 12] = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];
    const UNIX_DAYS: u64 = 365 * 1969 + 1969 / 4 - 1969 / 100 + 1969 / 400;
    fn decimal(bytes: &[u8], start: usize, end: usize) -> Result<u64, NativeTimeError> {
        bytes.get(start..end).ok_or(NativeTimeError::OutOfContract)?.iter()
            .try_fold(0_u64, |number, byte| {
                let digit = byte.checked_sub(b'0').filter(|digit| *digit <= 9)
                    .ok_or(NativeTimeError::OutOfContract)?;
                number.checked_mul(10).and_then(|n| n.checked_add(u64::from(digit)))
                    .ok_or(NativeTimeError::OutOfContract)
            })
    }
    let bytes = value.as_str().as_bytes();
    let year = decimal(bytes, 0, 4)?;
    let month = decimal(bytes, 5, 7)?;
    let day = decimal(bytes, 8, 10)?;
    let prior_year = year.checked_sub(1).ok_or(NativeTimeError::OutOfContract)?;
    let month_index = usize::try_from(month.checked_sub(1).ok_or(NativeTimeError::OutOfContract)?)
        .map_err(|_| NativeTimeError::OutOfContract)?;
    let offset = *MONTH_OFFSETS.get(month_index).ok_or(NativeTimeError::OutOfContract)?;
    let leap_day = u64::from(month > 2 && year % 4 == 0 && (year % 100 != 0 || year % 400 == 0));
    let days = (365 * prior_year + prior_year / 4 - prior_year / 100 + prior_year / 400
        + offset + leap_day + day.checked_sub(1).ok_or(NativeTimeError::OutOfContract)?)
        .checked_sub(UNIX_DAYS).ok_or(NativeTimeError::OutOfContract)?;
    // The shared four-digit year bound keeps these products within u64.
    let seconds = days * SECONDS_PER_DAY + decimal(bytes, 11, 13)? * SECONDS_PER_HOUR
        + decimal(bytes, 14, 16)? * SECONDS_PER_MINUTE + decimal(bytes, 17, 19)?;
    UNIX_EPOCH.checked_add(Duration::from_secs(seconds))
        .and_then(|time| time.checked_add(Duration::from_micros(decimal(bytes, 20, 26).ok()?)))
        .ok_or(NativeTimeError::OutOfContract)
}

#[cfg(test)]
mod tests {
    use super::*;

    const LEAP_INSTANT_MICROS: u64 = 951_827_696_789_123;

    #[test]
    fn canonical_formatter_formats_epoch_and_leap_day() {
        let epoch = timestamp_from_system_time(UNIX_EPOCH).expect("epoch");
        assert_eq!(epoch.as_str(), "1970-01-01T00:00:00.000000Z");

        let leap = UNIX_EPOCH
            .checked_add(Duration::from_micros(LEAP_INSTANT_MICROS))
            .expect("leap instant")
            .checked_add(Duration::from_nanos(999))
            .expect("sub-microsecond precision");
        let leap = timestamp_from_system_time(leap).expect("leap timestamp");
        assert_eq!(leap.as_str(), "2000-02-29T12:34:56.789123Z");
    }

    #[test]
    fn out_of_contract_year_and_pre_epoch_instants_fail_closed() {
        assert_eq!(
            civil_from_unix_days(2_932_897),
            Err(NativeTimeError::OutOfContract)
        );
        assert_eq!(
            timestamp_from_system_time(
                UNIX_EPOCH
                    .checked_sub(Duration::from_secs(1))
                    .expect("pre-epoch instant")
            ),
            Err(NativeTimeError::OutOfContract)
        );
    }

    #[test]
    fn canonical_value_round_trips_through_the_inverse() {
        let instant = UNIX_EPOCH
            .checked_add(Duration::from_micros(LEAP_INSTANT_MICROS))
            .expect("fixed instant");
        let timestamp = timestamp_from_system_time(instant).expect("timestamp");
        let inverse = system_time_from_timestamp(&timestamp).expect("inverse");
        assert_eq!(inverse, instant);
        assert_eq!(
            timestamp_from_system_time(inverse).expect("re-format").as_str(),
            timestamp.as_str()
        );
    }
}

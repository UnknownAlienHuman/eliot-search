//! Exact lifetime arithmetic over contract-validated, microsecond UTC timestamps.
//! No clock, timezone lookup, allocation, or external date dependency is needed.

use search_contracts::UtcTimestamp;

pub(super) fn fits_ttl(
    created_at: &UtcTimestamp,
    expires_at: &UtcTimestamp,
    ttl_millis: u64,
) -> bool {
    duration_micros(created_at, expires_at).is_some_and(|micros| {
        // Do not floor to milliseconds: one extra microsecond exceeds the cap.
        // Widen before multiplication so even u64::MAX remains an exact bound.
        micros > 0 && u128::from(micros) <= u128::from(ttl_millis) * 1_000
    })
}

pub(super) fn duration_micros(start: &UtcTimestamp, end: &UtcTimestamp) -> Option<u64> {
    timestamp_micros(end)?.checked_sub(timestamp_micros(start)?)
}

fn timestamp_micros(value: &UtcTimestamp) -> Option<u64> {
    // UtcTimestamp owns validation: years 0001..9999, real Gregorian dates,
    // seconds 00..59, and exactly six fractional digits followed by Z. This
    // projection must not accept alternate timestamp spellings or leap seconds.
    let text = value.as_str();
    let field = |range: core::ops::Range<usize>| -> Option<u64> {
        text.get(range)?.parse().ok()
    };
    let year = field(0..4)?;
    let month = field(5..7)?;
    let day = field(8..10)?;
    let hour = field(11..13)?;
    let minute = field(14..16)?;
    let second = field(17..19)?;
    let micros = field(20..26)?;

    let previous_year = year.checked_sub(1)?;
    let days_before_year = previous_year
        .checked_mul(365)?
        .checked_add(previous_year / 4)?
        .checked_sub(previous_year / 100)?
        .checked_add(previous_year / 400)?;
    let month_index = usize::try_from(month.checked_sub(1)?).ok()?;
    let days_before_month = *MONTH_STARTS.get(month_index)?;
    let leap_day = u64::from(
        month > 2
            && year.is_multiple_of(4)
            && (!year.is_multiple_of(100) || year.is_multiple_of(400)),
    );
    let days = days_before_year
        .checked_add(days_before_month)?
        .checked_add(leap_day)?
        .checked_add(day.checked_sub(1)?)?;
    days.checked_mul(24)?
        .checked_add(hour)?
        .checked_mul(60)?
        .checked_add(minute)?
        .checked_mul(60)?
        .checked_add(second)?
        .checked_mul(1_000_000)?
        .checked_add(micros)
}

const MONTH_STARTS: [u64; 12] = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];

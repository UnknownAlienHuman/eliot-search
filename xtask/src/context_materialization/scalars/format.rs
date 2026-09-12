//! Closed scalar grammar predicates.

/// Lowercase SHA-256 hex grammar.
#[must_use]
pub fn sha256_hex_valid(value: &str) -> bool {
    crate::ticket_planner::sha256_hex_valid(value)
}

/// `^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$`.
#[must_use]
pub fn opaque_id_valid(value: &str) -> bool {
    crate::ticket_planner::opaque_id_valid(value)
}

/// `actor:(user|service|reviewer|integration):<opaque>`.
#[must_use]
pub fn actor_identity_valid(value: &str) -> bool {
    crate::ticket_planner::actor_identity_valid(value)
}

pub(super) const RFC3339_DIGIT_POSITIONS: [usize; 14] =
    [0, 1, 2, 3, 5, 6, 8, 9, 11, 12, 14, 15, 17, 18];

const fn is_leap_year(year: i32) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

const fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap_year(year) {
                29
            } else {
                28
            }
        }
        _ => 0,
    }
}

/// Whole-second UTC RFC3339 shape plus calendar validation.
#[must_use]
pub fn rfc3339_valid(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 20 {
        return false;
    }
    for position in RFC3339_DIGIT_POSITIONS {
        if !bytes[position].is_ascii_digit() {
            return false;
        }
    }
    if bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes[10] != b'T'
        || bytes[13] != b':'
        || bytes[16] != b':'
        || bytes[19] != b'Z'
    {
        return false;
    }
    let number = |from: usize, length: usize| -> u32 {
        let mut result = 0_u32;
        for byte in &bytes[from..from + length] {
            result = result * 10 + u32::from(byte - b'0');
        }
        result
    };
    let year = number(0, 4);
    let month = number(5, 2);
    let day = number(8, 2);
    let hour = number(11, 2);
    let minute = number(14, 2);
    let second = number(17, 2);
    if !(1..=9999).contains(&year) || !(1..=12).contains(&month) {
        return false;
    }
    let year_i32 = i32::try_from(year).unwrap_or(0);
    if day < 1 || day > days_in_month(year_i32, month) {
        return false;
    }
    if hour > 23 || minute > 59 || second > 59 {
        return false;
    }

    let mut rendered = [0_u8; 20];
    let push_two = |buffer: &mut [u8; 20], at: usize, number: u32| {
        buffer[at] = b'0' + u8::try_from(number / 10).unwrap_or(9);
        buffer[at + 1] = b'0' + u8::try_from(number % 10).unwrap_or(9);
    };
    let year_bytes = format!("{year:04}");
    rendered[0..4].copy_from_slice(year_bytes.as_bytes());
    rendered[4] = b'-';
    push_two(&mut rendered, 5, month);
    rendered[7] = b'-';
    push_two(&mut rendered, 8, day);
    rendered[10] = b'T';
    push_two(&mut rendered, 11, hour);
    rendered[13] = b':';
    push_two(&mut rendered, 14, minute);
    rendered[16] = b':';
    push_two(&mut rendered, 17, second);
    rendered[19] = b'Z';
    rendered == *bytes
}

//! Bounded line framing for the local control protocol.
//!
//! A rejected frame terminates the session. Its unread suffix must never be
//! interpreted as a second command. The limit excludes LF or CRLF framing.

use std::fmt;
use std::io::{self, BufRead, Read};

#[derive(Debug)]
pub(crate) enum LineError {
    InvalidLimit,
    TooLarge,
    InvalidUtf8,
    Io(io::Error),
}

impl LineError {
    pub(crate) const fn code(&self) -> &'static str {
        match self {
            Self::InvalidLimit => "COMMAND_LIMIT_INVALID",
            Self::TooLarge => "COMMAND_TOO_LARGE",
            Self::InvalidUtf8 => "COMMAND_INVALID_UTF8",
            Self::Io(_) => "COMMAND_READ_FAILED",
        }
    }
}

impl fmt::Display for LineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for LineError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

/// Reads at most `max_bytes + 2` bytes, before allocating the complete frame.
/// A final unterminated frame is accepted, matching the former line protocol.
pub(crate) fn read_line(
    input: &mut impl BufRead,
    max_bytes: usize,
) -> Result<Option<String>, LineError> {
    if max_bytes == 0 {
        return Err(LineError::InvalidLimit);
    }
    let read_limit = max_bytes
        .checked_add(2)
        .and_then(|value| u64::try_from(value).ok())
        .ok_or(LineError::InvalidLimit)?;
    let mut bytes = Vec::new();
    let count = Read::take(&mut *input, read_limit)
        .read_until(b'\n', &mut bytes)
        .map_err(LineError::Io)?;
    if count == 0 {
        return Ok(None);
    }
    if bytes.last() == Some(&b'\n') {
        bytes.pop();
        if bytes.last() == Some(&b'\r') {
            bytes.pop();
        }
    }
    if bytes.len() > max_bytes {
        return Err(LineError::TooLarge);
    }
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|_| LineError::InvalidUtf8)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufReader, Cursor};

    #[test]
    fn exact_limit_accepts_lf_and_crlf_without_consuming_next_command() {
        for ending in ["\n", "\r\n"] {
            let mut input = Cursor::new(format!("health{ending}shutdown\n").into_bytes());
            assert_eq!(read_line(&mut input, 6).unwrap(), Some("health".to_owned()));
            assert_eq!(read_line(&mut input, 8).unwrap(), Some("shutdown".to_owned()));
            assert_eq!(read_line(&mut input, 8).unwrap(), None);
        }
    }

    #[test]
    fn oversized_unterminated_input_has_a_bounded_read() {
        let mut input = Cursor::new(vec![b'x'; 1_000_000]);
        assert!(matches!(read_line(&mut input, 1_024), Err(LineError::TooLarge)));
        assert_eq!(input.position(), 1_026);
    }

    #[test]
    fn one_byte_over_limit_is_rejected_even_when_newline_fits() {
        let mut input = Cursor::new(b"1234567\nshutdown\n");
        assert!(matches!(read_line(&mut input, 6), Err(LineError::TooLarge)));
        assert_eq!(input.position(), 8);
    }

    #[test]
    fn crlf_split_between_buffers_is_valid() {
        let mut input = BufReader::with_capacity(1, Cursor::new(b"health\r\n"));
        assert_eq!(read_line(&mut input, 6).unwrap(), Some("health".to_owned()));
    }

    #[test]
    fn utf8_limit_is_bytes_not_characters() {
        let mut input = Cursor::new("ёж\n".as_bytes());
        assert_eq!(read_line(&mut input, 4).unwrap(), Some("ёж".to_owned()));
        let mut input = Cursor::new("ёж\n".as_bytes());
        assert!(matches!(read_line(&mut input, 3), Err(LineError::TooLarge)));
    }

    #[test]
    fn invalid_utf8_is_not_replaced_or_logged() {
        let mut input = Cursor::new([0xff, b'\n']);
        let error = read_line(&mut input, 8).unwrap_err();
        assert!(matches!(error, LineError::InvalidUtf8));
        assert_eq!(error.to_string(), "COMMAND_INVALID_UTF8");
    }

    #[test]
    fn eof_and_empty_line_are_distinct() {
        let mut input = Cursor::new(b"\nhealth");
        assert_eq!(read_line(&mut input, 6).unwrap(), Some(String::new()));
        assert_eq!(read_line(&mut input, 6).unwrap(), Some("health".to_owned()));
        assert_eq!(read_line(&mut input, 6).unwrap(), None);
    }

    #[test]
    fn invalid_limits_do_not_read_input() {
        for limit in [0, usize::MAX] {
            let mut input = Cursor::new(b"health\n");
            assert!(matches!(read_line(&mut input, limit), Err(LineError::InvalidLimit)));
            assert_eq!(input.position(), 0);
        }
    }
}

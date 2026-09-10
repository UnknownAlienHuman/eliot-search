//! Bounded line framing for the local control protocol.
//!
//! Unified framing contract (T06, envelopes deferred to T19):
//! - one bounded contract for shell, service and proxy: at most `max_bytes`
//!   content bytes per frame, excluding LF or CRLF framing (`max_bytes + 2`
//!   bytes are read at most, the suffix is never drained);
//! - a rejected frame terminates the session: its unread suffix must never be
//!   interpreted as a second command, and the command is never retried;
//! - CRLF and LF are both accepted; a lone CR is content, not framing;
//! - the byte limit applies to UTF-8 bytes, not characters; invalid UTF-8 is
//!   rejected without replacement, logging or dispatch;
//! - EOF before any byte ends the session cleanly; an empty line is a distinct
//!   empty command, not EOF;
//! - a silent client is bounded by a read timeout, which is distinct from the
//!   socket configuration timeout and from the child request/cleanup deadlines:
//!   `LineError::Timeout` (`COMMAND_READ_TIMEOUT`) never aliases
//!   `LineError::Io` (`COMMAND_READ_FAILED`).
//! - provider envelopes remain T19 work; this contract checks only the local
//!   line shape, never a canonical provider request ID.

use std::fmt;
use std::io::{self, BufRead, Read};

#[derive(Debug)]
pub enum LineError {
    InvalidLimit,
    TooLarge,
    InvalidUtf8,
    Timeout,
    Io(io::Error),
}

impl LineError {
    pub(crate) const fn code(&self) -> &'static str {
        match self {
            Self::InvalidLimit => "COMMAND_LIMIT_INVALID",
            Self::TooLarge => "COMMAND_TOO_LARGE",
            Self::InvalidUtf8 => "COMMAND_INVALID_UTF8",
            Self::Timeout => "COMMAND_READ_TIMEOUT",
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

/// Maps a blocking read failure to the timeout/read-failure split of the
/// unified contract. `TimedOut`/`WouldBlock` (silent client, expired socket
/// read timeout) is `Timeout`; every other I/O failure stays `Io`.
fn map_read_error(error: io::Error) -> LineError {
    match error.kind() {
        io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock => LineError::Timeout,
        _ => LineError::Io(error),
    }
}

/// Reads at most `max_bytes + 2` bytes, before allocating the complete frame.
/// A final unterminated frame is accepted, matching the former line protocol.
pub fn read_line(
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
        .map_err(map_read_error)?;
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

    struct FailingReader {
        kind: io::ErrorKind,
    }

    impl Read for FailingReader {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::from(self.kind))
        }
    }

    impl BufRead for FailingReader {
        fn fill_buf(&mut self) -> io::Result<&[u8]> {
            Err(io::Error::from(self.kind))
        }
        fn consume(&mut self, _: usize) {}
    }

    #[test]
    fn silent_client_timeout_is_distinct_from_read_failure() {
        for (kind, is_timeout) in [
            (io::ErrorKind::TimedOut, true),
            (io::ErrorKind::WouldBlock, true),
            (io::ErrorKind::ConnectionReset, false),
            (io::ErrorKind::BrokenPipe, false),
        ] {
            let mut input = FailingReader { kind };
            let error = read_line(&mut input, 64).unwrap_err();
            if is_timeout {
                assert!(matches!(error, LineError::Timeout), "{kind:?}");
                assert_eq!(error.to_string(), "COMMAND_READ_TIMEOUT");
            } else {
                assert!(matches!(error, LineError::Io(_)), "{kind:?}");
                assert_eq!(error.to_string(), "COMMAND_READ_FAILED");
            }
        }
        // The timeout code never aliases the read-failure, limit, size or
        // encoding codes of the same contract.
        assert_ne!(
            LineError::Timeout.code(),
            LineError::Io(io::Error::from(io::ErrorKind::Other)).code()
        );
    }

    #[test]
    fn unified_framing_contract_holds_as_a_single_proven_property() {
        // LF and CRLF are the only accepted boundaries; the limit excludes
        // framing and never drains the suffix of a rejected frame.
        for ending in ["\n", "\r\n"] {
            let mut input = Cursor::new(format!("health{ending}shutdown\n").into_bytes());
            assert_eq!(read_line(&mut input, 6).unwrap(), Some("health".to_owned()));
            assert_eq!(read_line(&mut input, 8).unwrap(), Some("shutdown".to_owned()));
            assert_eq!(read_line(&mut input, 8).unwrap(), None);
        }
        // Oversized frames (terminated or not) stop after max + 2 bytes.
        let mut input = Cursor::new(vec![b'x'; 1_000_000]);
        assert!(matches!(read_line(&mut input, 1_024), Err(LineError::TooLarge)));
        assert_eq!(input.position(), 1_026);
        let mut input = Cursor::new(b"1234567\nshutdown\n");
        assert!(matches!(read_line(&mut input, 6), Err(LineError::TooLarge)));
        assert_eq!(input.position(), 8);
        // Limits are bytes; invalid UTF-8 is rejected without replacement.
        let mut input = Cursor::new("ёж\n".as_bytes());
        assert!(matches!(read_line(&mut input, 3), Err(LineError::TooLarge)));
        let mut input = Cursor::new([0xff, b'\n']);
        assert!(matches!(
            read_line(&mut input, 8).unwrap_err(),
            LineError::InvalidUtf8
        ));
        // EOF and empty line stay distinct; invalid limits never touch input.
        let mut input = Cursor::new(b"\nhealth");
        assert_eq!(read_line(&mut input, 6).unwrap(), Some(String::new()));
        assert_eq!(read_line(&mut input, 6).unwrap(), Some("health".to_owned()));
        assert_eq!(read_line(&mut input, 6).unwrap(), None);
        // A lone CR is content, not framing.
        let mut input = Cursor::new(b"a\rb\n");
        assert_eq!(read_line(&mut input, 8).unwrap(), Some("a\rb".to_owned()));
    }
}

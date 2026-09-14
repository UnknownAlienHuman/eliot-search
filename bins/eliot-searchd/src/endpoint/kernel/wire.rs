//! Strict bounded line I/O and redacted transport diagnostics.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::TcpStream;

pub(super) fn read_bounded_line(
    reader: &mut BufReader<TcpStream>,
    maximum_bytes: usize,
) -> Result<Option<String>, String> {
    let mut bytes = Vec::new();
    let mut limited =
        reader.take(u64::try_from(maximum_bytes.saturating_add(1)).unwrap_or(u64::MAX));
    let read = limited
        .read_until(b'\n', &mut bytes)
        .map_err(|error| match error.kind() {
            io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock => {
                "ENDPOINT_READ_TIMEOUT".to_owned()
            }
            _ => redacted_io_error("ENDPOINT_READ_ERROR", &error),
        })?;
    if read == 0 {
        return Ok(None);
    }
    if bytes.len() > maximum_bytes || !bytes.ends_with(b"\n") {
        return Err("ENDPOINT_FRAME_TOO_LARGE".to_owned());
    }
    bytes.pop();
    if bytes.last() == Some(&b'\r') {
        bytes.pop();
    }
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|_| "ENDPOINT_FRAME_INVALID_UTF8".to_owned())
}

pub(super) fn write_line(stream: &mut impl Write, value: &str) -> io::Result<()> {
    stream.write_all(value.as_bytes())?;
    stream.write_all(b"\n")?;
    stream.flush()
}

/// T40 closed transport error carrying only a redacted OS detail class.
pub(super) fn redacted_io_error(code: &str, error: &io::Error) -> String {
    format!("{code}:{:?}", error.kind())
}

pub(super) fn sanitize_code(error: &str) -> String {
    let code = error.split(':').next().unwrap_or("ENDPOINT_ERROR");
    let mut output = String::with_capacity(code.len().min(128));
    for character in code.chars().take(128) {
        if character.is_ascii_uppercase()
            || character.is_ascii_digit()
            || matches!(character, '_' | '-' | '.')
        {
            output.push(character);
        } else {
            output.push('_');
        }
    }
    if output.is_empty() {
        "ENDPOINT_ERROR".to_owned()
    } else {
        output
    }
}

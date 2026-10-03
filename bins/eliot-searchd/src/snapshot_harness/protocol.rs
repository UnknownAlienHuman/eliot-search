//! Bounded request framing, authentication helpers and JSON escaping.

use std::fmt::Write as _;
use std::io::{self, ErrorKind, Read, Write};
use std::net::TcpStream;

pub(super) const MAX_REQUEST_BYTES: usize = 4_096;
pub(super) const PROTOCOL_PREFIX: &str = "ELIOT_SEARCH/1";

const MAX_RESPONSE_BYTES: usize = 64 * 1_024;
const MAX_QUERY_BYTES: usize = 1_024;

pub(super) fn decode_query(
    stream: &mut TcpStream,
    encoded: &str,
) -> io::Result<Option<String>> {
    let query_bytes = match decode_hex(encoded) {
        Ok(bytes) => bytes,
        Err(error) => {
            write_error(stream, "INVALID_QUERY", &error.to_string())?;
            return Ok(None);
        }
    };
    if query_bytes.is_empty() || query_bytes.len() > MAX_QUERY_BYTES {
        write_error(stream, "INVALID_QUERY", "query exceeds its finite bounds")?;
        return Ok(None);
    }
    let Ok(query) = String::from_utf8(query_bytes) else {
        write_error(stream, "INVALID_QUERY", "query is not UTF-8")?;
        return Ok(None);
    };
    Ok(Some(query))
}

pub(super) fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    let mut difference = left.len() ^ right.len();
    let width = left.len().max(right.len());
    for index in 0..width {
        let left_byte = left.get(index).copied().unwrap_or(0);
        let right_byte = right.get(index).copied().unwrap_or(0);
        difference |= usize::from(left_byte ^ right_byte);
    }
    difference == 0
}

pub(super) fn write_error(stream: &mut TcpStream, code: &str, detail: &str) -> io::Result<()> {
    write_response(
        stream,
        &format!(
            "{{\"ok\":false,\"error\":\"{}\",\"detail\":\"{}\"}}",
            escape_json(code),
            escape_json(detail),
        ),
    )
}

pub(super) fn write_response(stream: &mut TcpStream, response: &str) -> io::Result<()> {
    if response.len() > MAX_RESPONSE_BYTES {
        return write_error_fallback(stream, "RESPONSE_TOO_LARGE");
    }
    stream.write_all(response.as_bytes())?;
    stream.write_all(b"\n")?;
    stream.flush()
}

pub(super) fn read_bounded_line(stream: &mut TcpStream, limit: usize) -> io::Result<String> {
    let mut bytes = Vec::with_capacity(256);
    let mut byte = [0_u8; 1];
    loop {
        match stream.read(&mut byte) {
            Ok(0) => break,
            Ok(_) if byte[0] == b'\n' => break,
            Ok(_) => {
                if bytes.len() >= limit {
                    return Err(io::Error::new(
                        ErrorKind::InvalidData,
                        "request exceeds the bounded protocol frame",
                    ));
                }
                bytes.push(byte[0]);
            }
            Err(error) => return Err(error),
        }
    }
    if bytes.last() == Some(&b'\r') {
        bytes.pop();
    }
    String::from_utf8(bytes)
        .map_err(|_| io::Error::new(ErrorKind::InvalidData, "request is not UTF-8"))
}

pub(super) fn decode_hex(value: &str) -> io::Result<Vec<u8>> {
    if !value.len().is_multiple_of(2) {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "invalid hexadecimal query",
        ));
    }
    let mut output = Vec::with_capacity(value.len() / 2);
    for pair in value.as_bytes().as_chunks::<2>().0 {
        output.push((hex_nibble(pair[0])? << 4) | hex_nibble(pair[1])?);
    }
    Ok(output)
}

pub(super) fn hex_bytes(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

pub(super) fn escape_json(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character if character.is_control() => {
                let _ = write!(output, "\\u{:04x}", u32::from(character));
            }
            character => output.push(character),
        }
    }
    output
}

fn write_error_fallback(stream: &mut TcpStream, code: &str) -> io::Result<()> {
    let response = format!("{{\"ok\":false,\"error\":\"{code}\"}}\n");
    stream.write_all(response.as_bytes())?;
    stream.flush()
}

fn hex_nibble(value: u8) -> io::Result<u8> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        b'A'..=b'F' => Ok(value - b'A' + 10),
        _ => Err(io::Error::new(
            ErrorKind::InvalidData,
            "invalid hexadecimal query",
        )),
    }
}

//! Closed service command scalar and byte codecs.

use std::ffi::OsString;
use std::path::PathBuf;

use crate::continuation::MAX_PAGE_SIZE;
use crate::development::MAX_SCAN_QUERY_BYTES;

use super::spec::MAX_PATH_BYTES;

pub(super) fn parse_search_mode(value: &str) -> Result<bool, String> {
    match value {
        "sensitive" => Ok(false),
        "ascii-insensitive" => Ok(true),
        _ => Err("SERVICE_SEARCH_MODE_INVALID".to_owned()),
    }
}

pub(super) fn parse_page_size(value: &str) -> Result<usize, String> {
    let value = value
        .parse::<usize>()
        .map_err(|_| "DIRECT_CONTINUATION_PAGE_SIZE_INVALID".to_owned())?;
    if value == 0 || value > MAX_PAGE_SIZE {
        Err("DIRECT_CONTINUATION_PAGE_SIZE_INVALID".to_owned())
    } else {
        Ok(value)
    }
}

pub(super) fn decode_query(value: &str) -> Result<String, String> {
    String::from_utf8(decode_hex(value, MAX_SCAN_QUERY_BYTES)?)
        .map_err(|_| "SERVICE_QUERY_NOT_UTF8".to_owned())
}

pub(super) fn decode_path(value: &str) -> Result<PathBuf, String> {
    decode_os_string(&decode_hex(value, MAX_PATH_BYTES)?).map(PathBuf::from)
}

#[cfg(unix)]
fn decode_os_string(bytes: &[u8]) -> Result<OsString, String> {
    use std::os::unix::ffi::OsStringExt;
    Ok(OsString::from_vec(bytes.to_vec()))
}

#[cfg(windows)]
fn decode_os_string(bytes: &[u8]) -> Result<OsString, String> {
    use std::os::windows::ffi::OsStringExt;
    if !bytes.len().is_multiple_of(2) {
        return Err("SERVICE_PATH_ENCODING_INVALID".to_owned());
    }
    let wide = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect::<Vec<_>>();
    Ok(OsString::from_wide(&wide))
}

#[cfg(not(any(unix, windows)))]
fn decode_os_string(bytes: &[u8]) -> Result<OsString, String> {
    String::from_utf8(bytes.to_vec())
        .map(OsString::from)
        .map_err(|_| "SERVICE_PATH_ENCODING_INVALID".to_owned())
}

fn decode_hex(value: &str, max_bytes: usize) -> Result<Vec<u8>, String> {
    if !value.len().is_multiple_of(2) || value.len() / 2 > max_bytes {
        return Err("SERVICE_HEX_INVALID".to_owned());
    }
    let mut output = Vec::with_capacity(value.len() / 2);
    for pair in value.as_bytes().as_chunks::<2>().0 {
        let high = hex_nibble(pair[0])
            .ok_or_else(|| "SERVICE_HEX_INVALID".to_owned())?;
        let low = hex_nibble(pair[1])
            .ok_or_else(|| "SERVICE_HEX_INVALID".to_owned())?;
        output.push((high << 4) | low);
    }
    Ok(output)
}

const fn hex_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

pub(super) fn parse_u64(value: &str, error: &'static str) -> Result<u64, String> {
    value.parse::<u64>().map_err(|_| error.to_owned())
}

//! Human-facing client for the authenticated loopback DIRECT endpoint.
//!
//! Each invocation opens one authenticated bounded connection and sends one
//! closed DIRECT command. Continuation and source-handle tokens can be carried
//! explicitly between invocations without exposing internal source identities.

use std::env;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::endpoint_client;

const DEFAULT_PAGE_SIZE: usize = 100;
const MAX_PAGE_SIZE: usize = 1_000;
const MAX_QUERY_BYTES: usize = 64 * 1024;
const MAX_PATH_BYTES: usize = 32 * 1024;

/// Intercepts `remote ADDRESS TOKEN_FILE COMMAND ...`.
pub(crate) fn maybe_run() -> Option<ExitCode> {
    let arguments = env::args_os().skip(1).collect::<Vec<_>>();
    if arguments.first().and_then(|value| value.to_str()) != Some("remote") {
        return None;
    }
    let result = run(&arguments[1..]);
    Some(match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{{\"error\":\"{}\"}}", sanitize_json(&error));
            ExitCode::from(2)
        }
    })
}

fn run(arguments: &[OsString]) -> Result<(), String> {
    if arguments.len() < 3 {
        return Err("REMOTE_USAGE_ERROR".to_owned());
    }
    let address = arguments[0]
        .to_str()
        .ok_or_else(|| "REMOTE_ADDRESS_NOT_UTF8".to_owned())?;
    let token_file = PathBuf::from(&arguments[1]);
    let command_name = arguments[2]
        .to_str()
        .ok_or_else(|| "REMOTE_COMMAND_NOT_UTF8".to_owned())?;
    let command = translate(command_name, &arguments[3..])?;
    endpoint_client::invoke_remote(address, &token_file, &command)
}

fn translate(command: &str, arguments: &[OsString]) -> Result<String, String> {
    match command {
        "health" | "version" | "verify" | "verify-directory-manifests"
        | "list-sources" | "shutdown" => {
            require_count(arguments, 0)?;
            Ok(command.to_owned())
        }
        "gc-dry-run" => {
            require_count(arguments, 0)?;
            Ok("gc\tdry-run".to_owned())
        }
        "gc-apply" => {
            require_count(arguments, 0)?;
            Ok("gc\tapply".to_owned())
        }
        "index-file" | "index-directory" | "sync-directory" => {
            require_count(arguments, 1)?;
            Ok(format!(
                "{command}\t{}",
                encode_path(Path::new(&arguments[0]))?,
            ))
        }
        "search" | "search-i" | "search-all" | "search-all-i" => {
            require_count(arguments, 1)?;
            let query = require_utf8(&arguments[0], "REMOTE_QUERY_NOT_UTF8")?;
            validate_query(query)?;
            let mode = if matches!(command, "search-i" | "search-all-i") {
                "ascii-insensitive"
            } else {
                "sensitive"
            };
            if matches!(command, "search-all" | "search-all-i") {
                Ok(format!("search\t{mode}\t{}", hex(query.as_bytes())))
            } else {
                Ok(format!(
                    "search-page\t{mode}\t{DEFAULT_PAGE_SIZE}\t{}",
                    hex(query.as_bytes()),
                ))
            }
        }
        "search-page" | "search-page-i" => {
            require_count(arguments, 2)?;
            let page_size = parse_page_size(require_utf8(
                &arguments[0],
                "REMOTE_PAGE_SIZE_NOT_UTF8",
            )?)?;
            let query = require_utf8(&arguments[1], "REMOTE_QUERY_NOT_UTF8")?;
            validate_query(query)?;
            let mode = if command == "search-page-i" {
                "ascii-insensitive"
            } else {
                "sensitive"
            };
            Ok(format!(
                "search-page\t{mode}\t{page_size}\t{}",
                hex(query.as_bytes()),
            ))
        }
        "continue" => {
            if !(1..=2).contains(&arguments.len()) {
                return Err("REMOTE_CONTINUE_USAGE".to_owned());
            }
            let token = require_token(&arguments[0])?;
            let page_size = if let Some(value) = arguments.get(1) {
                parse_page_size(require_utf8(value, "REMOTE_PAGE_SIZE_NOT_UTF8")?)?
            } else {
                DEFAULT_PAGE_SIZE
            };
            Ok(format!("continue\t{token}\t{page_size}"))
        }
        "expand-handle" => {
            require_count(arguments, 3)?;
            let token = require_token(&arguments[0])?;
            let start = parse_u64(&arguments[1], "REMOTE_HANDLE_START_INVALID")?;
            let end = parse_u64(&arguments[2], "REMOTE_HANDLE_END_INVALID")?;
            if start >= end {
                return Err("REMOTE_HANDLE_RANGE_INVALID".to_owned());
            }
            Ok(format!("expand-handle\t{token}\t{start}\t{end}"))
        }
        "retire" => {
            require_count(arguments, 1)?;
            let source_id = require_token(&arguments[0])?;
            Ok(format!("retire\t{source_id}"))
        }
        "read-revision" => {
            require_count(arguments, 3)?;
            let revision_id = require_token(&arguments[0])?;
            let start = parse_u64(&arguments[1], "REMOTE_REVISION_START_INVALID")?;
            let end = parse_u64(&arguments[2], "REMOTE_REVISION_END_INVALID")?;
            if start >= end {
                return Err("REMOTE_REVISION_RANGE_INVALID".to_owned());
            }
            Ok(format!("read-revision\t{revision_id}\t{start}\t{end}"))
        }
        "raw" => {
            require_count(arguments, 1)?;
            let value = require_utf8(&arguments[0], "REMOTE_RAW_COMMAND_NOT_UTF8")?;
            if value.is_empty() || value.contains('\n') || value.contains('\r') {
                return Err("REMOTE_RAW_COMMAND_INVALID".to_owned());
            }
            Ok(value.to_owned())
        }
        _ => Err("REMOTE_COMMAND_UNSUPPORTED".to_owned()),
    }
}

fn require_count(arguments: &[OsString], expected: usize) -> Result<(), String> {
    if arguments.len() == expected {
        Ok(())
    } else {
        Err("REMOTE_USAGE_ERROR".to_owned())
    }
}

fn require_utf8<'a>(value: &'a OsStr, error: &str) -> Result<&'a str, String> {
    value.to_str().ok_or_else(|| error.to_owned())
}

fn validate_query(query: &str) -> Result<(), String> {
    if query.is_empty() || query.len() > MAX_QUERY_BYTES {
        Err("REMOTE_QUERY_INVALID".to_owned())
    } else {
        Ok(())
    }
}

fn require_token(value: &OsStr) -> Result<&str, String> {
    let value = require_utf8(value, "REMOTE_TOKEN_NOT_UTF8")?;
    if value.is_empty()
        || value.len() > 256
        || !value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':')
        })
    {
        Err("REMOTE_TOKEN_INVALID".to_owned())
    } else {
        Ok(value)
    }
}

fn parse_page_size(value: &str) -> Result<usize, String> {
    let value = value
        .parse::<usize>()
        .map_err(|_| "REMOTE_PAGE_SIZE_INVALID".to_owned())?;
    if value == 0 || value > MAX_PAGE_SIZE {
        Err("REMOTE_PAGE_SIZE_INVALID".to_owned())
    } else {
        Ok(value)
    }
}

fn parse_u64(value: &OsStr, error: &str) -> Result<u64, String> {
    require_utf8(value, error)?
        .parse::<u64>()
        .map_err(|_| error.to_owned())
}

fn encode_path(path: &Path) -> Result<String, String> {
    let bytes = native_path_bytes(path)?;
    if bytes.is_empty() || bytes.len() > MAX_PATH_BYTES {
        return Err("REMOTE_PATH_INVALID".to_owned());
    }
    Ok(hex(&bytes))
}

#[cfg(windows)]
fn native_path_bytes(path: &Path) -> Result<Vec<u8>, String> {
    use std::os::windows::ffi::OsStrExt;

    let units = path.as_os_str().encode_wide().collect::<Vec<_>>();
    let mut bytes = Vec::with_capacity(units.len().saturating_mul(2));
    for unit in units {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    Ok(bytes)
}

#[cfg(unix)]
fn native_path_bytes(path: &Path) -> Result<Vec<u8>, String> {
    use std::os::unix::ffi::OsStrExt;

    Ok(path.as_os_str().as_bytes().to_vec())
}

#[cfg(not(any(unix, windows)))]
fn native_path_bytes(path: &Path) -> Result<Vec<u8>, String> {
    path.as_os_str()
        .to_str()
        .map(|value| value.as_bytes().to_vec())
        .ok_or_else(|| "REMOTE_PATH_NOT_UTF8".to_owned())
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

fn sanitize_json(value: &str) -> String {
    let mut output = String::with_capacity(value.len().min(512));
    for character in value.chars().take(512) {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character if character.is_control() => output.push('_'),
            character => output.push(character),
        }
    }
    output
}

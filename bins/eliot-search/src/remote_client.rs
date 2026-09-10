//! Human-facing client for the authenticated canonical provider endpoint.
//!
//! Each invocation opens one `pairing_blake3_v1` connection, negotiates the
//! exact provider version, and sends one sealed envelope or operation frame.
//! Continuation and source-handle tokens travel explicitly inside validated
//! frames without exposing internal source identities. Commands outside the
//! closed provider registry stay on the direct daemon paths and are refused
//! here with an explicit surface error instead of a silent reinterpretation.

use std::env;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::endpoint_client;
use crate::provider_client::{self, UnsignedRequest};

const MAX_PATH_BYTES: usize = 32 * 1024;

/// Intercepts `remote ADDRESS TOKEN_FILE COMMAND ...`.
pub fn maybe_run() -> Option<ExitCode> {
    let arguments = env::args_os().skip(1).collect::<Vec<_>>();
    if arguments.first().and_then(|value| value.to_str()) != Some("remote") {
        return None;
    }
    let result = run(&arguments[1..]);
    Some(match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{{\"error\":\"{}\"}}", sanitize_json(&error));
            ExitCode::from(provider_client::exit_for_error(&error))
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
    let request = translate(command_name, &arguments[3..])?;
    endpoint_client::invoke_remote(address, &token_file, &request)
}

fn translate(command: &str, arguments: &[OsString]) -> Result<UnsignedRequest, String> {
    match command {
        "health" => {
            require_count(arguments, 0)?;
            Ok(UnsignedRequest::Health)
        }
        "version" => {
            require_count(arguments, 0)?;
            Ok(UnsignedRequest::Version)
        }
        "shutdown" => {
            require_count(arguments, 0)?;
            Ok(UnsignedRequest::Shutdown)
        }
        "status" => {
            require_count(arguments, 0)?;
            Ok(UnsignedRequest::Status)
        }
        "search" | "search-i" | "search-all" | "search-all-i" => {
            require_count(arguments, 1)?;
            let query = require_utf8(&arguments[0], "REMOTE_QUERY_NOT_UTF8")?;
            UnsignedRequest::query(
                matches!(command, "search-i" | "search-all-i"),
                query.as_bytes(),
            )
        }
        "index-file" | "index-directory" | "sync-directory" => {
            require_count(arguments, 1)?;
            UnsignedRequest::ingest(&native_path_bytes(&arguments[0])?)
        }
        "cancel" => {
            require_count(arguments, 1)?;
            UnsignedRequest::cancel(require_utf8(&arguments[0], "REMOTE_TOKEN_NOT_UTF8")?)
        }
        "expand-handle" => {
            require_count(arguments, 3)?;
            let handle = require_utf8(&arguments[0], "REMOTE_TOKEN_NOT_UTF8")?;
            let start = parse_u64(&arguments[1], "REMOTE_HANDLE_START_INVALID")?;
            let end = parse_u64(&arguments[2], "REMOTE_HANDLE_END_INVALID")?;
            if start >= end {
                return Err("REMOTE_HANDLE_RANGE_INVALID".to_owned());
            }
            UnsignedRequest::expand(handle.as_bytes(), start, end)
        }
        _ => Err(format!("REMOTE_NOT_ON_PROVIDER_SURFACE:{command}")),
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

fn parse_u64(value: &OsStr, error: &str) -> Result<u64, String> {
    require_utf8(value, error)?
        .parse::<u64>()
        .map_err(|_| error.to_owned())
}

fn native_path_bytes(value: &OsStr) -> Result<Vec<u8>, String> {
    let path = Path::new(value);
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        let bytes = path.as_os_str().as_bytes().to_vec();
        if bytes.is_empty() || bytes.len() > MAX_PATH_BYTES {
            return Err("REMOTE_PATH_INVALID".to_owned());
        }
        Ok(bytes)
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        let units = path.as_os_str().encode_wide().collect::<Vec<_>>();
        let mut bytes = Vec::with_capacity(units.len().saturating_mul(2));
        for unit in units {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        if bytes.is_empty() || bytes.len() > MAX_PATH_BYTES {
            return Err("REMOTE_PATH_INVALID".to_owned());
        }
        Ok(bytes)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = path;
        let _ = MAX_PATH_BYTES;
        Err("REMOTE_PATH_NOT_UTF8".to_owned())
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn arg(value: &str) -> OsString {
        OsString::from(value)
    }

    #[test]
    fn provider_surface_translates_and_bounds() {
        assert!(matches!(
            translate("health", &[]),
            Ok(UnsignedRequest::Health)
        ));
        assert!(matches!(
            translate("status", &[]),
            Ok(UnsignedRequest::Status)
        ));
        assert!(translate("search", &[arg("needle")]).is_ok());
        assert!(translate("search", &[]).is_err());
        assert!(translate("expand-handle", &[arg("ab"), arg("0"), arg("8")]).is_ok());
        assert!(translate("expand-handle", &[arg("ab"), arg("8"), arg("8")]).is_err());
        assert!(translate("cancel", &[arg("00112233445566778899aabbccddeeff")]).is_ok());
        // Outside the closed registry: explicit surface error, never silent.
        assert_eq!(
            translate("raw", &[arg("health")]),
            Err("REMOTE_NOT_ON_PROVIDER_SURFACE:raw".to_owned())
        );
        assert_eq!(
            translate("verify", &[]),
            Err("REMOTE_NOT_ON_PROVIDER_SURFACE:verify".to_owned())
        );
        let long = "x".repeat(MAX_PATH_BYTES + 1);
        assert!(translate("index-file", &[arg(&long)]).is_err());
    }
}

//! Process argument interception for the authenticated loopback proxy.

use std::env;
use std::path::Path;
use std::process::ExitCode;

use super::server::run_proxy;
use super::spec::sanitize_json;

/// Intercepts `--serve-loopback-data-root ROOT PORT TOKEN_FILE`.
pub fn maybe_run() -> Option<ExitCode> {
    let raw = env::args_os().skip(1).collect::<Vec<_>>();
    let (arguments, _) = match crate::config_composition::strip_config_args_os(&raw) {
        Ok(split) => split,
        Err(error) => {
            eprintln!("{{\"error\":\"{}\"}}", sanitize_json(&error));
            return Some(ExitCode::from(2));
        }
    };
    if arguments.first().and_then(|value| value.to_str())
        != Some("--serve-loopback-data-root")
    {
        return None;
    }
    let result = match arguments.as_slice() {
        [_, root, port, token_file] => parse_port(port).and_then(|port| {
            run_proxy(Path::new(root), port, Path::new(token_file))
        }),
        _ => Err("LOOPBACK_SERVICE_USAGE_ERROR".to_owned()),
    };
    Some(match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{{\"error\":\"{}\"}}", sanitize_json(&error));
            ExitCode::from(2)
        }
    })
}

fn parse_port(value: &std::ffi::OsStr) -> Result<u16, String> {
    value
        .to_str()
        .ok_or_else(|| "LOOPBACK_PORT_NOT_UTF8".to_owned())?
        .parse::<u16>()
        .map_err(|_| "LOOPBACK_PORT_INVALID".to_owned())
}

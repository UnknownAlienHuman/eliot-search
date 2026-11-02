//! Process argument interception for the authenticated loopback proxy.

use std::ffi::OsString;
use std::path::Path;
use std::process::ExitCode;

use super::server::run_proxy;
use super::spec::sanitize_json;

/// Runs the explicitly selected legacy harness with the captured invocation.
pub fn run_command(
    arguments: &[OsString],
    config: &crate::config_composition::CliConfigArgs,
) -> ExitCode {
    let result = match arguments {
        [_, root, port, token_file] => parse_port(port).and_then(|port| {
            run_proxy(Path::new(root), port, Path::new(token_file), config)
        }),
        _ => Err("LOOPBACK_SERVICE_USAGE_ERROR".to_owned()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{{\"error\":\"{}\"}}", sanitize_json(&error));
            ExitCode::from(2)
        }
    }
}

fn parse_port(value: &std::ffi::OsStr) -> Result<u16, String> {
    value
        .to_str()
        .ok_or_else(|| "LOOPBACK_PORT_NOT_UTF8".to_owned())?
        .parse::<u16>()
        .map_err(|_| "LOOPBACK_PORT_INVALID".to_owned())
}

//! Process entry interception for the owner-fenced DIRECT runtime.

use std::env;
use std::process::ExitCode;
use std::time::Instant;

use crate::service_output::json_string;

use super::runtime::run_service;

/// Intercepts `--serve-data-root ROOT` before one-shot command dispatch.
pub fn maybe_run() -> Option<ExitCode> {
    let started = Instant::now();
    let raw = env::args_os().skip(1).take(257).collect::<Vec<_>>();
    let (arguments, _) = match crate::config_composition::strip_config_args_os(&raw) {
        Ok(split) => split,
        Err(error) => {
            eprintln!(
                "{{\"error\":{}}}",
                json_string(&eliot_searchd::diagnostics::sanitize_code(&error))
            );
            return Some(ExitCode::from(2));
        }
    };
    if arguments.first().and_then(|value| value.to_str()) != Some("--serve-data-root") {
        return None;
    }
    let result = match arguments.as_slice() {
        [_, _] if raw.len() <= 256 => run_service(&raw, started),
        _ => Err("USAGE_ERROR".to_owned()),
    };
    Some(match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!(
                "{{\"error\":{}}}",
                json_string(&eliot_searchd::diagnostics::sanitize_code(&error))
            );
            ExitCode::from(2)
        }
    })
}

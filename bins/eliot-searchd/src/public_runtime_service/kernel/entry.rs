//! Process entry interception for the owner-fenced DIRECT runtime.

use std::env;
use std::path::Path;
use std::process::ExitCode;

use crate::service_output::json_string;

use super::runtime::run_service;

/// Intercepts `--serve-data-root ROOT` before one-shot command dispatch.
pub fn maybe_run() -> Option<ExitCode> {
    let raw = env::args_os().skip(1).collect::<Vec<_>>();
    let (arguments, _) = match crate::config_composition::strip_config_args_os(&raw) {
        Ok(split) => split,
        Err(error) => {
            eprintln!("{{\"error\":{}}}", json_string(&error));
            return Some(ExitCode::from(2));
        }
    };
    if arguments.first().and_then(|value| value.to_str())
        != Some("--serve-data-root")
    {
        return None;
    }
    let result = match arguments.as_slice() {
        [_, root] => run_service(Path::new(root)),
        _ => Err("USAGE_ERROR".to_owned()),
    };
    Some(match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{{\"error\":{}}}", json_string(&error));
            ExitCode::from(2)
        }
    })
}

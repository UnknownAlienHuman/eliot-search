use std::env;
use std::process::ExitCode;

use super::dispatch::dispatch;
use super::output::emit_process_error;
use super::support::{help, is_persistent_command};

/// Intercepts every persistent DIRECT one-shot command plus primary help.
pub fn maybe_run() -> Option<ExitCode> {
    let raw = env::args_os().skip(1).collect::<Vec<_>>();
    let (arguments, _) = match crate::config_composition::strip_config_args_os(&raw) {
        Ok(split) => split,
        Err(error) => return Some(emit_process_error(&error)),
    };
    let command = arguments.first()?.to_str()?;
    if matches!(command, "--help" | "-h") {
        return Some(if arguments.len() == 1 {
            print!("{}", help());
            ExitCode::SUCCESS
        } else {
            emit_process_error("USAGE_ERROR")
        });
    }
    if !is_persistent_command(command) {
        return None;
    }
    let result = dispatch(command, &arguments);
    Some(match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => emit_process_error(&error),
    })
}

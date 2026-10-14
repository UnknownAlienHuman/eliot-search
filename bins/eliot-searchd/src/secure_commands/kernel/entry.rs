use std::env;
use std::process::ExitCode;
use std::time::Instant;

use super::dispatch::dispatch;
use super::output::emit_process_error;
use super::support::{
    LEGACY_EFFECTS_UNAVAILABLE, help, is_persistent_command, is_unavailable_effect,
};

/// Refuses legacy effects before any native route, root owner or store opens.
/// Global configuration flags are only stripped; their files are never read.
pub fn maybe_refuse_legacy_effects() -> Option<ExitCode> {
    let raw = env::args_os().skip(1).take(257).collect::<Vec<_>>();
    if raw
        .first()
        .and_then(|value| value.to_str())
        .is_some_and(is_unavailable_effect)
    {
        return Some(emit_process_error(LEGACY_EFFECTS_UNAVAILABLE));
    }
    let (arguments, _) = crate::config_composition::strip_config_args_os(&raw).ok()?;
    arguments
        .first()
        .and_then(|value| value.to_str())
        .filter(|command| is_unavailable_effect(command))
        .map(|_| emit_process_error(LEGACY_EFFECTS_UNAVAILABLE))
}

/// Intercepts every persistent DIRECT one-shot command plus primary help.
pub fn maybe_run() -> Option<ExitCode> {
    let started = Instant::now();
    let raw = env::args_os().skip(1).take(257).collect::<Vec<_>>();
    if raw.len() > 256 {
        return Some(emit_process_error("USAGE_ERROR"));
    }
    let Ok((arguments, _)) = crate::config_composition::strip_config_args_os(&raw) else {
        return Some(emit_process_error("USAGE_ERROR"));
    };
    if arguments
        .first()
        .and_then(|value| value.to_str())
        .is_some_and(is_unavailable_effect)
    {
        return Some(emit_process_error(LEGACY_EFFECTS_UNAVAILABLE));
    }
    if arguments
        .first()
        .and_then(|value| value.to_str())
        .is_some_and(super::source_sync::is_sync)
    {
        return Some(if super::source_sync::run(&raw, started).is_ok() {
            ExitCode::SUCCESS
        } else {
            ExitCode::from(2)
        });
    }
    if let Some(command) = arguments.first().and_then(|value| value.to_str())
        && super::source_registration::is_registration(command)
    {
        return Some(if super::source_registration::run(&raw, started).is_ok() {
            ExitCode::SUCCESS
        } else {
            ExitCode::from(2)
        });
    }
    if let Some(command) = arguments.first().and_then(|value| value.to_str())
        && super::read::is_read(command)
    {
        return Some(match super::read::run(&raw, started) {
            super::read::ReadExit::Completed => ExitCode::SUCCESS,
            super::read::ReadExit::Failed => ExitCode::from(2),
        });
    }
    let command = raw.first()?.to_str()?;
    if matches!(command, "--help" | "-h") {
        return Some(if raw.len() == 1 {
            print!("{}", help());
            ExitCode::SUCCESS
        } else {
            emit_process_error("USAGE_ERROR")
        });
    }
    if !is_persistent_command(command) {
        return None;
    }
    let result = dispatch(command, &raw);
    Some(match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => emit_process_error(&error),
    })
}

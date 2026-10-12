//! Paired catalog commands; DIRECT, credential and sync dispatch are excluded.

use std::ffi::OsString;
use std::io::{self, Write};
use std::path::Path;
use std::sync::{Arc, atomic::AtomicBool};
use std::time::{Duration, Instant};

use crate::owner_composition::{NativeRootLimits, NativeRootRequest, run_source_registration};
use crate::source_roots::SourceRootAction;
use search_runtime_owner::OwnerError;

pub(super) fn is_registration(command: &str) -> bool {
    matches!(
        command,
        "--register-source-root" | "--unregister-source-root"
    )
}

fn usage_refusal(arguments: &[OsString], output: &mut impl Write) -> io::Result<bool> {
    let Ok((selected, _)) = crate::config_composition::strip_config_args_os(arguments) else {
        return Ok(false);
    };
    if selected
        .first()
        .and_then(|command| command.to_str())
        .is_none_or(|command| !is_registration(command))
        || selected.len() == 3
    {
        return Ok(false);
    }
    writeln!(output, "{{\"error\":\"SOURCE_ROOT_USAGE_ERROR\"}}")?;
    output.flush()?;
    Ok(true)
}

pub(super) fn run(arguments: &[OsString], started: Instant) -> Result<(), String> {
    if usage_refusal(arguments, &mut io::stderr().lock())
        .map_err(|_| "SOURCE_ROOT_OUTPUT_FAILED".to_owned())?
    {
        return Err("SOURCE_ROOT_USAGE_ERROR".to_owned());
    }
    let deadline = started
        .checked_add(Duration::from_secs(120))
        .ok_or(OwnerError::OwnerDeadlineInvalid)
        .map_err(|error| error.code().to_owned())?;
    let limits =
        NativeRootLimits::new([256, 256 * 1024, 512 * 1024, 1024 * 1024 * 1024, 16384, 256])
            .map_err(|error| error.code().to_owned())?;
    let cancellation = Arc::new(AtomicBool::new(false));
    let (request, selected) = match NativeRootRequest::for_source_registration(
        arguments,
        deadline,
        Arc::clone(&cancellation),
        limits,
    ) {
        Ok(selected) => selected,
        Err(error) => {
            NativeRootRequest::emit_validation_refusal(
                deadline,
                &cancellation,
                limits,
                &mut io::stderr().lock(),
                error,
            )
            .map_err(|error| error.code().to_owned())?;
            return Err(error.code().to_owned());
        }
    };
    let action = match selected[0].to_str() {
        Some("--register-source-root") => SourceRootAction::Register,
        Some("--unregister-source-root") => SourceRootAction::Unregister,
        _ => return Err("SOURCE_ROOT_USAGE_ERROR".to_owned()),
    };
    run_source_registration(
        request,
        action,
        Path::new(&selected[2]),
        &mut io::stdout().lock(),
        &mut io::stderr().lock(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paired_arity_refuses_before_root_admission_with_original_error_frame() {
        for command in ["--register-source-root", "--unregister-source-root"] {
            for values in [
                vec![command],
                vec![command, "private-root"],
                vec![command, "private-root", "private-source", "extra"],
            ] {
                let args: Vec<_> = values.into_iter().map(OsString::from).collect();
                let mut output = Vec::new();
                assert!(usage_refusal(&args, &mut output).unwrap());
                assert_eq!(output, b"{\"error\":\"SOURCE_ROOT_USAGE_ERROR\"}\n");
            }
            let args = [
                OsString::from("--config-file"),
                OsString::from("private-config"),
                OsString::from(command),
                OsString::from("private-root"),
                OsString::from("private-source"),
            ];
            let mut output = Vec::new();
            assert!(!usage_refusal(&args, &mut output).unwrap());
            assert!(output.is_empty());
        }
    }
}

//! The real sync caller performs native preflight and refuses before store/effects.

use std::ffi::OsString;
use std::io::{self, Write};
use std::sync::{Arc, atomic::AtomicBool};
use std::time::{Duration, Instant};

use crate::owner_composition::{NativeRootLimits, NativeRootRequest, run_source_sync_preflight};
use search_runtime_owner::OwnerError;

pub(super) fn is_sync(command: &str) -> bool {
    command == "--sync-source-roots"
}

pub(super) fn run(arguments: &[OsString], started: Instant) -> Result<(), String> {
    let (selected, _) = crate::config_composition::strip_config_args_os(arguments)
        .map_err(|_| "SOURCE_ROOT_USAGE_ERROR".to_owned())?;
    if selected.len() != 2 {
        let mut output = io::stderr().lock();
        writeln!(output, "{{\"error\":\"SOURCE_ROOT_USAGE_ERROR\"}}")
            .and_then(|()| output.flush())
            .map_err(|_| "SOURCE_ROOT_OUTPUT_FAILED".to_owned())?;
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
    let request = match NativeRootRequest::for_source_sync(
        arguments,
        deadline,
        Arc::clone(&cancellation),
        limits,
    ) {
        Ok(request) => request,
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
    run_source_sync_preflight(request, &mut io::stderr().lock())
}

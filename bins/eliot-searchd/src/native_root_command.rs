//! Plain native root commands.

use std::process::ExitCode;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use crate::config_composition::BootstrapInvocation;
use search_runtime_owner::{
    PlainRootError, PlainRootStatus, discard_incomplete_plain, initialize_plain, inspect_plain,
    open_plain,
};

fn emit(error: &PlainRootError) -> ExitCode {
    eprintln!("{{\"error\":\"{}\"}}", error.code());
    ExitCode::from(2)
}

fn status_name(status: PlainRootStatus) -> &'static str {
    match status {
        PlainRootStatus::Absent => "absent",
        PlainRootStatus::Empty => "empty",
        PlainRootStatus::IncompleteInitialization => "incomplete_initialization",
        PlainRootStatus::Current => "current",
        PlainRootStatus::Corrupt => "corrupt",
        PlainRootStatus::Unsupported => "unsupported",
        PlainRootStatus::Busy => "busy",
        PlainRootStatus::Archive => "archive",
        PlainRootStatus::IncompleteImport => "incomplete_import",
    }
}

pub fn run_native_root_command(invocation: BootstrapInvocation, started: Instant) -> ExitCode {
    let root = invocation.settings.data_root;
    let Some(deadline) = started.checked_add(Duration::from_secs(120)) else {
        return emit(&PlainRootError::Cancelled);
    };
    static CANCELLED: AtomicBool = AtomicBool::new(false);
    let result = match invocation.command.as_str() {
        "--inspect-data-root" => inspect_plain(&root, deadline, &CANCELLED).map(|value| {
            println!(
                "{{\"owner_state\":\"{}\",\"owner_capability\":false}}",
                status_name(value.status)
            );
        }),
        "--check-existing-root-owner" => open_plain(&root, deadline, &CANCELLED).map(|guard| {
            println!("{{\"owner_state\":\"current\",\"owner_capability\":true}}");
            drop(guard);
        }),
        "--initialize-data-root" => initialize_plain(&root, deadline, &CANCELLED).map(|guard| {
            println!("{{\"owner_state\":\"initialized\",\"owner_capability\":true}}");
            drop(guard);
        }),
        "--discard-incomplete-data-root" => discard_incomplete_plain(&root, deadline, &CANCELLED)
            .map(|_| {
                println!("{{\"owner_state\":\"discarded\",\"owner_capability\":false}}");
            }),
        _ => return emit(&PlainRootError::InvalidPath),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => emit(&error),
    }
}

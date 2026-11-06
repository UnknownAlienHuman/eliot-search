//! Executes one plain-root operation and emits its observed final state.

use std::process::ExitCode;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use search_runtime_owner::{
    PlainRootError, PlainRootGuard, PlainRootStatus, discard_incomplete_plain, initialize_plain,
    inspect_plain, open_plain,
};

use crate::app::{RootCommand, RootInvocation};

static CANCELLED: AtomicBool = AtomicBool::new(false);

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

fn emit_snapshot(command: RootCommand, status: PlainRootStatus) {
    println!(
        "{{\"operation\":\"{}\",\"root_state\":\"{}\"}}",
        command.operation(),
        status_name(status)
    );
}

fn emit_error(error: PlainRootError) -> ExitCode {
    eprintln!("{{\"error\":\"{}\"}}", error.code());
    ExitCode::from(2)
}

fn emit_guarded(command: RootCommand, guard: PlainRootGuard) -> ExitCode {
    if !guard.is_current() {
        drop(guard);
        return emit_error(PlainRootError::Corrupt);
    }
    emit_snapshot(command, PlainRootStatus::Current);
    drop(guard);
    ExitCode::SUCCESS
}

pub(crate) fn run(invocation: RootInvocation, started: Instant) -> ExitCode {
    let Some(deadline) = started.checked_add(Duration::from_secs(120)) else {
        return emit_error(PlainRootError::Deadline);
    };
    let RootInvocation { command, data_root } = invocation;
    match command {
        RootCommand::Inspect => match inspect_plain(&data_root, deadline, &CANCELLED) {
            Ok(inspection) => {
                emit_snapshot(command, inspection.status);
                ExitCode::SUCCESS
            }
            Err(error) => emit_error(error),
        },
        RootCommand::CheckExistingOwner => match open_plain(&data_root, deadline, &CANCELLED) {
            Ok(guard) => emit_guarded(command, guard),
            Err(error) => emit_error(error),
        },
        RootCommand::Initialize => match initialize_plain(&data_root, deadline, &CANCELLED) {
            Ok(guard) => emit_guarded(command, guard),
            Err(error) => emit_error(error),
        },
        RootCommand::DiscardIncomplete => {
            match discard_incomplete_plain(&data_root, deadline, &CANCELLED)
                .and_then(|()| inspect_plain(&data_root, deadline, &CANCELLED))
            {
                Ok(inspection) => {
                    emit_snapshot(command, inspection.status);
                    ExitCode::SUCCESS
                }
                Err(error) => emit_error(error),
            }
        }
    }
}

//! Executes one plain-root operation and emits its observed final state.

use std::process::ExitCode;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use search_control_redb::current_source::{
    SourceControlError, capture_current_source, commit_current_source,
};
use search_revision_store::{ObjectStoreError, store_source_bytes};
use search_runtime_owner::{
    PlainRootError, PlainRootGuard, PlainRootStatus, discard_incomplete_plain, initialize_plain,
    inspect_plain, open_plain,
};
use search_safe_reader::{MAX_SOURCE_FILE_BYTES, acquire_file};
use search_source_registry::CurrentSourceRecord;

use crate::app::{RootCommand, RootInvocation};

static CANCELLED: AtomicBool = AtomicBool::new(false);

const fn status_name(status: PlainRootStatus) -> &'static str {
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

fn emit_source_error(code: &str) -> ExitCode {
    eprintln!("{{\"error\":\"{code}\"}}");
    ExitCode::from(2)
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(DIGITS[usize::from(byte >> 4)]));
        output.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    output
}

fn emit_source_record(record: &CurrentSourceRecord) {
    let object = record.object_ref;
    println!(
        "{{\"operation\":\"retain-file\",\"source_namespace_id\":\"{}\",\"source_id\":\"{}\",\"source_row_revision\":{},\"registration_revision\":{},\"object_version\":{},\"object_kind\":\"source\",\"object_digest\":\"{}\",\"object_bytes\":{},\"native_file_key\":\"{}\",\"readback\":\"verified\"}}",
        record.source_namespace_id,
        record.source_id,
        record.row_revision,
        record.registration_revision,
        object.format_version,
        hex(object.digest.as_bytes()),
        object.byte_length,
        hex(&record.file_key.to_bytes()),
    );
}

fn retain_file(
    data_root: &std::path::Path,
    file_path: std::path::PathBuf,
    deadline: Instant,
) -> ExitCode {
    let guard = match open_plain(data_root, deadline, &CANCELLED) {
        Ok(guard) => guard,
        Err(error) => return emit_error(error),
    };
    let capture =
        match capture_current_source(&guard.control_path(), &file_path, deadline, &CANCELLED) {
            Ok(capture) => capture,
            Err(error) => return emit_source_error(error.code()),
        };
    let acquired = match acquire_file(&file_path, MAX_SOURCE_FILE_BYTES, deadline, &CANCELLED) {
        Ok(acquired) => acquired,
        Err(error) => return emit_source_error(error.code()),
    };
    let (bytes, file_key, acquired_length) = acquired.into_parts();
    let object_ref = match store_source_bytes(&guard, bytes, deadline, &CANCELLED) {
        Ok(object_ref) if object_ref.byte_length == acquired_length => object_ref,
        Ok(_) => return emit_source_error("SOURCE_LENGTH_MISMATCH"),
        Err(error) => return emit_source_error(error.code()),
    };
    let record = match commit_current_source(
        &guard.control_path(),
        capture,
        file_key,
        object_ref,
        deadline,
        &CANCELLED,
    ) {
        Ok(record) => record,
        Err(error) => return emit_source_error(error.code()),
    };
    emit_source_record(&record);
    drop(guard);
    ExitCode::SUCCESS
}

pub fn run(invocation: RootInvocation, started: Instant) -> ExitCode {
    let Some(deadline) = started.checked_add(Duration::from_secs(120)) else {
        return emit_error(PlainRootError::Deadline);
    };
    let RootInvocation {
        command,
        data_root,
        source_file,
    } = invocation;
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
        RootCommand::RetainFile => match source_file {
            Some(file_path) => retain_file(&data_root, file_path, deadline),
            None => emit_source_error("SOURCE_FILE_REQUIRED"),
        },
    }
}

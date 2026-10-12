//! One-shot read dispatch over the same retained native read owner as the service.

use std::ffi::OsString;
use std::io::{self, Write};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use crate::direct_store::{DirectRead, NativeOutput, ReadOnlyStore};
use crate::owner_composition::{
    NativeRootAdmission, NativeRootLimits, NativeRootRequest, admit_existing,
};
use crate::service_output::{emit_streaming_search, json_string, write_line};
use crate::storage_security::StorageSecurityStatus;
use search_runtime_owner::{OwnerError, RootOpenState};

use super::support::{MAX_DIAGNOSTIC_REVISION_SLICE_BYTES, parse_u64};

pub(super) fn is_read(command: &str) -> bool {
    matches!(
        command,
        "--health-data-root"
            | "--search-root"
            | "--search-root-ascii-insensitive"
            | "--list-sources"
            | "--verify-root"
            | "--read-revision"
    )
}

pub(super) enum ReadExit {
    Completed,
    Failed,
}

pub(super) fn run(arguments: &[OsString], started: Instant) -> ReadExit {
    match try_run(arguments, started) {
        Ok(()) => ReadExit::Completed,
        Err(_) => ReadExit::Failed,
    }
}

fn try_run(arguments: &[OsString], started: Instant) -> Result<(), String> {
    let deadline = started
        .checked_add(Duration::from_secs(120))
        .ok_or(OwnerError::OwnerDeadlineInvalid)
        .map_err(|error| error.code().to_owned())?;
    let limits =
        NativeRootLimits::new([256, 256 * 1024, 512 * 1024, 1024 * 1024 * 1024, 16384, 256])
            .map_err(|error| error.code().to_owned())?;
    let (request, selected) = NativeRootRequest::for_read_one_shot(
        arguments,
        deadline,
        Arc::new(AtomicBool::new(false)),
        limits,
    )
    .map_err(|error| error.code().to_owned())?;
    if let Err(error) = validate(&selected) {
        request
            .emit(&mut io::stderr().lock(), &error_line(&error))
            .map_err(|error| error.code().to_owned())?;
        return Err(error);
    }
    let startup = match admit_existing(request).map_err(|error| error.code().to_owned())? {
        NativeRootAdmission::Inspection(owner) => owner
            .read_startup()
            .map_err(|error| error.code().to_owned())?,
        NativeRootAdmission::Unavailable(RootOpenState::Absent, request) => {
            request
                .emit(
                    &mut io::stderr().lock(),
                    &error_line("DIRECT_ROOT_UNPROVISIONED"),
                )
                .map_err(|error| error.code().to_owned())?;
            return Err("DIRECT_ROOT_UNPROVISIONED".to_owned());
        }
        NativeRootAdmission::Unavailable(_, request) => {
            request
                .emit(
                    &mut io::stderr().lock(),
                    &error_line("DIRECT_NATIVE_ROOT_UNAVAILABLE"),
                )
                .map_err(|error| error.code().to_owned())?;
            return Err("DIRECT_NATIVE_ROOT_UNAVAILABLE".to_owned());
        }
        NativeRootAdmission::Mutation(_) => {
            return Err("DIRECT_NATIVE_ROOT_UNAVAILABLE".to_owned());
        }
    };
    startup.run_one_shot(|child| {
        let root = child.root();
        child.checkpoint()?;
        let storage = StorageSecurityStatus::inspect(&root);
        child.checkpoint()?;
        let mut stdout = io::stdout().lock();
        let mut output = NativeOutput::new(child, &mut stdout);
        let result = storage.and_then(|storage| execute(&selected, child, &storage, &mut output));
        if let Err(error) = &result
            && !child.output_attempted()
            && child.native_checkpoint().is_ok()
        {
            let mut stderr = io::stderr().lock();
            let mut error_output = NativeOutput::new(child, &mut stderr);
            write_line(&mut error_output, &error_line(error))?;
            error_output
                .flush()
                .map_err(|_| "DIRECT_OUTPUT_FAILED".to_owned())?;
        }
        result?;
        output
            .flush()
            .map_err(|_| "DIRECT_OUTPUT_FAILED".to_owned())
    })
}

fn error_line(error: &str) -> String {
    format!(
        "{{\"error\":{}}}",
        json_string(&eliot_searchd::diagnostics::sanitize_code(error)),
    )
}

fn validate(arguments: &[OsString]) -> Result<(), String> {
    match arguments[0].to_str() {
        Some("--search-root" | "--search-root-ascii-insensitive") => {
            arguments[2]
                .to_str()
                .ok_or_else(|| "DIRECT_QUERY_NOT_UTF8".to_owned())?;
        }
        Some("--read-revision") => {
            arguments[2]
                .to_str()
                .ok_or_else(|| "DIRECT_REVISION_ID_INVALID".to_owned())?;
            let start = parse_u64(&arguments[3], "START_OFFSET")?;
            let end = parse_u64(&arguments[4], "END_OFFSET")?;
            if end.saturating_sub(start) > MAX_DIAGNOSTIC_REVISION_SLICE_BYTES {
                return Err("DIRECT_REVISION_SLICE_TOO_LARGE".to_owned());
            }
        }
        _ => {}
    }
    Ok(())
}

fn execute(
    arguments: &[OsString],
    store: &ReadOnlyStore<'_>,
    storage: &StorageSecurityStatus,
    writer: &mut impl Write,
) -> Result<(), String> {
    let command = arguments[0]
        .to_str()
        .ok_or_else(|| "USAGE_ERROR".to_owned())?;
    match command {
        "--health-data-root" => {
            let verification = store.verify()?;
            write_line(
                writer,
                &format!(
                    concat!(
                        "{{\"event\":\"health\",\"namespace_id\":{},",
                        "\"registered_sources\":{},\"active_sources\":{},",
                        "\"verified_revisions\":{},\"storage_security\":{},",
                        "\"encrypted_at_rest\":{}}}"
                    ),
                    json_string(&store.namespace_id()?),
                    verification.registered_sources,
                    verification.active_sources,
                    verification.verified_revisions,
                    storage.json(),
                    storage.encrypted_at_rest,
                ),
            )
        }
        "--search-root" | "--search-root-ascii-insensitive" => {
            store.verify()?;
            let query = arguments[2]
                .to_str()
                .ok_or_else(|| "DIRECT_QUERY_NOT_UTF8".to_owned())?;
            let result = store.search(query, command == "--search-root-ascii-insensitive")?;
            emit_streaming_search(writer, &store.namespace_id()?, &result, storage)
        }
        "--list-sources" => {
            store.verify()?;
            super::output::emit_sources(writer, store, storage)
        }
        "--verify-root" => super::output::emit_verification(writer, store, storage),
        "--read-revision" => {
            let revision = arguments[2]
                .to_str()
                .ok_or_else(|| "DIRECT_REVISION_ID_INVALID".to_owned())?;
            let slice = store.read_revision_range(
                revision,
                parse_u64(&arguments[3], "START_OFFSET")?,
                parse_u64(&arguments[4], "END_OFFSET")?,
            )?;
            write_line(
                writer,
                &format!(
                    concat!(
                        "{{\"event\":\"revision_slice\",\"revision_id\":{},",
                        "\"content_digest\":{},\"byte_start\":{},\"byte_end\":{},",
                        "\"encoding\":\"hex\",\"bytes\":{},\"source_backed\":true,",
                        "\"storage_backend\":{},\"encrypted_at_rest\":{}}}"
                    ),
                    json_string(&slice.revision_id),
                    json_string(&slice.content_digest),
                    slice.byte_start,
                    slice.byte_end,
                    json_string(&crate::sha256::hex(&slice.bytes)),
                    json_string(storage.backend),
                    storage.encrypted_at_rest,
                ),
            )
        }
        _ => Err("UNKNOWN_PERSISTENT_COMMAND".to_owned()),
    }
}

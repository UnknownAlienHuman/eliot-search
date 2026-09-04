//! Secure one-shot dispatcher for persistent DIRECT commands.
//!
//! This module runs before the legacy command application. Every persistent
//! source command therefore uses the revision-protected store facade and emits
//! storage claims derived from exact inventory, never a hard-coded capability.

use std::env;
use std::ffi::OsString;
use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;

use crate::development::DataRootGuard;
use crate::direct_store::{DirectStore, SourceSummary};
use crate::maintenance::repair_control_log;
use crate::maintenance_guard::guarded_collect_orphan_revisions;
use crate::service_output::{
    emit_indexed_source, emit_streaming_search, json_string, write_line,
};
use crate::sha256;
use crate::storage_security::StorageSecurityStatus;

const MAX_DIAGNOSTIC_REVISION_SLICE_BYTES: u64 = 24 * 1024;

/// Intercepts every persistent DIRECT one-shot command plus primary help.
pub(crate) fn maybe_run() -> Option<ExitCode> {
    let arguments = env::args_os().skip(1).collect::<Vec<_>>();
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

fn is_persistent_command(command: &str) -> bool {
    matches!(
        command,
        "--health-data-root"
            | "--index-file"
            | "--index-directory"
            | "--search-root"
            | "--search-root-ascii-insensitive"
            | "--list-sources"
            | "--verify-root"
            | "--retire-source"
            | "--read-revision"
            | "--repair-root"
            | "--gc-root"
    )
}

fn help() -> &'static str {
    concat!(
        "eliot-searchd ",
        env!("CARGO_PKG_VERSION"),
        "\n\n",
        "CONTROL:\n",
        "  eliot-searchd --help\n",
        "  eliot-searchd --version\n",
        "  eliot-searchd --health\n",
        "  eliot-searchd --health-data-root ROOT\n",
        "  eliot-searchd --self-test\n",
        "  eliot-searchd --stdio\n",
        "  eliot-searchd --serve-data-root ROOT\n",
        "  eliot-searchd --serve-loopback-data-root ROOT PORT TOKEN_FILE\n\n",
        "ONE-SHOT SEARCH:\n",
        "  eliot-searchd --scan-stdin QUERY\n",
        "  eliot-searchd --scan-stdin-ascii-insensitive QUERY\n",
        "  eliot-searchd --scan-file QUERY FILE\n",
        "  eliot-searchd --scan-file-ascii-insensitive QUERY FILE\n\n",
        "PERSISTENT DIRECT CORPUS:\n",
        "  eliot-searchd --index-file ROOT FILE\n",
        "  eliot-searchd --index-directory ROOT DIRECTORY\n",
        "  eliot-searchd --search-root ROOT QUERY\n",
        "  eliot-searchd --search-root-ascii-insensitive ROOT QUERY\n",
        "  eliot-searchd --list-sources ROOT\n",
        "  eliot-searchd --verify-root ROOT\n",
        "  eliot-searchd --retire-source ROOT SOURCE_ID\n",
        "  eliot-searchd --read-revision ROOT REVISION_ID START END\n\n",
        "MAINTENANCE:\n",
        "  eliot-searchd --repair-root ROOT\n",
        "  eliot-searchd --gc-root ROOT --dry-run\n",
        "  eliot-searchd --gc-root ROOT --apply\n\n",
        "Windows protects retained revisions with DPAPI and a per-namespace ",
        "Credential Manager secret. Other platforms retain the explicit ",
        "plaintext-development profile. Every persistent response derives ",
        "encrypted_at_rest from the complete current object inventory.\n",
    )
}

fn dispatch(command: &str, arguments: &[OsString]) -> Result<(), String> {
    match command {
        "--health-data-root" => {
            require_count(arguments, 2)?;
            with_store(Path::new(&arguments[1]), |root, store, storage| {
                let verification = store.verify()?;
                write_stdout(&format!(
                    concat!(
                        "{{\"event\":\"health\",\"namespace_id\":{},",
                        "\"registered_sources\":{},\"active_sources\":{},",
                        "\"verified_revisions\":{},\"storage_security\":{},",
                        "\"encrypted_at_rest\":{}}}"
                    ),
                    json_string(&store.namespace_id()),
                    verification.registered_sources,
                    verification.active_sources,
                    verification.verified_revisions,
                    storage.json(),
                    storage.encrypted_at_rest,
                ))?;
                let _ = root;
                Ok(())
            })
        }
        "--index-file" => {
            require_count(arguments, 3)?;
            with_store_mut(Path::new(&arguments[1]), |root, store| {
                let indexed = store.index_file(Path::new(&arguments[2]))?;
                store.verify()?;
                let storage = StorageSecurityStatus::inspect(root)?;
                let mut output = io::stdout().lock();
                emit_indexed_source(&mut output, &indexed, 0, 0, &storage)
            })
        }
        "--index-directory" => {
            require_count(arguments, 3)?;
            with_store_mut(Path::new(&arguments[1]), |root, store| {
                let indexed = store.index_directory(Path::new(&arguments[2]))?;
                let changed = indexed.iter().filter(|source| source.changed).count();
                store.verify()?;
                let storage = StorageSecurityStatus::inspect(root)?;
                let mut output = io::stdout().lock();
                for source in &indexed {
                    emit_indexed_source(&mut output, source, 0, 0, &storage)?;
                }
                write_line(
                    &mut output,
                    &format!(
                        concat!(
                            "{{\"event\":\"directory_index_complete\",",
                            "\"namespace_id\":{},\"sources\":{},",
                            "\"changed\":{},\"storage_security\":{},",
                            "\"encrypted_at_rest\":{}}}"
                        ),
                        json_string(&store.namespace_id()),
                        indexed.len(),
                        changed,
                        storage.json(),
                        storage.encrypted_at_rest,
                    ),
                )
            })
        }
        "--search-root" | "--search-root-ascii-insensitive" => {
            require_count(arguments, 3)?;
            let query = arguments[2]
                .to_str()
                .ok_or_else(|| "DIRECT_QUERY_NOT_UTF8".to_owned())?;
            with_store(Path::new(&arguments[1]), |_root, store, storage| {
                store.verify()?;
                let result = store.search(
                    query,
                    command == "--search-root-ascii-insensitive",
                )?;
                let mut output = io::stdout().lock();
                emit_streaming_search(
                    &mut output,
                    &store.namespace_id(),
                    &result,
                    storage,
                )
            })
        }
        "--list-sources" => {
            require_count(arguments, 2)?;
            with_store(Path::new(&arguments[1]), |_root, store, storage| {
                store.verify()?;
                emit_sources(store, storage)
            })
        }
        "--verify-root" => {
            require_count(arguments, 2)?;
            with_store(Path::new(&arguments[1]), |_root, store, storage| {
                emit_verification(store, storage)
            })
        }
        "--retire-source" => {
            require_count(arguments, 3)?;
            let source_id = arguments[2]
                .to_str()
                .ok_or_else(|| "DIRECT_SOURCE_ID_INVALID".to_owned())?;
            with_store_mut(Path::new(&arguments[1]), |root, store| {
                let source = store.retire_source(source_id)?;
                store.verify()?;
                let storage = StorageSecurityStatus::inspect(root)?;
                write_stdout(&format!(
                    concat!(
                        "{{\"event\":\"source_retired\",",
                        "\"source_id\":{},\"revision_id\":{},",
                        "\"sequence\":{},\"active\":false,",
                        "\"storage_security\":{},\"encrypted_at_rest\":{}}}"
                    ),
                    json_string(&source.source_id),
                    json_string(&source.revision_id),
                    source.sequence,
                    storage.json(),
                    storage.encrypted_at_rest,
                ))
            })
        }
        "--read-revision" => {
            require_count(arguments, 5)?;
            let revision_id = arguments[2]
                .to_str()
                .ok_or_else(|| "DIRECT_REVISION_ID_INVALID".to_owned())?;
            let start = parse_u64(&arguments[3], "START_OFFSET")?;
            let end = parse_u64(&arguments[4], "END_OFFSET")?;
            if end.saturating_sub(start) > MAX_DIAGNOSTIC_REVISION_SLICE_BYTES {
                return Err("DIRECT_REVISION_SLICE_TOO_LARGE".to_owned());
            }
            with_store(Path::new(&arguments[1]), |_root, store, storage| {
                let slice = store.read_revision_range(revision_id, start, end)?;
                write_stdout(&format!(
                    concat!(
                        "{{\"event\":\"revision_slice\",",
                        "\"revision_id\":{},\"content_digest\":{},",
                        "\"byte_start\":{},\"byte_end\":{},",
                        "\"encoding\":\"hex\",\"bytes\":{},",
                        "\"source_backed\":true,\"storage_backend\":{},",
                        "\"encrypted_at_rest\":{}}}"
                    ),
                    json_string(&slice.revision_id),
                    json_string(&slice.content_digest),
                    slice.byte_start,
                    slice.byte_end,
                    json_string(&sha256::hex(&slice.bytes)),
                    json_string(storage.backend),
                    storage.encrypted_at_rest,
                ))
            })
        }
        "--repair-root" => {
            require_count(arguments, 2)?;
            let guard = DataRootGuard::acquire(Path::new(&arguments[1]))?;
            let repair = repair_control_log(guard.canonical_root())?;
            let store = DirectStore::open(guard.canonical_root())?;
            store.verify()?;
            let storage = StorageSecurityStatus::inspect(guard.canonical_root())?;
            write_stdout(&format!(
                concat!(
                    "{{\"event\":\"direct_store_repair_complete\",",
                    "\"namespace_id\":{},\"repaired\":{},",
                    "\"removed_bytes\":{},\"retained_events\":{},",
                    "\"last_sequence\":{},\"last_digest\":{},",
                    "\"storage_security\":{},\"encrypted_at_rest\":{}}}"
                ),
                json_string(&store.namespace_id()),
                repair.repaired,
                repair.removed_bytes,
                repair.retained_events,
                repair.last_sequence,
                json_string(&repair.last_digest),
                storage.json(),
                storage.encrypted_at_rest,
            ))
        }
        "--gc-root" => {
            require_count(arguments, 3)?;
            let mode = arguments[2]
                .to_str()
                .ok_or_else(|| "DIRECT_GC_MODE_INVALID".to_owned())?;
            let apply = match mode {
                "--dry-run" => false,
                "--apply" => true,
                _ => return Err("DIRECT_GC_MODE_INVALID".to_owned()),
            };
            let guard = DataRootGuard::acquire(Path::new(&arguments[1]))?;
            let store = DirectStore::open(guard.canonical_root())?;
            store.verify()?;
            let result = guarded_collect_orphan_revisions(
                guard.canonical_root(),
                apply,
            )?;
            store.verify()?;
            let storage = StorageSecurityStatus::inspect(guard.canonical_root())?;
            write_stdout(&format!(
                concat!(
                    "{{\"event\":\"direct_store_gc_complete\",",
                    "\"namespace_id\":{},\"applied\":{},",
                    "\"referenced_revisions\":{},\"scanned_objects\":{},",
                    "\"plaintext_objects\":{},\"protected_objects\":{},",
                    "\"temporary_objects\":{},",
                    "\"referenced_plaintext_objects\":{},",
                    "\"referenced_protected_objects\":{},",
                    "\"orphan_objects\":{},\"orphan_bytes\":{},",
                    "\"deleted_objects\":{},\"deleted_bytes\":{},",
                    "\"unexpected_objects\":{},\"storage_security\":{},",
                    "\"encrypted_at_rest\":{}}}"
                ),
                json_string(&store.namespace_id()),
                result.applied,
                result.referenced_revisions,
                result.scanned_objects,
                result.plaintext_objects,
                result.protected_objects,
                result.temporary_objects,
                result.referenced_plaintext_objects,
                result.referenced_protected_objects,
                result.orphan_objects,
                result.orphan_bytes,
                result.deleted_objects,
                result.deleted_bytes,
                result.unexpected_objects,
                storage.json(),
                storage.encrypted_at_rest,
            ))
        }
        _ => Err("UNKNOWN_PERSISTENT_COMMAND".to_owned()),
    }
}

fn with_store<T>(
    root: &Path,
    operation: impl FnOnce(
        &Path,
        &DirectStore,
        &StorageSecurityStatus,
    ) -> Result<T, String>,
) -> Result<T, String> {
    let guard = DataRootGuard::acquire(root)?;
    let store = DirectStore::open(guard.canonical_root())?;
    let storage = StorageSecurityStatus::inspect(guard.canonical_root())?;
    operation(guard.canonical_root(), &store, &storage)
}

fn with_store_mut<T>(
    root: &Path,
    operation: impl FnOnce(&Path, &mut DirectStore) -> Result<T, String>,
) -> Result<T, String> {
    let guard = DataRootGuard::acquire(root)?;
    let mut store = DirectStore::open(guard.canonical_root())?;
    operation(guard.canonical_root(), &mut store)
}

fn emit_verification(
    store: &DirectStore,
    storage: &StorageSecurityStatus,
) -> Result<(), String> {
    let verification = store.verify()?;
    write_stdout(&format!(
        concat!(
            "{{\"event\":\"direct_store_verified\",",
            "\"namespace_id\":{},\"source_events\":{},",
            "\"registered_sources\":{},\"active_sources\":{},",
            "\"referenced_revisions\":{},\"verified_revisions\":{},",
            "\"total_revision_bytes\":{},\"source_backed\":true,",
            "\"storage_security\":{},\"encrypted_at_rest\":{}}}"
        ),
        json_string(&store.namespace_id()),
        verification.source_events,
        verification.registered_sources,
        verification.active_sources,
        verification.referenced_revisions,
        verification.verified_revisions,
        verification.total_revision_bytes,
        storage.json(),
        storage.encrypted_at_rest,
    ))
}

fn emit_sources(
    store: &DirectStore,
    storage: &StorageSecurityStatus,
) -> Result<(), String> {
    let sources = store.list_sources();
    let mut output = io::stdout().lock();
    for source in &sources {
        emit_source(&mut output, source)?;
    }
    write_line(
        &mut output,
        &format!(
            concat!(
                "{{\"event\":\"source_list_complete\",",
                "\"namespace_id\":{},\"sources\":{},",
                "\"storage_security\":{},\"encrypted_at_rest\":{}}}"
            ),
            json_string(&store.namespace_id()),
            sources.len(),
            storage.json(),
            storage.encrypted_at_rest,
        ),
    )
}

fn emit_source(writer: &mut impl Write, source: &SourceSummary) -> Result<(), String> {
    write_line(
        writer,
        &format!(
            concat!(
                "{{\"event\":\"source\",\"source_id\":{},",
                "\"revision_id\":{},\"content_digest\":{},",
                "\"path_digest\":{},\"byte_length\":{},",
                "\"identity_strength\":{},\"active\":{},",
                "\"sequence\":{},\"diagnostic_internal_identifiers\":true}}"
            ),
            json_string(&source.source_id),
            json_string(&source.revision_id),
            json_string(&source.content_digest),
            json_string(&source.path_digest),
            source.byte_length,
            json_string(source.identity_strength),
            source.active,
            source.sequence,
        ),
    )
}

fn require_count(arguments: &[OsString], expected: usize) -> Result<(), String> {
    if arguments.len() == expected {
        Ok(())
    } else {
        Err("USAGE_ERROR".to_owned())
    }
}

fn parse_u64(value: &OsString, label: &str) -> Result<u64, String> {
    value
        .to_str()
        .ok_or_else(|| format!("INVALID_{label}"))?
        .parse::<u64>()
        .map_err(|_| format!("INVALID_{label}"))
}

fn write_stdout(value: &str) -> Result<(), String> {
    let mut output = io::stdout().lock();
    write_line(&mut output, value)
}

fn emit_process_error(error: &str) -> ExitCode {
    eprintln!("{{\"error\":{}}}", json_string(error));
    ExitCode::from(2)
}

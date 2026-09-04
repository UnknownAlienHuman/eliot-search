//! Long-lived owner-fenced DIRECT runtime over bounded stdio commands.
//!
//! The runtime owns one data-root lock, one verified DIRECT store, and a finite
//! process-local continuation catalog. Tokens are session locators only; every
//! page is bound to current source state and is invalidated by source mutation.

use std::env;
use std::ffi::OsString;
use std::io::{self, BufRead};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::continuation::{
    ContinuationCatalog, ContinuationError, DEFAULT_PAGE_SIZE, MAX_PAGE_SIZE,
};
use crate::development::{DataRootGuard, Health, MAX_SCAN_QUERY_BYTES};
use crate::direct_store::DirectStore;
use crate::directory_manifest::{sync_directory, verify_directory_manifests};
use crate::maintenance_guard::guarded_collect_orphan_revisions;
use crate::service_output::{
    emit_indexed_source, emit_search_page, emit_streaming_search, json_string,
    write_error, write_line,
};
use crate::sha256;

const PROTOCOL_VERSION: u16 = 1;
const MAX_COMMAND_BYTES: usize = 256 * 1024;
const MAX_PATH_BYTES: usize = 32 * 1024;
const MAX_SERVICE_REVISION_SLICE_BYTES: u64 = 24 * 1024;

/// Intercepts `--serve-data-root ROOT` before one-shot command dispatch.
pub(crate) fn maybe_run() -> Option<ExitCode> {
    let arguments = env::args_os().skip(1).collect::<Vec<_>>();
    if arguments.first().and_then(|value| value.to_str()) != Some("--serve-data-root") {
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

fn run_service(root: &Path) -> Result<(), String> {
    let guard = DataRootGuard::acquire(root)?;
    let mut store = DirectStore::open(guard.canonical_root())?;
    let verification = store.verify()?;
    let manifests = verify_directory_manifests(
        guard.canonical_root(),
        &store.namespace_id(),
    )?;
    let mut continuations = ContinuationCatalog::new(&store.namespace_id());

    let input = io::stdin();
    let mut reader = input.lock();
    let output = io::stdout();
    let mut writer = output.lock();
    write_line(
        &mut writer,
        &format!(
            concat!(
                "{{\"event\":\"data_root_ready\",",
                "\"protocol_version\":{},\"namespace_id\":\"{}\",",
                "\"registered_sources\":{},\"active_sources\":{},",
                "\"directory_manifests\":{},",
                "\"runtime_owner_ready\":true,\"direct_store_ready\":true,",
                "\"source_backed_search_available\":true,",
                "\"paged_search_available\":true,",
                "\"default_page_size\":{},\"max_page_size\":{},",
                "\"encrypted_at_rest\":false}}"
            ),
            PROTOCOL_VERSION,
            store.namespace_id(),
            verification.registered_sources,
            verification.active_sources,
            manifests.manifest_files,
            DEFAULT_PAGE_SIZE,
            MAX_PAGE_SIZE,
        ),
    )?;

    loop {
        let line = match read_bounded_line(&mut reader) {
            Ok(Some(line)) => line,
            Ok(None) => break,
            Err(error) => {
                write_error(&mut writer, &error)?;
                continue;
            }
        };
        let command = match std::str::from_utf8(&line) {
            Ok(command) => command,
            Err(_) => {
                write_error(&mut writer, "SERVICE_COMMAND_NOT_UTF8")?;
                continue;
            }
        };
        match execute_command(
            command,
            &mut store,
            &mut continuations,
            guard.canonical_root(),
            &mut writer,
        ) {
            Ok(ServiceControl::Continue) => {}
            Ok(ServiceControl::Stop) => break,
            Err(error) => write_error(&mut writer, &error)?,
        }
    }
    write_line(
        &mut writer,
        "{\"event\":\"data_root_stopped\",\"clean\":true}",
    )?;
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ServiceControl {
    Continue,
    Stop,
}

fn execute_command(
    command: &str,
    store: &mut DirectStore,
    continuations: &mut ContinuationCatalog,
    canonical_root: &Path,
    writer: &mut impl std::io::Write,
) -> Result<ServiceControl, String> {
    let fields = command.split('\t').collect::<Vec<_>>();
    let Some(name) = fields.first().copied() else {
        return Err("SERVICE_COMMAND_EMPTY".to_owned());
    };
    match (name, fields.as_slice()) {
        ("health", [_]) => {
            let verification = store.verify()?;
            let manifests = verify_directory_manifests(
                canonical_root,
                &store.namespace_id(),
            )?;
            write_line(
                writer,
                &format!(
                    concat!(
                        "{{\"event\":\"health\",\"namespace_id\":\"{}\",",
                        "\"registered_sources\":{},\"active_sources\":{},",
                        "\"verified_revisions\":{},\"directory_manifests\":{},",
                        "\"live_continuations\":{},\"retained_continuation_matches\":{},",
                        "\"health\":{}}}"
                    ),
                    store.namespace_id(),
                    verification.registered_sources,
                    verification.active_sources,
                    verification.verified_revisions,
                    manifests.manifest_files,
                    continuations.live_count(),
                    continuations.retained_matches(),
                    Health::DIRECT_STORE.json(),
                ),
            )?;
        }
        ("version", [_]) => write_line(
            writer,
            &format!(
                "{{\"event\":\"version\",\"binary\":\"eliot-searchd\",\"version\":\"{}\",\"protocol_version\":{}}}",
                env!("CARGO_PKG_VERSION"),
                PROTOCOL_VERSION,
            ),
        )?,
        ("shutdown", [_]) => {
            write_line(
                writer,
                "{\"event\":\"draining\",\"accepted\":true}",
            )?;
            return Ok(ServiceControl::Stop);
        }
        ("verify", [_]) => emit_verification(writer, store, canonical_root)?,
        ("verify-directory-manifests", [_]) => {
            let manifests = verify_directory_manifests(
                canonical_root,
                &store.namespace_id(),
            )?;
            write_line(
                writer,
                &format!(
                    concat!(
                        "{{\"event\":\"directory_manifests_verified\",",
                        "\"namespace_id\":\"{}\",\"manifest_files\":{},",
                        "\"directories\":{},\"current_entries\":{},",
                        "\"highest_generation\":{},\"source_backed\":true,",
                        "\"encrypted_at_rest\":false}}"
                    ),
                    store.namespace_id(),
                    manifests.manifest_files,
                    manifests.directories,
                    manifests.current_entries,
                    manifests.highest_generation,
                ),
            )?;
        }
        ("list-sources", [_]) => emit_source_list(writer, store)?,
        ("index-file", [_, path_hex]) => {
            let indexed = store.index_file(&decode_path(path_hex)?)?;
            let invalidated = if indexed.changed {
                continuations.invalidate_all()
            } else {
                0
            };
            emit_indexed_source(writer, &indexed, invalidated)?;
        }
        ("index-directory", [_, path_hex]) => {
            let indexed = store.index_directory(&decode_path(path_hex)?)?;
            let changed = indexed.iter().filter(|source| source.changed).count();
            let invalidated = if changed == 0 {
                0
            } else {
                continuations.invalidate_all()
            };
            for source in &indexed {
                emit_indexed_source(writer, source, 0)?;
            }
            write_line(
                writer,
                &format!(
                    concat!(
                        "{{\"event\":\"directory_index_complete\",",
                        "\"namespace_id\":\"{}\",\"sources\":{},",
                        "\"changed\":{},\"invalidated_continuations\":{},",
                        "\"source_backed\":true,\"encrypted_at_rest\":false}}"
                    ),
                    store.namespace_id(),
                    indexed.len(),
                    changed,
                    invalidated,
                ),
            )?;
        }
        ("sync-directory", [_, path_hex]) => {
            let directory = decode_path(path_hex)?;
            let result = sync_directory(store, canonical_root, &directory)?;
            let invalidated = if result.changed_sources == 0 && result.retired_sources == 0 {
                0
            } else {
                continuations.invalidate_all()
            };
            store.verify()?;
            let manifests = verify_directory_manifests(
                canonical_root,
                &store.namespace_id(),
            )?;
            write_line(
                writer,
                &format!(
                    concat!(
                        "{{\"event\":\"directory_sync_complete\",",
                        "\"namespace_id\":\"{}\",\"directory_digest\":\"{}\",",
                        "\"previous_generation\":{},\"generation\":{},",
                        "\"previous_sources\":{},\"indexed_sources\":{},",
                        "\"changed_sources\":{},\"missing_sources\":{},",
                        "\"retired_sources\":{},\"moved_or_rebound_sources\":{},",
                        "\"manifest_digest\":\"{}\",\"manifest_files\":{},",
                        "\"invalidated_continuations\":{},",
                        "\"source_backed\":true,\"encrypted_at_rest\":false}}"
                    ),
                    result.namespace_id,
                    result.directory_digest,
                    result
                        .previous_generation
                        .map_or_else(|| "null".to_owned(), |value| value.to_string()),
                    result.generation,
                    result.previous_sources,
                    result.indexed_sources,
                    result.changed_sources,
                    result.missing_sources,
                    result.retired_sources,
                    result.moved_or_rebound_sources,
                    result.manifest_digest,
                    manifests.manifest_files,
                    invalidated,
                ),
            )?;
        }
        ("search", [_, mode, query_hex]) => {
            let query = decode_query(query_hex)?;
            let result = store.search(&query, parse_search_mode(mode)?)?;
            emit_streaming_search(writer, &store.namespace_id(), &result)?;
        }
        ("search-page", [_, mode, page_size, query_hex]) => {
            let query = decode_query(query_hex)?;
            let page_size = parse_page_size(page_size)?;
            let result = store.search(&query, parse_search_mode(mode)?)?;
            let page = continuations
                .create_page(store, result, page_size)
                .map_err(continuation_error)?;
            emit_search_page(writer, &page)?;
        }
        ("continue", [_, token, page_size]) => {
            let page_size = parse_page_size(page_size)?;
            let page = continuations
                .continue_page(store, token, page_size)
                .map_err(continuation_error)?;
            emit_search_page(writer, &page)?;
        }
        ("retire", [_, source_id]) => {
            let source = store.retire_source(source_id)?;
            let invalidated = continuations.invalidate_all();
            write_line(
                writer,
                &format!(
                    concat!(
                        "{{\"event\":\"source_retired\",",
                        "\"source_id\":\"{}\",\"revision_id\":\"{}\",",
                        "\"sequence\":{},\"active\":false,",
                        "\"invalidated_continuations\":{}}}"
                    ),
                    source.source_id,
                    source.revision_id,
                    source.sequence,
                    invalidated,
                ),
            )?;
        }
        ("read-revision", [_, revision_id, start, end]) => {
            let start = parse_u64(start, "SERVICE_START_OFFSET_INVALID")?;
            let end = parse_u64(end, "SERVICE_END_OFFSET_INVALID")?;
            if end.saturating_sub(start) > MAX_SERVICE_REVISION_SLICE_BYTES {
                return Err("SERVICE_REVISION_SLICE_TOO_LARGE".to_owned());
            }
            let slice = store.read_revision_range(revision_id, start, end)?;
            write_line(
                writer,
                &format!(
                    concat!(
                        "{{\"event\":\"revision_slice\",",
                        "\"revision_id\":\"{}\",\"content_digest\":\"{}\",",
                        "\"byte_start\":{},\"byte_end\":{},",
                        "\"encoding\":\"hex\",\"bytes\":\"{}\",",
                        "\"source_backed\":true,\"encrypted_at_rest\":false}}"
                    ),
                    slice.revision_id,
                    slice.content_digest,
                    slice.byte_start,
                    slice.byte_end,
                    sha256::hex(&slice.bytes),
                ),
            )?;
        }
        ("gc", [_, mode]) => {
            let apply = match *mode {
                "dry-run" => false,
                "apply" => true,
                _ => return Err("SERVICE_GC_MODE_INVALID".to_owned()),
            };
            store.verify()?;
            let result = guarded_collect_orphan_revisions(canonical_root, apply)?;
            write_line(
                writer,
                &format!(
                    concat!(
                        "{{\"event\":\"direct_store_gc_complete\",",
                        "\"namespace_id\":\"{}\",\"applied\":{},",
                        "\"referenced_revisions\":{},\"scanned_objects\":{},",
                        "\"orphan_objects\":{},\"orphan_bytes\":{},",
                        "\"deleted_objects\":{},\"deleted_bytes\":{},",
                        "\"unexpected_objects\":{}}}"
                    ),
                    store.namespace_id(),
                    result.applied,
                    result.referenced_revisions,
                    result.scanned_objects,
                    result.orphan_objects,
                    result.orphan_bytes,
                    result.deleted_objects,
                    result.deleted_bytes,
                    result.unexpected_objects,
                ),
            )?;
        }
        _ => return Err("SERVICE_COMMAND_INVALID".to_owned()),
    }
    Ok(ServiceControl::Continue)
}

fn emit_verification(
    writer: &mut impl std::io::Write,
    store: &DirectStore,
    canonical_root: &Path,
) -> Result<(), String> {
    let verification = store.verify()?;
    let manifests = verify_directory_manifests(canonical_root, &store.namespace_id())?;
    write_line(
        writer,
        &format!(
            concat!(
                "{{\"event\":\"direct_store_verified\",",
                "\"namespace_id\":\"{}\",\"source_events\":{},",
                "\"registered_sources\":{},\"active_sources\":{},",
                "\"referenced_revisions\":{},\"verified_revisions\":{},",
                "\"total_revision_bytes\":{},\"manifest_files\":{},",
                "\"manifest_directories\":{},\"source_backed\":true,",
                "\"encrypted_at_rest\":false}}"
            ),
            store.namespace_id(),
            verification.source_events,
            verification.registered_sources,
            verification.active_sources,
            verification.referenced_revisions,
            verification.verified_revisions,
            verification.total_revision_bytes,
            manifests.manifest_files,
            manifests.directories,
        ),
    )
}

fn emit_source_list(
    writer: &mut impl std::io::Write,
    store: &DirectStore,
) -> Result<(), String> {
    let sources = store.list_sources();
    for source in &sources {
        write_line(
            writer,
            &format!(
                concat!(
                    "{{\"event\":\"source\",\"source_id\":\"{}\",",
                    "\"revision_id\":\"{}\",\"content_digest\":\"{}\",",
                    "\"path_digest\":\"{}\",\"byte_length\":{},",
                    "\"identity_strength\":\"{}\",\"active\":{},",
                    "\"sequence\":{}}}"
                ),
                source.source_id,
                source.revision_id,
                source.content_digest,
                source.path_digest,
                source.byte_length,
                source.identity_strength,
                source.active,
                source.sequence,
            ),
        )?;
    }
    write_line(
        writer,
        &format!(
            "{{\"event\":\"source_list_complete\",\"namespace_id\":\"{}\",\"sources\":{}}}",
            store.namespace_id(),
            sources.len(),
        ),
    )
}

fn continuation_error(error: ContinuationError) -> String {
    error.code().to_owned()
}

fn parse_search_mode(value: &str) -> Result<bool, String> {
    match value {
        "sensitive" => Ok(false),
        "ascii-insensitive" => Ok(true),
        _ => Err("SERVICE_SEARCH_MODE_INVALID".to_owned()),
    }
}

fn parse_page_size(value: &str) -> Result<usize, String> {
    let value = value
        .parse::<usize>()
        .map_err(|_| "DIRECT_CONTINUATION_PAGE_SIZE_INVALID".to_owned())?;
    if value == 0 || value > MAX_PAGE_SIZE {
        Err("DIRECT_CONTINUATION_PAGE_SIZE_INVALID".to_owned())
    } else {
        Ok(value)
    }
}

fn decode_query(value: &str) -> Result<String, String> {
    String::from_utf8(decode_hex(value, MAX_SCAN_QUERY_BYTES)?)
        .map_err(|_| "SERVICE_QUERY_NOT_UTF8".to_owned())
}

fn read_bounded_line(reader: &mut impl BufRead) -> Result<Option<Vec<u8>>, String> {
    let mut output = Vec::new();
    let mut too_large = false;
    loop {
        let buffer = reader
            .fill_buf()
            .map_err(|error| format!("SERVICE_READ_ERROR:{error}"))?;
        if buffer.is_empty() {
            return if output.is_empty() {
                Ok(None)
            } else if too_large {
                Err("SERVICE_COMMAND_TOO_LARGE".to_owned())
            } else {
                Ok(Some(output))
            };
        }
        let newline = buffer.iter().position(|byte| *byte == b'\n');
        let consumed = newline.map_or(buffer.len(), |index| index + 1);
        let content_length = newline.unwrap_or(buffer.len());
        if !too_large {
            if output.len().saturating_add(content_length) > MAX_COMMAND_BYTES {
                too_large = true;
                output.clear();
            } else {
                output.extend_from_slice(&buffer[..content_length]);
            }
        }
        reader.consume(consumed);
        if newline.is_some() {
            if output.last() == Some(&b'\r') {
                output.pop();
            }
            return if too_large {
                Err("SERVICE_COMMAND_TOO_LARGE".to_owned())
            } else {
                Ok(Some(output))
            };
        }
    }
}

fn decode_path(value: &str) -> Result<PathBuf, String> {
    decode_os_string(decode_hex(value, MAX_PATH_BYTES)?).map(PathBuf::from)
}

#[cfg(unix)]
fn decode_os_string(bytes: Vec<u8>) -> Result<OsString, String> {
    use std::os::unix::ffi::OsStringExt;
    Ok(OsString::from_vec(bytes))
}

#[cfg(windows)]
fn decode_os_string(bytes: Vec<u8>) -> Result<OsString, String> {
    use std::os::windows::ffi::OsStringExt;
    if bytes.len() % 2 != 0 {
        return Err("SERVICE_PATH_ENCODING_INVALID".to_owned());
    }
    let wide = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect::<Vec<_>>();
    Ok(OsString::from_wide(&wide))
}

#[cfg(not(any(unix, windows)))]
fn decode_os_string(bytes: Vec<u8>) -> Result<OsString, String> {
    String::from_utf8(bytes)
        .map(OsString::from)
        .map_err(|_| "SERVICE_PATH_ENCODING_INVALID".to_owned())
}

fn decode_hex(value: &str, max_bytes: usize) -> Result<Vec<u8>, String> {
    if value.len() % 2 != 0 || value.len() / 2 > max_bytes {
        return Err("SERVICE_HEX_INVALID".to_owned());
    }
    let mut output = Vec::with_capacity(value.len() / 2);
    for pair in value.as_bytes().chunks_exact(2) {
        let high = hex_nibble(pair[0]).ok_or_else(|| "SERVICE_HEX_INVALID".to_owned())?;
        let low = hex_nibble(pair[1]).ok_or_else(|| "SERVICE_HEX_INVALID".to_owned())?;
        output.push((high << 4) | low);
    }
    Ok(output)
}

fn hex_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

fn parse_u64(value: &str, error: &'static str) -> Result<u64, String> {
    value.parse::<u64>().map_err(|_| error.to_owned())
}

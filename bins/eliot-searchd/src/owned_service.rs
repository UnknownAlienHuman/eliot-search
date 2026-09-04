//! Long-lived owner-fenced DIRECT service over bounded stdio commands.
//!
//! Command arguments are hexadecimal so tabs, newlines, and native path bytes
//! cannot escape framing. This is a development local transport; the production
//! authenticated endpoint remains a separate adapter.

use std::env;
use std::ffi::OsString;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::development::{DataRootGuard, Health, MAX_SCAN_QUERY_BYTES};
use crate::direct_store::{DirectStore, IndexedSource, StoreSearchResult};
use crate::directory_manifest::{sync_directory, verify_directory_manifests};
use crate::maintenance_guard::guarded_collect_orphan_revisions;
use crate::sha256;

const PROTOCOL_VERSION: u16 = 1;
const MAX_COMMAND_BYTES: usize = 256 * 1024;
const MAX_PATH_BYTES: usize = 32 * 1024;
const MAX_RESPONSE_BYTES: usize = 64 * 1024;
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
                "\"encrypted_at_rest\":false}}"
            ),
            PROTOCOL_VERSION,
            store.namespace_id(),
            verification.registered_sources,
            verification.active_sources,
            manifests.manifest_files,
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
    canonical_root: &Path,
    writer: &mut impl Write,
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
                        "\"health\":{}}}"
                    ),
                    store.namespace_id(),
                    verification.registered_sources,
                    verification.active_sources,
                    verification.verified_revisions,
                    manifests.manifest_files,
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
        ("verify", [_]) => {
            let verification = store.verify()?;
            let manifests = verify_directory_manifests(
                canonical_root,
                &store.namespace_id(),
            )?;
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
            )?;
        }
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
        ("list-sources", [_]) => {
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
            )?;
        }
        ("index-file", [_, path_hex]) => {
            let indexed = store.index_file(&decode_path(path_hex)?)?;
            emit_indexed_source(writer, &indexed)?;
        }
        ("index-directory", [_, path_hex]) => {
            let indexed = store.index_directory(&decode_path(path_hex)?)?;
            let changed = indexed.iter().filter(|source| source.changed).count();
            for source in &indexed {
                emit_indexed_source(writer, source)?;
            }
            write_line(
                writer,
                &format!(
                    concat!(
                        "{{\"event\":\"directory_index_complete\",",
                        "\"namespace_id\":\"{}\",\"sources\":{},",
                        "\"changed\":{},\"source_backed\":true,",
                        "\"encrypted_at_rest\":false}}"
                    ),
                    store.namespace_id(),
                    indexed.len(),
                    changed,
                ),
            )?;
        }
        ("sync-directory", [_, path_hex]) => {
            let directory = decode_path(path_hex)?;
            let result = sync_directory(store, canonical_root, &directory)?;
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
                ),
            )?;
        }
        ("search", [_, mode, query_hex]) => {
            let query = String::from_utf8(decode_hex(query_hex, MAX_SCAN_QUERY_BYTES)?)
                .map_err(|_| "SERVICE_QUERY_NOT_UTF8".to_owned())?;
            let ascii_insensitive = match *mode {
                "sensitive" => false,
                "ascii-insensitive" => true,
                _ => return Err("SERVICE_SEARCH_MODE_INVALID".to_owned()),
            };
            let result = store.search(&query, ascii_insensitive)?;
            emit_store_search(writer, &store.namespace_id(), &result)?;
        }
        ("retire", [_, source_id]) => {
            let source = store.retire_source(source_id)?;
            write_line(
                writer,
                &format!(
                    concat!(
                        "{{\"event\":\"source_retired\",",
                        "\"source_id\":\"{}\",\"revision_id\":\"{}\",",
                        "\"sequence\":{},\"active\":false}}"
                    ),
                    source.source_id, source.revision_id, source.sequence,
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

fn emit_indexed_source(
    writer: &mut impl Write,
    source: &IndexedSource,
) -> Result<(), String> {
    write_line(
        writer,
        &format!(
            concat!(
                "{{\"event\":\"source_indexed\",",
                "\"source_id\":\"{}\",\"revision_id\":\"{}\",",
                "\"content_digest\":\"{}\",\"path_digest\":\"{}\",",
                "\"byte_length\":{},\"identity_strength\":\"{}\",",
                "\"changed\":{},\"source_backed\":true,",
                "\"durable_revision\":true,\"encrypted_at_rest\":false}}"
            ),
            source.source_id,
            source.revision_id,
            source.content_digest,
            source.path_digest,
            source.byte_length,
            source.identity_strength,
            source.changed,
        ),
    )
}

fn emit_store_search(
    writer: &mut impl Write,
    namespace_id: &str,
    result: &StoreSearchResult,
) -> Result<(), String> {
    write_line(
        writer,
        &format!(
            concat!(
                "{{\"event\":\"corpus_search_started\",",
                "\"namespace_id\":\"{}\",\"registered_sources\":{},",
                "\"active_sources\":{},\"source_backed\":true,",
                "\"durable_revision\":true,\"encrypted_at_rest\":false}}"
            ),
            namespace_id,
            result.registered_sources,
            result.active_sources,
        ),
    )?;
    for gap in &result.gaps {
        write_line(
            writer,
            &format!(
                concat!(
                    "{{\"event\":\"source_gap\",\"source_id\":\"{}\",",
                    "\"revision_id\":\"{}\",\"reason\":\"{}\"}}"
                ),
                gap.source_id, gap.revision_id, gap.reason,
            ),
        )?;
    }
    for item in &result.matches {
        write_line(
            writer,
            &format!(
                concat!(
                    "{{\"event\":\"match\",\"source_id\":\"{}\",",
                    "\"revision_id\":\"{}\",\"content_digest\":\"{}\",",
                    "\"path_digest\":\"{}\",\"evidence_id\":\"{}\",",
                    "\"byte_start\":{},\"byte_end\":{},",
                    "\"line\":{},\"column_bytes\":{},",
                    "\"source_backed\":true}}"
                ),
                item.source_id,
                item.revision_id,
                item.content_digest,
                item.path_digest,
                item.evidence_id,
                item.byte_start,
                item.byte_end,
                item.line,
                item.column_bytes,
            ),
        )?;
    }
    write_line(
        writer,
        &format!(
            concat!(
                "{{\"event\":\"corpus_search_complete\",",
                "\"matches\":{},\"gaps\":{},\"searched_sources\":{},",
                "\"searched_bytes\":{},\"active_sources\":{},",
                "\"complete\":{},\"match_limit_reached\":{},",
                "\"source_budget_exhausted\":{},",
                "\"byte_budget_exhausted\":{},\"source_backed\":true,",
                "\"encrypted_at_rest\":false}}"
            ),
            result.matches.len(),
            result.gaps.len(),
            result.searched_sources,
            result.searched_bytes,
            result.active_sources,
            result.complete,
            result.match_limit_reached,
            result.source_budget_exhausted,
            result.byte_budget_exhausted,
        ),
    )
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

fn write_line(writer: &mut impl Write, value: &str) -> Result<(), String> {
    if value.len() > MAX_RESPONSE_BYTES {
        return Err("SERVICE_RESPONSE_TOO_LARGE".to_owned());
    }
    writer
        .write_all(value.as_bytes())
        .and_then(|()| writer.write_all(b"\n"))
        .and_then(|()| writer.flush())
        .map_err(|error| format!("SERVICE_WRITE_ERROR:{error}"))
}

fn write_error(writer: &mut impl Write, error: &str) -> Result<(), String> {
    write_line(
        writer,
        &format!("{{\"event\":\"error\",\"error\":{}}}", json_string(error)),
    )
}

fn json_string(value: &str) -> String {
    let mut output = String::with_capacity(value.len().saturating_add(2));
    output.push('"');
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character if character.is_control() => {
                use std::fmt::Write as _;
                let _ = write!(&mut output, "\\u{:04x}", u32::from(character));
            }
            character => output.push(character),
        }
    }
    output.push('"');
    output
}

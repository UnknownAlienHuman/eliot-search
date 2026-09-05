//! Command application for the ELIOT Search daemon binary.

use std::io::{self, BufRead, Write};
use std::path::Path;
use std::process::ExitCode;

use crate::development::{
    DataRootGuard, Health, ScanResult, read_file_bounded, read_stdin_bounded,
    scan_text,
};
use crate::direct_store::{DirectStore, IndexedSource, StoreSearchResult};
use crate::maintenance::{collect_orphan_revisions, repair_control_log};
use crate::sha256;

const PROTOCOL_VERSION: u16 = 1;
const MAX_COMMAND_BYTES: usize = 1_024;
const MAX_RESPONSE_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Command {
    Health,
    Version,
    Shutdown,
}

impl Command {
    fn parse(value: &str) -> Result<Self, &'static str> {
        match value.trim() {
            "health" | "HEALTH" => Ok(Self::Health),
            "version" | "VERSION" => Ok(Self::Version),
            "shutdown" | "SHUTDOWN" => Ok(Self::Shutdown),
            _ => Err("UNSUPPORTED_COMMAND"),
        }
    }
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
        "  eliot-searchd --serve-data-root ROOT\n\n",
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
        "PERSISTENT SOURCE-ROOT REGISTRATION:\n",
        "  eliot-searchd --source-roots ROOT\n",
        "  eliot-searchd --register-source-root ROOT DIRECTORY\n",
        "  eliot-searchd --unregister-source-root ROOT DIRECTORY\n",
        "  eliot-searchd --sync-source-roots ROOT\n",
        "Registration controls explicit observation, not access grants or purge.\n",
        "Unregistering does not revoke already retained revisions.\n\n",
        "MAINTENANCE:\n",
        "  eliot-searchd --repair-root ROOT\n",
        "  eliot-searchd --gc-root ROOT --dry-run\n",
        "  eliot-searchd --gc-root ROOT --apply\n\n",
        "Persistent DIRECT search is source-backed by verified immutable ",
        "revision objects. The current development object store is plaintext and ",
        "reports encrypted_at_rest=false.\n",
    )
}

fn version_json() -> String {
    format!(
        "{{\"binary\":\"eliot-searchd\",\"version\":\"{}\",\"protocol_version\":{}}}",
        env!("CARGO_PKG_VERSION"),
        PROTOCOL_VERSION,
    )
}

fn write_response(output: &mut impl Write, value: &str) -> io::Result<()> {
    if value.len() > MAX_RESPONSE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "RESPONSE_TOO_LARGE",
        ));
    }
    output.write_all(value.as_bytes())?;
    output.write_all(b"\n")?;
    output.flush()
}

fn serve_stdio(health: Health) -> io::Result<()> {
    serve_control(health, &mut io::stdin().lock(), &mut io::stdout().lock())
}

fn serve_control(
    health: Health,
    input: &mut impl BufRead,
    output: &mut impl Write,
) -> io::Result<()> {
    write_response(
        output,
        &format!(
            concat!(
                "{{\"event\":\"ready\",\"protocol_version\":{},",
                "\"runtime_owner_ready\":{},",
                "\"direct_store_ready\":{},",
                "\"source_backed_search_available\":{}}}"
            ),
            PROTOCOL_VERSION,
            health.runtime_owner_ready,
            health.direct_store_ready,
            health.source_backed_search_available,
        ),
    )?;

    loop {
        let line = match crate::protocol_io::read_line(input, MAX_COMMAND_BYTES) {
            Ok(Some(line)) => line,
            Ok(None) => return Ok(()),
            Err(error) => {
                write_response(output, &format!("{{\"error\":\"{}\"}}", error.code()))?;
                // Do not drain an unbounded frame or execute its unread suffix.
                return Err(io::Error::new(io::ErrorKind::InvalidData, error));
            }
        };
        match Command::parse(&line) {
            Ok(Command::Health) => write_response(output, &health.json())?,
            Ok(Command::Version) => write_response(output, &version_json())?,
            Ok(Command::Shutdown) => {
                write_response(
                    output,
                    "{\"status\":\"draining\",\"accepted\":true}",
                )?;
                write_response(
                    output,
                    "{\"status\":\"stopped\",\"clean\":true}",
                )?;
                return Ok(());
            }
            Err(code) => write_response(
                output,
                &format!("{{\"error\":\"{code}\"}}"),
            )?,
        }
    }
}

fn open_direct_store(root: &Path) -> Result<(DataRootGuard, DirectStore), String> {
    let guard = DataRootGuard::acquire(root)?;
    let store = DirectStore::open(guard.canonical_root())?;
    Ok((guard, store))
}

fn emit_one_shot_scan(
    source: &str,
    query: &str,
    ascii_insensitive: bool,
    text: String,
    same_handle_verified: bool,
    source_backed: bool,
) -> Result<(), String> {
    let result = scan_text(&text, query, ascii_insensitive)?;
    let content_digest = sha256::hex(&sha256::digest(text.as_bytes()));
    println!(
        concat!(
            "{{\"event\":\"scan_started\",\"source\":\"{}\",",
            "\"mode\":\"{}\",\"input_bytes\":{},",
            "\"content_digest\":\"{}\",",
            "\"same_handle_verified\":{},\"source_backed\":{},",
            "\"durable_revision\":false,\"encrypted_at_rest\":false}}"
        ),
        source,
        if ascii_insensitive {
            "ascii_insensitive"
        } else {
            "sensitive"
        },
        result.coverage.input_bytes,
        content_digest,
        same_handle_verified,
        source_backed,
    );
    emit_scan_matches(&result, &content_digest, source_backed);
    Ok(())
}

fn emit_scan_matches(result: &ScanResult, content_digest: &str, source_backed: bool) {
    for item in &result.matches {
        let evidence_id = sha256::hex(&sha256::digest_parts(
            b"eliot-search/one-shot-evidence/v1",
            &[
                content_digest.as_bytes(),
                &u64::try_from(item.byte_start)
                    .unwrap_or(u64::MAX)
                    .to_be_bytes(),
                &u64::try_from(item.byte_end)
                    .unwrap_or(u64::MAX)
                    .to_be_bytes(),
            ],
        ));
        println!(
            concat!(
                "{{\"event\":\"match\",\"evidence_id\":\"{}\",",
                "\"byte_start\":{},\"byte_end\":{},",
                "\"line\":{},\"column_bytes\":{},",
                "\"source_backed\":{}}}"
            ),
            evidence_id,
            item.byte_start,
            item.byte_end,
            item.line,
            item.column_bytes,
            source_backed,
        );
    }
    println!(
        concat!(
            "{{\"event\":\"scan_complete\",\"matches\":{},",
            "\"match_limit_reached\":{},\"complete\":{},",
            "\"source_backed\":{}}}"
        ),
        result.matches.len(),
        result.coverage.match_limit_reached,
        result.coverage.complete,
        source_backed,
    );
}

fn emit_indexed_source(source: &IndexedSource) {
    println!(
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
    );
}

fn emit_store_search(namespace_id: &str, result: &StoreSearchResult) {
    println!(
        concat!(
            "{{\"event\":\"corpus_search_started\",",
            "\"namespace_id\":\"{}\",\"registered_sources\":{},",
            "\"active_sources\":{},\"source_backed\":true,",
            "\"durable_revision\":true,\"encrypted_at_rest\":false}}"
        ),
        namespace_id,
        result.registered_sources,
        result.active_sources,
    );
    for gap in &result.gaps {
        println!(
            concat!(
                "{{\"event\":\"source_gap\",\"source_id\":\"{}\",",
                "\"revision_id\":\"{}\",\"reason\":\"{}\"}}"
            ),
            gap.source_id, gap.revision_id, gap.reason,
        );
    }
    for item in &result.matches {
        println!(
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
        );
    }
    println!(
        concat!(
            "{{\"event\":\"corpus_search_complete\",",
            "\"matches\":{},\"gaps\":{},\"searched_sources\":{},",
            "\"active_sources\":{},\"complete\":{},",
            "\"match_limit_reached\":{},\"source_backed\":true,",
            "\"encrypted_at_rest\":false}}"
        ),
        result.matches.len(),
        result.gaps.len(),
        result.searched_sources,
        result.active_sources,
        result.complete,
        result.match_limit_reached,
    );
}

fn parse_u64(value: &str, name: &str) -> Result<u64, String> {
    value
        .parse::<u64>()
        .map_err(|_| format!("INVALID_{name}"))
}

fn self_test() -> Result<(), &'static str> {
    if Command::parse("health") != Ok(Command::Health) {
        return Err("HEALTH_COMMAND_PARSE_FAILED");
    }
    if Command::parse("shutdown") != Ok(Command::Shutdown) {
        return Err("SHUTDOWN_COMMAND_PARSE_FAILED");
    }
    let health = Health::SHELL.json();
    if !health.contains("\"source_backed_search_available\":false") {
        return Err("HEALTH_TRUTHFULNESS_FAILED");
    }
    let result = scan_text("alpha\nbeta alpha", "alpha", false)
        .map_err(|_| "SCAN_FAILED")?;
    if result.matches.len() != 2 || result.matches[1].line != 1 {
        return Err("SCAN_COORDINATE_FAILED");
    }
    if sha256::hex(&sha256::digest(b"abc"))
        != "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    {
        return Err("SHA256_VECTOR_FAILED");
    }
    Ok(())
}

fn require_argument_count(arguments: &[String], expected: usize) -> Result<(), String> {
    if arguments.len() == expected {
        Ok(())
    } else {
        Err("USAGE_ERROR".to_owned())
    }
}

fn run() -> Result<(), String> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let Some(argument) = arguments.first().map(String::as_str) else {
        print!("{}", help());
        return Ok(());
    };

    match argument {
        "--help" | "-h" => {
            require_argument_count(&arguments, 1)?;
            print!("{}", help());
        }
        "--version" | "-V" => {
            require_argument_count(&arguments, 1)?;
            println!("{}", version_json());
        }
        "--health" => {
            require_argument_count(&arguments, 1)?;
            println!("{}", Health::SHELL.json());
        }
        "--health-data-root" => {
            require_argument_count(&arguments, 2)?;
            let (_guard, store) = open_direct_store(Path::new(&arguments[1]))?;
            let verification = store.verify()?;
            println!(
                concat!(
                    "{{\"event\":\"health\",\"namespace_id\":\"{}\",",
                    "\"registered_sources\":{},\"active_sources\":{},",
                    "\"verified_revisions\":{},\"health\":{}}}"
                ),
                store.namespace_id(),
                verification.registered_sources,
                verification.active_sources,
                verification.verified_revisions,
                Health::DIRECT_STORE.json(),
            );
        }
        "--self-test" => {
            require_argument_count(&arguments, 1)?;
            self_test().map_err(str::to_owned)?;
            println!(
                "{}",
                concat!(
                    "{\"status\":\"ok\",\"component\":\"eliot-searchd\",",
                    "\"development_stdin_scan_available\":true,",
                    "\"development_file_scan_available\":true,",
                    "\"persistent_direct_store_available\":true}"
                )
            );
        }
        "--stdio" => {
            require_argument_count(&arguments, 1)?;
            serve_stdio(Health::SHELL)
                .map_err(|error| format!("STDIO_ERROR:{error}"))?;
        }
        "--serve-data-root" => {
            require_argument_count(&arguments, 2)?;
            let (guard, store) = open_direct_store(Path::new(&arguments[1]))?;
            store.verify()?;
            println!(
                "{{\"event\":\"data_root_ready\",\"namespace_id\":\"{}\",\"encrypted_at_rest\":false}}",
                store.namespace_id(),
            );
            serve_stdio(Health::DIRECT_STORE)
                .map_err(|error| format!("STDIO_ERROR:{error}"))?;
            drop(store);
            drop(guard);
        }
        "--source-roots" | "--register-source-root" | "--unregister-source-root"
        | "--sync-source-roots" => crate::source_root_commands::run(&arguments)?,
        "--scan-stdin" | "--scan-stdin-ascii-insensitive" => {
            require_argument_count(&arguments, 2)?;
            emit_one_shot_scan(
                "stdin",
                &arguments[1],
                argument == "--scan-stdin-ascii-insensitive",
                read_stdin_bounded()?,
                false,
                false,
            )?;
        }
        "--scan-file" | "--scan-file-ascii-insensitive" => {
            require_argument_count(&arguments, 3)?;
            emit_one_shot_scan(
                "file",
                &arguments[1],
                argument == "--scan-file-ascii-insensitive",
                read_file_bounded(Path::new(&arguments[2]))?,
                true,
                true,
            )?;
        }
        "--index-file" => {
            require_argument_count(&arguments, 3)?;
            let (_guard, mut store) = open_direct_store(Path::new(&arguments[1]))?;
            let indexed = store.index_file(Path::new(&arguments[2]))?;
            emit_indexed_source(&indexed);
        }
        "--index-directory" => {
            require_argument_count(&arguments, 3)?;
            let (_guard, mut store) = open_direct_store(Path::new(&arguments[1]))?;
            let indexed = store.index_directory(Path::new(&arguments[2]))?;
            let changed = indexed.iter().filter(|source| source.changed).count();
            for source in &indexed {
                emit_indexed_source(source);
            }
            println!(
                concat!(
                    "{{\"event\":\"directory_index_complete\",",
                    "\"namespace_id\":\"{}\",\"sources\":{},",
                    "\"changed\":{},\"source_backed\":true,",
                    "\"encrypted_at_rest\":false}}"
                ),
                store.namespace_id(),
                indexed.len(),
                changed,
            );
        }
        "--search-root" | "--search-root-ascii-insensitive" => {
            require_argument_count(&arguments, 3)?;
            let (_guard, store) = open_direct_store(Path::new(&arguments[1]))?;
            let result = store.search(
                &arguments[2],
                argument == "--search-root-ascii-insensitive",
            )?;
            emit_store_search(&store.namespace_id(), &result);
        }
        "--list-sources" => {
            require_argument_count(&arguments, 2)?;
            let (_guard, store) = open_direct_store(Path::new(&arguments[1]))?;
            let sources = store.list_sources();
            for source in &sources {
                println!(
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
                );
            }
            println!(
                "{{\"event\":\"source_list_complete\",\"namespace_id\":\"{}\",\"sources\":{}}}",
                store.namespace_id(),
                sources.len(),
            );
        }
        "--verify-root" => {
            require_argument_count(&arguments, 2)?;
            let (_guard, store) = open_direct_store(Path::new(&arguments[1]))?;
            let verification = store.verify()?;
            println!(
                concat!(
                    "{{\"event\":\"direct_store_verified\",",
                    "\"namespace_id\":\"{}\",\"source_events\":{},",
                    "\"registered_sources\":{},\"active_sources\":{},",
                    "\"referenced_revisions\":{},\"verified_revisions\":{},",
                    "\"total_revision_bytes\":{},\"source_backed\":true,",
                    "\"encrypted_at_rest\":false}}"
                ),
                store.namespace_id(),
                verification.source_events,
                verification.registered_sources,
                verification.active_sources,
                verification.referenced_revisions,
                verification.verified_revisions,
                verification.total_revision_bytes,
            );
        }
        "--retire-source" => {
            require_argument_count(&arguments, 3)?;
            let (_guard, mut store) = open_direct_store(Path::new(&arguments[1]))?;
            let source = store.retire_source(&arguments[2])?;
            println!(
                concat!(
                    "{{\"event\":\"source_retired\",\"source_id\":\"{}\",",
                    "\"revision_id\":\"{}\",\"sequence\":{},",
                    "\"active\":false}}"
                ),
                source.source_id, source.revision_id, source.sequence,
            );
        }
        "--read-revision" => {
            require_argument_count(&arguments, 5)?;
            let (_guard, store) = open_direct_store(Path::new(&arguments[1]))?;
            let start = parse_u64(&arguments[3], "START_OFFSET")?;
            let end = parse_u64(&arguments[4], "END_OFFSET")?;
            let slice = store.read_revision_range(&arguments[2], start, end)?;
            println!(
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
            );
        }
        "--repair-root" => {
            require_argument_count(&arguments, 2)?;
            let guard = DataRootGuard::acquire(Path::new(&arguments[1]))?;
            let repair = repair_control_log(guard.canonical_root())?;
            let store = DirectStore::open(guard.canonical_root())?;
            store.verify()?;
            println!(
                concat!(
                    "{{\"event\":\"direct_store_repair_complete\",",
                    "\"namespace_id\":\"{}\",\"repaired\":{},",
                    "\"removed_bytes\":{},\"retained_events\":{},",
                    "\"last_sequence\":{},\"last_digest\":\"{}\"}}"
                ),
                store.namespace_id(),
                repair.repaired,
                repair.removed_bytes,
                repair.retained_events,
                repair.last_sequence,
                repair.last_digest,
            );
        }
        "--gc-root" => {
            require_argument_count(&arguments, 3)?;
            let apply = match arguments[2].as_str() {
                "--dry-run" => false,
                "--apply" => true,
                _ => return Err("USAGE_ERROR".to_owned()),
            };
            let (_guard, store) = open_direct_store(Path::new(&arguments[1]))?;
            store.verify()?;
            let result = collect_orphan_revisions(Path::new(&arguments[1]), apply)?;
            println!(
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
            );
        }
        _ => return Err(format!("UNKNOWN_ARGUMENT:{argument}")),
    }
    Ok(())
}

/// Runs the daemon command application and maps failures to process status.
pub(crate) fn run_main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{{\"error\":\"{}\"}}", crate::source_root_commands::escape_json(&error));
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn oversized_frame_terminates_session_without_executing_suffix() {
        let mut bytes = vec![b'x'; MAX_COMMAND_BYTES + 3];
        bytes.extend_from_slice(b"\nshutdown\n");
        let mut input = Cursor::new(bytes);
        let mut output = Vec::new();
        assert!(serve_control(Health::SHELL, &mut input, &mut output).is_err());
        let response = String::from_utf8(output).unwrap();
        assert!(response.contains("COMMAND_TOO_LARGE"));
        assert!(!response.contains("draining"));
        assert!(!response.contains("stopped"));
        assert_eq!(input.position(), (MAX_COMMAND_BYTES + 2) as u64);
    }

    #[test]
    fn valid_control_session_reports_health_and_clean_shutdown() {
        let mut input = Cursor::new(b"health\r\nversion\nshutdown\n");
        let mut output = Vec::new();
        serve_control(Health::SHELL, &mut input, &mut output).unwrap();
        let response = String::from_utf8(output).unwrap();
        assert_eq!(response.lines().count(), 5);
        assert!(response.contains("\"source_backed_search_available\":false"));
        assert!(response.contains("\"clean\":true"));
    }
}

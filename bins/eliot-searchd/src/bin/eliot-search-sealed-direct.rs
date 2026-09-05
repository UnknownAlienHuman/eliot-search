//! End-to-end development exact search over a DPAPI-sealed immutable object.
//!
//! This binary provides a usable encrypted-at-rest path without overstating the
//! product boundary. It does not claim source-catalog, owner-epoch, scope, or
//! retained EvidenceHandle integration.

#![deny(unsafe_op_in_unsafe_fn)]

#[path = "../sealed_store.rs"]
mod sealed_store;
#[path = "../sealed_transaction.rs"]
mod sealed_transaction;

use std::env;
use std::ffi::OsStr;
use std::fs::{self, File};
#[cfg(windows)]
use std::fs::OpenOptions;
use std::io::{self, Read};
use std::path::Path;
use std::process::ExitCode;
use std::time::SystemTime;

use sealed_store::{MAX_PLAINTEXT_BYTES, SensitiveBytes, open_sealed, verify_sealed};
use sealed_transaction::{put_idempotent, transaction_status};

const MAX_QUERY_BYTES: usize = 64 * 1024;
const MAX_MATCHES: usize = 100_000;

#[derive(Clone, Debug, Eq, PartialEq)]
struct FileObservation {
    length: u64,
    modified: Option<SystemTime>,
    identity: FileIdentity,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum FileIdentity {
    #[cfg(windows)]
    Windows {
        volume_serial: u32,
        file_index: u64,
    },
    #[cfg(unix)]
    Unix { device: u64, inode: u64 },
    #[cfg(not(any(unix, windows)))]
    Portable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ExactMatch {
    byte_start: usize,
    byte_end: usize,
    line: usize,
    column_bytes: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SearchResult {
    matches: Vec<ExactMatch>,
    input_bytes: usize,
    complete: bool,
    match_limit_reached: bool,
}

fn help() -> &'static str {
    concat!(
        "eliot-search-sealed-direct\n\n",
        "USAGE:\n",
        "  eliot-search-sealed-direct ingest-file DATA_ROOT OPERATION_ID OBJECT_ID FILE\n",
        "  eliot-search-sealed-direct search DATA_ROOT OBJECT_ID QUERY\n",
        "  eliot-search-sealed-direct search-ascii-insensitive DATA_ROOT OBJECT_ID QUERY\n",
        "  eliot-search-sealed-direct verify DATA_ROOT OBJECT_ID\n",
        "  eliot-search-sealed-direct transaction-status DATA_ROOT OPERATION_ID\n\n",
        "Windows only for sealed-object operations. Results are sealed-object-backed ",
        "but not yet catalog/owner/scope bound.\n",
    )
}

fn utf8_argument<'a>(value: &'a OsStr, code: &str) -> Result<&'a str, String> {
    value.to_str().ok_or_else(|| code.to_owned())
}

fn read_final_file(path: &Path) -> Result<Vec<u8>, String> {
    let link_metadata = fs::symlink_metadata(path)
        .map_err(|_| "SEALED_DIRECT_FILE_OPEN_FAILED".to_owned())?;
    if link_metadata.file_type().is_symlink() || is_reparse(&link_metadata) {
        return Err("SEALED_DIRECT_FILE_REPARSE_DENIED".to_owned());
    }
    if !link_metadata.is_file() {
        return Err("SEALED_DIRECT_FILE_NOT_REGULAR".to_owned());
    }

    let mut file = open_final(path).map_err(|_| "SEALED_DIRECT_FILE_OPEN_FAILED".to_owned())?;
    let before = observe_file(&file)?;
    if before.length > u64::try_from(MAX_PLAINTEXT_BYTES).unwrap_or(u64::MAX) {
        return Err("SEALED_STORE_PLAINTEXT_TOO_LARGE".to_owned());
    }
    let mut bytes = Vec::with_capacity(
        usize::try_from(before.length)
            .map_err(|_| "SEALED_STORE_PLAINTEXT_TOO_LARGE".to_owned())?,
    );
    (&mut file)
        .take(u64::try_from(MAX_PLAINTEXT_BYTES + 1).unwrap_or(u64::MAX))
        .read_to_end(&mut bytes)
        .map_err(|_| "SEALED_DIRECT_FILE_READ_FAILED".to_owned())?;
    if bytes.len() > MAX_PLAINTEXT_BYTES {
        return Err("SEALED_STORE_PLAINTEXT_TOO_LARGE".to_owned());
    }
    let after = observe_file(&file)?;
    if before != after || bytes.len() != usize::try_from(before.length).unwrap_or(usize::MAX) {
        return Err("SEALED_DIRECT_FILE_CHANGED_DURING_READ".to_owned());
    }
    Ok(bytes)
}

#[cfg(windows)]
fn open_final(path: &Path) -> io::Result<File> {
    use std::os::windows::fs::OpenOptionsExt;

    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    OpenOptions::new()
        .read(true)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
}

#[cfg(not(windows))]
fn open_final(path: &Path) -> io::Result<File> {
    File::open(path)
}

fn observe_file(file: &File) -> Result<FileObservation, String> {
    let metadata = file
        .metadata()
        .map_err(|_| "SEALED_DIRECT_FILE_METADATA_FAILED".to_owned())?;
    if !metadata.is_file() || is_reparse(&metadata) {
        return Err("SEALED_DIRECT_FILE_NOT_REGULAR".to_owned());
    }

    #[cfg(windows)]
    let identity = {
        let observed = eliot_searchd::native_file::observe(file)
            .map_err(|error| error.code().to_owned())?;
        FileIdentity::Windows {
            volume_serial: observed.volume_serial,
            file_index: observed.file_index,
        }
    };
    #[cfg(unix)]
    let identity = {
        use std::os::unix::fs::MetadataExt;

        FileIdentity::Unix {
            device: metadata.dev(),
            inode: metadata.ino(),
        }
    };
    #[cfg(not(any(unix, windows)))]
    let identity = FileIdentity::Portable;

    Ok(FileObservation {
        length: metadata.len(),
        modified: metadata.modified().ok(),
        identity,
    })
}

#[cfg(windows)]
fn is_reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_reparse(_metadata: &fs::Metadata) -> bool {
    false
}

fn scan_exact(
    text: &str,
    query: &str,
    ascii_insensitive: bool,
) -> Result<SearchResult, String> {
    if query.is_empty() {
        return Err("SEALED_DIRECT_QUERY_EMPTY".to_owned());
    }
    if query.len() > MAX_QUERY_BYTES {
        return Err("SEALED_DIRECT_QUERY_TOO_LARGE".to_owned());
    }
    if query.len() > text.len() {
        return Ok(SearchResult {
            matches: Vec::new(),
            input_bytes: text.len(),
            complete: true,
            match_limit_reached: false,
        });
    }

    let text_bytes = text.as_bytes();
    let query_bytes = query.as_bytes();
    let mut line_starts = vec![0_usize];
    for (index, byte) in text_bytes.iter().copied().enumerate() {
        if byte == b'\n' {
            line_starts.push(index.saturating_add(1));
        }
    }

    let mut matches = Vec::new();
    let mut truncated = false;
    for start in 0..=text_bytes.len() - query_bytes.len() {
        if !text.is_char_boundary(start) {
            continue;
        }
        let end = start + query_bytes.len();
        if !text.is_char_boundary(end) {
            continue;
        }
        let equal = if ascii_insensitive {
            text_bytes[start..end].eq_ignore_ascii_case(query_bytes)
        } else {
            &text_bytes[start..end] == query_bytes
        };
        if !equal {
            continue;
        }
        if matches.len() == MAX_MATCHES {
            truncated = true;
            break;
        }
        let line = line_starts
            .partition_point(|line_start| *line_start <= start)
            .saturating_sub(1);
        matches.push(ExactMatch {
            byte_start: start,
            byte_end: end,
            line,
            column_bytes: start - line_starts[line],
        });
    }

    Ok(SearchResult {
        matches,
        input_bytes: text.len(),
        complete: !truncated,
        match_limit_reached: truncated,
    })
}

fn emit_search(object_id: &str, result: &SearchResult, ascii_insensitive: bool) {
    println!(
        concat!(
            "{{\"event\":\"search_started\",\"object_id\":\"{}\",",
            "\"mode\":\"{}\",\"input_bytes\":{},",
            "\"sealed_object_backed\":true,\"dpapi_authenticated\":true,",
            "\"catalog_bound\":false,\"owner_epoch_bound\":false,",
            "\"scope_bound\":false,\"production_ready\":false}}"
        ),
        object_id,
        if ascii_insensitive {
            "ascii_insensitive"
        } else {
            "sensitive"
        },
        result.input_bytes,
    );
    for item in &result.matches {
        println!(
            concat!(
                "{{\"event\":\"match\",\"byte_start\":{},",
                "\"byte_end\":{},\"line\":{},\"column_bytes\":{}}}"
            ),
            item.byte_start, item.byte_end, item.line, item.column_bytes,
        );
    }
    println!(
        concat!(
            "{{\"event\":\"search_complete\",\"matches\":{},",
            "\"match_limit_reached\":{},\"complete\":{},",
            "\"sealed_object_backed\":true,\"catalog_bound\":false,",
            "\"owner_epoch_bound\":false,\"scope_bound\":false,",
            "\"production_ready\":false}}"
        ),
        result.matches.len(),
        result.match_limit_reached,
        result.complete,
    );
}

fn run() -> Result<(), String> {
    let arguments = env::args_os().skip(1).collect::<Vec<_>>();
    let Some(raw_command) = arguments.first() else {
        print!("{}", help());
        return Ok(());
    };
    let command = utf8_argument(raw_command, "SEALED_DIRECT_COMMAND_INVALID")?;
    if matches!(command, "--help" | "-h") {
        if arguments.len() != 1 {
            return Err("SEALED_DIRECT_USAGE_ERROR".to_owned());
        }
        print!("{}", help());
        return Ok(());
    }

    match command {
        "ingest-file" if arguments.len() == 5 => {
            let data_root = Path::new(&arguments[1]);
            let operation_id =
                utf8_argument(&arguments[2], "SEALED_TRANSACTION_OPERATION_ID_INVALID")?;
            let object_id = utf8_argument(&arguments[3], "SEALED_STORE_OBJECT_ID_INVALID")?;
            let plaintext = SensitiveBytes::new(read_final_file(Path::new(&arguments[4]))?)
                .map_err(|error| error.code().to_owned())?;
            let receipt = put_idempotent(data_root, operation_id, object_id, plaintext)
                .map_err(|error| error.code().to_owned())?;
            println!(
                concat!(
                    "{{\"status\":\"COMMITTED\",\"disposition\":\"{}\",",
                    "\"operation_id\":\"{}\",\"object_id\":\"{}\",",
                    "\"plaintext_bytes\":{},\"ciphertext_bytes\":{},",
                    "\"sealed_readback_verified\":{},",
                    "\"receipt_readback_verified\":{},",
                    "\"catalog_bound\":false,\"owner_epoch_bound\":false,",
                    "\"production_ready\":false}}"
                ),
                receipt.disposition.as_str(),
                receipt.operation_id,
                receipt.object_id,
                receipt.plaintext_bytes,
                receipt.ciphertext_bytes,
                receipt.sealed_readback_verified,
                receipt.receipt_readback_verified,
            );
        }
        "search" | "search-ascii-insensitive" if arguments.len() == 4 => {
            let data_root = Path::new(&arguments[1]);
            let object_id = utf8_argument(&arguments[2], "SEALED_STORE_OBJECT_ID_INVALID")?;
            let query = utf8_argument(&arguments[3], "SEALED_DIRECT_QUERY_INVALID_UTF8")?;
            let plaintext = open_sealed(data_root, object_id)
                .map_err(|error| error.code().to_owned())?;
            let text = core::str::from_utf8(plaintext.expose())
                .map_err(|_| "SEALED_DIRECT_OBJECT_NOT_UTF8".to_owned())?;
            let result = scan_exact(text, query, command == "search-ascii-insensitive")?;
            emit_search(
                object_id,
                &result,
                command == "search-ascii-insensitive",
            );
        }
        "verify" if arguments.len() == 3 => {
            let data_root = Path::new(&arguments[1]);
            let object_id = utf8_argument(&arguments[2], "SEALED_STORE_OBJECT_ID_INVALID")?;
            let receipt = verify_sealed(data_root, object_id)
                .map_err(|error| error.code().to_owned())?;
            println!(
                concat!(
                    "{{\"status\":\"VERIFIED\",\"object_id\":\"{}\",",
                    "\"plaintext_bytes\":{},\"ciphertext_bytes\":{},",
                    "\"protection_scope\":\"{}\",\"authenticated\":{},",
                    "\"catalog_bound\":false,\"owner_epoch_bound\":false,",
                    "\"production_ready\":false}}"
                ),
                receipt.object_id,
                receipt.plaintext_bytes,
                receipt.ciphertext_bytes,
                receipt.protection_scope,
                receipt.authenticated,
            );
        }
        "transaction-status" if arguments.len() == 3 => {
            let data_root = Path::new(&arguments[1]);
            let operation_id =
                utf8_argument(&arguments[2], "SEALED_TRANSACTION_OPERATION_ID_INVALID")?;
            let status = transaction_status(data_root, operation_id)
                .map_err(|error| error.code().to_owned())?;
            println!(
                concat!(
                    "{{\"status\":\"{}\",\"operation_id\":\"{}\",",
                    "\"catalog_bound\":false,\"owner_epoch_bound\":false,",
                    "\"production_ready\":false}}"
                ),
                status.as_str(),
                operation_id,
            );
        }
        _ => return Err("SEALED_DIRECT_USAGE_ERROR".to_owned()),
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{{\"error\":\"{}\"}}", error.replace('"', "'"));
            ExitCode::from(2)
        }
    }
}

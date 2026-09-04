//! Catalog-bound encrypted ingest, verification, and exact search.

#![deny(unsafe_op_in_unsafe_fn)]

#[path = "../sealed_catalog.rs"]
mod sealed_catalog;
#[path = "../sealed_digest.rs"]
mod sealed_digest;
#[path = "../sealed_exact.rs"]
mod sealed_exact;
#[path = "../sealed_file_reader.rs"]
mod sealed_file_reader;
#[path = "../sealed_root_lock.rs"]
mod sealed_root_lock;
#[path = "../sealed_store.rs"]
mod sealed_store;
#[path = "../sealed_transaction.rs"]
mod sealed_transaction;
#[path = "../sealed_transaction_guard.rs"]
mod sealed_transaction_guard;

use std::env;
use std::ffi::OsStr;
use std::path::Path;
use std::process::ExitCode;

use sealed_catalog::{bind_revision, read_revision, verify_revision};
use sealed_exact::{ExactSearchResult, scan_exact};
use sealed_file_reader::read_final_file;
use sealed_root_lock::SealedRootLease;
use sealed_transaction::transaction_status;
use sealed_transaction_guard::put_idempotent_verified;

fn help() -> &'static str {
    concat!(
        "eliot-search-sealed-catalog\n\n",
        "USAGE:\n",
        "  eliot-search-sealed-catalog ingest-file DATA_ROOT CONTENT_OPERATION_ID CONTENT_OBJECT_ID CATALOG_OPERATION_ID CATALOG_OBJECT_ID SOURCE_ID SOURCE_REVISION_ID FILE\n",
        "  eliot-search-sealed-catalog search DATA_ROOT CATALOG_OBJECT_ID SOURCE_ID SOURCE_REVISION_ID QUERY\n",
        "  eliot-search-sealed-catalog search-ascii-insensitive DATA_ROOT CATALOG_OBJECT_ID SOURCE_ID SOURCE_REVISION_ID QUERY\n",
        "  eliot-search-sealed-catalog verify DATA_ROOT CATALOG_OBJECT_ID SOURCE_ID SOURCE_REVISION_ID\n",
        "  eliot-search-sealed-catalog transaction-status DATA_ROOT OPERATION_ID\n\n",
        "Windows only. The path is DPAPI-encrypted, immutable, catalog-bound, ",
        "SHA-256 checked, and protected by an exclusive data-root lock. A ",
        "monotone OwnerEpoch and scope/policy authority are not composed yet.\n",
    )
}

fn utf8_argument<'a>(value: &'a OsStr, code: &str) -> Result<&'a str, String> {
    value.to_str().ok_or_else(|| code.to_owned())
}

fn acquire_root(path: &Path) -> Result<SealedRootLease, String> {
    let lease = SealedRootLease::acquire(path).map_err(|error| error.code().to_owned())?;
    if !lease.is_held() {
        return Err("SEALED_ROOT_LOCK_FAILED".to_owned());
    }
    Ok(lease)
}

fn emit_search(
    catalog_object_id: &str,
    source_id: &str,
    source_revision_id: &str,
    content_sha256: &str,
    result: &ExactSearchResult,
    ascii_insensitive: bool,
) {
    println!(
        concat!(
            "{{\"event\":\"search_started\",",
            "\"catalog_object_id\":\"{}\",\"source_id\":\"{}\",",
            "\"source_revision_id\":\"{}\",\"content_sha256\":\"{}\",",
            "\"mode\":\"{}\",\"input_bytes\":{},",
            "\"sealed_object_backed\":true,\"catalog_bound\":true,",
            "\"data_root_lock_held\":true,\"owner_epoch_bound\":false,",
            "\"scope_bound\":false,\"production_ready\":false}}"
        ),
        catalog_object_id,
        source_id,
        source_revision_id,
        content_sha256,
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
            "\"sealed_object_backed\":true,\"catalog_bound\":true,",
            "\"data_root_lock_held\":true,\"owner_epoch_bound\":false,",
            "\"scope_bound\":false,\"production_ready\":false}}"
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
    let command = utf8_argument(raw_command, "SEALED_CATALOG_COMMAND_INVALID")?;
    if matches!(command, "--help" | "-h") {
        if arguments.len() != 1 {
            return Err("SEALED_CATALOG_USAGE_ERROR".to_owned());
        }
        print!("{}", help());
        return Ok(());
    }

    match command {
        "ingest-file" if arguments.len() == 9 => {
            let data_root = Path::new(&arguments[1]);
            let _root_lease = acquire_root(data_root)?;
            let content_operation_id = utf8_argument(
                &arguments[2],
                "SEALED_TRANSACTION_OPERATION_ID_INVALID",
            )?;
            let content_object_id =
                utf8_argument(&arguments[3], "SEALED_STORE_OBJECT_ID_INVALID")?;
            let catalog_operation_id = utf8_argument(
                &arguments[4],
                "SEALED_TRANSACTION_OPERATION_ID_INVALID",
            )?;
            let catalog_object_id =
                utf8_argument(&arguments[5], "SEALED_STORE_OBJECT_ID_INVALID")?;
            let source_id =
                utf8_argument(&arguments[6], "SEALED_CATALOG_IDENTIFIER_INVALID")?;
            let source_revision_id =
                utf8_argument(&arguments[7], "SEALED_CATALOG_IDENTIFIER_INVALID")?;
            let final_read = read_final_file(Path::new(&arguments[8]))
                .map_err(|error| error.code().to_owned())?;
            let same_handle_receipt = final_read.receipt.clone();
            let content_transaction = put_idempotent_verified(
                data_root,
                content_operation_id,
                content_object_id,
                final_read.plaintext,
            )
            .map_err(|error| error.code().to_owned())?;
            let catalog = bind_revision(
                data_root,
                content_operation_id,
                content_object_id,
                catalog_operation_id,
                catalog_object_id,
                source_id,
                source_revision_id,
            )
            .map_err(|error| error.code().to_owned())?;
            println!(
                concat!(
                    "{{\"status\":\"CATALOG_COMMITTED\",",
                    "\"source_id\":\"{}\",\"source_revision_id\":\"{}\",",
                    "\"content_object_id\":\"{}\",",
                    "\"catalog_object_id\":\"{}\",",
                    "\"content_sha256\":\"{}\",",
                    "\"content_disposition\":\"{}\",",
                    "\"catalog_disposition\":\"{}\",",
                    "\"plaintext_bytes\":{},\"ciphertext_bytes\":{},",
                    "\"same_handle_verified\":{},\"reparse_free\":{},",
                    "\"content_readback_verified\":{},",
                    "\"catalog_readback_verified\":{},",
                    "\"sealed_object_backed\":true,\"catalog_bound\":true,",
                    "\"data_root_lock_held\":true,\"owner_epoch_bound\":false,",
                    "\"scope_bound\":false,\"production_ready\":false}}"
                ),
                catalog.binding.source_id,
                catalog.binding.source_revision_id,
                catalog.binding.content_object_id,
                catalog.catalog_object_id,
                catalog.binding.content_sha256,
                content_transaction.disposition.as_str(),
                catalog.catalog_transaction.disposition.as_str(),
                catalog.binding.content_plaintext_bytes,
                catalog.binding.content_ciphertext_bytes,
                same_handle_receipt.same_handle_verified,
                same_handle_receipt.reparse_free,
                catalog.content_transaction.sealed_readback_verified,
                catalog.catalog_readback_verified,
            );
        }
        "search" | "search-ascii-insensitive" if arguments.len() == 6 => {
            let data_root = Path::new(&arguments[1]);
            let _root_lease = acquire_root(data_root)?;
            let catalog_object_id =
                utf8_argument(&arguments[2], "SEALED_STORE_OBJECT_ID_INVALID")?;
            let source_id =
                utf8_argument(&arguments[3], "SEALED_CATALOG_IDENTIFIER_INVALID")?;
            let source_revision_id =
                utf8_argument(&arguments[4], "SEALED_CATALOG_IDENTIFIER_INVALID")?;
            let query = utf8_argument(&arguments[5], "SEALED_EXACT_QUERY_INVALID_UTF8")?;
            let read = read_revision(
                data_root,
                catalog_object_id,
                source_id,
                source_revision_id,
            )
            .map_err(|error| error.code().to_owned())?;
            let result = scan_exact(
                read.content.expose(),
                query,
                command == "search-ascii-insensitive",
            )
            .map_err(|error| error.code().to_owned())?;
            emit_search(
                catalog_object_id,
                source_id,
                source_revision_id,
                &read.binding.content_sha256.to_hex(),
                &result,
                command == "search-ascii-insensitive",
            );
        }
        "verify" if arguments.len() == 5 => {
            let data_root = Path::new(&arguments[1]);
            let _root_lease = acquire_root(data_root)?;
            let catalog_object_id =
                utf8_argument(&arguments[2], "SEALED_STORE_OBJECT_ID_INVALID")?;
            let source_id =
                utf8_argument(&arguments[3], "SEALED_CATALOG_IDENTIFIER_INVALID")?;
            let source_revision_id =
                utf8_argument(&arguments[4], "SEALED_CATALOG_IDENTIFIER_INVALID")?;
            let receipt = verify_revision(
                data_root,
                catalog_object_id,
                source_id,
                source_revision_id,
            )
            .map_err(|error| error.code().to_owned())?;
            println!(
                concat!(
                    "{{\"status\":\"CATALOG_VERIFIED\",",
                    "\"catalog_object_id\":\"{}\",\"source_id\":\"{}\",",
                    "\"source_revision_id\":\"{}\",",
                    "\"content_object_id\":\"{}\",",
                    "\"content_sha256\":\"{}\",",
                    "\"plaintext_bytes\":{},\"ciphertext_bytes\":{},",
                    "\"authenticated\":{},\"catalog_bound\":true,",
                    "\"data_root_lock_held\":true,\"owner_epoch_bound\":false,",
                    "\"scope_bound\":false,\"production_ready\":false}}"
                ),
                receipt.catalog_object_id,
                receipt.source_id,
                receipt.source_revision_id,
                receipt.content_object_id,
                receipt.content_sha256,
                receipt.content_plaintext_bytes,
                receipt.content_ciphertext_bytes,
                receipt.authenticated,
            );
        }
        "transaction-status" if arguments.len() == 3 => {
            let data_root = Path::new(&arguments[1]);
            let _root_lease = acquire_root(data_root)?;
            let operation_id = utf8_argument(
                &arguments[2],
                "SEALED_TRANSACTION_OPERATION_ID_INVALID",
            )?;
            let status = transaction_status(data_root, operation_id)
                .map_err(|error| error.code().to_owned())?;
            println!(
                concat!(
                    "{{\"status\":\"{}\",\"operation_id\":\"{}\",",
                    "\"data_root_lock_held\":true,\"production_ready\":false}}"
                ),
                status.as_str(),
                operation_id,
            );
        }
        _ => return Err("SEALED_CATALOG_USAGE_ERROR".to_owned()),
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

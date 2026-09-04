//! Intent/readback/reconciliation CLI for immutable DPAPI-sealed objects.

#![deny(unsafe_op_in_unsafe_fn)]

#[path = "../sealed_store.rs"]
mod sealed_store;
#[path = "../sealed_transaction.rs"]
mod sealed_transaction;

use std::io::{self, Read};
use std::path::Path;
use std::process::ExitCode;

use sealed_store::{MAX_PLAINTEXT_BYTES, SensitiveBytes};
use sealed_transaction::{put_idempotent, transaction_status};

fn help() -> &'static str {
    concat!(
        "eliot-search-sealed-transaction\n\n",
        "USAGE:\n",
        "  eliot-search-sealed-transaction put DATA_ROOT OPERATION_ID OBJECT_ID < plaintext\n",
        "  eliot-search-sealed-transaction status DATA_ROOT OPERATION_ID\n\n",
        "A retry of put must supply the exact same plaintext. An unknown earlier ",
        "write is accepted only after decrypting and comparing every byte.\n",
    )
}

fn read_plaintext() -> Result<SensitiveBytes, String> {
    let mut bytes = Vec::new();
    io::stdin()
        .take(u64::try_from(MAX_PLAINTEXT_BYTES + 1).unwrap_or(u64::MAX))
        .read_to_end(&mut bytes)
        .map_err(|_| "SEALED_TRANSACTION_STDIN_READ_FAILED".to_owned())?;
    if bytes.len() > MAX_PLAINTEXT_BYTES {
        return Err("SEALED_STORE_PLAINTEXT_TOO_LARGE".to_owned());
    }
    SensitiveBytes::new(bytes).map_err(|error| error.code().to_owned())
}

fn run() -> Result<(), String> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let Some(command) = arguments.first().map(String::as_str) else {
        print!("{}", help());
        return Ok(());
    };
    if matches!(command, "--help" | "-h") {
        if arguments.len() != 1 {
            return Err("SEALED_TRANSACTION_USAGE_ERROR".to_owned());
        }
        print!("{}", help());
        return Ok(());
    }
    match command {
        "put" if arguments.len() == 4 => {
            let receipt = put_idempotent(
                Path::new(&arguments[1]),
                &arguments[2],
                &arguments[3],
                read_plaintext()?,
            )
            .map_err(|error| error.code().to_owned())?;
            println!(
                concat!(
                    "{{\"status\":\"COMMITTED\",\"disposition\":\"{}\",",
                    "\"operation_id\":\"{}\",\"object_id\":\"{}\",",
                    "\"plaintext_bytes\":{},\"ciphertext_bytes\":{},",
                    "\"sealed_readback_verified\":{},",
                    "\"receipt_readback_verified\":{}}}"
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
        "status" if arguments.len() == 3 => {
            let status = transaction_status(Path::new(&arguments[1]), &arguments[2])
                .map_err(|error| error.code().to_owned())?;
            println!(
                "{{\"status\":\"{}\",\"operation_id\":\"{}\"}}",
                status.as_str(),
                arguments[2],
            );
        }
        _ => return Err("SEALED_TRANSACTION_USAGE_ERROR".to_owned()),
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

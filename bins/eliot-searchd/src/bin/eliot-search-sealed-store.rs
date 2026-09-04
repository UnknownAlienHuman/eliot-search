//! Operational CLI for the Windows CurrentUser DPAPI sealed-object adapter.

#![deny(unsafe_op_in_unsafe_fn)]

#[path = "../sealed_store.rs"]
mod sealed_store;

use std::io::{self, Read, Write};
use std::path::Path;
use std::process::ExitCode;

use sealed_store::{
    MAX_PLAINTEXT_BYTES, SensitiveBytes, delete_sealed, open_sealed,
    seal_immutable, verify_sealed,
};

fn help() -> &'static str {
    concat!(
        "eliot-search-sealed-store\n\n",
        "USAGE:\n",
        "  eliot-search-sealed-store put DATA_ROOT OBJECT_ID < plaintext\n",
        "  eliot-search-sealed-store get DATA_ROOT OBJECT_ID > plaintext\n",
        "  eliot-search-sealed-store verify DATA_ROOT OBJECT_ID\n",
        "  eliot-search-sealed-store delete DATA_ROOT OBJECT_ID\n\n",
        "Windows only. Protection scope is the current Windows user.\n",
        "put is immutable and refuses to replace an existing object.\n",
        "delete is logical only and never claims physical media erasure.\n",
    )
}

fn read_plaintext() -> Result<SensitiveBytes, String> {
    let mut bytes = Vec::new();
    io::stdin()
        .take(u64::try_from(MAX_PLAINTEXT_BYTES + 1).unwrap_or(u64::MAX))
        .read_to_end(&mut bytes)
        .map_err(|_| "SEALED_STORE_STDIN_READ_FAILED".to_owned())?;
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
            return Err("SEALED_STORE_USAGE_ERROR".to_owned());
        }
        print!("{}", help());
        return Ok(());
    }
    if arguments.len() != 3 {
        return Err("SEALED_STORE_USAGE_ERROR".to_owned());
    }
    let root = Path::new(&arguments[1]);
    let object_id = &arguments[2];
    match command {
        "put" => {
            let receipt = seal_immutable(root, object_id, read_plaintext()?)
                .map_err(|error| error.code().to_owned())?;
            println!(
                concat!(
                    "{{\"status\":\"SEALED\",\"object_id\":\"{}\",",
                    "\"plaintext_bytes\":{},\"ciphertext_bytes\":{},",
                    "\"format_version\":{},\"protection_scope\":\"{}\",",
                    "\"readback_verified\":{}}}"
                ),
                receipt.object_id,
                receipt.plaintext_bytes,
                receipt.ciphertext_bytes,
                receipt.format_version,
                receipt.protection_scope,
                receipt.readback_verified,
            );
        }
        "get" => {
            let plaintext = open_sealed(root, object_id)
                .map_err(|error| error.code().to_owned())?;
            let mut output = io::stdout().lock();
            output
                .write_all(plaintext.expose())
                .and_then(|()| output.flush())
                .map_err(|_| "SEALED_STORE_STDOUT_WRITE_FAILED".to_owned())?;
        }
        "verify" => {
            let receipt = verify_sealed(root, object_id)
                .map_err(|error| error.code().to_owned())?;
            println!(
                concat!(
                    "{{\"status\":\"VERIFIED\",\"object_id\":\"{}\",",
                    "\"plaintext_bytes\":{},\"ciphertext_bytes\":{},",
                    "\"format_version\":{},\"protection_scope\":\"{}\",",
                    "\"authenticated\":{}}}"
                ),
                receipt.object_id,
                receipt.plaintext_bytes,
                receipt.ciphertext_bytes,
                receipt.format_version,
                receipt.protection_scope,
                receipt.authenticated,
            );
        }
        "delete" => {
            let receipt = delete_sealed(root, object_id)
                .map_err(|error| error.code().to_owned())?;
            println!(
                concat!(
                    "{{\"status\":\"DELETED\",\"object_id\":\"{}\",",
                    "\"logical_delete_complete\":{},",
                    "\"physical_erasure_guaranteed\":{}}}"
                ),
                receipt.object_id,
                receipt.logical_delete_complete,
                receipt.physical_erasure_guaranteed,
            );
        }
        _ => return Err("SEALED_STORE_USAGE_ERROR".to_owned()),
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

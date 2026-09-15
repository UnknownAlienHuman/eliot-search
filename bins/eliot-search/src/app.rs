//! Protocol-only CLI application facade.
//!
//! The existing command application remains byte-for-byte in `core.rs`.
//! Indexed search has a separately bounded parser so adding the new verb does
//! not duplicate or destabilize the mature DIRECT command surface.

mod core;
mod indexed;

use std::env;
use std::process::ExitCode;

/// Runs the indexed verb when selected, otherwise delegates to the canonical
/// existing command application.
pub fn run_main() -> ExitCode {
    if let Some(exit) = indexed::maybe_run() {
        return exit;
    }

    let append_indexed_help = {
        let arguments = env::args_os().skip(1).collect::<Vec<_>>();
        arguments.is_empty()
            || (arguments.len() == 1
                && matches!(arguments[0].to_str(), Some("--help" | "-h")))
    };
    let exit = core::run_main();
    if append_indexed_help {
        println!(concat!(
            "\nINDEXED PROVIDER QUERY:\n",
            "  eliot-search indexed \"TERMS\" ",
            "[--data-root DIR] [--address IP:PORT] [--token-file PATH]\n",
            "  Requires accepted search + Qdrant qualification; otherwise ",
            "exits 2 with PROVIDER_INDEXED_QUERY_UNAVAILABLE."
        ));
    }
    exit
}

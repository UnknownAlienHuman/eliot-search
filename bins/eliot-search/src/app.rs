//! Protocol-only CLI application facade.
//!
//! The primary build exposes DIRECT/stdin operations and the canonical local
//! provider bootstrap only. Obsolete address/token-file endpoint commands are
//! refused before entering the legacy parser unless the explicit harness feature
//! is enabled.

mod core;
#[cfg(feature = "legacy-loopback-harness")]
mod indexed;

use std::env;
#[cfg(not(feature = "legacy-loopback-harness"))]
use std::ffi::{OsStr, OsString};
use std::process::ExitCode;

/// Runs the supported CLI surface.
pub fn run_main() -> ExitCode {
    let arguments = env::args_os().skip(1).collect::<Vec<_>>();

    #[cfg(not(feature = "legacy-loopback-harness"))]
    {
        if is_help(&arguments) {
            print_primary_help();
            return ExitCode::SUCCESS;
        }
        if is_legacy_provider_request(&arguments) {
            eprintln!(
                "{{\"error\":\"LEGACY_LOOPBACK_HARNESS_DISABLED\"}}"
            );
            return ExitCode::from(2);
        }
    }

    #[cfg(feature = "legacy-loopback-harness")]
    if let Some(exit) = indexed::maybe_run() {
        return exit;
    }

    let exit = core::run_main();

    #[cfg(feature = "legacy-loopback-harness")]
    {
        let append_indexed_help = arguments.is_empty()
            || (arguments.len() == 1
                && matches!(arguments[0].to_str(), Some("--help" | "-h")));
        if append_indexed_help {
            println!(concat!(
                "\nLEGACY LOOPBACK HARNESS QUERY:\n",
                "  eliot-search indexed \"TERMS\" ",
                "[--data-root DIR] [--address IP:PORT] [--token-file PATH]\n",
                "  Test fixture only; never final product transport."
            ));
        }
    }

    exit
}

#[cfg(not(feature = "legacy-loopback-harness"))]
fn is_help(arguments: &[OsString]) -> bool {
    arguments.is_empty()
        || (arguments.len() == 1
            && matches!(arguments[0].to_str(), Some("--help" | "-h" | "help")))
}

#[cfg(not(feature = "legacy-loopback-harness"))]
fn is_legacy_provider_request(arguments: &[OsString]) -> bool {
    let Some(command) = arguments.first().and_then(|value| value.to_str()) else {
        return false;
    };
    if matches!(
        command,
        "remote" | "status" | "search" | "lexical" | "refresh" | "indexed"
    ) {
        return true;
    }
    matches!(command, "health" | "version" | "shutdown")
        && arguments.iter().skip(1).any(|argument| {
            argument == OsStr::new("--address")
                || argument == OsStr::new("--token-file")
                || argument == OsStr::new("--data-root")
        })
}

#[cfg(not(feature = "legacy-loopback-harness"))]
fn print_primary_help() {
    print!(concat!(
        "eliot-search ",
        env!("CARGO_PKG_VERSION"),
        "\n\n",
        "CONTROL:\n",
        "  eliot-search --help\n",
        "  eliot-search --version\n",
        "  eliot-search health [--daemon PATH]\n",
        "  eliot-search health-data-root ROOT [--daemon PATH]\n",
        "  eliot-search shutdown [--daemon PATH]\n",
        "  eliot-search self-test [--daemon PATH]\n",
        "  eliot-search serve-data-root ROOT [--daemon PATH]\n\n",
        "ONE-SHOT SEARCH:\n",
        "  eliot-search scan-stdin QUERY [--daemon PATH]\n",
        "  eliot-search scan-stdin-ascii-insensitive QUERY [--daemon PATH]\n",
        "  eliot-search scan-file QUERY FILE [--daemon PATH]\n",
        "  eliot-search scan-file-ascii-insensitive QUERY FILE [--daemon PATH]\n\n",
        "PERSISTENT DIRECT CORPUS:\n",
        "  eliot-search index-file ROOT FILE [--daemon PATH]\n",
        "  eliot-search index-directory ROOT DIRECTORY [--daemon PATH]\n",
        "  eliot-search sync-directory ROOT DIRECTORY [--daemon PATH]\n",
        "  eliot-search search-root ROOT QUERY [--daemon PATH]\n",
        "  eliot-search search-root-ascii-insensitive ROOT QUERY [--daemon PATH]\n",
        "  eliot-search list-sources ROOT [--daemon PATH]\n",
        "  eliot-search verify-root ROOT [--daemon PATH]\n",
        "  eliot-search verify-directory-manifests ROOT [--daemon PATH]\n",
        "  eliot-search retire-source ROOT SOURCE_ID [--daemon PATH]\n",
        "  eliot-search read-revision ROOT REVISION_ID START END [--daemon PATH]\n\n",
        "MAINTENANCE:\n",
        "  eliot-search repair-root ROOT [--daemon PATH]\n",
        "  eliot-search gc-root ROOT --dry-run [--daemon PATH]\n",
        "  eliot-search gc-root ROOT --apply [--daemon PATH]\n\n",
        "The canonical authenticated provider transport is installation-scoped ",
        "local IPC. Address, port, endpoint-file and token-file routing are not ",
        "available in the primary build.\n"
    ));
}

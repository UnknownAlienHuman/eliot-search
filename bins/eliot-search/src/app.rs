//! One bounded invocation and dispatch for the installed CLI.
//!
//! Existing command handlers retain their effects until their connected cutover.
//! Legacy endpoint commands remain available only in the explicit harness build.

mod core;
#[cfg(feature = "legacy-loopback-harness")]
mod indexed;

#[cfg(not(feature = "legacy-loopback-harness"))]
use std::ffi::OsStr;
use std::ffi::OsString;
use std::process::ExitCode;

const MAX_ARGUMENTS: usize = 256;
const MAX_ARGUMENT_BYTES: usize = 256 * 1024;

/// Captures native arguments once, before any command can open or spawn anything.
pub fn run_main() -> ExitCode {
    let arguments = std::env::args_os()
        .skip(1)
        .take(MAX_ARGUMENTS + 1)
        .collect::<Vec<_>>();
    let bytes = arguments
        .iter()
        .try_fold(0_usize, |total, value| total.checked_add(value.len()));
    if arguments.len() > MAX_ARGUMENTS || bytes.is_none_or(|size| size > MAX_ARGUMENT_BYTES) {
        return reject("ARGUMENT_LIMIT_EXCEEDED", 1);
    }
    dispatch(&arguments)
}

fn dispatch(arguments: &[OsString]) -> ExitCode {
    let Some((command, tail)) = arguments.split_first() else {
        print_help();
        return ExitCode::SUCCESS;
    };
    let Some(command) = command.to_str() else {
        return reject("COMMAND_NOT_UTF8", 1);
    };

    #[cfg(not(feature = "legacy-loopback-harness"))]
    if is_legacy_provider_request(command, tail) {
        return reject("LEGACY_LOOPBACK_HARNESS_DISABLED", 2);
    }

    match command {
        "--help" | "-h" | "help" => {
            if !tail.is_empty() {
                return reject("USAGE_ERROR", 1);
            }
            print_help();
            ExitCode::SUCCESS
        }
        "--version" | "-V" => local_version(tail),
        "version" if tail.is_empty() => local_version(tail),
        "serve-data-root" => crate::public_client::run_command(tail),
        #[cfg(feature = "legacy-loopback-harness")]
        "remote" => crate::remote_client::run_command(tail),
        #[cfg(feature = "legacy-loopback-harness")]
        "indexed" => indexed::run_command(tail),
        _ => core::run_command(command, tail),
    }
}

fn local_version(arguments: &[OsString]) -> ExitCode {
    if !arguments.is_empty() {
        return reject("USAGE_ERROR", 1);
    }
    println!(
        "{{\"binary\":\"eliot-search\",\"version\":\"{}\",\"protocol_version\":1}}",
        env!("CARGO_PKG_VERSION")
    );
    ExitCode::SUCCESS
}

// Only fixed internal codes enter this diagnostic; never echo untrusted argv.
fn reject(code: &str, status: u8) -> ExitCode {
    eprintln!("{{\"error\":\"{code}\"}}");
    ExitCode::from(status)
}

#[cfg(not(feature = "legacy-loopback-harness"))]
fn is_legacy_provider_request(command: &str, tail: &[OsString]) -> bool {
    matches!(
        command,
        "remote" | "status" | "search" | "lexical" | "refresh" | "indexed"
    )
        || (matches!(command, "health" | "version" | "shutdown")
            && tail.iter().any(|argument| {
                argument == OsStr::new("--address")
                    || argument == OsStr::new("--token-file")
                    || argument == OsStr::new("--data-root")
            }))
}

fn print_help() {
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
        "  eliot-search search-root ROOT QUERY [--daemon PATH]\n",
        "  eliot-search search-root-ascii-insensitive ROOT QUERY [--daemon PATH]\n",
        "  eliot-search list-sources ROOT [--daemon PATH]\n",
        "  eliot-search verify-root ROOT [--daemon PATH]\n",
        "  eliot-search read-revision ROOT REVISION_ID START END [--daemon PATH]\n\n",
        "Legacy write and cleanup commands are unavailable.\n",
        "These are existing DIRECT commands, not the target Qdrant Search product.\n"
    ));
    #[cfg(not(feature = "legacy-loopback-harness"))]
    println!("Address, endpoint-file and token-file routing are unavailable in this build.");
    #[cfg(feature = "legacy-loopback-harness")]
    print!(concat!(
        "\nLEGACY LOOPBACK HARNESS ONLY:\n",
        "  eliot-search remote ADDRESS TOKEN_FILE COMMAND ...\n",
        "  eliot-search status|refresh [--data-root DIR] [--address IP:PORT] [--token-file PATH]\n",
        "  eliot-search search|lexical|indexed TERMS [same options]\n",
        "  eliot-search health|version|shutdown with --address/--token-file use the endpoint\n"
    ));
}

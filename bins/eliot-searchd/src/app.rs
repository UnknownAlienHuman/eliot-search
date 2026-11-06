//! Bounded root-command dispatch and consumed bootstrap settings.

mod bootstrap;

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use crate::native_root_command;

const MAX_ARGUMENTS: usize = 256;
const MAX_ARGUMENT_BYTES: usize = 256 * 1024;

#[derive(Clone, Copy)]
pub(crate) enum RootCommand {
    Inspect,
    Initialize,
    CheckExistingOwner,
    DiscardIncomplete,
}

impl RootCommand {
    pub(crate) const fn operation(self) -> &'static str {
        match self {
            Self::Inspect => "inspect-data-root",
            Self::Initialize => "initialize-data-root",
            Self::CheckExistingOwner => "check-existing-root-owner",
            Self::DiscardIncomplete => "discard-incomplete-data-root",
        }
    }
}

pub(crate) struct RootInvocation {
    pub(crate) command: RootCommand,
    pub(crate) data_root: PathBuf,
}

fn emit_error(code: &str) -> ExitCode {
    eprintln!("{{\"error\":\"{code}\"}}");
    ExitCode::from(2)
}

fn capture_arguments() -> Result<Vec<OsString>, &'static str> {
    let arguments = std::env::args_os()
        .skip(1)
        .take(MAX_ARGUMENTS + 1)
        .collect::<Vec<_>>();
    let bytes = arguments
        .iter()
        .try_fold(0_usize, |total, value| total.checked_add(value.len()));
    if arguments.len() > MAX_ARGUMENTS || bytes.is_none_or(|size| size > MAX_ARGUMENT_BYTES) {
        return Err("ARGUMENT_LIMIT_EXCEEDED");
    }
    Ok(arguments)
}

fn print_help() {
    print!(concat!(
        "eliot-searchd ",
        env!("CARGO_PKG_VERSION"),
        "\n\n",
        "  eliot-searchd --help\n",
        "  eliot-searchd --version\n",
        "  eliot-searchd --inspect-data-root [ROOT] [--data-root ROOT] [--config-file PATH]\n",
        "  eliot-searchd --initialize-data-root [ROOT] [--data-root ROOT] [--config-file PATH]\n",
        "  eliot-searchd --check-existing-root-owner [ROOT] [--data-root ROOT] [--config-file PATH]\n",
        "  eliot-searchd --discard-incomplete-data-root [ROOT] [--data-root ROOT] [--config-file PATH]\n",
    ));
}

pub(crate) fn run_main() -> ExitCode {
    let started = Instant::now();
    let arguments = match capture_arguments() {
        Ok(arguments) => arguments,
        Err(code) => return emit_error(code),
    };
    let parsed = match bootstrap::parse(&arguments) {
        Ok(parsed) => parsed,
        Err(error) => return emit_error(&error),
    };
    match parsed {
        bootstrap::Parsed::Help => {
            print_help();
            ExitCode::SUCCESS
        }
        bootstrap::Parsed::Version => {
            println!(
                "{{\"binary\":\"eliot-searchd\",\"version\":\"{}\"}}",
                env!("CARGO_PKG_VERSION")
            );
            ExitCode::SUCCESS
        }
        bootstrap::Parsed::Root(cli) => {
            let startup_cwd = match std::env::current_dir() {
                Ok(path) => path,
                Err(_) => return emit_error("BOOTSTRAP_SETTINGS_INVALID:startup_cwd"),
            };
            let invocation = match bootstrap::resolve(cli, &startup_cwd) {
                Ok(invocation) => invocation,
                Err(error) => return emit_error(&error),
            };
            native_root_command::run(invocation, started)
        }
    }
}

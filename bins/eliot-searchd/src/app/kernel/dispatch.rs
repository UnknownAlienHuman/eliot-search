//! Top-level daemon argument dispatch and process exit mapping.

use std::ffi::OsString;
use std::process::ExitCode;
use std::time::Instant;

use crate::development::{Health, scan_text};
use crate::sha256;

use super::commands::{cmd_scan_file, cmd_scan_stdin};
use super::protocol::serve_stdio;
use super::spec::{Command, help, version_json};
use super::status::{config_status_line, shell_health_effective};

fn self_test() -> Result<(), &'static str> {
    if Command::parse("health") != Ok(Command::Health) {
        return Err("HEALTH_COMMAND_PARSE_FAILED");
    }
    if Command::parse("shutdown") != Ok(Command::Shutdown) {
        return Err("SHUTDOWN_COMMAND_PARSE_FAILED");
    }
    let health = Health::SHELL.json();
    let effective = crate::config_composition::build_effective_defaults()
        .map_err(|_| "HEALTH_TRUTHFULNESS_FAILED")?;
    let report = crate::config_composition::derive_readiness(
        &effective,
        crate::config_composition::shell_dependencies(),
        crate::config_composition::AcceptedReceipts::default(),
    );
    if report.capabilities.search_available || report.capabilities.indexed_search_available {
        return Err("HEALTH_TRUTHFULNESS_FAILED");
    }
    let derived = Health::from_readiness(&report);
    if derived.capabilities.search_available || derived.capabilities.indexed_search_available {
        return Err("HEALTH_TRUTHFULNESS_FAILED");
    }
    if !derived.json().contains("\"search_available\":false") {
        return Err("HEALTH_TRUTHFULNESS_FAILED");
    }
    if !health.contains("\"source_backed_search_available\":false") {
        return Err("HEALTH_TRUTHFULNESS_FAILED");
    }
    let result = scan_text("alpha\nbeta alpha", "alpha", false).map_err(|_| "SCAN_FAILED")?;
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

const MAX_ARGUMENTS: usize = 256;
const MAX_ARGUMENT_BYTES: usize = 256 * 1024;

fn require_argument_count(arguments: &[OsString], expected: usize) -> Result<(), String> {
    if arguments.len() == expected {
        Ok(())
    } else {
        Err("USAGE_ERROR".to_owned())
    }
}

fn capture_arguments() -> Result<Vec<OsString>, String> {
    let mut arguments = Vec::new();
    let mut bytes = 0_usize;
    for value in std::env::args_os().skip(1) {
        bytes = bytes
            .checked_add(value.len())
            .ok_or("ARGUMENT_LIMIT_EXCEEDED")?;
        if arguments.len() == MAX_ARGUMENTS || bytes > MAX_ARGUMENT_BYTES {
            return Err("ARGUMENT_LIMIT_EXCEEDED".to_owned());
        }
        arguments.push(value);
    }
    Ok(arguments)
}

fn run() -> Result<ExitCode, String> {
    let started = Instant::now();
    let raw = capture_arguments()?;
    if crate::config_composition::looks_like_root_command(&raw) {
        let invocation = crate::config_composition::load_bootstrap_settings(&raw)?;
        return Ok(crate::native_root_command::run_native_root_command(
            invocation, started,
        ));
    }
    let (arguments, cli) = crate::config_composition::strip_config_args_os(&raw)?;
    let Some(first) = arguments.first() else {
        if !raw.is_empty() {
            return Err("USAGE_ERROR".to_owned());
        }
        print!("{}", help());
        return Ok(ExitCode::SUCCESS);
    };
    let command = first.to_str().ok_or("COMMAND_NOT_UTF8")?;
    match command {
        "--index-file" | "--index-directory" | "--retire-source" | "--repair-root"
        | "--gc-root" => Err("LEGACY_EFFECTS_UNAVAILABLE".to_owned()),
        "--register-source-root"
        | "--register-source-root-v2"
        | "--admit-source-v2"
        | "--prepare-revision-dek-v1"
        | "--select-revision-dek-v1"
        | "--unregister-source-root"
        | "--source-roots"
        | "--health-data-root"
        | "--verify-root"
        | "--list-sources"
        | "--serve-data-root"
        | "--search-root"
        | "--search-root-ascii-insensitive"
        | "--read-revision"
        | "--sync-source-roots" => Ok(crate::owner_composition::run_direct_registry(&raw, started)),
        "--plan-control-migration"
        | "--bootstrap-control-migration"
        | "--stage-source-control-migration" => {
            Ok(crate::source_migration_command::run_command(&raw, started))
        }
        "--prepare-revision" | "--prepare-root" => {
            require_argument_count(&raw, arguments.len())?;
            Ok(crate::preparation_composition::run_command(&arguments))
        }
        #[cfg(feature = "legacy-loopback-harness")]
        "--serve-loopback-data-root" => {
            Ok(crate::authenticated_proxy::run_command(&arguments, &cli))
        }
        _ => run_shell(&arguments, command, &cli).map(|()| ExitCode::SUCCESS),
    }
}

fn run_shell(
    arguments: &[OsString],
    command: &str,
    cli: &crate::config_composition::CliConfigArgs,
) -> Result<(), String> {
    match command {
        "--help" | "-h" => {
            require_argument_count(arguments, 1)?;
            print!("{}", help());
        }
        "--version" | "-V" => {
            require_argument_count(arguments, 1)?;
            println!("{}", version_json());
        }
        "--health" => {
            require_argument_count(arguments, 1)?;
            println!("{}", shell_health_effective(cli)?.json());
        }
        "--config-status" => {
            require_argument_count(arguments, 1)?;
            println!("{}", config_status_line(cli)?);
        }
        "--self-test" => {
            require_argument_count(arguments, 1)?;
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
            require_argument_count(arguments, 1)?;
            serve_stdio(shell_health_effective(cli)?)
                .map_err(|error| format!("STDIO_ERROR:{error}"))?;
        }
        "--scan-stdin" | "--scan-stdin-ascii-insensitive" => {
            require_argument_count(arguments, 2)?;
            cmd_scan_stdin(arguments, command)?;
        }
        "--scan-file" | "--scan-file-ascii-insensitive" => {
            require_argument_count(arguments, 3)?;
            cmd_scan_file(arguments, command)?;
        }
        _ => return Err("UNKNOWN_ARGUMENT".to_owned()),
    }
    Ok(())
}

/// Captures one native invocation and maps the selected handler's result to process status.
pub fn run_main() -> ExitCode {
    match run() {
        Ok(exit) => exit,
        Err(error) => {
            eprintln!(
                "{{\"error\":\"{}\"}}",
                super::source_root_commands::escape_json(&error)
            );
            ExitCode::from(2)
        }
    }
}

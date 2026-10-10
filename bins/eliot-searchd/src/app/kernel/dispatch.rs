//! Top-level daemon argument dispatch and process exit mapping.

use std::process::ExitCode;

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

fn require_argument_count(arguments: &[String], expected: usize) -> Result<(), String> {
    if arguments.len() == expected {
        Ok(())
    } else {
        Err("USAGE_ERROR".to_owned())
    }
}

fn run() -> Result<(), String> {
    let raw = std::env::args().skip(1).collect::<Vec<_>>();
    let (arguments, _) = crate::config_composition::parse_cli_config_args(&raw)?;
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
            println!("{}", shell_health_effective()?.json());
        }
        "--config-status" => {
            require_argument_count(&arguments, 1)?;
            println!("{}", config_status_line()?);
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
            serve_stdio(shell_health_effective()?)
                .map_err(|error| format!("STDIO_ERROR:{error}"))?;
        }
        "--source-roots"
        | "--register-source-root"
        | "--unregister-source-root"
        | "--sync-source-roots" => super::source_root_commands::run(&arguments)?,
        "--scan-stdin" | "--scan-stdin-ascii-insensitive" => {
            require_argument_count(&arguments, 2)?;
            cmd_scan_stdin(&arguments, argument)?;
        }
        "--scan-file" | "--scan-file-ascii-insensitive" => {
            require_argument_count(&arguments, 3)?;
            cmd_scan_file(&arguments, argument)?;
        }
        _ => return Err(format!("UNKNOWN_ARGUMENT:{argument}")),
    }
    Ok(())
}

/// Runs the daemon command application and maps failures to process status.
pub fn run_main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!(
                "{{\"error\":\"{}\"}}",
                super::source_root_commands::escape_json(&error)
            );
            ExitCode::from(2)
        }
    }
}

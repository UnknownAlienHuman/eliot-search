//! Top-level daemon argument dispatch and process exit mapping.

use std::process::ExitCode;

use crate::development::{Health, scan_text};
use crate::sha256;

use super::commands::{
    cmd_gc_root, cmd_health_data_root, cmd_index_directory, cmd_index_file,
    cmd_list_sources, cmd_read_revision, cmd_repair_root,
    cmd_retire_source, cmd_scan_file, cmd_scan_stdin, cmd_search_root,
    cmd_serve_data_root, cmd_verify_root,
};
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
    if report.capabilities.search_available
        || report.capabilities.indexed_search_available
    {
        return Err("HEALTH_TRUTHFULNESS_FAILED");
    }
    let derived = Health::from_readiness(&report);
    if derived.capabilities.search_available
        || derived.capabilities.indexed_search_available
    {
        return Err("HEALTH_TRUTHFULNESS_FAILED");
    }
    if !derived.json().contains("\"search_available\":false") {
        return Err("HEALTH_TRUTHFULNESS_FAILED");
    }
    if !health.contains("\"source_backed_search_available\":false") {
        return Err("HEALTH_TRUTHFULNESS_FAILED");
    }
    let result = scan_text("alpha\nbeta alpha", "alpha", false)
        .map_err(|_| "SCAN_FAILED")?;
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

fn require_argument_count(
    arguments: &[String],
    expected: usize,
) -> Result<(), String> {
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
        "--health-data-root" => {
            require_argument_count(&arguments, 2)?;
            cmd_health_data_root(&arguments)?;
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
        "--serve-data-root" => {
            require_argument_count(&arguments, 2)?;
            cmd_serve_data_root(&arguments)?;
        }
        "--source-roots"
        | "--register-source-root"
        | "--unregister-source-root"
        | "--sync-source-roots" => crate::source_root_commands::run(&arguments)?,
        "--scan-stdin" | "--scan-stdin-ascii-insensitive" => {
            require_argument_count(&arguments, 2)?;
            cmd_scan_stdin(&arguments, argument)?;
        }
        "--scan-file" | "--scan-file-ascii-insensitive" => {
            require_argument_count(&arguments, 3)?;
            cmd_scan_file(&arguments, argument)?;
        }
        "--index-file" => {
            require_argument_count(&arguments, 3)?;
            cmd_index_file(&arguments)?;
        }
        "--index-directory" => {
            require_argument_count(&arguments, 3)?;
            cmd_index_directory(&arguments)?;
        }
        "--search-root" | "--search-root-ascii-insensitive" => {
            require_argument_count(&arguments, 3)?;
            cmd_search_root(&arguments, argument)?;
        }
        "--list-sources" => {
            require_argument_count(&arguments, 2)?;
            cmd_list_sources(&arguments)?;
        }
        "--verify-root" => {
            require_argument_count(&arguments, 2)?;
            cmd_verify_root(&arguments)?;
        }
        "--retire-source" => {
            require_argument_count(&arguments, 3)?;
            cmd_retire_source(&arguments)?;
        }
        "--read-revision" => {
            require_argument_count(&arguments, 5)?;
            cmd_read_revision(&arguments)?;
        }
        "--repair-root" => {
            require_argument_count(&arguments, 2)?;
            cmd_repair_root(&arguments)?;
        }
        "--gc-root" => {
            require_argument_count(&arguments, 3)?;
            cmd_gc_root(&arguments)?;
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
                crate::source_root_commands::escape_json(&error)
            );
            ExitCode::from(2)
        }
    }
}

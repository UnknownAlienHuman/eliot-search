//! CLI forwarding for immutable directory inventory operations.

use std::env;
use std::ffi::{OsStr, OsString};
use std::path::PathBuf;
use std::process::{Command, ExitCode, Stdio};

/// Intercepts directory inventory commands before the general CLI dispatcher.
pub(crate) fn maybe_run() -> Option<ExitCode> {
    let arguments = env::args_os().skip(1).collect::<Vec<_>>();
    let command = arguments.first().and_then(|value| value.to_str())?;
    let result = match command {
        "sync-directory" => forward("--sync-directory", &arguments[1..], 2),
        "verify-directory-manifests" => {
            forward("--verify-directory-manifests", &arguments[1..], 1)
        }
        _ => return None,
    };
    Some(match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{{\"error\":{}}}", json_string(&error));
            ExitCode::from(2)
        }
    })
}

fn forward(
    daemon_command: &str,
    arguments: &[OsString],
    required_arguments: usize,
) -> Result<(), String> {
    let (arguments, explicit_daemon) = split_daemon_option(arguments, required_arguments)?;
    let daemon = daemon_path(explicit_daemon);
    let status = Command::new(daemon)
        .arg(daemon_command)
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|error| format!("DAEMON_START_ERROR:{error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("DAEMON_COMMAND_FAILED:{status}"))
    }
}

fn split_daemon_option(
    arguments: &[OsString],
    required_arguments: usize,
) -> Result<(Vec<OsString>, Option<PathBuf>), String> {
    if arguments.len() == required_arguments {
        return Ok((arguments.to_vec(), None));
    }
    if arguments.len() == required_arguments.saturating_add(2)
        && arguments[required_arguments] == OsStr::new("--daemon")
    {
        return Ok((
            arguments[..required_arguments].to_vec(),
            Some(PathBuf::from(&arguments[required_arguments + 1])),
        ));
    }
    Err("USAGE_ERROR".to_owned())
}

fn daemon_path(explicit: Option<PathBuf>) -> PathBuf {
    if let Some(path) = explicit {
        return path;
    }
    if let Some(path) = env::var_os("ELIOT_SEARCHD_BIN") {
        return PathBuf::from(path);
    }
    let executable = env::current_exe().unwrap_or_else(|_| PathBuf::from("eliot-search"));
    let sibling = executable.with_file_name(if cfg!(windows) {
        "eliot-searchd.exe"
    } else {
        "eliot-searchd"
    });
    if sibling.exists() {
        sibling
    } else {
        PathBuf::from(if cfg!(windows) {
            "eliot-searchd.exe"
        } else {
            "eliot-searchd"
        })
    }
}

fn json_string(value: &str) -> String {
    let mut output = String::with_capacity(value.len().saturating_add(2));
    output.push('"');
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character if character.is_control() => {
                use std::fmt::Write as _;
                let _ = write!(&mut output, "\\u{:04x}", u32::from(character));
            }
            character => output.push(character),
        }
    }
    output.push('"');
    output
}

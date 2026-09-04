//! Command application for the protocol-only ELIOT Search CLI.

use std::env;
use std::ffi::{OsStr, OsString};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

const MAX_RESPONSE_BYTES: usize = 64 * 1024;

fn help() -> &'static str {
    concat!(
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
        "  eliot-search search-root ROOT QUERY [--daemon PATH]\n",
        "  eliot-search search-root-ascii-insensitive ROOT QUERY [--daemon PATH]\n",
        "  eliot-search list-sources ROOT [--daemon PATH]\n",
        "  eliot-search verify-root ROOT [--daemon PATH]\n",
        "  eliot-search retire-source ROOT SOURCE_ID [--daemon PATH]\n",
        "  eliot-search read-revision ROOT REVISION_ID START END [--daemon PATH]\n\n",
        "MAINTENANCE:\n",
        "  eliot-search repair-root ROOT [--daemon PATH]\n",
        "  eliot-search gc-root ROOT --dry-run [--daemon PATH]\n",
        "  eliot-search gc-root ROOT --apply [--daemon PATH]\n\n",
        "The CLI prints newline-delimited JSON. Persistent DIRECT results are ",
        "source-backed by immutable revision readback. The current development ",
        "revision store is plaintext and reports encrypted_at_rest=false.\n",
    )
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

fn read_bounded_line(reader: &mut impl BufRead) -> Result<String, String> {
    let mut line = String::new();
    let limit = u64::try_from(MAX_RESPONSE_BYTES + 1).unwrap_or(u64::MAX);
    let bytes = reader
        .take(limit)
        .read_line(&mut line)
        .map_err(|error| format!("DAEMON_READ_ERROR:{error}"))?;
    if bytes == 0 {
        return Err("DAEMON_CLOSED_PROTOCOL".to_owned());
    }
    if bytes > MAX_RESPONSE_BYTES {
        return Err("DAEMON_RESPONSE_TOO_LARGE".to_owned());
    }
    Ok(line.trim_end_matches(['\r', '\n']).to_owned())
}

fn invoke_stdio(daemon: &Path, request: &str) -> Result<Vec<String>, String> {
    let mut child = Command::new(daemon)
        .arg("--stdio")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|error| format!("DAEMON_START_ERROR:{error}"))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "DAEMON_STDOUT_UNAVAILABLE".to_owned())?;
    let mut reader = BufReader::new(stdout);
    let ready = read_bounded_line(&mut reader)?;
    if !ready.contains("\"event\":\"ready\"") {
        let _ = child.kill();
        let _ = child.wait();
        return Err(format!("DAEMON_NOT_READY:{ready}"));
    }

    let stdin = child
        .stdin
        .as_mut()
        .ok_or_else(|| "DAEMON_STDIN_UNAVAILABLE".to_owned())?;
    stdin
        .write_all(request.as_bytes())
        .and_then(|()| stdin.write_all(b"\n"))
        .and_then(|()| stdin.flush())
        .map_err(|error| format!("DAEMON_WRITE_ERROR:{error}"))?;

    let mut responses = vec![read_bounded_line(&mut reader)?];
    if request == "shutdown" {
        responses.push(read_bounded_line(&mut reader)?);
    } else {
        let stdin = child
            .stdin
            .as_mut()
            .ok_or_else(|| "DAEMON_STDIN_UNAVAILABLE".to_owned())?;
        stdin
            .write_all(b"shutdown\n")
            .and_then(|()| stdin.flush())
            .map_err(|error| format!("DAEMON_WRITE_ERROR:{error}"))?;
        let _ = read_bounded_line(&mut reader)?;
        let _ = read_bounded_line(&mut reader)?;
    }

    drop(child.stdin.take());
    let status = child
        .wait()
        .map_err(|error| format!("DAEMON_WAIT_ERROR:{error}"))?;
    if !status.success() {
        return Err(format!("DAEMON_EXITED:{status}"));
    }
    Ok(responses)
}

fn invoke_daemon(
    daemon: &Path,
    arguments: &[OsString],
    inherit_stdin: bool,
) -> Result<(), String> {
    let mut command = Command::new(daemon);
    command.args(arguments);
    command.stdin(if inherit_stdin {
        Stdio::inherit()
    } else {
        Stdio::null()
    });
    let status = command
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

fn forward(
    daemon_command: &str,
    arguments: &[OsString],
    required_arguments: usize,
    inherit_stdin: bool,
) -> Result<(), String> {
    let (arguments, explicit_daemon) =
        split_daemon_option(arguments, required_arguments)?;
    let daemon = daemon_path(explicit_daemon);
    let mut forwarded = Vec::with_capacity(arguments.len().saturating_add(1));
    forwarded.push(OsString::from(daemon_command));
    forwarded.extend(arguments);
    invoke_daemon(&daemon, &forwarded, inherit_stdin)
}

fn run() -> Result<(), String> {
    let arguments = env::args_os().skip(1).collect::<Vec<_>>();
    let Some(command) = arguments.first().and_then(|value| value.to_str()) else {
        if arguments.is_empty() {
            print!("{}", help());
            return Ok(());
        }
        return Err("COMMAND_NOT_UTF8".to_owned());
    };
    let tail = &arguments[1..];

    match command {
        "--help" | "-h" => {
            if !tail.is_empty() {
                return Err("USAGE_ERROR".to_owned());
            }
            print!("{}", help());
        }
        "--version" | "-V" | "version" => {
            if !tail.is_empty() {
                return Err("USAGE_ERROR".to_owned());
            }
            println!(
                "{{\"binary\":\"eliot-search\",\"version\":\"{}\",\"protocol_version\":1}}",
                env!("CARGO_PKG_VERSION")
            );
        }
        "health" => {
            let (_, explicit_daemon) = split_daemon_option(tail, 0)?;
            let daemon = daemon_path(explicit_daemon);
            for response in invoke_stdio(&daemon, "health")? {
                println!("{response}");
            }
        }
        "shutdown" => {
            let (_, explicit_daemon) = split_daemon_option(tail, 0)?;
            let daemon = daemon_path(explicit_daemon);
            for response in invoke_stdio(&daemon, "shutdown")? {
                println!("{response}");
            }
        }
        "self-test" => {
            let (_, explicit_daemon) = split_daemon_option(tail, 0)?;
            let daemon = daemon_path(explicit_daemon);
            let output = Command::new(daemon)
                .arg("--self-test")
                .output()
                .map_err(|error| format!("DAEMON_START_ERROR:{error}"))?;
            if !output.status.success() {
                return Err(format!("DAEMON_SELF_TEST_FAILED:{}", output.status));
            }
            if output.stdout.len() > MAX_RESPONSE_BYTES {
                return Err("DAEMON_RESPONSE_TOO_LARGE".to_owned());
            }
            print!("{}", String::from_utf8_lossy(&output.stdout));
        }
        "health-data-root" => forward("--health-data-root", tail, 1, false)?,
        "serve-data-root" => forward("--serve-data-root", tail, 1, true)?,
        "scan-stdin" => forward("--scan-stdin", tail, 1, true)?,
        "scan-stdin-ascii-insensitive" => {
            forward("--scan-stdin-ascii-insensitive", tail, 1, true)?;
        }
        "scan-file" => forward("--scan-file", tail, 2, false)?,
        "scan-file-ascii-insensitive" => {
            forward("--scan-file-ascii-insensitive", tail, 2, false)?;
        }
        "index-file" => forward("--index-file", tail, 2, false)?,
        "index-directory" => forward("--index-directory", tail, 2, false)?,
        "search-root" => forward("--search-root", tail, 2, false)?,
        "search-root-ascii-insensitive" => {
            forward("--search-root-ascii-insensitive", tail, 2, false)?;
        }
        "list-sources" => forward("--list-sources", tail, 1, false)?,
        "verify-root" => forward("--verify-root", tail, 1, false)?,
        "retire-source" => forward("--retire-source", tail, 2, false)?,
        "read-revision" => forward("--read-revision", tail, 4, false)?,
        "repair-root" => forward("--repair-root", tail, 1, false)?,
        "gc-root" => forward("--gc-root", tail, 2, false)?,
        _ => return Err(format!("UNKNOWN_COMMAND:{command}")),
    }
    Ok(())
}

/// Runs the CLI and maps failures to process status.
pub(crate) fn run_main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{{\"error\":\"{}\"}}", error.replace('"', "'"));
            ExitCode::from(2)
        }
    }
}

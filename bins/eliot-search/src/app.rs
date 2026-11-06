//! Four root operations over one bounded, owned daemon child.

use std::env;
use std::ffi::{OsStr, OsString};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const MAX_ARGUMENTS: usize = 256;
const MAX_ARGUMENT_BYTES: usize = 256 * 1024;
const MAX_PATH_BYTES: usize = 32 * 1024;
const MAX_CAPTURE_BYTES: usize = 64 * 1024;
const COMMAND_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Clone, Copy)]
enum RootCommand {
    Inspect,
    Initialize,
    CheckExistingOwner,
    DiscardIncomplete,
}

impl RootCommand {
    const fn name(self) -> &'static str {
        match self {
            Self::Inspect => "inspect-data-root",
            Self::Initialize => "initialize-data-root",
            Self::CheckExistingOwner => "check-existing-root-owner",
            Self::DiscardIncomplete => "discard-incomplete-data-root",
        }
    }

    const fn daemon_flag(self) -> &'static str {
        match self {
            Self::Inspect => "--inspect-data-root",
            Self::Initialize => "--initialize-data-root",
            Self::CheckExistingOwner => "--check-existing-root-owner",
            Self::DiscardIncomplete => "--discard-incomplete-data-root",
        }
    }
}

enum Parsed {
    Help,
    Version,
    Root {
        command: RootCommand,
        settings: Vec<OsString>,
        daemon: Option<PathBuf>,
    },
}

struct Captured {
    bytes: Vec<u8>,
    exceeded: bool,
}

fn emit_error(code: &str) -> i32 {
    eprintln!("{{\"error\":\"{code}\"}}");
    2
}

fn emit_child_exit(status: ExitStatus) -> i32 {
    let code = status.code().unwrap_or(1);
    eprintln!("{{\"error\":\"DAEMON_EXITED\",\"exit_code\":{code}}}");
    code
}

fn emit_error_status(code: &str, status: i32) -> i32 {
    eprintln!("{{\"error\":\"{code}\"}}");
    status
}

fn capture_arguments() -> Result<Vec<OsString>, &'static str> {
    let arguments = env::args_os()
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

fn is_help_flag(value: &OsStr) -> bool {
    matches!(value.to_str(), Some("--help" | "-h"))
}

fn is_version_flag(value: &OsStr) -> bool {
    matches!(value.to_str(), Some("--version" | "-V"))
}

fn is_help_command(value: &OsStr) -> bool {
    value == OsStr::new("help")
}

fn is_version_command(value: &OsStr) -> bool {
    value == OsStr::new("version")
}

fn command(value: &OsStr) -> Option<RootCommand> {
    match value.to_str()? {
        "inspect-data-root" => Some(RootCommand::Inspect),
        "initialize-data-root" => Some(RootCommand::Initialize),
        "check-existing-root-owner" => Some(RootCommand::CheckExistingOwner),
        "discard-incomplete-data-root" => Some(RootCommand::DiscardIncomplete),
        _ => None,
    }
}

fn is_daemon_root_flag(value: &OsStr) -> bool {
    matches!(
        value.to_str(),
        Some(
            "--inspect-data-root"
                | "--initialize-data-root"
                | "--check-existing-root-owner"
                | "--discard-incomplete-data-root"
        )
    )
}

fn validate_path(value: &OsStr) -> Result<(), &'static str> {
    if value.is_empty() || value.len() > MAX_PATH_BYTES || value.as_encoded_bytes().contains(&0) {
        return Err("USAGE_ERROR");
    }
    Ok(())
}

fn parse(arguments: &[OsString]) -> Result<Parsed, &'static str> {
    if arguments.is_empty() {
        return Ok(Parsed::Help);
    }
    if arguments.len() == 1 && (is_help_flag(&arguments[0]) || is_help_command(&arguments[0])) {
        return Ok(Parsed::Help);
    }
    if arguments.len() == 1 && (is_version_flag(&arguments[0]) || is_version_command(&arguments[0]))
    {
        return Ok(Parsed::Version);
    }
    let Some(root_command) = command(&arguments[0]) else {
        return Err("UNKNOWN_COMMAND");
    };

    let mut settings = Vec::new();
    let mut daemon = None;
    let mut data_root_seen = false;
    let mut positional_root_seen = false;
    let mut config_seen = false;
    let mut index = 1;
    while index < arguments.len() {
        let argument = &arguments[index];
        if is_help_flag(argument) || is_version_flag(argument) {
            return Err("USAGE_ERROR");
        }
        if is_daemon_root_flag(argument) {
            return Err("USAGE_ERROR");
        }
        if argument == OsStr::new("--daemon") {
            if daemon.is_some() {
                return Err("USAGE_ERROR");
            }
            let value = arguments.get(index + 1).ok_or("USAGE_ERROR")?;
            validate_path(value)?;
            if is_help_flag(value) || is_version_flag(value) || is_daemon_root_flag(value) {
                return Err("USAGE_ERROR");
            }
            daemon = Some(PathBuf::from(value));
            index += 2;
        } else if argument == OsStr::new("--data-root") {
            if data_root_seen || positional_root_seen {
                return Err("USAGE_ERROR");
            }
            let value = arguments.get(index + 1).ok_or("USAGE_ERROR")?;
            validate_path(value)?;
            if is_help_flag(value) || is_version_flag(value) || is_daemon_root_flag(value) {
                return Err("USAGE_ERROR");
            }
            data_root_seen = true;
            settings.push(argument.clone());
            settings.push(value.clone());
            index += 2;
        } else if argument == OsStr::new("--config-file") {
            if config_seen {
                return Err("USAGE_ERROR");
            }
            let value = arguments.get(index + 1).ok_or("USAGE_ERROR")?;
            validate_path(value)?;
            if is_help_flag(value) || is_version_flag(value) || is_daemon_root_flag(value) {
                return Err("USAGE_ERROR");
            }
            config_seen = true;
            settings.push(argument.clone());
            settings.push(value.clone());
            index += 2;
        } else if argument
            .to_str()
            .is_some_and(|value| value.starts_with("--"))
        {
            return Err("UNKNOWN_ARGUMENT");
        } else {
            if positional_root_seen || data_root_seen {
                return Err("USAGE_ERROR");
            }
            validate_path(argument)?;
            positional_root_seen = true;
            settings.push(argument.clone());
            index += 1;
        }
    }
    Ok(Parsed::Root {
        command: root_command,
        settings,
        daemon,
    })
}

fn daemon_path(explicit: Option<PathBuf>) -> Result<PathBuf, &'static str> {
    if let Some(path) = explicit {
        return Ok(path);
    }
    if let Some(path) = env::var_os("ELIOT_SEARCHD_BIN") {
        validate_path(&path).map_err(|_| "DAEMON_PATH_INVALID")?;
        return Ok(PathBuf::from(path));
    }

    let executable_name = if cfg!(windows) {
        "eliot-searchd.exe"
    } else {
        "eliot-searchd"
    };
    let sibling = env::current_exe()
        .ok()
        .map(|path| path.with_file_name(executable_name));
    if let Some(path) = sibling.filter(|path| path.exists()) {
        Ok(path)
    } else {
        Ok(PathBuf::from(executable_name))
    }
}

fn capture_stream(mut reader: impl Read) -> Result<Captured, ()> {
    let mut bytes = Vec::with_capacity(MAX_CAPTURE_BYTES);
    let mut buffer = [0_u8; 8 * 1024];
    let mut exceeded = false;
    loop {
        let count = reader.read(&mut buffer).map_err(|_| ())?;
        if count == 0 {
            break;
        }
        let remaining = MAX_CAPTURE_BYTES.saturating_sub(bytes.len());
        let keep = remaining.min(count);
        bytes.extend_from_slice(&buffer[..keep]);
        exceeded |= keep < count;
    }
    Ok(Captured { bytes, exceeded })
}

fn spawn_reader(
    reader: impl Read + Send + 'static,
    name: &'static str,
) -> Result<JoinHandle<Result<Captured, ()>>, ()> {
    thread::Builder::new()
        .name(name.to_owned())
        .spawn(move || capture_stream(reader))
        .map_err(|_| ())
}

fn stop_and_reap(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

fn wait_for_child(child: &mut Child, deadline: Instant) -> Result<ExitStatus, &'static str> {
    loop {
        if Instant::now() >= deadline {
            let _ = child.kill();
            return match child.wait() {
                Ok(_) => Err("DAEMON_TIMEOUT"),
                Err(_) => Err("DAEMON_WAIT_ERROR"),
            };
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                return if Instant::now() < deadline {
                    Ok(status)
                } else {
                    Err("DAEMON_TIMEOUT")
                };
            }
            Ok(None) => {}
            Err(_) => {
                stop_and_reap(child);
                return Err("DAEMON_WAIT_ERROR");
            }
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn join_reader(reader: JoinHandle<Result<Captured, ()>>) -> Result<Captured, &'static str> {
    reader
        .join()
        .map_err(|_| "DAEMON_OUTPUT_READ_ERROR")?
        .map_err(|_| "DAEMON_OUTPUT_READ_ERROR")
}

fn capture_child(
    daemon: &Path,
    command: RootCommand,
    settings: &[OsString],
    deadline: Instant,
) -> Result<
    (
        ExitStatus,
        Result<Captured, &'static str>,
        Result<Captured, &'static str>,
    ),
    &'static str,
> {
    let mut child = Command::new(daemon)
        .arg(command.daemon_flag())
        .args(settings)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| "DAEMON_START_ERROR")?;

    let stdout = match child.stdout.take() {
        Some(stdout) => stdout,
        None => {
            stop_and_reap(&mut child);
            return Err("DAEMON_STDOUT_UNAVAILABLE");
        }
    };
    let stdout_reader = match spawn_reader(stdout, "eliot-searchd-stdout") {
        Ok(reader) => reader,
        Err(()) => {
            stop_and_reap(&mut child);
            return Err("DAEMON_OUTPUT_SETUP_ERROR");
        }
    };

    let stderr = match child.stderr.take() {
        Some(stderr) => stderr,
        None => {
            stop_and_reap(&mut child);
            let _ = stdout_reader.join();
            return Err("DAEMON_STDERR_UNAVAILABLE");
        }
    };
    let stderr_reader = match spawn_reader(stderr, "eliot-searchd-stderr") {
        Ok(reader) => reader,
        Err(()) => {
            stop_and_reap(&mut child);
            let _ = stdout_reader.join();
            return Err("DAEMON_OUTPUT_SETUP_ERROR");
        }
    };

    let status = match wait_for_child(&mut child, deadline) {
        Ok(status) => status,
        Err(error) => {
            let _ = stdout_reader.join();
            let _ = stderr_reader.join();
            return Err(error);
        }
    };
    let stdout = join_reader(stdout_reader);
    let stderr = join_reader(stderr_reader);
    Ok((status, stdout, stderr))
}

fn validated_snapshot(bytes: &[u8], command: RootCommand) -> Option<String> {
    let text = std::str::from_utf8(bytes).ok()?;
    let line = text
        .strip_suffix("\r\n")
        .or_else(|| text.strip_suffix('\n'))
        .unwrap_or(text);
    if line.is_empty() || line.contains('\r') || line.contains('\n') {
        return None;
    }
    let states: &[&str] = match command {
        RootCommand::Initialize | RootCommand::CheckExistingOwner => &["current"],
        RootCommand::Inspect | RootCommand::DiscardIncomplete => &[
            "absent",
            "empty",
            "incomplete_initialization",
            "current",
            "corrupt",
            "unsupported",
            "busy",
            "archive",
            "incomplete_import",
        ],
    };
    states
        .iter()
        .map(|state| {
            format!(
                "{{\"operation\":\"{}\",\"root_state\":\"{}\"}}",
                command.name(),
                state
            )
        })
        .find(|expected| expected == line)
}

fn print_help() {
    print!(concat!(
        "eliot-search ",
        env!("CARGO_PKG_VERSION"),
        "\n\n",
        "  eliot-search --help\n",
        "  eliot-search --version\n",
        "  eliot-search inspect-data-root [ROOT] [--data-root ROOT] [--config-file PATH] [--daemon PATH]\n",
        "  eliot-search initialize-data-root [ROOT] [--data-root ROOT] [--config-file PATH] [--daemon PATH]\n",
        "  eliot-search check-existing-root-owner [ROOT] [--data-root ROOT] [--config-file PATH] [--daemon PATH]\n",
        "  eliot-search discard-incomplete-data-root [ROOT] [--data-root ROOT] [--config-file PATH] [--daemon PATH]\n",
    ));
}

pub(super) fn run_main() -> i32 {
    let started = Instant::now();
    let arguments = match capture_arguments() {
        Ok(arguments) => arguments,
        Err(error) => return emit_error(error),
    };
    let parsed = match parse(&arguments) {
        Ok(parsed) => parsed,
        Err(error) => return emit_error(error),
    };
    let (command, settings, explicit_daemon) = match parsed {
        Parsed::Root {
            command,
            settings,
            daemon,
        } => (command, settings, daemon),
        Parsed::Help => {
            print_help();
            return 0;
        }
        Parsed::Version => {
            println!(
                "{{\"binary\":\"eliot-search\",\"version\":\"{}\"}}",
                env!("CARGO_PKG_VERSION")
            );
            return 0;
        }
    };

    let Some(deadline) = started.checked_add(COMMAND_TIMEOUT) else {
        return emit_error("DEADLINE_UNAVAILABLE");
    };
    let daemon = match daemon_path(explicit_daemon) {
        Ok(path) => path,
        Err(error) => return emit_error(error),
    };
    let (status, stdout, stderr) = match capture_child(&daemon, command, &settings, deadline) {
        Ok(output) => output,
        Err(error) => {
            let status = if error == "DAEMON_TIMEOUT" { 124 } else { 2 };
            return emit_error_status(error, status);
        }
    };
    if !status.success() {
        return emit_child_exit(status);
    }
    let stdout = match stdout {
        Ok(stdout) => stdout,
        Err(error) => return emit_error(error),
    };
    let stderr = match stderr {
        Ok(stderr) => stderr,
        Err(error) => return emit_error(error),
    };
    if stdout.exceeded || stderr.exceeded {
        return emit_error("DAEMON_OUTPUT_TOO_LARGE");
    }
    if !stderr.bytes.is_empty() {
        return emit_error("DAEMON_RESULT_UNKNOWN");
    }
    let Some(snapshot) = validated_snapshot(&stdout.bytes, command) else {
        return emit_error("DAEMON_RESULT_UNKNOWN");
    };
    let mut output = std::io::stdout().lock();
    if output
        .write_all(snapshot.as_bytes())
        .and_then(|()| output.write_all(b"\n"))
        .and_then(|()| output.flush())
        .is_err()
    {
        return emit_error("OUTPUT_WRITE_ERROR");
    }
    0
}

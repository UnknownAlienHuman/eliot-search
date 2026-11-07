//! Root operations over one bounded, owned daemon child.

use search_source_registry::{MAX_PREPARATION_RESPONSE_BYTES, MAX_PUBLICATION_RESPONSE_BYTES};
use std::env;
use std::ffi::{OsStr, OsString};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const MAX_ARGUMENTS: usize = 256;
const MAX_ARGUMENT_BYTES: usize = 256 * 1024;
const MAX_PATH_BYTES: usize = 32 * 1024;
const MAX_CAPTURE_BYTES: usize = 64 * 1024;
const MAX_ERROR_CODE_BYTES: usize = 128;
const MAX_SOURCE_RESULT_BYTES: u64 = 8 * 1024 * 1024;
const COMMAND_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Clone, Copy)]
enum RootCommand {
    Inspect,
    Initialize,
    CheckExistingOwner,
    DiscardIncomplete,
    RetainFile,
    PrepareSource,
    PublishPrepared,
    Index,
    Search,
}

impl RootCommand {
    const fn name(self) -> &'static str {
        match self {
            Self::Inspect => "inspect-data-root",
            Self::Initialize => "initialize-data-root",
            Self::CheckExistingOwner => "check-existing-root-owner",
            Self::DiscardIncomplete => "discard-incomplete-data-root",
            Self::RetainFile => "retain-file",
            Self::PrepareSource => "prepare-source",
            Self::PublishPrepared => "publish-prepared",
            Self::Index => "index",
            Self::Search => "search",
        }
    }

    const fn daemon_flag(self) -> &'static str {
        match self {
            Self::Inspect => "--inspect-data-root",
            Self::Initialize => "--initialize-data-root",
            Self::CheckExistingOwner => "--check-existing-root-owner",
            Self::DiscardIncomplete => "--discard-incomplete-data-root",
            Self::RetainFile => "--retain-file",
            Self::PrepareSource => "--prepare-source",
            Self::PublishPrepared => "--publish-prepared",
            Self::Index => "--index",
            Self::Search => "--search-stdio",
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
        preparation: PreparationRequest,
        publication: PublicationRequest,
    },
}

#[derive(Default)]
struct PreparationRequest {
    source_id: Option<String>,
    unit: Option<u64>,
}

#[derive(Default)]
struct PublicationRequest {
    endpoint: Option<String>,
    query: Option<String>,
    limit: Option<usize>,
}

struct Captured {
    bytes: Vec<u8>,
    exceeded: bool,
}

struct OutputReader {
    completion: Receiver<Result<Captured, ChildFailure>>,
    // A blocked pipe read cannot be interrupted portably. Unfinished readers
    // are dropped only on return to entry.rs's unconditional process exit.
    _thread: JoinHandle<()>,
}

struct ChildOutput {
    status: ExitStatus,
    stdout: Captured,
    stderr: Captured,
}

enum ChildFailure {
    Start,
    StdoutUnavailable,
    StderrUnavailable,
    OutputSetup,
    Wait,
    Timeout,
    ResultUnknown,
}

impl ChildFailure {
    const fn code(&self) -> &'static str {
        match self {
            Self::Start => "DAEMON_START_ERROR",
            Self::StdoutUnavailable => "DAEMON_STDOUT_UNAVAILABLE",
            Self::StderrUnavailable => "DAEMON_STDERR_UNAVAILABLE",
            Self::OutputSetup => "DAEMON_OUTPUT_SETUP_ERROR",
            Self::Wait => "DAEMON_WAIT_ERROR",
            Self::Timeout => "DAEMON_TIMEOUT",
            Self::ResultUnknown => "DAEMON_RESULT_UNKNOWN",
        }
    }

    const fn exit_code(&self) -> i32 {
        match self {
            Self::Timeout => 124,
            _ => 2,
        }
    }
}

enum DaemonResponse<'a> {
    Snapshot(&'a str),
    Retained(&'a str),
    Prepared(&'a str),
    Published(&'a str),
    Indexed(&'a str),
    Error(&'a str),
}

fn emit_error(code: &str) -> i32 {
    emit_error_status(code, 2)
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
        "retain-file" => Some(RootCommand::RetainFile),
        "prepare-source" => Some(RootCommand::PrepareSource),
        "publish-prepared" => Some(RootCommand::PublishPrepared),
        "index" => Some(RootCommand::Index),
        "search" => Some(RootCommand::Search),
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
                | "--retain-file"
                | "--prepare-source"
                | "--publish-prepared"
                | "--index"
                | "--search-stdio"
        )
    )
}

fn validate_path(value: &OsStr) -> Result<(), &'static str> {
    if value.is_empty() || value.len() > MAX_PATH_BYTES || value.as_encoded_bytes().contains(&0) {
        return Err("USAGE_ERROR");
    }
    Ok(())
}

fn local_invocation(arguments: &[OsString]) -> Option<Parsed> {
    if arguments.is_empty() {
        return Some(Parsed::Help);
    }
    if arguments.len() == 1 && (is_help_flag(&arguments[0]) || is_help_command(&arguments[0])) {
        return Some(Parsed::Help);
    }
    if arguments.len() == 1 && (is_version_flag(&arguments[0]) || is_version_command(&arguments[0]))
    {
        return Some(Parsed::Version);
    }
    None
}
fn path_option(arguments: &[OsString], index: usize) -> Result<&OsString, &'static str> {
    let value = arguments.get(index + 1).ok_or("USAGE_ERROR")?;
    validate_path(value)?;
    if is_help_flag(value) || is_version_flag(value) || is_daemon_root_flag(value) {
        return Err("USAGE_ERROR");
    }
    Ok(value)
}
fn source_id_option(
    arguments: &[OsString],
    index: usize,
    command: RootCommand,
) -> Result<(&OsString, &str), &'static str> {
    if !matches!(command, RootCommand::PrepareSource) {
        return Err("USAGE_ERROR");
    }
    let value = arguments.get(index + 1).ok_or("USAGE_ERROR")?;
    let source_id = value.to_str().ok_or("USAGE_ERROR")?;
    if !uuid_text(source_id) {
        return Err("USAGE_ERROR");
    }
    Ok((value, source_id))
}
fn unit_option(
    arguments: &[OsString],
    index: usize,
    command: RootCommand,
) -> Result<(&OsString, u64), &'static str> {
    if !matches!(command, RootCommand::PrepareSource) {
        return Err("USAGE_ERROR");
    }
    let value = arguments.get(index + 1).ok_or("USAGE_ERROR")?;
    let unit = value
        .to_str()
        .and_then(|s| canonical_decimal(s, true))
        .ok_or("USAGE_ERROR")?;
    Ok((value, unit))
}
const fn validate_request_selection(
    command: RootCommand,
    file: bool,
    index_path: bool,
    preparation: &PreparationRequest,
    publication: &PublicationRequest,
) -> Result<(), &'static str> {
    let id = preparation.source_id.is_some();
    let valid_source = match command {
        RootCommand::RetainFile => file && !id,
        RootCommand::PrepareSource => id && !file,
        _ => !file && !id,
    };
    if !valid_source {
        return Err("USAGE_ERROR");
    }
    if matches!(
        command,
        RootCommand::PublishPrepared | RootCommand::Index | RootCommand::Search
    ) != publication.endpoint.is_some()
        || (matches!(command, RootCommand::Index) != index_path)
        || (publication.limit.is_some() && publication.query.is_none())
        || (matches!(command, RootCommand::Search) && publication.query.is_none())
    {
        return Err("USAGE_ERROR");
    }
    Ok(())
}

impl PreparationRequest {
    fn take_option(
        &mut self,
        command: RootCommand,
        arguments: &[OsString],
        index: usize,
        settings: &mut Vec<OsString>,
    ) -> Result<bool, &'static str> {
        let argument = &arguments[index];
        let value = if argument == OsStr::new("--source-id") {
            if self.source_id.is_some() {
                return Err("USAGE_ERROR");
            }
            let (value, id) = source_id_option(arguments, index, command)?;
            self.source_id = Some(id.to_owned());
            value
        } else if argument == OsStr::new("--unit") {
            if self.unit.is_some() {
                return Err("USAGE_ERROR");
            }
            let (value, unit) = unit_option(arguments, index, command)?;
            self.unit = Some(unit);
            value
        } else {
            return Ok(false);
        };
        settings.push(argument.clone());
        settings.push(value.clone());
        Ok(true)
    }
}

impl PublicationRequest {
    fn take_option(
        &mut self,
        command: RootCommand,
        arguments: &[OsString],
        index: usize,
        settings: &mut Vec<OsString>,
    ) -> Result<bool, &'static str> {
        let argument = &arguments[index];
        if argument != OsStr::new("--qdrant-url")
            && argument != OsStr::new("--query")
            && argument != OsStr::new("--limit")
        {
            return Ok(false);
        }
        if !matches!(
            command,
            RootCommand::PublishPrepared | RootCommand::Index | RootCommand::Search
        ) {
            return Err("USAGE_ERROR");
        }
        if !matches!(command, RootCommand::Search) && argument != OsStr::new("--qdrant-url") {
            return Err("USAGE_ERROR");
        }
        let value = arguments
            .get(index + 1)
            .and_then(|v| v.to_str())
            .ok_or("USAGE_ERROR")?;
        if argument == OsStr::new("--limit") {
            if self.limit.is_some() {
                return Err("USAGE_ERROR");
            }
            self.limit = Some(
                canonical_decimal(value, false)
                    .and_then(|n| usize::try_from(n).ok())
                    .filter(|n| *n > 0 && *n <= 64)
                    .ok_or("USAGE_ERROR")?,
            );
        } else {
            if value.is_empty() || value.len() > 4096 || value.as_bytes().contains(&0) {
                return Err("USAGE_ERROR");
            }
            let slot = if argument == OsStr::new("--query") {
                &mut self.query
            } else {
                &mut self.endpoint
            };
            if slot.replace(value.to_owned()).is_some() {
                return Err("USAGE_ERROR");
            }
        }
        if !matches!(command, RootCommand::Search) || argument == OsStr::new("--qdrant-url") {
            settings.push(argument.clone());
            settings.push(OsString::from(value));
        }
        Ok(true)
    }
}

fn parse(arguments: &[OsString]) -> Result<Parsed, &'static str> {
    if let Some(local) = local_invocation(arguments) {
        return Ok(local);
    }
    let Some(root_command) = command(&arguments[0]) else {
        return Err("UNKNOWN_COMMAND");
    };

    let mut settings = Vec::new();
    let mut daemon = None;
    let mut data_root_seen = false;
    let mut positional_root_seen = false;
    let mut config_seen = false;
    let mut source_file_seen = false;
    let mut index_path_seen = false;
    let mut preparation = PreparationRequest::default();
    let mut publication = PublicationRequest::default();
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
            let value = path_option(arguments, index)?;
            daemon = Some(PathBuf::from(value));
            index += 2;
        } else if argument == OsStr::new("--data-root") {
            if data_root_seen || positional_root_seen {
                return Err("USAGE_ERROR");
            }
            let value = path_option(arguments, index)?;
            data_root_seen = true;
            settings.push(argument.clone());
            settings.push(value.clone());
            index += 2;
        } else if argument == OsStr::new("--config-file") {
            if config_seen {
                return Err("USAGE_ERROR");
            }
            let value = path_option(arguments, index)?;
            config_seen = true;
            settings.push(argument.clone());
            settings.push(value.clone());
            index += 2;
        } else if argument == OsStr::new("--file") {
            if source_file_seen || !matches!(root_command, RootCommand::RetainFile) {
                return Err("USAGE_ERROR");
            }
            let value = arguments.get(index + 1).ok_or("USAGE_ERROR")?;
            validate_path(value)?;
            settings.push(OsString::from("--source-file"));
            settings.push(value.clone());
            source_file_seen = true;
            index += 2;
        } else if preparation.take_option(root_command, arguments, index, &mut settings)?
            || publication.take_option(root_command, arguments, index, &mut settings)?
        {
            index += 2;
        } else if (index_path_seen || !matches!(root_command, RootCommand::Index))
            && argument
                .to_str()
                .is_some_and(|value| value.starts_with("--"))
        {
            return Err("UNKNOWN_ARGUMENT");
        } else {
            validate_path(argument)?;
            if matches!(root_command, RootCommand::Index) {
                if index_path_seen {
                    return Err("USAGE_ERROR");
                }
                index_path_seen = true;
                settings.push(OsString::from("--index-path"));
            } else {
                if positional_root_seen || data_root_seen {
                    return Err("USAGE_ERROR");
                }
                positional_root_seen = true;
            }
            settings.push(argument.clone());
            index += 1;
        }
    }
    validate_request_selection(
        root_command,
        source_file_seen,
        index_path_seen,
        &preparation,
        &publication,
    )?;
    Ok(Parsed::Root {
        command: root_command,
        settings,
        daemon,
        preparation,
        publication,
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
    Ok(sibling
        .filter(|path| path.exists())
        .unwrap_or_else(|| PathBuf::from(executable_name)))
}

fn capture_stream(
    mut reader: impl Read,
    deadline: Instant,
    maximum: usize,
) -> Result<Captured, ChildFailure> {
    let mut bytes = Vec::with_capacity(maximum);
    let mut buffer = [0_u8; 8 * 1024];
    let mut exceeded = false;
    loop {
        if Instant::now() >= deadline {
            return Err(ChildFailure::Timeout);
        }
        let count = reader
            .read(&mut buffer)
            .map_err(|_| ChildFailure::ResultUnknown)?;
        if Instant::now() >= deadline {
            return Err(ChildFailure::Timeout);
        }
        if count == 0 {
            break;
        }
        let remaining = maximum.saturating_sub(bytes.len());
        let keep = remaining.min(count);
        bytes.extend_from_slice(&buffer[..keep]);
        exceeded |= keep < count;
    }
    Ok(Captured { bytes, exceeded })
}

fn spawn_reader(
    reader: impl Read + Send + 'static,
    name: &'static str,
    deadline: Instant,
    maximum: usize,
) -> Result<OutputReader, ChildFailure> {
    let (completed, completion) = mpsc::sync_channel(1);
    let thread = thread::Builder::new()
        .name(name.to_owned())
        .spawn(move || {
            let result = capture_stream(reader, deadline, maximum);
            let _ = completed.send(result);
        })
        .map_err(|_| ChildFailure::OutputSetup)?;
    Ok(OutputReader {
        completion,
        _thread: thread,
    })
}

fn complete_reader(reader: OutputReader, deadline: Instant) -> Result<Captured, ChildFailure> {
    let OutputReader {
        completion,
        _thread,
    } = reader;
    let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
        return Err(ChildFailure::Timeout);
    };
    let result = completion
        .recv_timeout(remaining)
        .map_err(|error| match error {
            RecvTimeoutError::Timeout => ChildFailure::Timeout,
            RecvTimeoutError::Disconnected => ChildFailure::ResultUnknown,
        })?;
    if Instant::now() >= deadline {
        return Err(ChildFailure::Timeout);
    }
    result
}

fn stop_and_reap(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

fn wait_for_child(child: &mut Child, deadline: Instant) -> Result<ExitStatus, ChildFailure> {
    loop {
        if Instant::now() >= deadline {
            let _ = child.kill();
            return match child.wait() {
                Ok(_) => Err(ChildFailure::Timeout),
                Err(_) => Err(ChildFailure::Wait),
            };
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                return if Instant::now() < deadline {
                    Ok(status)
                } else {
                    Err(ChildFailure::Timeout)
                };
            }
            Ok(None) => {}
            Err(_) => {
                stop_and_reap(child);
                return Err(ChildFailure::Wait);
            }
        }
        thread::sleep(
            deadline
                .saturating_duration_since(Instant::now())
                .min(Duration::from_millis(10)),
        );
    }
}

fn capture_child(
    daemon: &Path,
    command: RootCommand,
    settings: &[OsString],
    deadline: Instant,
) -> Result<ChildOutput, ChildFailure> {
    let mut process = Command::new(daemon);
    process
        .arg(command.daemon_flag())
        .args(settings)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if Instant::now() >= deadline {
        return Err(ChildFailure::Timeout);
    }
    let mut child = process.spawn().map_err(|_| ChildFailure::Start)?;
    if Instant::now() >= deadline {
        stop_and_reap(&mut child);
        return Err(ChildFailure::Timeout);
    }

    let Some(stdout) = child.stdout.take() else {
        stop_and_reap(&mut child);
        return Err(ChildFailure::StdoutUnavailable);
    };
    let stdout_limit = if matches!(command, RootCommand::PrepareSource) {
        MAX_PREPARATION_RESPONSE_BYTES
    } else if matches!(command, RootCommand::PublishPrepared) {
        MAX_PUBLICATION_RESPONSE_BYTES
    } else {
        MAX_CAPTURE_BYTES
    };
    let Ok(stdout_reader) = spawn_reader(stdout, "eliot-searchd-stdout", deadline, stdout_limit)
    else {
        stop_and_reap(&mut child);
        return Err(ChildFailure::OutputSetup);
    };

    let Some(stderr) = child.stderr.take() else {
        stop_and_reap(&mut child);
        return Err(ChildFailure::StderrUnavailable);
    };
    let Ok(stderr_reader) =
        spawn_reader(stderr, "eliot-searchd-stderr", deadline, MAX_CAPTURE_BYTES)
    else {
        stop_and_reap(&mut child);
        return Err(ChildFailure::OutputSetup);
    };

    let status = wait_for_child(&mut child, deadline)?;
    let stdout = complete_reader(stdout_reader, deadline);
    let stderr = complete_reader(stderr_reader, deadline);
    let stdout = stdout?;
    let stderr = stderr?;
    if stdout.exceeded || stderr.exceeded {
        return Err(ChildFailure::ResultUnknown);
    }
    Ok(ChildOutput {
        status,
        stdout,
        stderr,
    })
}

fn validated_line(bytes: &[u8]) -> Option<&str> {
    let text = std::str::from_utf8(bytes).ok()?;
    let line = text
        .strip_suffix("\r\n")
        .or_else(|| text.strip_suffix('\n'))
        .unwrap_or(text);
    if line.is_empty() || line.contains('\r') || line.contains('\n') {
        return None;
    }
    Some(line)
}

fn validated_snapshot(bytes: &[u8], command: RootCommand) -> Option<&str> {
    let line = validated_line(bytes)?;
    let (operation, state) = line
        .strip_prefix("{\"operation\":\"")?
        .split_once("\",\"root_state\":\"")?;
    let state = state.strip_suffix("\"}")?;
    if operation != command.name() {
        return None;
    }
    let states: &[&str] = match command {
        RootCommand::Initialize | RootCommand::CheckExistingOwner => &["current"],
        RootCommand::RetainFile
        | RootCommand::PrepareSource
        | RootCommand::PublishPrepared
        | RootCommand::Index
        | RootCommand::Search => {
            return None;
        }
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
    states.contains(&state).then_some(line)
}

fn lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn uuid_text(value: &str) -> bool {
    if value.len() != 36 {
        return false;
    }
    value.bytes().enumerate().all(|(index, byte)| match index {
        8 | 13 | 18 | 23 => byte == b'-',
        _ => byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte),
    })
}

fn canonical_decimal(value: &str, allow_zero: bool) -> Option<u64> {
    let parsed = value.parse::<u64>().ok()?;
    if (!allow_zero && parsed == 0) || parsed.to_string() != value {
        None
    } else {
        Some(parsed)
    }
}

fn validated_retained(bytes: &[u8], command: RootCommand) -> Option<&str> {
    if !matches!(command, RootCommand::RetainFile) {
        return None;
    }
    let line = validated_line(bytes)?;
    let rest = line.strip_prefix("{\"operation\":\"retain-file\",\"source_namespace_id\":\"")?;
    let (namespace_id, rest) = rest.split_once("\",\"source_id\":\"")?;
    if !uuid_text(namespace_id) {
        return None;
    }
    let (source_id, rest) = rest.split_once("\",\"source_row_revision\":")?;
    if !uuid_text(source_id) {
        return None;
    }
    let (row_revision, rest) = rest.split_once(",\"registration_revision\":")?;
    canonical_decimal(row_revision, false)?;
    let (registration_revision, rest) = rest.split_once(",\"object_version\":")?;
    canonical_decimal(registration_revision, false)?;
    let (object_version, rest) =
        rest.split_once(",\"object_kind\":\"source\",\"object_digest\":\"")?;
    if object_version != "1" {
        return None;
    }
    let (digest, rest) = rest.split_once("\",\"object_bytes\":")?;
    if !lower_hex(digest, 64) {
        return None;
    }
    let (object_bytes, rest) = rest.split_once(",\"native_file_key\":\"")?;
    let object_bytes = canonical_decimal(object_bytes, true)?;
    if object_bytes > MAX_SOURCE_RESULT_BYTES {
        return None;
    }
    let (native_file_key, rest) = rest.split_once("\",\"readback\":\"verified\"}")?;
    if !lower_hex(native_file_key, 34)
        || !matches!(&native_file_key[..2], "01" | "02")
        || !rest.is_empty()
    {
        return None;
    }
    Some(line)
}

fn validated_daemon_error(bytes: &[u8]) -> Option<&str> {
    let line = validated_line(bytes)?;
    let code = line.strip_prefix("{\"error\":\"")?.strip_suffix("\"}")?;
    if code.len() > MAX_ERROR_CODE_BYTES
        || !code.as_bytes().first()?.is_ascii_alphabetic()
        || !code
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b':'))
    {
        return None;
    }
    Some(line)
}

fn validated_response<'a>(
    output: &'a ChildOutput,
    command: RootCommand,
    preparation: &PreparationRequest,
) -> Option<DaemonResponse<'a>> {
    if output.stderr.bytes.is_empty() {
        if matches!(command, RootCommand::Index) {
            validated_line(&output.stdout.bytes)
                .filter(|line| *line == "{\"operation\":\"index\",\"state\":\"refreshed\"}")
                .map(DaemonResponse::Indexed)
        } else if matches!(command, RootCommand::PublishPrepared) {
            crate::published_response::validate(&output.stdout.bytes).map(DaemonResponse::Published)
        } else if matches!(command, RootCommand::PrepareSource) {
            crate::prepared_response::validate(
                &output.stdout.bytes,
                preparation.source_id.as_deref()?,
                preparation.unit.unwrap_or(0),
            )
            .map(DaemonResponse::Prepared)
        } else if matches!(command, RootCommand::RetainFile) {
            validated_retained(&output.stdout.bytes, command).map(DaemonResponse::Retained)
        } else {
            validated_snapshot(&output.stdout.bytes, command).map(DaemonResponse::Snapshot)
        }
    } else if output.stdout.bytes.is_empty() {
        validated_daemon_error(&output.stderr.bytes).map(DaemonResponse::Error)
    } else {
        None
    }
}

fn write_line(output: &mut impl Write, line: &str) -> std::io::Result<()> {
    output
        .write_all(line.as_bytes())
        .and_then(|()| output.write_all(b"\n"))
        .and_then(|()| output.flush())
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
        "  eliot-search retain-file --file PATH [ROOT] [--data-root ROOT] [--config-file PATH] [--daemon PATH]\n",
        "  eliot-search prepare-source --source-id UUID [--unit N] [--data-root ROOT] [--config-file PATH] [--daemon PATH]\n",
        "  eliot-search publish-prepared --qdrant-url URL [--data-root ROOT] [--config-file PATH] [--daemon PATH]\n",
        "  eliot-search index PATH --qdrant-url URL [--data-root ROOT] [--config-file PATH] [--daemon PATH]\n",
        "  eliot-search search --query TEXT [--limit N] --qdrant-url URL [--data-root ROOT] [--config-file PATH] [--daemon PATH]\n",
    ));
}

pub fn run_main() -> i32 {
    let started = Instant::now();
    let arguments = match capture_arguments() {
        Ok(arguments) => arguments,
        Err(error) => return emit_error(error),
    };
    let parsed = match parse(&arguments) {
        Ok(parsed) => parsed,
        Err(error) => return emit_error(error),
    };
    let (command, settings, explicit_daemon, preparation, publication) = match parsed {
        Parsed::Root {
            command,
            settings,
            daemon,
            preparation,
            publication,
        } => (command, settings, daemon, preparation, publication),
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
    if matches!(command, RootCommand::Search) {
        let Some(query) = publication.query else {
            return emit_error("USAGE_ERROR");
        };
        let Ok(limit) = u8::try_from(publication.limit.unwrap_or(10)) else {
            return emit_error("USAGE_ERROR");
        };
        let mut arguments = vec![OsString::from("--search-stdio")];
        arguments.extend(settings);
        return crate::search_command::run(
            &daemon,
            &arguments,
            search_provider_protocol::SearchRequest { query, limit },
            deadline,
        );
    }
    let output = match capture_child(&daemon, command, &settings, deadline) {
        Ok(output) => output,
        Err(failure) => {
            return emit_error_status(failure.code(), failure.exit_code());
        }
    };
    let Some(response) = validated_response(&output, command, &preparation) else {
        return emit_error("DAEMON_RESULT_UNKNOWN");
    };
    let (written, exit_code) = match (output.status.success(), response) {
        (
            true,
            DaemonResponse::Snapshot(result)
            | DaemonResponse::Retained(result)
            | DaemonResponse::Prepared(result)
            | DaemonResponse::Published(result)
            | DaemonResponse::Indexed(result),
        ) => (write_line(&mut std::io::stdout().lock(), result), 0),
        (false, DaemonResponse::Error(error)) => (
            write_line(&mut std::io::stderr().lock(), error),
            output.status.code().unwrap_or(1),
        ),
        _ => return emit_error("DAEMON_RESULT_UNKNOWN"),
    };
    if written.is_err() {
        return emit_error("OUTPUT_WRITE_ERROR");
    }
    exit_code
}

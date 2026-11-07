//! Root operations over one bounded, owned daemon child.

mod watch;

use search_provider_protocol::{DecodedUtf8Request, RawExactRequest};
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
    CollectObjects,
    ArchiveExport,
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
            Self::CollectObjects => "collect-objects",
            Self::ArchiveExport => "archive-export",
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
            Self::CollectObjects => "--collect-objects",
            Self::ArchiveExport => "--archive-export",
        }
    }
}

enum Parsed {
    Help,
    Version,
    Watch(watch::Request),
    Session {
        settings: Vec<OsString>,
        daemon: Option<PathBuf>,
    },
    RawExact {
        settings: Vec<OsString>,
        daemon: Option<PathBuf>,
        request: RawExactRequest,
    },
    DecodedUtf8 {
        settings: Vec<OsString>,
        daemon: Option<PathBuf>,
        request: DecodedUtf8Request,
    },
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

#[derive(Default)]
struct RootPaths {
    daemon: Option<PathBuf>,
    data_root: Option<PathBuf>,
    config: Option<PathBuf>,
    archive: Option<PathBuf>,
}

impl RootPaths {
    fn take_option(
        &mut self,
        command: RootCommand,
        arguments: &[OsString],
        index: usize,
        positional_root: bool,
        settings: &mut Vec<OsString>,
    ) -> Result<bool, &'static str> {
        let argument = &arguments[index];
        let slot = match argument.to_str() {
            Some("--daemon") => &mut self.daemon,
            Some("--data-root") => {
                if positional_root {
                    return Err("USAGE_ERROR");
                }
                &mut self.data_root
            }
            Some("--config-file") => &mut self.config,
            Some("--archive-path") => {
                if !matches!(command, RootCommand::ArchiveExport) {
                    return Err("USAGE_ERROR");
                }
                &mut self.archive
            }
            _ => return Ok(false),
        };
        if slot.is_some() {
            return Err("USAGE_ERROR");
        }
        let value = path_option(arguments, index)?;
        *slot = Some(PathBuf::from(value));
        if argument != OsStr::new("--daemon") {
            settings.push(argument.clone());
            settings.push(value.clone());
        }
        Ok(true)
    }
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

#[derive(Clone, Copy)]
enum ChildFailure {
    Start,
    StdoutUnavailable,
    StderrUnavailable,
    OutputSetup,
    Wait,
    Timeout,
    ResultUnknown,
    Cancelled,
    Signal,
    Cleanup,
    DaemonPath,
}

impl ChildFailure {
    const fn code(self) -> &'static str {
        match self {
            Self::Start => "DAEMON_START_ERROR",
            Self::StdoutUnavailable => "DAEMON_STDOUT_UNAVAILABLE",
            Self::StderrUnavailable => "DAEMON_STDERR_UNAVAILABLE",
            Self::OutputSetup => "DAEMON_OUTPUT_SETUP_ERROR",
            Self::Wait => "DAEMON_WAIT_ERROR",
            Self::Timeout => "DAEMON_TIMEOUT",
            Self::ResultUnknown => "DAEMON_RESULT_UNKNOWN",
            Self::Cancelled => "DAEMON_CANCELLED",
            Self::Signal => "SIGNAL_UNAVAILABLE",
            Self::Cleanup => "DAEMON_CLEANUP_UNRESOLVED",
            Self::DaemonPath => "DAEMON_PATH_INVALID",
        }
    }

    const fn exit_code(self) -> i32 {
        match self {
            Self::Timeout => 124,
            Self::Cancelled => 130,
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
    Collected(&'a str),
    Archived(&'a str),
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
        "collect-objects" => Some(RootCommand::CollectObjects),
        "archive-export" => Some(RootCommand::ArchiveExport),
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
                | "--session-stdio"
                | "--collect-objects"
                | "--archive-export"
        )
    )
}

fn validate_path(value: &OsStr) -> Result<(), &'static str> {
    if value.is_empty() || value.len() > MAX_PATH_BYTES || value.as_encoded_bytes().contains(&0) {
        return Err("USAGE_ERROR");
    }
    Ok(())
}

fn standalone_invocation(arguments: &[OsString]) -> Option<Result<Parsed, &'static str>> {
    if arguments.is_empty() {
        return Some(Ok(Parsed::Help));
    }
    if arguments[0] == OsStr::new("watch") {
        return Some(watch::parse(arguments));
    }
    if arguments[0] == OsStr::new("exact") {
        return Some(parse_exact(arguments));
    }
    if arguments[0] == OsStr::new("session") {
        return Some(parse_session(arguments));
    }
    if arguments.len() == 1 && (is_help_flag(&arguments[0]) || is_help_command(&arguments[0])) {
        return Some(Ok(Parsed::Help));
    }
    if arguments.len() == 1 && (is_version_flag(&arguments[0]) || is_version_command(&arguments[0]))
    {
        return Some(Ok(Parsed::Version));
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
        || (index_path && !matches!(command, RootCommand::Index))
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
    if let Some(local) = standalone_invocation(arguments) {
        return local;
    }
    let Some(root_command) = command(&arguments[0]) else {
        return Err("UNKNOWN_COMMAND");
    };

    let mut settings = Vec::new();
    let mut paths = RootPaths::default();
    let mut positional_root_seen = false;
    let mut source_file_seen = false;
    let mut index_path_seen = false;
    let mut preparation = PreparationRequest::default();
    let mut publication = PublicationRequest::default();
    let mut index = 1;
    while index < arguments.len() {
        let argument = &arguments[index];
        if is_help_flag(argument) || is_version_flag(argument) || is_daemon_root_flag(argument) {
            return Err("USAGE_ERROR");
        }
        if paths.take_option(
            root_command,
            arguments,
            index,
            positional_root_seen,
            &mut settings,
        )? {
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
                if positional_root_seen || paths.data_root.is_some() {
                    return Err("USAGE_ERROR");
                }
                positional_root_seen = true;
            }
            settings.push(argument.clone());
            index += 1;
        }
    }
    if matches!(root_command, RootCommand::ArchiveExport) != paths.archive.is_some() {
        return Err("USAGE_ERROR");
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
        daemon: paths.daemon,
        preparation,
        publication,
    })
}

fn parse_exact(arguments: &[OsString]) -> Result<Parsed, &'static str> {
    let mut settings = Vec::new();
    let mut daemon = None;
    let mut data_root_seen = false;
    let mut config_seen = false;
    let mut decoded_utf8 = None;
    let mut literal = None;
    let mut source_id = None;
    let mut limit = None;
    let mut index = 1;
    while index < arguments.len() {
        let argument = &arguments[index];
        match argument.to_str() {
            Some("--raw" | "--decoded-utf8") => {
                if decoded_utf8
                    .replace(argument == OsStr::new("--decoded-utf8"))
                    .is_some()
                {
                    return Err("USAGE_ERROR");
                }
                index += 1;
                continue;
            }
            Some("--literal" | "--source-id") => {
                let value = arguments
                    .get(index + 1)
                    .and_then(|v| v.to_str())
                    .ok_or("USAGE_ERROR")?;
                let slot = if argument == OsStr::new("--literal") {
                    &mut literal
                } else {
                    &mut source_id
                };
                if slot.replace(value.to_owned()).is_some() {
                    return Err("USAGE_ERROR");
                }
            }
            Some("--limit") => {
                if limit.is_some() {
                    return Err("USAGE_ERROR");
                }
                limit = Some(
                    arguments
                        .get(index + 1)
                        .and_then(|v| v.to_str())
                        .and_then(|v| canonical_decimal(v, false))
                        .and_then(|n| u8::try_from(n).ok())
                        .filter(|n| usize::from(*n) <= search_provider_protocol::MAX_ITEMS)
                        .ok_or("USAGE_ERROR")?,
                );
            }
            Some("--data-root" | "--config-file") => {
                let seen = if argument == OsStr::new("--data-root") {
                    &mut data_root_seen
                } else {
                    &mut config_seen
                };
                if *seen {
                    return Err("USAGE_ERROR");
                }
                let value = path_option(arguments, index)?;
                *seen = true;
                settings.push(argument.clone());
                settings.push(value.clone());
            }
            Some("--daemon") => {
                if daemon.is_some() {
                    return Err("USAGE_ERROR");
                }
                daemon = Some(PathBuf::from(path_option(arguments, index)?));
            }
            _ => return Err("UNKNOWN_ARGUMENT"),
        }
        index += 2;
    }
    if !data_root_seen {
        return Err("USAGE_ERROR");
    }
    build_exact_request(
        decoded_utf8.ok_or("USAGE_ERROR")?,
        literal.ok_or("USAGE_ERROR")?,
        source_id.map(|id| vec![id]),
        limit.unwrap_or(10),
        settings,
        daemon,
    )
}

fn build_exact_request(
    decoded_utf8: bool,
    literal: String,
    source_ids: Option<Vec<String>>,
    limit: u8,
    settings: Vec<OsString>,
    daemon: Option<PathBuf>,
) -> Result<Parsed, &'static str> {
    if decoded_utf8 {
        let request = DecodedUtf8Request {
            literal,
            source_ids,
            limit,
        };
        request.validate().map_err(|_| "USAGE_ERROR")?;
        Ok(Parsed::DecodedUtf8 {
            settings,
            daemon,
            request,
        })
    } else {
        let request = RawExactRequest {
            literal,
            source_ids,
            limit,
        };
        request.validate().map_err(|_| "USAGE_ERROR")?;
        Ok(Parsed::RawExact {
            settings,
            daemon,
            request,
        })
    }
}

fn parse_session(arguments: &[OsString]) -> Result<Parsed, &'static str> {
    let mut settings = Vec::new();
    let mut daemon = None;
    let mut root_seen = false;
    let mut config_seen = false;
    let mut qdrant_seen = false;
    let mut index = 1;
    while index < arguments.len() {
        let argument = &arguments[index];
        match argument.to_str() {
            Some("--data-root" | "--config-file") => {
                let seen = if argument == OsStr::new("--data-root") {
                    &mut root_seen
                } else {
                    &mut config_seen
                };
                if *seen {
                    return Err("USAGE_ERROR");
                }
                let value = path_option(arguments, index)?;
                *seen = true;
                settings.push(argument.clone());
                settings.push(value.clone());
            }
            Some("--daemon") => {
                if daemon.is_some() {
                    return Err("USAGE_ERROR");
                }
                daemon = Some(PathBuf::from(path_option(arguments, index)?));
            }
            Some("--qdrant-url") => {
                if qdrant_seen {
                    return Err("USAGE_ERROR");
                }
                let value = arguments
                    .get(index + 1)
                    .and_then(|v| v.to_str())
                    .ok_or("USAGE_ERROR")?;
                if value.is_empty() || value.len() > 4096 || value.as_bytes().contains(&0) {
                    return Err("USAGE_ERROR");
                }
                qdrant_seen = true;
                settings.push(argument.clone());
                settings.push(OsString::from(value));
            }
            _ => return Err("UNKNOWN_ARGUMENT"),
        }
        index += 2;
    }
    if !root_seen {
        return Err("USAGE_ERROR");
    }
    Ok(Parsed::Session { settings, daemon })
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
        | RootCommand::Search
        | RootCommand::CollectObjects
        | RootCommand::ArchiveExport => {
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

fn validated_collected(bytes: &[u8]) -> Option<&str> {
    let line = validated_line(bytes)?;
    let value: serde_json::Value = serde_json::from_str(line).ok()?;
    if value.as_object()?.len() != 6
        || value["operation"] != "collect-objects"
        || value["state"] != "collected"
        || value["marked"].as_u64()? != value["verified"].as_u64()?
    {
        return None;
    }
    value["deleted"].as_u64()?;
    value["preserved"].as_u64()?;
    Some(line)
}

fn validated_archived(bytes: &[u8]) -> Option<&str> {
    let line = validated_line(bytes)?;
    let value: serde_json::Value = serde_json::from_str(line).ok()?;
    if value.as_object()?.len() != 5
        || value["operation"] != "archive-export"
        || value["state"] != "complete"
        || value["generation"].as_u64()? == 0
        || value["control_bytes"].as_u64()? == 0
    {
        return None;
    }
    value["objects"].as_u64()?;
    Some(line)
}

fn validated_response<'a>(
    output: &'a ChildOutput,
    command: RootCommand,
    preparation: &PreparationRequest,
) -> Option<DaemonResponse<'a>> {
    if output.stderr.bytes.is_empty() {
        if matches!(command, RootCommand::ArchiveExport) {
            validated_archived(&output.stdout.bytes).map(DaemonResponse::Archived)
        } else if matches!(command, RootCommand::CollectObjects) {
            validated_collected(&output.stdout.bytes).map(DaemonResponse::Collected)
        } else if matches!(command, RootCommand::Index) {
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
        "  eliot-search collect-objects [ROOT] [--data-root ROOT] [--config-file PATH] [--daemon PATH]\n",
        "  eliot-search archive-export --archive-path OUT [ROOT] [--data-root ROOT] [--config-file PATH] [--daemon PATH]\n",
        "  eliot-search session --data-root ROOT [--qdrant-url URL] [--config-file PATH] [--daemon PATH]\n",
        "    stdin/stdout: SessionClientFrame/SessionReply JSON, one object per line\n",
        "  eliot-search inspect-data-root [ROOT] [--data-root ROOT] [--config-file PATH] [--daemon PATH]\n",
        "  eliot-search initialize-data-root [ROOT] [--data-root ROOT] [--config-file PATH] [--daemon PATH]\n",
        "  eliot-search check-existing-root-owner [ROOT] [--data-root ROOT] [--config-file PATH] [--daemon PATH]\n",
        "  eliot-search discard-incomplete-data-root [ROOT] [--data-root ROOT] [--config-file PATH] [--daemon PATH]\n",
        "  eliot-search retain-file --file PATH [ROOT] [--data-root ROOT] [--config-file PATH] [--daemon PATH]\n",
        "  eliot-search prepare-source --source-id UUID [--unit N] [--data-root ROOT] [--config-file PATH] [--daemon PATH]\n",
        "  eliot-search publish-prepared --qdrant-url URL [--data-root ROOT] [--config-file PATH] [--daemon PATH]\n",
        "  eliot-search index [PATH] --qdrant-url URL [--data-root ROOT] [--config-file PATH] [--daemon PATH]\n",
        "  eliot-search watch [--interval-ms N] --qdrant-url URL [--data-root ROOT] [--config-file PATH] [--daemon PATH]\n",
        "  eliot-search search --query TEXT [--limit N] --qdrant-url URL [--data-root ROOT] [--config-file PATH] [--daemon PATH]\n",
        "  eliot-search exact --raw --literal TEXT --data-root ROOT [--source-id UUID] [--limit N] [--config-file PATH] [--daemon PATH]\n",
        "  eliot-search exact --decoded-utf8 --literal TEXT --data-root ROOT [--source-id UUID] [--limit N] [--config-file PATH] [--daemon PATH]\n",
    ));
}

fn run_raw_exact(
    settings: Vec<OsString>,
    explicit_daemon: Option<PathBuf>,
    request: RawExactRequest,
    started: Instant,
) -> i32 {
    let Some(deadline) = started.checked_add(COMMAND_TIMEOUT) else {
        return emit_error("DEADLINE_UNAVAILABLE");
    };
    let daemon = match daemon_path(explicit_daemon) {
        Ok(path) => path,
        Err(error) => return emit_error(error),
    };
    let mut arguments = vec![OsString::from("--search-stdio")];
    arguments.extend(settings);
    crate::search_command::run_raw_exact(&daemon, &arguments, request, deadline)
}

fn run_decoded_utf8(
    settings: Vec<OsString>,
    explicit_daemon: Option<PathBuf>,
    request: DecodedUtf8Request,
    started: Instant,
) -> i32 {
    let Some(deadline) = started.checked_add(COMMAND_TIMEOUT) else {
        return emit_error("DEADLINE_UNAVAILABLE");
    };
    let daemon = match daemon_path(explicit_daemon) {
        Ok(path) => path,
        Err(error) => return emit_error(error),
    };
    let mut arguments = vec![OsString::from("--search-stdio")];
    arguments.extend(settings);
    crate::search_command::run_decoded_utf8(&daemon, &arguments, request, deadline)
}

fn run_session(settings: Vec<OsString>, explicit_daemon: Option<PathBuf>, started: Instant) -> i32 {
    let Some(deadline) = started.checked_add(COMMAND_TIMEOUT) else {
        return emit_error("DEADLINE_UNAVAILABLE");
    };
    let daemon = match daemon_path(explicit_daemon) {
        Ok(path) => path,
        Err(error) => return emit_error(error),
    };
    let mut arguments = vec![OsString::from("--session-stdio")];
    arguments.extend(settings);
    crate::search_command::run_session(&daemon, &arguments, deadline)
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
        Parsed::Watch(request) => return watch::run(request),
        Parsed::Session { settings, daemon } => return run_session(settings, daemon, started),
        Parsed::RawExact {
            settings,
            daemon,
            request,
        } => {
            return run_raw_exact(settings, daemon, request, started);
        }
        Parsed::DecodedUtf8 {
            settings,
            daemon,
            request,
        } => {
            return run_decoded_utf8(settings, daemon, request, started);
        }
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
            | DaemonResponse::Indexed(result)
            | DaemonResponse::Collected(result)
            | DaemonResponse::Archived(result),
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

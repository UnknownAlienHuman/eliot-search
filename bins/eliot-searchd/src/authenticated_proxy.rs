//! Authenticated loopback proxy for the owner-fenced DIRECT runtime.
//!
//! The proxy never reuses a child channel after an incomplete exchange. A
//! transport failure after dispatch may hide committed effects; it terminates
//! the child, refuses later requests, and never restarts/replays automatically.

use std::env;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, ExitCode, Stdio};

use crate::endpoint::{self, EndpointAction};

#[path = "proxy_exchange.rs"]
mod exchange;
use exchange::{ExchangeFence, Reply, forward_reply};

const MAX_CHILD_LINE_BYTES: usize = 64 * 1024;
const MAX_CHILD_RESPONSE_LINES: usize = 1_000_000;
const MAX_PROXY_COMMAND_BYTES: usize = 128 * 1024;

/// Intercepts `--serve-loopback-data-root ROOT PORT TOKEN_FILE`.
pub(crate) fn maybe_run() -> Option<ExitCode> {
    let arguments = env::args_os().skip(1).collect::<Vec<_>>();
    if arguments.first().and_then(|value| value.to_str())
        != Some("--serve-loopback-data-root")
    {
        return None;
    }
    let result = match arguments.as_slice() {
        [_, root, port, token_file] => parse_port(port)
            .and_then(|port| run_proxy(Path::new(root), port, Path::new(token_file))),
        _ => Err("LOOPBACK_SERVICE_USAGE_ERROR".to_owned()),
    };
    Some(match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{{\"error\":\"{}\"}}", sanitize_json(&error));
            ExitCode::from(2)
        }
    })
}

fn parse_port(value: &std::ffi::OsStr) -> Result<u16, String> {
    value.to_str()
        .ok_or_else(|| "LOOPBACK_PORT_NOT_UTF8".to_owned())?
        .parse::<u16>()
        .map_err(|_| "LOOPBACK_PORT_INVALID".to_owned())
}

fn run_proxy(root: &Path, port: u16, token_file: &Path) -> Result<(), String> {
    let mut child = DirectChild::spawn(root)?;
    println!(concat!(
        "{{\"event\":\"direct_child_ready\",",
        "\"runtime_owner_ready\":true,",
        "\"source_backed_search_available\":true}}"
    ));
    let endpoint_result = endpoint::serve_loopback(port, token_file, |command, stream| {
        child.dispatch(command, stream)
    });
    if endpoint_result.is_err() {
        child.abort();
    }
    let child_result = child.finish();
    endpoint_result?;
    child_result
}

struct DirectChild {
    child: Child,
    input: Option<ChildStdin>,
    output: BufReader<ChildStdout>,
    fence: ExchangeFence,
    stopped: bool,
}

impl DirectChild {
    fn spawn(root: &Path) -> Result<Self, String> {
        let executable = env::current_exe()
            .map_err(|error| format!("LOOPBACK_CURRENT_EXE_ERROR:{error}"))?;
        let mut child = Command::new(executable)
            .arg("--serve-data-root").arg(root)
            .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::inherit())
            .spawn()
            .map_err(|error| format!("LOOPBACK_DIRECT_CHILD_START_ERROR:{error}"))?;
        let input = child.stdin.take()
            .ok_or_else(|| "LOOPBACK_DIRECT_CHILD_STDIN_MISSING".to_owned())?;
        let output = child.stdout.take()
            .ok_or_else(|| "LOOPBACK_DIRECT_CHILD_STDOUT_MISSING".to_owned())?;
        let mut service = Self {
            child, input: Some(input), output: BufReader::new(output),
            fence: ExchangeFence::default(), stopped: false,
        };
        let ready = read_child_line(&mut service.output)?
            .ok_or_else(|| "LOOPBACK_DIRECT_CHILD_CLOSED_BEFORE_READY".to_owned())?;
        if !ready.contains("\"event\":\"data_root_ready\"") {
            return Err("LOOPBACK_DIRECT_CHILD_NOT_READY".to_owned());
        }
        Ok(service)
    }

    fn dispatch(&mut self, command: &str, stream: &mut TcpStream) -> Result<EndpointAction, String> {
        if self.stopped || self.fence.blocked() {
            return Err("LOOPBACK_DIRECT_CHANNEL_REQUIRES_RESTART".to_owned());
        }
        if command.is_empty() || command.len() > MAX_PROXY_COMMAND_BYTES
            || command.contains('\n') || command.contains('\r')
        {
            return Err("LOOPBACK_DIRECT_COMMAND_INVALID".to_owned());
        }
        let terminal = Terminal::for_command(command)?;
        let input = self.input.as_mut()
            .ok_or_else(|| "LOOPBACK_DIRECT_CHILD_STDIN_MISSING".to_owned())?;
        let output = &mut self.output;
        let result = self.fence.run(|| {
            input.write_all(command.as_bytes())
                .and_then(|()| input.write_all(b"\n"))
                .and_then(|()| input.flush())
                .map_err(|_| "LOOPBACK_DIRECT_CHILD_WRITE_ERROR".to_owned())?;
            forward_reply(
                || read_child_line(output), stream, |line| terminal.reached(line),
                terminal == Terminal::Shutdown, MAX_CHILD_RESPONSE_LINES,
            )
        });
        match result {
            Ok(Reply::Complete) => Ok(EndpointAction::Continue),
            Ok(Reply::Rejected) => Err("LOOPBACK_DIRECT_COMMAND_FAILED".to_owned()),
            Ok(Reply::Shutdown) => {
                self.stopped = true;
                self.input.take();
                Ok(EndpointAction::Shutdown)
            }
            Err(_) => {
                // No bounded drain can prove that an incomplete mutation did
                // not happen. Kill this channel rather than serving old output
                // to a new client, or issuing the command a second time.
                self.abort();
                Err("LOOPBACK_DIRECT_OUTCOME_UNKNOWN_CHANNEL_CLOSED".to_owned())
            }
        }
    }

    fn abort(&mut self) {
        if self.stopped { return; }
        // Never write shutdown into a potentially blocked or desynchronized pipe.
        self.input.take();
        let _ = self.child.kill();
        let _ = self.child.wait();
        self.stopped = true;
    }

    fn finish(mut self) -> Result<(), String> {
        self.input.take();
        let status = self.child.wait()
            .map_err(|error| format!("LOOPBACK_DIRECT_CHILD_WAIT_ERROR:{error}"))?;
        self.stopped = true;
        if status.success() { Ok(()) } else { Err(format!("LOOPBACK_DIRECT_CHILD_EXITED:{status}")) }
    }
}

impl Drop for DirectChild {
    fn drop(&mut self) { self.abort(); }
}

fn read_child_line(output: &mut BufReader<ChildStdout>) -> Result<Option<String>, String> {
    let mut bytes = Vec::new();
    let mut limited = output.by_ref().take((MAX_CHILD_LINE_BYTES + 1) as u64);
    let read = limited.read_until(b'\n', &mut bytes)
        .map_err(|_| "LOOPBACK_DIRECT_CHILD_READ_ERROR".to_owned())?;
    if read == 0 { return Ok(None); }
    if bytes.len() > MAX_CHILD_LINE_BYTES || !bytes.ends_with(b"\n") {
        return Err("LOOPBACK_DIRECT_CHILD_FRAME_TOO_LARGE".to_owned());
    }
    bytes.pop();
    if bytes.last() == Some(&b'\r') { bytes.pop(); }
    String::from_utf8(bytes).map(Some)
        .map_err(|_| "LOOPBACK_DIRECT_CHILD_FRAME_NOT_UTF8".to_owned())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Terminal { Single, DirectoryIndex, StreamingSearch, SearchPage, SourceList, Shutdown }

impl Terminal {
    fn for_command(command: &str) -> Result<Self, String> {
        let name = command.split('\t').next()
            .ok_or_else(|| "LOOPBACK_DIRECT_COMMAND_INVALID".to_owned())?;
        Ok(match name {
            "index-directory" => Self::DirectoryIndex,
            "search" => Self::StreamingSearch,
            "search-page" | "continue" => Self::SearchPage,
            "list-sources" => Self::SourceList,
            "shutdown" => Self::Shutdown,
            _ => Self::Single,
        })
    }
    fn reached(self, line: &str) -> bool {
        match self {
            Self::Single => true,
            Self::DirectoryIndex => line.contains("\"event\":\"directory_index_complete\""),
            Self::StreamingSearch => line.contains("\"event\":\"corpus_search_complete\""),
            Self::SearchPage => line.contains("\"event\":\"search_page_complete\""),
            Self::SourceList => line.contains("\"event\":\"source_list_complete\""),
            Self::Shutdown => line.contains("\"event\":\"data_root_stopped\""),
        }
    }
}

fn sanitize_json(value: &str) -> String {
    let mut output = String::with_capacity(value.len().min(512));
    for character in value.chars().take(512) {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character if character.is_control() => output.push('_'),
            character => output.push(character),
        }
    }
    output
}

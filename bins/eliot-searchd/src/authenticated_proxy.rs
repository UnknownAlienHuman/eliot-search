//! Authenticated loopback proxy for the owner-fenced DIRECT runtime.
//!
//! The proxy never reuses a child channel after an incomplete exchange. A
//! transport failure after dispatch may hide committed effects; it terminates
//! the child, refuses later requests, and never restarts/replays automatically.

use std::env;
use std::net::TcpStream;
use std::path::Path;
use std::process::{Command, ExitCode};

use crate::endpoint::{self, EndpointAction};

#[path = "proxy_exchange.rs"]
mod exchange;
use exchange::{ExchangeFence, Reply, event_name};

#[path = "proxy_child.rs"]
mod child_io;
use child_io::{ChildIo, ChildLimits};
const MAX_PROXY_COMMAND_BYTES: usize = 128 * 1024;

/// Intercepts `--serve-loopback-data-root ROOT PORT TOKEN_FILE`.
pub fn maybe_run() -> Option<ExitCode> {
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
    io: ChildIo,
    fence: ExchangeFence,
}

impl DirectChild {
    fn spawn(root: &Path) -> Result<Self, String> {
        let executable = env::current_exe()
            .map_err(|_| "LOOPBACK_CURRENT_EXE_ERROR".to_owned())?;
        let mut command = Command::new(executable);
        command.arg("--serve-data-root").arg(root);
        Ok(Self { io: ChildIo::spawn(command, ChildLimits::DEFAULT)?, fence: ExchangeFence::default() })
    }

    fn dispatch(&mut self, command: &str, stream: &TcpStream) -> Result<EndpointAction, String> {
        if self.fence.blocked() {
            self.abort();
            return Ok(EndpointAction::Abort);
        }
        if command.is_empty() || command.len() > MAX_PROXY_COMMAND_BYTES
            || command.contains('\n') || command.contains('\r')
        {
            return Err("LOOPBACK_DIRECT_COMMAND_INVALID".to_owned());
        }
        let terminal = Terminal::for_command(command)?;
        let io = &mut self.io;
        let result = self.fence.run(|| io.exchange(command, stream, terminal));
        match result {
            Ok(Reply::Complete) => Ok(EndpointAction::Continue),
            Ok(Reply::Rejected) => Err("LOOPBACK_DIRECT_COMMAND_FAILED".to_owned()),
            // ChildIo returns Shutdown only after actual exit/reaping and pipe
            // cleanup; the child's STOPPED frame alone cannot release the owner.
            Ok(Reply::Shutdown) => Ok(EndpointAction::Shutdown),
            Ok(Reply::Fatal) | Err(_) => {
                self.abort();
                Ok(EndpointAction::Abort)
            }
        }
    }

    fn abort(&mut self) { self.io.abort(); }
    fn finish(mut self) -> Result<(), String> { self.io.finish() }
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
        let event = event_name(line);
        let expected = match self {
            Self::Single => return event.is_some(),
            Self::DirectoryIndex => "directory_index_complete",
            Self::StreamingSearch => "corpus_search_complete",
            Self::SearchPage => "search_page_complete",
            Self::SourceList => "source_list_complete",
            Self::Shutdown => "data_root_stopped",
        };
        event == Some(expected)
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

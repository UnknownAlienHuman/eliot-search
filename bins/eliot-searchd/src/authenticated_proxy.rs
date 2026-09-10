//! Authenticated loopback proxy for the owner-fenced DIRECT runtime.
//!
//! The proxy never reuses a child channel after an incomplete exchange. A
//! transport failure after dispatch may hide committed effects; it terminates
//! the child, refuses later requests, and never restarts/replays automatically.
//!
//! T19 provider routing: after the pairing ceremony, the connection speaks
//! the canonical provider protocol. `op\thello` negotiates the exact version
//! and draws a fresh per-connection server nonce; `envelope\t<seq>\t<hex>`
//! lines carry sealed `health`/`version`/`shutdown` envelopes admitted in
//! fixed order (version, nonce, keyed proof, sequence, replay, ceiling) and
//! completed exactly once; `op\t...` lines carry `status`/`cancel` plus the
//! capability-gated `ingest`/`query`/`expand` operations. Bare tab commands
//! are refused with `PROVIDER_UNKNOWN_COMMAND`: the provider surface is
//! envelope-only, and unsupported recipes return explicit unavailable with
//! their T12 blockers instead of empty success.

use std::cell::Cell;
use std::env;
use std::net::TcpStream;
use std::path::Path;
use std::process::{Command, ExitCode};
use std::rc::Rc;

use search_provider_protocol::{CancelOutcome, DEFAULT_PROTOCOL_LIMITS};

use crate::endpoint::{self, EndpointAction};

use crate::provider_composition::ProviderOperation;

#[path = "proxy_exchange.rs"]
mod exchange;
use exchange::{ExchangeFence, Reply, event_name};

#[path = "proxy_child.rs"]
mod child_io;
use child_io::{ChildIo, ChildLimits};
const MAX_PROXY_COMMAND_BYTES: usize = 128 * 1024;

/// Intercepts `--serve-loopback-data-root ROOT PORT TOKEN_FILE`.
pub fn maybe_run() -> Option<ExitCode> {
    let raw = env::args_os().skip(1).collect::<Vec<_>>();
    let (arguments, _) = match crate::config_composition::strip_config_args_os(&raw) {
        Ok(split) => split,
        Err(error) => {
            eprintln!("{{\"error\":\"{}\"}}", sanitize_json(&error));
            return Some(ExitCode::from(2));
        }
    };
    if arguments.first().and_then(|value| value.to_str()) != Some("--serve-loopback-data-root") {
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
    value
        .to_str()
        .ok_or_else(|| "LOOPBACK_PORT_NOT_UTF8".to_owned())?
        .parse::<u16>()
        .map_err(|_| "LOOPBACK_PORT_INVALID".to_owned())
}

/// Development-compat key source: the token file seeds an ephemeral key.
///
/// Mirrors the endpoint development shim byte-for-byte through the shared
/// [`crate::provider_composition::read_shim_key_file`] derivation. The key is cached in a
/// shared cell on first use so the envelope dispatcher can recompute keyed
/// proofs without a second secret read; the cache lifetime equals the
/// process lifetime exactly like the shim source it replaces. The product
/// lease-bound path stays owned by `secret_composition`.
struct ShimKeySource {
    key: [u8; 32],
    cache: Rc<Cell<Option<[u8; 32]>>>,
}

impl core::fmt::Debug for ShimKeySource {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ShimKeySource")
            .field("key", &"<redacted>")
            .finish()
    }
}

impl Drop for ShimKeySource {
    fn drop(&mut self) {
        self.key.fill(0);
    }
}

impl endpoint::EndpointKeySource for ShimKeySource {
    fn with_endpoint_key<T>(&mut self, use_key: impl FnOnce(&[u8; 32]) -> T) -> Result<T, String> {
        self.cache.set(Some(self.key));
        Ok(use_key(&self.key))
    }
}

fn provider_capabilities() -> Result<crate::provider_composition::ProviderCapabilities, String> {
    let args: Vec<String> = env::args().skip(1).collect();
    let (_, cli) = crate::config_composition::parse_cli_config_args(&args)?;
    let effective = crate::config_composition::effective_from_process(&cli)?;
    let readiness = crate::config_composition::derive_readiness(
        &effective,
        crate::config_composition::direct_dependencies(),
        crate::config_composition::AcceptedReceipts::default(),
    );
    let evidence = crate::provider_composition::CapabilityEvidence::from_parts(
        readiness.capabilities.source_backed_search_available,
        readiness.capabilities.search_available,
        readiness.capabilities.indexed_search_available,
        readiness.blockers,
    )
    .map_err(|_| "DAEMON_CONFIG_INVALID".to_owned())?;
    Ok(crate::provider_composition::negotiate_capabilities(&evidence))
}

fn run_proxy(root: &Path, port: u16, token_file: &Path) -> Result<(), String> {
    let key = crate::provider_composition::read_shim_key_file(token_file)?;
    let cache = Rc::new(Cell::new(None));
    let mut source = ShimKeySource {
        key,
        cache: Rc::clone(&cache),
    };
    let capabilities = provider_capabilities()?;
    let mut child = DirectChild::spawn(root)?;
    println!(concat!(
        "{{\"event\":\"direct_child_ready\",",
        "\"runtime_owner_ready\":true,",
        "\"source_backed_search_available\":true}}"
    ));
    let mut router: Option<crate::provider_composition::ProviderRouter> = None;
    let mut hello_counter: u64 = 0;
    let endpoint_result =
        endpoint::serve_loopback_with_source(port, &mut source, |command, stream| {
            dispatch_provider_command(
                command,
                stream,
                &mut child,
                &mut router,
                &mut hello_counter,
                &capabilities,
                &cache,
            )
        });
    if endpoint_result.is_err() {
        child.abort();
    }
    let child_result = child.finish();
    endpoint_result?;
    child_result
}

fn cached_key(cache: &Rc<Cell<Option<[u8; 32]>>>) -> Result<[u8; 32], String> {
    cache
        .get()
        .ok_or_else(|| crate::provider_composition::PROVIDER_HELLO_REQUIRED.to_owned())
}

fn write_provider_line(stream: &mut TcpStream, line: &str) -> Result<(), String> {
    use std::io::Write;
    stream
        .write_all(line.as_bytes())
        .and_then(|()| stream.write_all(b"\n"))
        .and_then(|()| stream.flush())
        .map_err(|_| "LOOPBACK_PROXY_WRITE_ERROR".to_owned())
}

fn fail_with_provider_error(
    stream: &mut TcpStream,
    reason: &str,
) -> Result<EndpointAction, String> {
    let _ = write_provider_line(stream, &crate::provider_composition::render_provider_error(reason));
    Err(reason.to_owned())
}

fn dispatch_provider_command(
    command: &str,
    stream: &mut TcpStream,
    child: &mut DirectChild,
    router: &mut Option<crate::provider_composition::ProviderRouter>,
    hello_counter: &mut u64,
    capabilities: &crate::provider_composition::ProviderCapabilities,
    cache: &Rc<Cell<Option<[u8; 32]>>>,
) -> Result<EndpointAction, String> {
    if command.is_empty()
        || command.len() > MAX_PROXY_COMMAND_BYTES
        || command.contains('\n')
        || command.contains('\r')
    {
        return Err("LOOPBACK_DIRECT_COMMAND_INVALID".to_owned());
    }
    if command == "op\thello" || command.starts_with("op\thello\t") {
        let key = cached_key(cache)?;
        return do_hello(command, stream, router, hello_counter, capabilities, &key);
    }
    if command.starts_with(crate::provider_composition::ENVELOPE_LINE_PREFIX) {
        let key = cached_key(cache)?;
        return do_envelope(command, stream, child, router, &key);
    }
    if command.starts_with(crate::provider_composition::OP_LINE_PREFIX) {
        return do_op(command, stream, child, router, capabilities);
    }
    fail_with_provider_error(stream, crate::provider_composition::PROVIDER_UNKNOWN_COMMAND)
}

fn do_hello(
    command: &str,
    stream: &mut TcpStream,
    router: &mut Option<crate::provider_composition::ProviderRouter>,
    hello_counter: &mut u64,
    capabilities: &crate::provider_composition::ProviderCapabilities,
    key: &[u8; 32],
) -> Result<EndpointAction, String> {
    let range = match crate::provider_composition::parse_hello_line(command) {
        Ok(range) => range.unwrap_or(crate::provider_composition::PROVIDER_PROTOCOL_RANGE),
        Err(reason) => return fail_with_provider_error(stream, reason),
    };
    let version = match crate::provider_composition::negotiate_connection_version(range) {
        Ok(version) => version,
        Err(error) => {
            return fail_with_provider_error(stream, crate::provider_composition::protocol_reason(error));
        }
    };
    *hello_counter = hello_counter.wrapping_add(1);
    let nonce = match crate::provider_composition::derive_server_nonce(key, *hello_counter) {
        Ok(nonce) => nonce,
        Err(error) => {
            return fail_with_provider_error(stream, crate::provider_composition::protocol_reason(error));
        }
    };
    // Re-hello rebinds: the previous connection state (if any) is cancelled
    // deterministically and its count is reported, never silently dropped.
    let reconnect_cancelled = router
        .as_mut()
        .map_or(0, |bound| bound.disconnect().cancelled_requests());
    match crate::provider_composition::ProviderRouter::open(key, version, nonce, DEFAULT_PROTOCOL_LIMITS) {
        Ok(bound) => *router = Some(bound),
        Err(error) => {
            *router = None;
            return fail_with_provider_error(stream, crate::provider_composition::protocol_reason(error));
        }
    }
    match crate::provider_composition::render_hello(version, &nonce, capabilities, reconnect_cancelled) {
        Ok(line) => {
            if write_provider_line(stream, &line).is_err() {
                *router = None;
                return Err("LOOPBACK_PROXY_WRITE_ERROR".to_owned());
            }
            Ok(EndpointAction::Continue)
        }
        Err(reason) => fail_with_provider_error(stream, reason),
    }
}

fn do_envelope(
    command: &str,
    stream: &mut TcpStream,
    child: &mut DirectChild,
    router: &mut Option<crate::provider_composition::ProviderRouter>,
    key: &[u8; 32],
) -> Result<EndpointAction, String> {
    let (sequence, frame) = match crate::provider_composition::parse_envelope_line(command) {
        Ok(parsed) => parsed,
        Err(reason) => return fail_with_provider_error(stream, reason),
    };
    // Decode before borrowing the router so a malformed frame cannot disturb
    // connection state.
    let envelope = match crate::provider_composition::decode_envelope_frame(&frame) {
        Ok(envelope) => envelope,
        Err(error) => {
            return fail_with_provider_error(stream, crate::provider_composition::protocol_reason(error));
        }
    };
    let Some(router) = router.as_mut() else {
        return fail_with_provider_error(stream, crate::provider_composition::PROVIDER_HELLO_REQUIRED);
    };
    let admitted = match router.admit(&envelope, key, sequence, crate::provider_composition::monotonic_millis(), None)
    {
        Ok(guard) => guard,
        Err(error) => {
            return fail_with_provider_error(stream, crate::provider_composition::protocol_reason(error));
        }
    };
    let request_id = *admitted.request_id();
    let child_command = crate::provider_composition::child_command_for_envelope(envelope.command());
    let shutdown =
        envelope.command() == search_provider_protocol::request::ControlCommand::Shutdown;
    let terminal = Terminal::for_command(child_command)
        .map_err(|_| "LOOPBACK_DIRECT_COMMAND_INVALID".to_owned())?;
    match child.dispatch_provider(child_command, terminal, stream) {
        Ok(reply) => {
            let (_, terminal_kind) = crate::provider_composition::status_for_reply(reply);
            // A fatal child reply already aborted the channel in dispatch;
            // seal the outcome-unknown terminal without granting shutdown.
            if reply == crate::provider_composition::ChildReply::Fatal {
                return complete_envelope(
                    stream,
                    router,
                    key,
                    &request_id,
                    terminal_kind,
                    false,
                )
                .and(Ok(EndpointAction::Abort))
                .or(Ok(EndpointAction::Abort));
            }
            complete_envelope(stream, router, key, &request_id, terminal_kind, shutdown)
        }
        Err(_) => {
            // The exchange failed after dispatch: committed effects are
            // possible, so the terminal is outcome-unknown and the channel is
            // aborted. No success or ordinary failure is ever reported here.
            complete_envelope(
                stream,
                router,
                key,
                &request_id,
                search_provider_protocol::TerminalKind::OutcomeUnknown,
                false,
            )
            .and(Ok(EndpointAction::Abort))
            .or(Ok(EndpointAction::Abort))
        }
    }
}

fn complete_envelope(
    stream: &mut TcpStream,
    router: &mut crate::provider_composition::ProviderRouter,
    key: &[u8; 32],
    request_id: &search_contracts::RequestId,
    terminal_kind: search_provider_protocol::TerminalKind,
    shutdown: bool,
) -> Result<EndpointAction, String> {
    let (assigned_status, provider_sequence) = match router.note_terminal(request_id, terminal_kind)
    {
        Ok(terminal) => terminal,
        Err(error) => {
            return fail_with_provider_error(stream, crate::provider_composition::protocol_reason(error));
        }
    };
    let response = crate::provider_composition::seal_response_with_receipt(
        key,
        router.version(),
        *router.server_nonce(),
        *request_id,
        assigned_status,
        provider_sequence,
    );
    match crate::provider_composition::encode_response_frame(&response) {
        Ok(frame) => {
            let line = format!(
                "{}{}",
                crate::provider_composition::RESPONSE_LINE_PREFIX,
                crate::provider_composition::hex_encode(&frame)
            );
            if write_provider_line(stream, &line).is_err() {
                return Ok(EndpointAction::Abort);
            }
        }
        Err(error) => {
            return fail_with_provider_error(stream, crate::provider_composition::protocol_reason(error));
        }
    }
    Ok(if shutdown {
        EndpointAction::Shutdown
    } else {
        EndpointAction::Continue
    })
}

fn do_op(
    command: &str,
    stream: &mut TcpStream,
    child: &mut DirectChild,
    router: &mut Option<crate::provider_composition::ProviderRouter>,
    capabilities: &crate::provider_composition::ProviderCapabilities,
) -> Result<EndpointAction, String> {
    let (operation, argument) = match crate::provider_composition::parse_op_line(command) {
        Ok(parsed) => parsed,
        Err(reason) => return fail_with_provider_error(stream, reason),
    };
    let Some(router) = router.as_mut() else {
        return fail_with_provider_error(stream, crate::provider_composition::PROVIDER_HELLO_REQUIRED);
    };
    if let Err(denial) = crate::provider_composition::gate_operation(operation, capabilities) {
        let line = match crate::provider_composition::render_op_response(
            operation,
            crate::provider_composition::OpStatus::Unavailable,
            denial.reason,
            &denial.blockers,
        ) {
            Ok(line) => line,
            Err(reason) => return fail_with_provider_error(stream, reason),
        };
        if write_provider_line(stream, &line).is_err() {
            return Err("LOOPBACK_PROXY_WRITE_ERROR".to_owned());
        }
        return Err(denial.reason.to_owned());
    }
    match (operation, argument) {
        (ProviderOperation::Status, crate::provider_composition::OpArgument::None) => {
            match child.dispatch_provider("status", Terminal::Single, stream) {
                Ok(crate::provider_composition::ChildReply::Rejected) => {
                    let line = crate::provider_composition::render_op_response(
                        operation,
                        crate::provider_composition::OpStatus::Failed,
                        "LOOPBACK_DIRECT_COMMAND_FAILED",
                        &[],
                    )
                    .map_err(str::to_owned)?;
                    let _ = write_provider_line(stream, &line);
                    Err("LOOPBACK_DIRECT_COMMAND_FAILED".to_owned())
                }
                Ok(_) => {
                    let line = crate::provider_composition::render_op_response(
                        operation,
                        crate::provider_composition::OpStatus::Ok,
                        crate::provider_composition::PROVIDER_OK,
                        &[],
                    )
                    .map_err(str::to_owned)?;
                    if write_provider_line(stream, &line).is_err() {
                        return Err("LOOPBACK_PROXY_WRITE_ERROR".to_owned());
                    }
                    Ok(EndpointAction::Continue)
                }
                Err(_) => Ok(EndpointAction::Abort),
            }
        }
        (ProviderOperation::Cancel, crate::provider_composition::OpArgument::CancelTarget(target)) => {
            let (status, reason) = match router.cancel(&target) {
                CancelOutcome::Cancelled { .. } => {
                    (crate::provider_composition::OpStatus::Cancelled, crate::provider_composition::PROVIDER_OK)
                }
                CancelOutcome::UnknownOrTerminal => (
                    crate::provider_composition::OpStatus::UnknownOrTerminal,
                    crate::provider_composition::PROVIDER_CANCEL_UNKNOWN_OR_TERMINAL,
                ),
            };
            let line = crate::provider_composition::render_op_response(operation, status, reason, &[])
                .map_err(str::to_owned)?;
            if write_provider_line(stream, &line).is_err() {
                return Err("LOOPBACK_PROXY_WRITE_ERROR".to_owned());
            }
            // Idempotent by construction: even an unknown identity is a
            // successful acknowledgement, never a rejection.
            Ok(EndpointAction::Continue)
        }
        _ => {
            // A gated recipe with negotiated availability still has no
            // loopback executor in the W1 shell: fail closed instead of
            // inventing execution. Unreachable while receipts stay default.
            let line = crate::provider_composition::render_op_response(
                operation,
                crate::provider_composition::OpStatus::Failed,
                crate::provider_composition::PROVIDER_RECIPE_NOT_BOUND,
                &capabilities.blockers,
            )
            .map_err(str::to_owned)?;
            let _ = write_provider_line(stream, &line);
            Err(crate::provider_composition::PROVIDER_RECIPE_NOT_BOUND.to_owned())
        }
    }
}

struct DirectChild {
    io: ChildIo,
    fence: ExchangeFence,
}

impl DirectChild {
    fn spawn(root: &Path) -> Result<Self, String> {
        let executable = env::current_exe().map_err(|_| "LOOPBACK_CURRENT_EXE_ERROR".to_owned())?;
        let mut command = Command::new(executable);
        command.arg("--serve-data-root").arg(root);
        Ok(Self {
            io: ChildIo::spawn(command, ChildLimits::DEFAULT)?,
            fence: ExchangeFence::default(),
        })
    }

    /// Forwards one provider-routed child command and reports the consumed
    /// terminal class without interpreting payload semantics.
    fn dispatch_provider(
        &mut self,
        child_command: &str,
        terminal: Terminal,
        stream: &TcpStream,
    ) -> Result<crate::provider_composition::ChildReply, String> {
        if self.fence.blocked() {
            self.abort();
            return Err("LOOPBACK_DIRECT_CHANNEL_REQUIRES_RESTART".to_owned());
        }
        if child_command.is_empty()
            || child_command.len() > MAX_PROXY_COMMAND_BYTES
            || child_command.contains('\n')
            || child_command.contains('\r')
        {
            return Err("LOOPBACK_DIRECT_COMMAND_INVALID".to_owned());
        }
        let io = &mut self.io;
        let result = self
            .fence
            .run(|| io.exchange(child_command, stream, terminal));
        match result {
            Ok(Reply::Complete) => Ok(crate::provider_composition::ChildReply::Complete),
            Ok(Reply::Rejected) => Ok(crate::provider_composition::ChildReply::Rejected),
            // ChildIo returns Shutdown only after actual exit/reaping and pipe
            // cleanup; the child's STOPPED frame alone cannot release the owner.
            Ok(Reply::Shutdown) => Ok(crate::provider_composition::ChildReply::Shutdown),
            Ok(Reply::Fatal) => {
                self.abort();
                Ok(crate::provider_composition::ChildReply::Fatal)
            }
            Err(_) => {
                self.abort();
                Err("LOOPBACK_DIRECT_OUTCOME_UNKNOWN_CHANNEL_CLOSED".to_owned())
            }
        }
    }

    fn abort(&mut self) {
        self.io.abort();
    }
    fn finish(mut self) -> Result<(), String> {
        self.io.finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Terminal {
    Single,
    DirectoryIndex,
    StreamingSearch,
    SearchPage,
    SourceList,
    Shutdown,
}

impl Terminal {
    fn for_command(command: &str) -> Result<Self, String> {
        let name = command
            .split('\t')
            .next()
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

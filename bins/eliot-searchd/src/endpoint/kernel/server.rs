//! Loopback listener lifetime, authenticated connection loop and acknowledgements.

use std::io::Write;
use std::net::{Ipv4Addr, TcpListener, TcpStream};

use search_provider_protocol::pairing::PairingLedger;

use super::input::EndpointInput;
use super::pairing::authenticate_connection;
use super::spec::{
    EndpointAction, EndpointKeySource,
    MAX_COMMANDS_PER_CONNECTION, MAX_PAIRING_CHALLENGES,
    PAIRING_AUTHENTICATION_ID, READ_TIMEOUT, WRITE_TIMEOUT,
};
use super::wire::{redacted_io_error, sanitize_code, write_line};

/// Connection-owned application state around authenticated command dispatch.
///
/// The listener resets state before pairing and on every connection exit,
/// including failed pairing, EOF, timeout and unwind. A handler must not retain
/// authority from a previous transport. Durable service state has a separate owner.
pub trait EndpointConnectionHandler {
    /// Dispatches only after the current TCP connection has completed pairing.
    fn command(
        &mut self,
        command: &str,
        stream: &mut TcpStream,
    ) -> Result<EndpointAction, String>;

    /// Gives a stateful handler the same reader used by pairing and dispatch.
    /// Poll only for current-request controls while execution is in flight;
    /// ordinary callers retain the command-only behavior.
    fn command_with_input(
        &mut self,
        command: &str,
        stream: &mut TcpStream,
        _input: &mut EndpointInput,
    ) -> Result<EndpointAction, String> {
        self.command(command, stream)
    }

    /// Supplies the one completion for the current transport request.
    /// A handler may include it in its own output transaction; otherwise the
    /// endpoint emits the ordinary acknowledgement after the handler returns.
    fn command_with_completion(
        &mut self,
        command: &str,
        stream: &mut TcpStream,
        input: &mut EndpointInput,
        _completion: &mut EndpointCompletion,
    ) -> Result<EndpointAction, String> {
        self.command_with_input(command, stream, input)
    }

    /// Cancels connection-local work and clears authentication state without I/O.
    /// This operation must be infallible, idempotent and must not panic.
    fn disconnected(&mut self);
}

/// Single-use transport acknowledgement for the current request sequence.
///
/// Constructed only by the endpoint loop. The provider may deliver this frame
/// inside its terminal preparation, before recording lifecycle completion. This
/// is local write/flush completion, not proof that the peer received the bytes.
#[derive(Debug)]
pub struct EndpointCompletion {
    sequence: u64,
    state: CompletionState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CompletionState {
    Pending,
    Failed,
    Delivered,
}

impl EndpointCompletion {
    fn new(sequence: u64) -> Self {
        Self {
            sequence,
            state: CompletionState::Pending,
        }
    }

    /// Writes the exact acknowledgement through the handler's output boundary.
    ///
    /// The callback must write the supplied frame with its newline and flush,
    /// after the response it acknowledges, under the original deadline and
    /// cancellation checks. A failed, repeated or unwound attempt cannot be
    /// retried by the endpoint and cannot trigger an extra completion frame.
    /// The frame's `ok` describes transport handling; the sealed response retains
    /// the operation's success, failure, partial or cancellation classification.
    ///
    /// # Errors
    ///
    /// Returns a closed endpoint error for reuse, or the callback's error.
    pub fn deliver(
        &mut self,
        output: impl FnOnce(&str) -> Result<(), String>,
    ) -> Result<(), String> {
        if self.state != CompletionState::Pending {
            self.state = CompletionState::Failed;
            return Err("ENDPOINT_COMPLETION_ALREADY_ATTEMPTED".to_owned());
        }
        // Arm before formatting/output: even a callback error before its first
        // write is not permission to append an unbudgeted fallback response.
        self.state = CompletionState::Failed;
        let frame = completion_frame(self.sequence, "\"ok\":true");
        output(&frame)?;
        self.state = CompletionState::Delivered;
        Ok(())
    }

    fn finish(
        self,
        writer: &mut impl Write,
        outcome: Result<EndpointAction, String>,
    ) -> EndpointAction {
        match (self.state, outcome) {
            (CompletionState::Pending, outcome) => {
                complete_request(writer, outcome, self.sequence)
            }
            (CompletionState::Delivered, Ok(action)) => action,
            _ => EndpointAction::Abort,
        }
    }
}

// Compatibility for command-only callers that have no connection-local state.
struct CommandHandler<F>(F);

impl<F> EndpointConnectionHandler for CommandHandler<F>
where
    F: FnMut(&str, &mut TcpStream) -> Result<EndpointAction, String>,
{
    fn command(&mut self, command: &str, stream: &mut TcpStream) -> Result<EndpointAction, String> {
        self.0(command, stream)
    }

    fn disconnected(&mut self) {}
}

struct ConnectionScope<'a, H: EndpointConnectionHandler>(&'a mut H);

impl<H: EndpointConnectionHandler> Drop for ConnectionScope<'_, H> {
    fn drop(&mut self) {
        self.0.disconnected();
    }
}

/// Command-only compatibility entry; stateful handlers use
/// [`serve_loopback_with_handler`] to bind cleanup to TCP lifetime.
pub fn serve_loopback_with_source<F, S>(
    port: u16,
    source: &mut S,
    handler: F,
) -> Result<(), String>
where
    F: FnMut(&str, &mut TcpStream) -> Result<EndpointAction, String>,
    S: EndpointKeySource,
{
    serve_loopback_with_handler(port, source, CommandHandler(handler))
}

/// Lease-bound listener with explicit, unconditional connection teardown.
pub fn serve_loopback_with_handler<H, S>(
    port: u16,
    source: &mut S,
    handler: H,
) -> Result<(), String>
where
    H: EndpointConnectionHandler,
    S: EndpointKeySource,
{
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, port))
        .map_err(|error| redacted_io_error("ENDPOINT_BIND_ERROR", &error))?;
    let local = listener
        .local_addr()
        .map_err(|error| redacted_io_error("ENDPOINT_LOCAL_ADDRESS_ERROR", &error))?;
    if !local.ip().is_loopback() {
        return Err("ENDPOINT_NON_LOOPBACK_BIND_DENIED".to_owned());
    }
    println!(
        "{{\"event\":\"loopback_ready\",\"address\":\"{local}\",\"protocol_version\":1,\"authentication\":\"{PAIRING_AUTHENTICATION_ID}\"}}",
    );

    let ledger = PairingLedger::new(MAX_PAIRING_CHALLENGES)
        .map_err(|_| "ENDPOINT_REPLAY_LEDGER_INVALID".to_owned())?;
    serve_listener_with_handler(&listener, source, ledger, handler)
}

#[cfg(test)]
pub(super) fn serve_listener<F, S>(
    listener: &TcpListener,
    source: &mut S,
    ledger: PairingLedger,
    handler: F,
) -> Result<(), String>
where
    F: FnMut(&str, &mut TcpStream) -> Result<EndpointAction, String>,
    S: EndpointKeySource,
{
    serve_listener_with_handler(listener, source, ledger, CommandHandler(handler))
}

fn serve_listener_with_handler<H, S>(
    listener: &TcpListener,
    source: &mut S,
    mut ledger: PairingLedger,
    mut handler: H,
) -> Result<(), String>
where
    H: EndpointConnectionHandler,
    S: EndpointKeySource,
{
    let mut connection_sequence = 0_u64;
    for incoming in listener.incoming() {
        let mut stream = match incoming {
            Ok(stream) => stream,
            Err(error) => {
                eprintln!(
                    "{{\"error\":\"ENDPOINT_ACCEPT_ERROR\",\"detail_class\":\"{}\"}}",
                    error.kind()
                );
                continue;
            }
        };
        connection_sequence = connection_sequence
            .checked_add(1)
            .ok_or_else(|| "ENDPOINT_CONNECTION_SEQUENCE_EXHAUSTED".to_owned())?;
        let peer = stream
            .peer_addr()
            .map_err(|error| redacted_io_error("ENDPOINT_PEER_ADDRESS_ERROR", &error))?;
        if !peer.ip().is_loopback() {
            let _ = write_line(&mut stream, "{\"error\":\"ENDPOINT_LOOPBACK_REQUIRED\"}");
            continue;
        }
        stream
            .set_read_timeout(Some(READ_TIMEOUT))
            .and_then(|()| stream.set_write_timeout(Some(WRITE_TIMEOUT)))
            .map_err(|error| redacted_io_error("ENDPOINT_TIMEOUT_CONFIGURATION_ERROR", &error))?;

        match serve_connection(
            stream,
            connection_sequence,
            source,
            &mut ledger,
            &mut handler,
        ) {
            Ok(EndpointAction::Continue) => {}
            Ok(EndpointAction::Shutdown) => return Ok(()),
            Ok(EndpointAction::Abort) => return Err("ENDPOINT_HANDLER_ABORTED".to_owned()),
            Err(error) => {
                eprintln!(
                    "{{\"error\":\"{}\",\"connection_sequence\":{}}}",
                    sanitize_code(&error),
                    connection_sequence,
                );
            }
        }
    }
    Ok(())
}

fn serve_connection<H, S>(
    mut stream: TcpStream,
    connection_sequence: u64,
    source: &mut S,
    ledger: &mut PairingLedger,
    handler: &mut H,
) -> Result<EndpointAction, String>
where
    H: EndpointConnectionHandler,
    S: EndpointKeySource,
{
    // Reset before the key source exposes this connection's pairing material.
    // Install teardown before the first fallible authentication or I/O operation.
    handler.disconnected();
    let connection = ConnectionScope(handler);
    let reader = authenticate_connection(&mut stream, connection_sequence, source, ledger)?;
    let mut input = EndpointInput::new(reader);

    let mut request_sequence = 0_u64;
    loop {
        let Some(command) = input.read_command()? else {
            return Ok(EndpointAction::Continue);
        };
        if command.is_empty() {
            return Err("ENDPOINT_EMPTY_COMMAND".to_owned());
        }
        if request_sequence >= u64::try_from(MAX_COMMANDS_PER_CONNECTION).unwrap_or(u64::MAX) {
            write_line(
                &mut stream,
                "{\"event\":\"request_complete\",\"ok\":false,\"error\":\"ENDPOINT_REQUEST_LIMIT_EXCEEDED\"}",
            )
            .map_err(|error| redacted_io_error("ENDPOINT_WRITE_ERROR", &error))?;
            return Ok(EndpointAction::Continue);
        }
        write_line(
            &mut stream,
            &format!("{{\"event\":\"request_started\",\"sequence\":{request_sequence}}}"),
        )
        .map_err(|error| redacted_io_error("ENDPOINT_WRITE_ERROR", &error))?;
        let mut completion = EndpointCompletion::new(request_sequence);
        let outcome = connection.0.command_with_completion(
            &command,
            &mut stream,
            &mut input,
            &mut completion,
        );
        match completion.finish(&mut stream, outcome) {
            EndpointAction::Continue => {
                request_sequence = request_sequence
                    .checked_add(1)
                    .ok_or_else(|| "ENDPOINT_REQUEST_SEQUENCE_EXHAUSTED".to_owned())?;
            }
            action => return Ok(action),
        }
    }
}

/// Finishes one request without appending bytes after an unusable channel.
pub(super) fn complete_request(
    writer: &mut impl Write,
    outcome: Result<EndpointAction, String>,
    sequence: u64,
) -> EndpointAction {
    let (action, status) = match outcome {
        Ok(EndpointAction::Abort) => return EndpointAction::Abort,
        Ok(action) => (action, "\"ok\":true".to_owned()),
        Err(error) => (
            EndpointAction::Continue,
            format!("\"ok\":false,\"error\":\"{}\"", sanitize_code(&error)),
        ),
    };
    let frame = completion_frame(sequence, &status);
    if write_line(writer, &frame).is_err() {
        EndpointAction::Abort
    } else {
        action
    }
}

fn completion_frame(sequence: u64, status: &str) -> String {
    format!("{{\"event\":\"request_complete\",\"sequence\":{sequence},{status}}}")
}

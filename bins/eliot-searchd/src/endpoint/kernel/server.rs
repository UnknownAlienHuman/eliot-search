//! Loopback listener lifetime, authenticated connection loop and acknowledgements.

use std::io::Write;
use std::net::{Ipv4Addr, TcpListener, TcpStream};

use search_provider_protocol::pairing::PairingLedger;

use super::pairing::authenticate_connection;
use super::spec::{
    EndpointAction, EndpointKeySource, MAX_COMMAND_LINE_BYTES,
    MAX_COMMANDS_PER_CONNECTION, MAX_PAIRING_CHALLENGES,
    PAIRING_AUTHENTICATION_ID, READ_TIMEOUT, WRITE_TIMEOUT,
};
use super::wire::{read_bounded_line, redacted_io_error, sanitize_code, write_line};

/// Lease-bound listener entry using mutual keyed pairing transcripts.
pub fn serve_loopback_with_source<F, S>(
    port: u16,
    source: &mut S,
    handler: F,
) -> Result<(), String>
where
    F: FnMut(&str, &mut TcpStream) -> Result<EndpointAction, String>,
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
    serve_listener(&listener, source, ledger, handler)
}

pub(super) fn serve_listener<F, S>(
    listener: &TcpListener,
    source: &mut S,
    mut ledger: PairingLedger,
    mut handler: F,
) -> Result<(), String>
where
    F: FnMut(&str, &mut TcpStream) -> Result<EndpointAction, String>,
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

fn serve_connection<F, S>(
    mut stream: TcpStream,
    connection_sequence: u64,
    source: &mut S,
    ledger: &mut PairingLedger,
    handler: &mut F,
) -> Result<EndpointAction, String>
where
    F: FnMut(&str, &mut TcpStream) -> Result<EndpointAction, String>,
    S: EndpointKeySource,
{
    let mut reader = authenticate_connection(&mut stream, connection_sequence, source, ledger)?;

    let mut request_sequence = 0_u64;
    loop {
        let Some(command) = read_bounded_line(&mut reader, MAX_COMMAND_LINE_BYTES)? else {
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
        let outcome = handler(&command, &mut stream);
        match complete_request(&mut stream, outcome, request_sequence) {
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
    let frame = format!("{{\"event\":\"request_complete\",\"sequence\":{sequence},{status}}}");
    if write_line(writer, &frame).is_err() {
        EndpointAction::Abort
    } else {
        action
    }
}

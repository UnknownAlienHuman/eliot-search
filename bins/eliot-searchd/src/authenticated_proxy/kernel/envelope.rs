//! Sealed provider envelope admission and terminal completion.

use std::net::TcpStream;

use crate::endpoint::EndpointAction;

use super::child::DirectChild;
use super::wire::{fail_with_provider_error, write_provider_line};
use super::Terminal;

pub(super) fn do_envelope(
    command: &str,
    stream: &mut TcpStream,
    child: &mut DirectChild,
    router: &mut Option<crate::provider_composition::ProviderRouter>,
    key: &[u8; 32],
) -> Result<EndpointAction, String> {
    let (sequence, frame) =
        match crate::provider_composition::parse_envelope_line(command) {
            Ok(parsed) => parsed,
            Err(reason) => return fail_with_provider_error(stream, reason),
        };
    // Decode before borrowing the router so malformed bytes cannot disturb
    // connection state.
    let envelope = match crate::provider_composition::decode_envelope_frame(&frame) {
        Ok(envelope) => envelope,
        Err(error) => {
            return fail_with_provider_error(
                stream,
                crate::provider_composition::protocol_reason(error),
            );
        }
    };
    let Some(router) = router.as_mut() else {
        return fail_with_provider_error(
            stream,
            crate::provider_composition::PROVIDER_HELLO_REQUIRED,
        );
    };
    let admitted = match router.admit(
        &envelope,
        key,
        sequence,
        crate::provider_composition::monotonic_millis(),
        None,
    ) {
        Ok(guard) => guard,
        Err(error) => {
            return fail_with_provider_error(
                stream,
                crate::provider_composition::protocol_reason(error),
            );
        }
    };
    let request_id = *admitted.request_id();
    let child_command =
        crate::provider_composition::child_command_for_envelope(envelope.command());
    let shutdown = envelope.command()
        == search_provider_protocol::request::ControlCommand::Shutdown;
    let terminal = Terminal::for_command(child_command)
        .map_err(|_| "LOOPBACK_DIRECT_COMMAND_INVALID".to_owned())?;
    match child.dispatch_provider(child_command, terminal, stream) {
        Ok(reply) => {
            let (_, terminal_kind) =
                crate::provider_composition::status_for_reply(reply);
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
            complete_envelope(
                stream,
                router,
                key,
                &request_id,
                terminal_kind,
                shutdown,
            )
        }
        Err(_) => complete_envelope(
            stream,
            router,
            key,
            &request_id,
            search_provider_protocol::TerminalKind::OutcomeUnknown,
            false,
        )
        .and(Ok(EndpointAction::Abort))
        .or(Ok(EndpointAction::Abort)),
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
    let (assigned_status, provider_sequence) =
        match router.note_terminal(request_id, terminal_kind) {
            Ok(terminal) => terminal,
            Err(error) => {
                return fail_with_provider_error(
                    stream,
                    crate::provider_composition::protocol_reason(error),
                );
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
            return fail_with_provider_error(
                stream,
                crate::provider_composition::protocol_reason(error),
            );
        }
    }
    Ok(if shutdown {
        EndpointAction::Shutdown
    } else {
        EndpointAction::Continue
    })
}

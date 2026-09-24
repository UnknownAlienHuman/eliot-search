//! Sealed provider envelope admission and terminal completion.

use std::net::TcpStream;
use std::time::Instant;

use search_provider_protocol::request::RequestGuard;

use crate::endpoint::EndpointAction;

use super::child::DirectChild;
use super::child_io::write_admitted_line;
use super::wire::fail_with_provider_error;
use super::Terminal;

pub(super) fn do_envelope(
    command: &str,
    stream: &mut TcpStream,
    child: &mut DirectChild,
    router: &mut Option<crate::provider_composition::ProviderRouter>,
    key: &[u8; 32],
    mut input: Option<&mut crate::endpoint::EndpointInput>,
    completion: Option<&mut crate::endpoint::EndpointCompletion>,
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
    // These closed control commands have no body. Bind the digest to the
    // actual empty body before sequence/replay admission or child dispatch;
    // a correctly signed but different digest is not the command we execute.
    let empty_body = search_provider_protocol::ProofDigest::from_bytes(
        *blake3::hash(&[]).as_bytes(),
    );
    if !search_provider_protocol::pairing::verify_proof(&empty_body, envelope.body_digest()) {
        return fail_with_provider_error(
            stream,
            crate::provider_composition::protocol_reason(
                search_provider_protocol::ProtocolError::InvalidEnvelope,
            ),
        );
    }
    // One server-owned budget begins before admission and is shared with the
    // queued worker and terminal writer. These bodyless wire commands do not
    // carry a client-selected deadline; do not mint an unlimited RequestGuard.
    let (deadline, relative_deadline_ms) = match child.request_budget() {
        Ok(budget) => budget,
        Err(reason) => return fail_with_provider_error(stream, &reason),
    };
    let admitted = match router.admit(
        &envelope,
        key,
        sequence,
        crate::provider_composition::monotonic_millis(),
        Some(relative_deadline_ms),
    ) {
        Ok(guard) => guard,
        Err(error) => {
            return fail_with_provider_error(
                stream,
                crate::provider_composition::protocol_reason(error),
            );
        }
    };
    let child_command =
        crate::provider_composition::child_command_for_envelope(envelope.command());
    let shutdown = envelope.command()
        == search_provider_protocol::request::ControlCommand::Shutdown;
    let terminal = Terminal::for_command(child_command)
        .map_err(|_| "LOOPBACK_DIRECT_COMMAND_INVALID".to_owned())?;
    let mut observer = admitted.clone();
    let mut poll = || {
        let Some(input) = input.as_deref_mut() else {
            return Ok(());
        };
        let observed = input.poll_control(|line| {
            // Do not decode ordinary query bodies during each polling pass.
            line.starts_with("op\tcancel\t") && matches!(
                crate::provider_composition::parse_op_line(line),
                Ok((crate::provider_composition::ProviderOperation::Cancel,
                    crate::provider_composition::OpArgument::CancelTarget(target)))
                    if &target == observer.request_id()
            )
        });
        match observed {
            Ok(false) => Ok(()),
            Ok(true) => {
                observer.mark_cancelled();
                Err("LOOPBACK_DIRECT_REQUEST_CANCELLED".to_owned())
            }
            Err(error) => {
                observer.mark_cancelled();
                Err(error)
            }
        }
    };
    match child.dispatch_admitted(child_command, terminal, stream, &admitted, deadline, &mut poll) {
        Ok(reply) => {
            use crate::provider_composition::ChildReply;

            // A complete fatal frame can carry one sealed outcome-unknown
            // terminal. A failed exchange below has no trustworthy frame
            // boundary and must never trigger another write.
            let unexpected_shutdown = matches!(reply, ChildReply::Shutdown) && !shutdown;
            let unconfirmed_shutdown = shutdown && matches!(reply, ChildReply::Complete);
            let fatal = matches!(reply, ChildReply::Fatal)
                || unexpected_shutdown
                || unconfirmed_shutdown;
            let terminal_kind = if fatal {
                search_provider_protocol::TerminalKind::OutcomeUnknown
            } else {
                crate::provider_composition::status_for_reply(reply).1
            };
            let action = complete_envelope(
                stream,
                router,
                key,
                &admitted,
                deadline,
                terminal_kind,
                shutdown && matches!(reply, ChildReply::Shutdown),
                // Fatal/unknown exchanges intentionally have no normal outer
                // acknowledgement. The endpoint sees Abort and writes nothing.
                if fatal {
                    None
                } else {
                    completion
                },
                &mut poll,
            );
            if fatal {
                let _ = router.disconnect();
                return Ok(EndpointAction::Abort);
            }
            action
        }
        Err(_) => {
            // The child owner has already fenced/aborted an uncertain exchange.
            // It may have forwarded partial output or encountered a socket
            // failure. Do not append a fabricated response or an error frame.
            let _ = router.disconnect();
            Ok(EndpointAction::Abort)
        }
    }
}

fn complete_envelope(
    stream: &mut TcpStream,
    router: &mut crate::provider_composition::ProviderRouter,
    key: &[u8; 32],
    request: &RequestGuard,
    deadline: Instant,
    terminal_kind: search_provider_protocol::TerminalKind,
    shutdown: bool,
    completion: Option<&mut crate::endpoint::EndpointCompletion>,
    poll: &mut dyn FnMut() -> Result<(), String>,
) -> Result<EndpointAction, String> {
    let request_id = request.request_id();
    let cancellation = request.cancellation();
    let version = router.version();
    let nonce = *router.server_nonce();
    let delivered = match router.prepare_terminal(request_id, terminal_kind) {
        Ok(prepared) => prepared.deliver(|assigned_status, provider_sequence| {
            let response = crate::provider_composition::seal_response_with_receipt(
                key,
                version,
                nonce,
                *request_id,
                assigned_status,
                provider_sequence,
            );
            let frame = crate::provider_composition::encode_response_frame(&response)
                .map_err(|error| crate::provider_composition::protocol_reason(error).to_owned())?;
            let line = format!(
                "{}{}",
                crate::provider_composition::RESPONSE_LINE_PREFIX,
                crate::provider_composition::hex_encode(&frame)
            );
            if let Some(completion) = completion {
                // Keep the exclusive router preparation until BOTH frames are
                // written/flushed. A failed acknowledgement must not consume
                // the request slot or publish the next provider sequence.
                completion.deliver(|acknowledgement| {
                    write_admitted_line(stream, &line, deadline, &cancellation, poll)?;
                    write_admitted_line(
                        stream,
                        acknowledgement,
                        deadline,
                        &cancellation,
                        poll,
                    )
                })
            } else {
                // Command-only compatibility and fatal outcome-unknown output
                // keep their existing caller-managed acknowledgement contract.
                write_admitted_line(stream, &line, deadline, &cancellation, poll)
            }
        }).is_ok(),
        Err(_) => false,
    };
    if !delivered {
        // Child output has already started. Neither failed preparation nor a
        // partial terminal/acknowledgement write can be repaired by appending
        // an error frame or retrying completion with a fresh output budget.
        // The preparation also closes on output error/unwind; disconnect here
        // additionally covers rejection before a preparation could be created.
        let _ = router.disconnect();
        return Ok(EndpointAction::Abort);
    }
    Ok(if shutdown {
        EndpointAction::Shutdown
    } else {
        EndpointAction::Continue
    })
}

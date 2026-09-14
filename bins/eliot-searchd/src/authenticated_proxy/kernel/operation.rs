//! Capability-gated provider operation routing.

use std::net::TcpStream;

use search_provider_protocol::CancelOutcome;

use crate::endpoint::EndpointAction;
use crate::provider_composition::ProviderOperation;

use super::child::DirectChild;
use super::wire::{fail_with_provider_error, write_provider_line};
use super::Terminal;

pub(super) fn do_op(
    command: &str,
    stream: &mut TcpStream,
    child: &mut DirectChild,
    router: &mut Option<crate::provider_composition::ProviderRouter>,
    capabilities: &crate::provider_composition::ProviderCapabilities,
) -> Result<EndpointAction, String> {
    let (operation, argument) =
        match crate::provider_composition::parse_op_line(command) {
            Ok(parsed) => parsed,
            Err(reason) => return fail_with_provider_error(stream, reason),
        };
    let Some(router) = router.as_mut() else {
        return fail_with_provider_error(
            stream,
            crate::provider_composition::PROVIDER_HELLO_REQUIRED,
        );
    };
    if let Err(denial) =
        crate::provider_composition::gate_operation(operation, capabilities)
    {
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
        (
            ProviderOperation::Status,
            crate::provider_composition::OpArgument::None,
        ) => match child.dispatch_provider("status", Terminal::Single, stream) {
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
        },
        (
            ProviderOperation::Cancel,
            crate::provider_composition::OpArgument::CancelTarget(target),
        ) => {
            let (status, reason) = match router.cancel(&target) {
                CancelOutcome::Cancelled { .. } => (
                    crate::provider_composition::OpStatus::Cancelled,
                    crate::provider_composition::PROVIDER_OK,
                ),
                CancelOutcome::UnknownOrTerminal => (
                    crate::provider_composition::OpStatus::UnknownOrTerminal,
                    crate::provider_composition::PROVIDER_CANCEL_UNKNOWN_OR_TERMINAL,
                ),
            };
            let line = crate::provider_composition::render_op_response(
                operation,
                status,
                reason,
                &[],
            )
            .map_err(str::to_owned)?;
            if write_provider_line(stream, &line).is_err() {
                return Err("LOOPBACK_PROXY_WRITE_ERROR".to_owned());
            }
            Ok(EndpointAction::Continue)
        }
        _ => {
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

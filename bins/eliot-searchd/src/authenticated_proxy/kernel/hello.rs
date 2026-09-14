//! Provider hello/version negotiation and connection rebinding.

use std::net::TcpStream;

use search_provider_protocol::DEFAULT_PROTOCOL_LIMITS;

use crate::endpoint::EndpointAction;

use super::wire::{fail_with_provider_error, write_provider_line};

pub(super) fn do_hello(
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
            return fail_with_provider_error(
                stream,
                crate::provider_composition::protocol_reason(error),
            );
        }
    };
    *hello_counter = hello_counter.wrapping_add(1);
    let nonce = match crate::provider_composition::derive_server_nonce(
        key,
        *hello_counter,
    ) {
        Ok(nonce) => nonce,
        Err(error) => {
            return fail_with_provider_error(
                stream,
                crate::provider_composition::protocol_reason(error),
            );
        }
    };
    // Re-hello rebinds and deterministically cancels prior connection state.
    let reconnect_cancelled = router
        .as_mut()
        .map_or(0, |bound| bound.disconnect().cancelled_requests());
    match crate::provider_composition::ProviderRouter::open(
        key,
        version,
        nonce,
        DEFAULT_PROTOCOL_LIMITS,
    ) {
        Ok(bound) => *router = Some(bound),
        Err(error) => {
            *router = None;
            return fail_with_provider_error(
                stream,
                crate::provider_composition::protocol_reason(error),
            );
        }
    }
    match crate::provider_composition::render_hello(
        version,
        &nonce,
        capabilities,
        reconnect_cancelled,
    ) {
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

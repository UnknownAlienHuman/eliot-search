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
    // An accepted re-hello ends the prior session before any new preparation.
    // Failure cannot leave an old router available behind a rejected hello.
    let reconnect_cancelled = router
        .take()
        .map_or(0, |mut bound| bound.disconnect().cancelled_requests());
    let Some(next_counter) = hello_counter.checked_add(1) else {
        return fail_with_provider_error(
            stream,
            search_provider_protocol::ProtocolError::SequenceExhausted.code(),
        );
    };
    *hello_counter = next_counter;
    // A stable token file and a restarted counter must not repeat an envelope
    // nonce. Every hello uses the existing OS CSPRNG owner; no clock fallback.
    let entropy = match crate::qualified_entropy::qualified_entropy_32() {
        Ok(entropy) => entropy,
        Err(reason) => return fail_with_provider_error(stream, reason),
    };
    let nonce_key = blake3::keyed_hash(key, &entropy);
    let nonce = match crate::provider_composition::derive_server_nonce(
        nonce_key.as_bytes(),
        next_counter,
    ) {
        Ok(nonce) => nonce,
        Err(error) => {
            return fail_with_provider_error(
                stream,
                crate::provider_composition::protocol_reason(error),
            );
        }
    };
    let bound = match crate::provider_composition::ProviderRouter::open(
        key,
        version,
        nonce,
        DEFAULT_PROTOCOL_LIMITS,
    ) {
        Ok(bound) => bound,
        Err(error) => {
            return fail_with_provider_error(
                stream,
                crate::provider_composition::protocol_reason(error),
            );
        }
    };
    let line = match crate::provider_composition::render_hello(
        version,
        &nonce,
        capabilities,
        reconnect_cancelled,
    ) {
        Ok(line) => line,
        Err(reason) => return fail_with_provider_error(stream, reason),
    };
    if write_provider_line(stream, &line).is_err() {
        // Possible partial output: do not append an error or outer completion.
        return Ok(EndpointAction::Abort);
    }
    // Publish the new router only after the hello was fully written/flushed.
    // A later endpoint-completion failure triggers connection-scope teardown.
    *router = Some(bound);
    Ok(EndpointAction::Continue)
}

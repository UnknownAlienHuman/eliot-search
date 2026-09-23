//! Loopback endpoint composition behind the stable daemon-local facade.

mod codec;
mod input;
mod pairing;
mod server;
mod spec;
mod wire;

pub use input::EndpointInput;
pub use pairing::{keyed_proof, pairing_binding_digest};
pub use server::{
    EndpointConnectionHandler, serve_loopback_with_handler, serve_loopback_with_source,
};
pub use spec::{
    EndpointAction, EndpointKeySource, PAIRING_AUTHENTICATION_ID,
    PAIRING_PROTOCOL_VERSION,
};

#[cfg(test)]
pub use codec::{
    ParsedChallenge, client_proof_for_challenge, parse_challenge_line,
    parse_verified_line, verify_provider_proof,
};

#[cfg(test)]
use codec::{encode_challenge, hex_encode};
#[cfg(test)]
use pairing::derive_ceremony_material;
#[cfg(test)]
use server::{complete_request, serve_listener};
#[cfg(test)]
use spec::{
    MAX_CHALLENGE_LINE_BYTES, MAX_PAIRING_CHALLENGES,
    MAX_VERIFIED_LINE_BYTES, READ_TIMEOUT, WRITE_TIMEOUT,
};
#[cfg(test)]
use wire::{read_bounded_line, redacted_io_error, write_line};

#[cfg(test)]
mod tests;

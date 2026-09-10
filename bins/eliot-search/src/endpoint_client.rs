//! Authenticated canonical provider transport for one CLI invocation.
//!
//! Each call opens one `pairing_blake3_v1` connection to a loopback address,
//! negotiates the exact provider version, seals a single envelope or
//! operation frame, verifies the sealed response, and prints daemon payload
//! lines to stdout. There is no fallback transport and no alternate daemon:
//! a missing descriptor, a failed handshake or a rejected frame fails closed.

use std::net::SocketAddr;
use std::path::Path;

use crate::provider_client::{self, UnsignedRequest};

/// Runs one validated provider request against the canonical endpoint.
///
/// Prints daemon payload lines to stdout. Returns the typed provider reason
/// on rejection (for exit-code mapping) or a local `REMOTE_*`/`ENDPOINT_*`
/// code on transport failure.
pub fn invoke_remote(
    address: &str,
    token_file: &Path,
    request: &UnsignedRequest,
) -> Result<(), String> {
    let address = address
        .parse::<SocketAddr>()
        .map_err(|_| "REMOTE_ADDRESS_INVALID".to_owned())?;
    if !address.ip().is_loopback() {
        return Err("REMOTE_NON_LOOPBACK_DENIED".to_owned());
    }
    let key = provider_client::read_shim_key(token_file)?;
    let mut session = provider_client::open_session(address, key)?;
    session.invoke(request)
}

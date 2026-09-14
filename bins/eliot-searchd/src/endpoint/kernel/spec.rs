//! Closed loopback endpoint constants and public boundary types.

use std::time::Duration;

use search_contracts::ProtocolVersion;

/// Negotiated loopback-pairing version bound into every proof transcript.
pub const PAIRING_PROTOCOL_VERSION: ProtocolVersion = ProtocolVersion { major: 1, minor: 0 };
/// Wire authentication identifier; the legacy `sha256_challenge_v1` is gone.
pub const PAIRING_AUTHENTICATION_ID: &str = "pairing_blake3_v1";

#[cfg(test)]
pub(super) const MAX_CHALLENGE_LINE_BYTES: usize = 512;
pub(super) const MAX_AUTH_LINE_BYTES: usize = 256;
#[cfg(test)]
pub(super) const MAX_VERIFIED_LINE_BYTES: usize = 256;
pub(super) const MAX_COMMAND_LINE_BYTES: usize = 128 * 1024;
pub(super) const MAX_COMMANDS_PER_CONNECTION: usize = 4096;
pub(super) const MAX_PAIRING_CHALLENGES: usize = 4096;

// Silent-client read bound and slow-reader write bound. They share a value
// but never a meaning: the read timeout is not the socket configuration, and
// neither is the proxy child request/startup/cleanup deadline.
pub(super) const READ_TIMEOUT: Duration = Duration::from_secs(30);
pub(super) const WRITE_TIMEOUT: Duration = Duration::from_secs(30);

/// Connection-handler outcome at the authenticated endpoint boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EndpointAction {
    /// Keep the listener and this connection usable.
    Continue,
    /// Complete the current exchange and stop the listener cleanly.
    Shutdown,
    /// Unusable handler/output channel; close the listener without another reply.
    Abort,
}

/// Per-connection key supply owned by the secret-owning side.
///
/// The key is exposed only for the duration of one callback so proofs are
/// computed inside the lease window without the key crossing the socket,
/// reaching logs or escaping by value.
pub trait EndpointKeySource {
    /// Exposes the active 32-byte pairing key for one callback.
    fn with_endpoint_key<T>(&mut self, use_key: impl FnOnce(&[u8; 32]) -> T) -> Result<T, String>;
}

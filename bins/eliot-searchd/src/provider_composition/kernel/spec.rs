//! Closed provider protocol vocabulary, limits and reason codes.

use search_contracts::{ProtocolRange, ProtocolVersion};
use search_provider_protocol::ProtocolError;

/// Exact negotiated provider version: the daemon speaks `1.0` only.
pub const PROVIDER_PROTOCOL_VERSION: ProtocolVersion = ProtocolVersion { major: 1, minor: 0 };

/// Exact supported range: `1.0-1.0`. Minor/extension negotiation is explicit;
/// anything outside this range fails with `PROTOCOL_NO_COMPATIBLE_VERSION`.
pub const PROVIDER_PROTOCOL_RANGE: ProtocolRange = ProtocolRange {
    minimum: PROVIDER_PROTOCOL_VERSION,
    maximum: PROVIDER_PROTOCOL_VERSION,
};

/// Client-to-daemon line carrying one authenticated envelope with the
/// client-assigned connection sequence: `envelope\t<seq>\t<hex>`.
pub const ENVELOPE_LINE_PREFIX: &str = "envelope\t";
/// Client-to-daemon line carrying one provider operation: `op\t<name>...`.
pub const OP_LINE_PREFIX: &str = "op\t";
/// Daemon-to-client line carrying one sealed envelope response: `response\t<hex>`.
pub const RESPONSE_LINE_PREFIX: &str = "response\t";

/// Maximum hex characters accepted after the `envelope\t<seq>\t` prefix
/// (128 KiB of frame bytes, matching the endpoint command-line ceiling).
pub const MAX_ENVELOPE_HEX: usize = 256 * 1024;
/// Maximum hex characters accepted as one `op` argument (64 KiB of bytes).
pub const MAX_OP_ARG_HEX: usize = 128 * 1024;
/// Maximum rendered provider JSON line in bytes.
pub const MAX_RENDERED_LINE_BYTES: usize = 64 * 1024;
/// Maximum blockers carried in one evidence snapshot or rendered line.
pub const MAX_BLOCKERS: usize = 8;

/// Closed T19 provider operation registry.
///
/// `Health`/`Version`/`Shutdown` require an authenticated envelope;
/// `Status`/`Cancel` are connection-local operations on a bound router;
/// `Ingest`/`Query`/`Expand` are shape-validated, then capability-gated.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ProviderOperation {
    /// Bounded daemon health routed as an authenticated envelope.
    Health,
    /// Local readiness/capability diagnostics (always available).
    Status,
    /// Local version envelope routing.
    Version,
    /// Content admission, gated by search acceptance.
    Ingest,
    /// Recipe query, gated by search acceptance.
    Query,
    /// Handle expansion, gated by search acceptance.
    Expand,
    /// Idempotent in-flight cancellation.
    Cancel,
    /// Authenticated graceful shutdown routed as an authenticated envelope.
    Shutdown,
}

impl ProviderOperation {
    /// Closed registry: every operation has a stable wire spelling.
    pub const ALL: &'static [Self] = &[
        Self::Health,
        Self::Status,
        Self::Version,
        Self::Ingest,
        Self::Query,
        Self::Expand,
        Self::Cancel,
        Self::Shutdown,
    ];

    /// Stable wire spelling used on `op` lines.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Health => "health",
            Self::Status => "status",
            Self::Version => "version",
            Self::Ingest => "ingest",
            Self::Query => "query",
            Self::Expand => "expand",
            Self::Cancel => "cancel",
            Self::Shutdown => "shutdown",
        }
    }

    /// Parses a closed-registry operation; anything else is an unknown command.
    pub fn parse(value: &str) -> Result<Self, &'static str> {
        Self::ALL
            .iter()
            .copied()
            .find(|operation| operation.as_str() == value)
            .ok_or(PROVIDER_UNKNOWN_COMMAND)
    }

    /// Whether this operation must arrive as an authenticated envelope.
    #[must_use]
    pub const fn requires_envelope(self) -> bool {
        matches!(self, Self::Health | Self::Version | Self::Shutdown)
    }
}

/// Unknown provider operation or line shape.
pub const PROVIDER_UNKNOWN_COMMAND: &str = "PROVIDER_UNKNOWN_COMMAND";
/// An envelope-only operation arrived as a bare `op` line.
pub const PROVIDER_ENVELOPE_REQUIRED: &str = "PROVIDER_ENVELOPE_REQUIRED";
/// Any envelope or operation arrived before `op\thello` bound the connection.
pub const PROVIDER_HELLO_REQUIRED: &str = "PROVIDER_HELLO_REQUIRED";
/// Explicit success marker for `op` responses (never an empty frame).
pub const PROVIDER_OK: &str = "PROVIDER_OK";
/// Ingest is validated but unavailable without search acceptance.
pub const PROVIDER_INGEST_UNAVAILABLE: &str = "PROVIDER_INGEST_UNAVAILABLE";
/// Query is validated but unavailable without search acceptance.
pub const PROVIDER_QUERY_UNAVAILABLE: &str = "PROVIDER_QUERY_UNAVAILABLE";
/// Expansion is validated but unavailable without search acceptance.
pub const PROVIDER_EXPAND_UNAVAILABLE: &str = "PROVIDER_EXPAND_UNAVAILABLE";
/// Cancellation found no live in-flight identity (idempotent, not an error).
pub const PROVIDER_CANCEL_UNKNOWN_OR_TERMINAL: &str = "PROVIDER_CANCEL_UNKNOWN_OR_TERMINAL";
/// A negotiated-available recipe has no loopback executor bound in this shell.
pub const PROVIDER_RECIPE_NOT_BOUND: &str = "PROVIDER_RECIPE_NOT_BOUND";
/// Token file is missing, malformed or too short for the shim key.
pub const PROVIDER_TOKEN_INVALID: &str = "PROVIDER_TOKEN_INVALID";

/// Maps a protocol failure to its stable machine-readable reason code.
///
/// `PROTOCOL_*` codes are the typed provider reasons for the transport and
/// admission layers; `PROVIDER_*` codes cover the operation layer above.
#[must_use]
pub const fn protocol_reason(error: ProtocolError) -> &'static str {
    error.code()
}

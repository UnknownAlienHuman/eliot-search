//! Canonical provider-protocol routing core for the loopback daemon (T19).
//!
//! This module routes versioned `health`/`status`/`version`/`ingest`/`query`/
//! `expand`/`cancel`/`shutdown` operations through the shared validated
//! envelopes from `search-provider-protocol` inside one real daemon. It owns
//! no socket, pipe, filesystem or secret-store I/O: the transport adapter
//! (`authenticated_proxy`) supplies complete lines and the pairing key, and
//! the child service supplies terminal output.
//!
//! Layering, bottom to top:
//!
//! 1. [`FrameCodec`](search_provider_protocol::frame::FrameCodec) carries
//!    length-delimited envelope bytes; this module only moves them inside
//!    `envelope\t<seq>\t<hex>` lines.
//! 2. Version negotiation uses
//!    [`negotiate_hello`](search_provider_protocol::negotiation::negotiate_hello)
//!    over the canonical [`ProtocolRange`]; the daemon range is exactly
//!    `1.0-1.0`.
//! 3. Pairing stays owned by the endpoint ceremony. Per-connection envelope
//!    admission additionally requires the pairing key: every envelope proof
//!    is recomputed here with keyed BLAKE3 over the exact
//!    [`envelope_transcript`](search_provider_protocol::request::envelope_transcript)
//!    and compared in fixed-work time. A named-pipe ACL or loopback match
//!    alone admits nothing.
//! 4. Admission order is fixed in [`ProviderRouter::admit`]: negotiated
//!    version, then incarnation server nonce, then keyed proof, then
//!    connection sequence, then replay, then in-flight ceiling.
//! 5. Capability gating uses [`CapabilityEvidence`] derived from the T12
//!    readiness report. Unsupported recipes return explicit typed
//!    `PROVIDER_*_UNAVAILABLE` with the exact blockers, never empty success.
//!
//! The closed [`ControlCommand`](search_provider_protocol::request::ControlCommand)
//! registry currently carries only `health`/`version`/`shutdown`; those three
//! travel as authenticated envelopes. `status`/`cancel` are connection-local
//! provider operations, and `ingest`/`query`/`expand` are validated for shape
//! and bounds, then gated to explicit unavailable until an owning wave opens
//! the recipe registry (a `search-provider-protocol` contract change, not a
//! local reinterpretation).
//!
//! T19 temporary wiring: `authenticated_proxy` includes this file with
//! `#[path]` until `entry.rs` connects `mod provider_composition;`, because
//! `entry.rs` is owned by a parallel agent this turn.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use search_contracts::{ProtocolRange, ProtocolVersion, RequestId};
use search_provider_protocol::negotiation::negotiate_hello;
use search_provider_protocol::request::{
    AuthenticatedEnvelope, AuthenticatedResponse, ControlCommand, InFlightEntry, InFlightRegistry,
    MonotonicMillis, RequestGuard, RequestStatus, decode_envelope, encode_response,
    envelope_transcript, response_transcript, seal_response, verify_envelope_proof,
};
use search_provider_protocol::{
    BindingKey, CancelOutcome, DEFAULT_PROTOCOL_LIMITS, DisconnectReceipt, ProofDigest,
    ProtocolError, ProtocolLimits, SequenceTracker, ServerNonce, SessionMachine, TerminalKind,
    cancel_request, disconnect_all,
};

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

/// Domain separating provider server-nonce draws from pairing material.
const SERVER_NONCE_DOMAIN: &[u8] = b"eliot-provider-server-nonce/v1\0";
/// Domain separating the session anchor from pairing material.
const SESSION_ANCHOR_DOMAIN: &[u8] = b"eliot-provider-session/v1\0";
/// Domain separating the development token-file key from pairing material.
///
/// Must stay byte-equal to the endpoint development-compat derivation; the
/// process test proves a real CLI/daemon round trip over this derivation.
const SHIM_KEY_DOMAIN: &[u8] = b"eliot-search/loopback-dev-key/v1\0";

/// Maximum token-file bytes read for the development-compat key.
pub const MAX_TOKEN_FILE_BYTES: usize = 4096;
/// Minimum trimmed token bytes accepted for the development-compat key.
pub const MIN_TOKEN_BYTES: usize = 32;

// ---------------------------------------------------------------------------
// Closed provider operation registry.
// ---------------------------------------------------------------------------

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
    /// Protocol and build version routed as an authenticated envelope.
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

// ---------------------------------------------------------------------------
// Typed provider reasons.
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Capability negotiation from T12 readiness evidence.
// ---------------------------------------------------------------------------

/// Plain readiness evidence for capability negotiation.
///
/// Built by the daemon adapter from the T12 `ReadinessReport`
/// (`source_backed_search_available`, `search_available`,
/// `indexed_search_available`, `blockers`); tests construct it literally.
/// It carries availability flags and blocker codes only — no paths, secrets
/// or content.
#[derive(Clone, Debug)]
pub struct CapabilityEvidence {
    /// DIRECT source-backed search over verified immutable revisions.
    pub source_backed_search_available: bool,
    /// General search with an accepted receipt.
    pub search_available: bool,
    /// Indexed search with qualified artifacts and routes.
    pub indexed_search_available: bool,
    /// Closed blocker codes, at most [`MAX_BLOCKERS`].
    pub blockers: Vec<&'static str>,
}

impl CapabilityEvidence {
    /// Builds evidence with a bounded blocker list (excess is never silent:
    /// construction fails instead of truncating).
    pub fn from_parts(
        source_backed_search_available: bool,
        search_available: bool,
        indexed_search_available: bool,
        blockers: Vec<&'static str>,
    ) -> Result<Self, &'static str> {
        if blockers.len() > MAX_BLOCKERS {
            return Err("PROVIDER_EVIDENCE_TOO_LARGE");
        }
        Ok(Self {
            source_backed_search_available,
            search_available,
            indexed_search_available,
            blockers,
        })
    }
}

/// Binding-visible capability snapshot for one connection.
///
/// One boolean per closed provider operation by construction; the lint
/// allowance below is intentional (a capability matrix is a bool bag, and
/// collapsing it into bitflags would hide the per-operation mapping).
#[derive(Clone, Debug)]
#[allow(clippy::struct_excessive_bools)]
pub struct ProviderCapabilities {
    /// Local health envelope routing.
    pub health_available: bool,
    /// Local status diagnostics.
    pub status_available: bool,
    /// Local version envelope routing.
    pub version_available: bool,
    /// Local shutdown envelope routing.
    pub shutdown_available: bool,
    /// Connection-local cancellation.
    pub cancel_available: bool,
    /// Content admission (needs search acceptance).
    pub ingest_available: bool,
    /// Recipe query (needs search acceptance).
    pub query_available: bool,
    /// Handle expansion (needs search acceptance).
    pub expand_available: bool,
    /// Indexed search (needs indexed acceptance plus qualified routes).
    pub indexed_available: bool,
    /// Closed blocker codes explaining every unavailable capability.
    pub blockers: Vec<&'static str>,
}

/// Negotiates binding-visible capabilities from T12 readiness evidence.
///
/// Local shell operations are always available; recipes follow the accepted
/// receipts. Availability grants no authority — it only decides between
/// routing and explicit unavailable.
#[must_use]
pub fn negotiate_capabilities(evidence: &CapabilityEvidence) -> ProviderCapabilities {
    // Recipes require both an accepted receipt and verified source-backed
    // revisions; `search_available` already implies source-backed via T12,
    // so the extra conjunction preserves behavior while binding the evidence
    // field instead of leaving it unread.
    let recipes_available = evidence.search_available && evidence.source_backed_search_available;
    ProviderCapabilities {
        health_available: true,
        status_available: true,
        version_available: true,
        shutdown_available: true,
        cancel_available: true,
        ingest_available: recipes_available,
        query_available: recipes_available,
        expand_available: recipes_available,
        indexed_available: evidence.indexed_search_available
            && evidence.source_backed_search_available,
        blockers: evidence.blockers.clone(),
    }
}

/// Denial for a gated operation: a typed reason plus the exact blockers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderDenial {
    /// Stable `PROVIDER_*_UNAVAILABLE` reason code.
    pub reason: &'static str,
    /// Closed blocker codes from the capability evidence.
    pub blockers: Vec<&'static str>,
}

/// Gates one operation against negotiated capabilities.
///
/// Envelope-only operations pass here (their admission is envelope-driven);
/// recipes fail with explicit unavailable instead of empty success.
pub fn gate_operation(
    operation: ProviderOperation,
    capabilities: &ProviderCapabilities,
) -> Result<(), ProviderDenial> {
    let deny = |reason: &'static str| {
        Err(ProviderDenial {
            reason,
            blockers: capabilities.blockers.clone(),
        })
    };
    match operation {
        ProviderOperation::Health
        | ProviderOperation::Status
        | ProviderOperation::Version
        | ProviderOperation::Cancel
        | ProviderOperation::Shutdown => Ok(()),
        ProviderOperation::Ingest if capabilities.ingest_available => Ok(()),
        ProviderOperation::Ingest => deny(PROVIDER_INGEST_UNAVAILABLE),
        ProviderOperation::Query if capabilities.query_available => Ok(()),
        ProviderOperation::Query => deny(PROVIDER_QUERY_UNAVAILABLE),
        ProviderOperation::Expand if capabilities.expand_available => Ok(()),
        ProviderOperation::Expand => deny(PROVIDER_EXPAND_UNAVAILABLE),
    }
}

// ---------------------------------------------------------------------------
// Keyed envelope proofs (secret-owning side).
// ---------------------------------------------------------------------------

/// Recomputes the expected keyed proof over the exact envelope transcript.
///
/// The key never leaves this call; only the digest is compared, in
/// fixed-work time, by [`verify_envelope_proof`].
#[must_use]
pub fn expected_envelope_proof(key: &[u8; 32], envelope: &AuthenticatedEnvelope) -> ProofDigest {
    ProofDigest::from_bytes(*blake3::keyed_hash(key, &envelope_transcript(envelope)).as_bytes())
}

/// Verifies one envelope proof against the pairing key.
pub fn verify_envelope(
    key: &[u8; 32],
    envelope: &AuthenticatedEnvelope,
) -> Result<(), ProtocolError> {
    let expected = expected_envelope_proof(key, envelope);
    verify_envelope_proof(envelope, &expected)
}

/// Renders the exact receipt bytes bound into a response body digest.
///
/// `provider-response\t<major>.<minor>\t<request-hex>\t<status>\t<seq>`:
/// the client re-renders the same bytes from the decoded response and its
/// expected provider sequence, so a swapped identity, status or sequence
/// fails the digest check instead of aliasing another request.
#[must_use]
pub fn render_response_receipt(
    version: ProtocolVersion,
    request_id: &RequestId,
    status: RequestStatus,
    provider_sequence: u64,
) -> Vec<u8> {
    let mut receipt = Vec::with_capacity(128);
    receipt.extend_from_slice(b"provider-response\t");
    receipt.extend_from_slice(version.major.to_string().as_bytes());
    receipt.extend_from_slice(b".");
    receipt.extend_from_slice(version.minor.to_string().as_bytes());
    receipt.extend_from_slice(b"\t");
    receipt.extend_from_slice(hex_encode(request_id.as_bytes()).as_bytes());
    receipt.extend_from_slice(b"\t");
    receipt.extend_from_slice(status.as_str().as_bytes());
    receipt.extend_from_slice(b"\t");
    receipt.extend_from_slice(provider_sequence.to_string().as_bytes());
    receipt
}

/// Seals an authenticated response binding identity, status and sequence.
///
/// The body digest commits to [`render_response_receipt`]; the proof is the
/// keyed digest over the exact response transcript. Both digests are
/// computed inside the key callback window; the key itself never crosses.
#[must_use]
pub fn seal_response_with_receipt(
    key: &[u8; 32],
    version: ProtocolVersion,
    nonce: ServerNonce,
    request_id: RequestId,
    status: RequestStatus,
    provider_sequence: u64,
) -> AuthenticatedResponse {
    let receipt = render_response_receipt(version, &request_id, status, provider_sequence);
    let body_digest = ProofDigest::from_bytes(*blake3::hash(&receipt).as_bytes());
    let stub = seal_response(
        version,
        nonce,
        request_id,
        status,
        body_digest,
        ProofDigest::from_bytes([0; 32]),
    );
    let proof =
        ProofDigest::from_bytes(*blake3::keyed_hash(key, &response_transcript(&stub)).as_bytes());
    seal_response(version, nonce, request_id, status, body_digest, proof)
}

/// Derives the per-connection server nonce from the pairing key and a
/// connection-local counter (first draw wins; one bounded retry).
///
/// Every hello draws a fresh nonce, so envelopes from a previous connection
/// fail the nonce check instead of replaying across reconnects.
pub fn derive_server_nonce(
    key: &[u8; 32],
    connection_counter: u64,
) -> Result<ServerNonce, ProtocolError> {
    for counter in [connection_counter, connection_counter.wrapping_add(1)] {
        let mut input = Vec::with_capacity(SERVER_NONCE_DOMAIN.len() + 8);
        input.extend_from_slice(SERVER_NONCE_DOMAIN);
        input.extend_from_slice(&counter.to_le_bytes());
        let digest = blake3::keyed_hash(key, &input);
        let mut raw = [0_u8; 16];
        raw.copy_from_slice(&digest.as_bytes()[..16]);
        if let Ok(nonce) = ServerNonce::from_bytes(raw) {
            return Ok(nonce);
        }
    }
    Err(ProtocolError::InvalidNonce)
}

/// Derives the development-compat pairing key from trimmed token bytes.
///
/// One-way domain hash; the file bytes never cross the socket. Mirrors the
/// endpoint development shim byte-for-byte; the live round-trip test proves
/// the agreement.
pub fn shim_key_from_bytes(trimmed: &[u8]) -> Result<[u8; 32], &'static str> {
    if trimmed.len() < MIN_TOKEN_BYTES {
        return Err(PROVIDER_TOKEN_INVALID);
    }
    let mut hasher = blake3::Hasher::new();
    hasher.update(SHIM_KEY_DOMAIN);
    hasher.update(trimmed);
    let key = *hasher.finalize().as_bytes();
    if key.iter().all(|byte| *byte == 0) {
        return Err(PROVIDER_TOKEN_INVALID);
    }
    Ok(key)
}

/// Reads a token file and derives the development-compat pairing key.
///
/// Regular files only (symlinks refused), bounded size, ASCII-trimmed,
/// minimum length enforced. The raw buffer is zeroed before return.
pub fn read_shim_key_file(path: &std::path::Path) -> Result<[u8; 32], String> {
    use std::fs::File;
    use std::io::Read;
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|error| format!("{PROVIDER_TOKEN_INVALID}:{error}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(PROVIDER_TOKEN_INVALID.to_owned());
    }
    if metadata.len() > u64::try_from(MAX_TOKEN_FILE_BYTES).unwrap_or(u64::MAX) {
        return Err(PROVIDER_TOKEN_INVALID.to_owned());
    }
    let mut file = File::open(path).map_err(|error| format!("{PROVIDER_TOKEN_INVALID}:{error}"))?;
    let mut bytes = Vec::with_capacity(
        usize::try_from(metadata.len()).map_err(|_| PROVIDER_TOKEN_INVALID.to_owned())?,
    );
    (&mut file)
        .take(u64::try_from(MAX_TOKEN_FILE_BYTES + 1).unwrap_or(u64::MAX))
        .read_to_end(&mut bytes)
        .map_err(|error| format!("{PROVIDER_TOKEN_INVALID}:{error}"))?;
    if bytes.len() > MAX_TOKEN_FILE_BYTES {
        bytes.fill(0);
        return Err(PROVIDER_TOKEN_INVALID.to_owned());
    }
    let start = bytes
        .iter()
        .position(|byte| !byte.is_ascii_whitespace())
        .unwrap_or(bytes.len());
    let end = bytes
        .iter()
        .rposition(|byte| !byte.is_ascii_whitespace())
        .map_or(start, |index| index + 1);
    let key = shim_key_from_bytes(&bytes[start..end]).map_err(str::to_owned)?;
    bytes.fill(0);
    Ok(key)
}

// ---------------------------------------------------------------------------
// Line framing.
// ---------------------------------------------------------------------------

/// Parsed `op` line: operation plus its validated argument.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OpArgument {
    /// No argument (`status`).
    None,
    /// Cancellation target identity.
    CancelTarget(RequestId),
    /// Opaque validated hex argument for gated recipes.
    Blob(Vec<u8>),
}

/// Parses one `op\t...` line with strict arity and bounded hex validation.
///
/// Envelope-only operations (`health`/`version`/`shutdown`) are rejected
/// here with [`PROVIDER_ENVELOPE_REQUIRED`]: they must travel as sealed
/// envelopes, never as bare operation names.
pub fn parse_op_line(line: &str) -> Result<(ProviderOperation, OpArgument), &'static str> {
    let rest = line
        .strip_prefix(OP_LINE_PREFIX)
        .ok_or(PROVIDER_UNKNOWN_COMMAND)?;
    let (name, args) = rest.find('\t').map_or((rest, None), |index| {
        (&rest[..index], Some(&rest[index + 1..]))
    });
    let operation = ProviderOperation::parse(name)?;
    if operation.requires_envelope() {
        return Err(PROVIDER_ENVELOPE_REQUIRED);
    }
    match (operation, args) {
        (ProviderOperation::Status, None) => Ok((operation, OpArgument::None)),
        (ProviderOperation::Cancel, Some(arg)) => {
            let raw = decode_hex_exact(arg, 16).ok_or(PROVIDER_UNKNOWN_COMMAND)?;
            let mut id = [0_u8; 16];
            id.copy_from_slice(&raw);
            Ok((
                operation,
                OpArgument::CancelTarget(RequestId::from_bytes(id)),
            ))
        }
        (
            ProviderOperation::Ingest | ProviderOperation::Query | ProviderOperation::Expand,
            Some(arg),
        ) => Ok((operation, OpArgument::Blob(decode_op_hex(arg)?))),
        _ => Err(PROVIDER_UNKNOWN_COMMAND),
    }
}

/// Parses `op\thello[\t<min_major>.<min_minor>-<maj>.<min>]`.
///
/// The range form is strict digits with no whitespace; a bare `op\thello`
/// assumes the exact daemon range.
pub fn parse_hello_line(line: &str) -> Result<Option<ProtocolRange>, &'static str> {
    let rest = line
        .strip_prefix(OP_LINE_PREFIX)
        .ok_or(PROVIDER_UNKNOWN_COMMAND)?;
    let (name, args) = rest.find('\t').map_or((rest, None), |index| {
        (&rest[..index], Some(&rest[index + 1..]))
    });
    if name != "hello" {
        return Err(PROVIDER_UNKNOWN_COMMAND);
    }
    args.map(parse_client_range).transpose()
}

/// Parses a strict `MAJ.MIN-MAJ.MIN` client range with `min <= max`.
pub fn parse_client_range(text: &str) -> Result<ProtocolRange, &'static str> {
    if text.is_empty()
        || text.len() > 23
        || !text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'.' | b'-'))
    {
        return Err(PROVIDER_UNKNOWN_COMMAND);
    }
    let (min_text, max_text) = text.split_once('-').ok_or(PROVIDER_UNKNOWN_COMMAND)?;
    let parse_version = |part: &str| {
        let (major, minor) = part.split_once('.').ok_or(PROVIDER_UNKNOWN_COMMAND)?;
        if major.is_empty() || minor.is_empty() || major.len() > 5 || minor.len() > 5 {
            return Err(PROVIDER_UNKNOWN_COMMAND);
        }
        if (major.len() > 1 && major.starts_with('0'))
            || (minor.len() > 1 && minor.starts_with('0'))
        {
            return Err(PROVIDER_UNKNOWN_COMMAND);
        }
        let major = major.parse::<u16>().map_err(|_| PROVIDER_UNKNOWN_COMMAND)?;
        let minor = minor.parse::<u16>().map_err(|_| PROVIDER_UNKNOWN_COMMAND)?;
        Ok(ProtocolVersion { major, minor })
    };
    let minimum = parse_version(min_text)?;
    let maximum = parse_version(max_text)?;
    ProtocolRange::new(minimum, maximum).map_err(|_| PROVIDER_UNKNOWN_COMMAND)
}

/// Negotiates the connection version between the daemon range and the client.
pub fn negotiate_connection_version(
    client: ProtocolRange,
) -> Result<ProtocolVersion, ProtocolError> {
    negotiate_hello(PROVIDER_PROTOCOL_RANGE, client)
}

/// Parses one `envelope\t<seq>\t<hex>` line.
///
/// The client sequence is a strict decimal `u64`; the frame hex is lowercase,
/// even-length and bounded by [`MAX_ENVELOPE_HEX`].
pub fn parse_envelope_line(line: &str) -> Result<(u64, Vec<u8>), &'static str> {
    let rest = line
        .strip_prefix(ENVELOPE_LINE_PREFIX)
        .ok_or(PROVIDER_UNKNOWN_COMMAND)?;
    let (seq_text, hex) = rest.split_once('\t').ok_or(PROVIDER_UNKNOWN_COMMAND)?;
    if seq_text.is_empty()
        || seq_text.len() > 20
        || !seq_text.bytes().all(|byte| byte.is_ascii_digit())
        || (seq_text.len() > 1 && seq_text.starts_with('0'))
    {
        return Err(protocol_reason(ProtocolError::InvalidEnvelope));
    }
    let sequence = seq_text
        .parse::<u64>()
        .map_err(|_| protocol_reason(ProtocolError::InvalidEnvelope))?;
    if hex.len() > MAX_ENVELOPE_HEX {
        return Err(protocol_reason(ProtocolError::FrameTooLarge));
    }
    let frame = decode_hex(hex).ok_or(protocol_reason(ProtocolError::InvalidEnvelope))?;
    Ok((sequence, frame))
}

/// Decodes one framed envelope after prefix validation against the daemon range.
pub fn decode_envelope_frame(frame: &[u8]) -> Result<AuthenticatedEnvelope, ProtocolError> {
    decode_envelope(frame, DEFAULT_PROTOCOL_LIMITS, PROVIDER_PROTOCOL_RANGE)
}

/// Encodes one envelope response to transmittable frame bytes.
pub fn encode_response_frame(response: &AuthenticatedResponse) -> Result<Vec<u8>, ProtocolError> {
    encode_response(response, DEFAULT_PROTOCOL_LIMITS).map(|bounded| bounded.as_slice().to_vec())
}

// ---------------------------------------------------------------------------
// Child-outcome mapping.
// ---------------------------------------------------------------------------

/// Terminal child-reply class observed by the proxy exchange.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChildReply {
    /// Terminal frame fully consumed and forwarded.
    Complete,
    /// Ordinary rejection frame fully consumed and forwarded.
    Rejected,
    /// Shutdown terminal consumed; the child exited cleanly.
    Shutdown,
    /// Fatal service frame (possible effects without a success receipt).
    Fatal,
}

/// Maps a consumed child reply to its response status and terminal kind.
///
/// Fatal frames map to `outcome_unknown`: a possible mutation after dispatch
/// is never relabeled success or ordinary failure.
#[must_use]
pub const fn status_for_reply(reply: ChildReply) -> (RequestStatus, TerminalKind) {
    match reply {
        ChildReply::Complete | ChildReply::Shutdown => (RequestStatus::Ok, TerminalKind::Success),
        ChildReply::Rejected => (RequestStatus::Failed, TerminalKind::Failed),
        ChildReply::Fatal => (RequestStatus::OutcomeUnknown, TerminalKind::OutcomeUnknown),
    }
}

/// Maps an envelope command to the child tab command it dispatches.
#[must_use]
pub const fn child_command_for_envelope(command: ControlCommand) -> &'static str {
    match command {
        ControlCommand::Health => "health",
        ControlCommand::Version => "version",
        ControlCommand::Shutdown => "shutdown",
    }
}

// ---------------------------------------------------------------------------
// Rendered provider lines.
// ---------------------------------------------------------------------------

/// Renders the `provider_hello` JSON line: version, nonce, capabilities.
///
/// Bounded by [`MAX_RENDERED_LINE_BYTES`]; overlong output fails instead of
/// truncating, so capability loss is never silent.
pub fn render_hello(
    version: ProtocolVersion,
    nonce: &ServerNonce,
    capabilities: &ProviderCapabilities,
    reconnect_cancelled: usize,
) -> Result<String, &'static str> {
    let line = format!(
        concat!(
            "{{\"event\":\"provider_hello\",\"version\":\"{}.{}\",",
            "\"nonce\":\"{}\",\"reconnect_cancelled\":{},",
            "\"capabilities\":{{\"health\":{},\"status\":{},",
            "\"version\":{},\"shutdown\":{},\"cancel\":{},",
            "\"ingest\":{},\"query\":{},\"expand\":{},",
            "\"indexed\":{}}},\"blockers\":[{}]}}"
        ),
        version.major,
        version.minor,
        hex_encode(nonce.as_bytes()),
        reconnect_cancelled,
        capabilities.health_available,
        capabilities.status_available,
        capabilities.version_available,
        capabilities.shutdown_available,
        capabilities.cancel_available,
        capabilities.ingest_available,
        capabilities.query_available,
        capabilities.expand_available,
        capabilities.indexed_available,
        capabilities
            .blockers
            .iter()
            .map(|code| format!("\"{code}\""))
            .collect::<Vec<_>>()
            .join(","),
    );
    if line.len() > MAX_RENDERED_LINE_BYTES {
        return Err(protocol_reason(ProtocolError::FrameTooLarge));
    }
    Ok(line)
}

/// Outcome class for `provider_op` response lines.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OpStatus {
    /// Operation completed its declared work.
    Ok,
    /// Operation is validated but unavailable (explicit blockers attached).
    Unavailable,
    /// Operation failed before a verified success postcondition.
    Failed,
    /// Cancellation released a live in-flight identity.
    Cancelled,
    /// Cancellation found no live identity (idempotent outcome).
    UnknownOrTerminal,
}

impl OpStatus {
    /// Stable wire spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Unavailable => "unavailable",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::UnknownOrTerminal => "unknown_or_terminal",
        }
    }
}

/// Renders one `provider_op` JSON line with a typed reason and blockers.
///
/// Unavailable outcomes always carry their blockers; the line is never an
/// empty success.
pub fn render_op_response(
    operation: ProviderOperation,
    status: OpStatus,
    reason: &str,
    blockers: &[&str],
) -> Result<String, &'static str> {
    if blockers.len() > MAX_BLOCKERS || reason.len() > 256 {
        return Err(protocol_reason(ProtocolError::FrameTooLarge));
    }
    let line = format!(
        "{{\"event\":\"provider_op\",\"op\":\"{}\",\"status\":\"{}\",\"reason\":{},\"blockers\":[{}]}}",
        operation.as_str(),
        status.as_str(),
        json_string(reason),
        blockers
            .iter()
            .map(|code| json_string(code))
            .collect::<Vec<_>>()
            .join(","),
    );
    if line.len() > MAX_RENDERED_LINE_BYTES {
        return Err(protocol_reason(ProtocolError::FrameTooLarge));
    }
    Ok(line)
}

/// Renders one `provider_error` JSON line for failures without a request ID.
pub fn render_provider_error(reason: &str) -> String {
    format!(
        "{{\"event\":\"provider_error\",\"reason\":{}}}",
        json_string(&reason.chars().take(256).collect::<String>())
    )
}

// ---------------------------------------------------------------------------
// Connection router: admission order, sequencing, terminal, cancel.
// ---------------------------------------------------------------------------

/// One open provider connection: authenticated admission plus exactly-one
/// terminal per request.
///
/// Admission order is fixed in code: negotiated version, then incarnation
/// server nonce, then keyed proof, then connection sequence, then replay,
/// then in-flight ceiling. Terminal responses are emitted in admission
/// (FIFO) order; out-of-order completion fails closed. Cancellation is
/// idempotent and releases exactly one in-flight slot.
pub struct ProviderRouter {
    session: SessionMachine,
    inflight: InFlightRegistry,
    guards: BTreeMap<RequestId, RequestGuard>,
    pending: VecDeque<RequestId>,
    completed: BTreeSet<RequestId>,
    provider_sequence: SequenceTracker,
    nonce: ServerNonce,
    version: ProtocolVersion,
    limits: ProtocolLimits,
}

impl ProviderRouter {
    /// Opens one connection from the pairing key, negotiated version and a
    /// fresh server nonce.
    ///
    /// The session anchor proves key possession at open; connection
    /// authentication itself stays owned by the pairing ceremony that
    /// supplied the key. A zero key or an out-of-range version fails closed.
    pub fn open(
        key: &[u8; 32],
        version: ProtocolVersion,
        nonce: ServerNonce,
        limits: ProtocolLimits,
    ) -> Result<Self, ProtocolError> {
        let limits = limits.validate()?;
        if !PROVIDER_PROTOCOL_RANGE.contains(version) {
            return Err(ProtocolError::NoCompatibleVersion);
        }
        let binding = BindingKey::from_bytes(*key)?;
        let anchor = binding.with_bytes(|bytes| {
            let mut input = Vec::with_capacity(SESSION_ANCHOR_DOMAIN.len() + 2 + 2 + 16);
            input.extend_from_slice(SESSION_ANCHOR_DOMAIN);
            input.extend_from_slice(&version.major.to_le_bytes());
            input.extend_from_slice(&version.minor.to_le_bytes());
            input.extend_from_slice(nonce.as_bytes());
            ProofDigest::from_bytes(*blake3::keyed_hash(bytes, &input).as_bytes())
        });
        let mut session = SessionMachine::new(limits, 1, 1)?;
        session.negotiate(version)?;
        session.activate(&anchor, &anchor)?;
        Ok(Self {
            session,
            inflight: InFlightRegistry::new(limits.max_in_flight_requests)?,
            guards: BTreeMap::new(),
            pending: VecDeque::new(),
            completed: BTreeSet::new(),
            provider_sequence: SequenceTracker::new(1),
            nonce,
            version,
            limits,
        })
    }

    /// Whether the connection currently admits requests.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.session.state() == search_provider_protocol::SessionState::Active
    }

    /// Negotiated version bound to this connection.
    #[must_use]
    pub const fn version(&self) -> ProtocolVersion {
        self.version
    }

    /// Per-connection server nonce bound into every envelope proof.
    #[must_use]
    pub const fn server_nonce(&self) -> &ServerNonce {
        &self.nonce
    }

    /// Number of currently in-flight requests.
    #[must_use]
    pub fn in_flight_len(&self) -> usize {
        self.inflight.len()
    }

    /// Number of admitted requests awaiting their terminal response.
    #[must_use]
    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }

    /// Admits one authenticated envelope with the pairing key.
    ///
    /// The deadline check runs before any session state mutates, so an
    /// expired deadline leaves sequence, replay and in-flight state
    /// untouched.
    pub fn admit(
        &mut self,
        envelope: &AuthenticatedEnvelope,
        key: &[u8; 32],
        sequence: u64,
        now: MonotonicMillis,
        relative_deadline_ms: Option<u64>,
    ) -> Result<RequestGuard, ProtocolError> {
        if !self.is_active() {
            return Err(match self.session.state() {
                search_provider_protocol::SessionState::Draining => ProtocolError::SessionDraining,
                search_provider_protocol::SessionState::Closed => ProtocolError::SessionClosed,
                search_provider_protocol::SessionState::Quarantined => ProtocolError::Quarantined,
                _ => ProtocolError::AuthenticationRequired,
            });
        }
        if envelope.version() != self.version {
            return Err(ProtocolError::NoCompatibleVersion);
        }
        if envelope.server_nonce() != &self.nonce {
            return Err(ProtocolError::AuthenticationFailed);
        }
        verify_envelope(key, envelope)?;
        let guard = RequestGuard::new(
            *envelope.request_id(),
            sequence,
            now,
            relative_deadline_ms,
            self.limits,
        )?;
        if self.in_flight_len() >= self.limits.max_in_flight_requests {
            return Err(ProtocolError::ResourceExhausted);
        }
        self.session
            .admit_request(*envelope.request_id(), sequence)?;
        if self
            .inflight
            .insert(
                *envelope.request_id(),
                InFlightEntry::new(sequence, now, guard.deadline()),
            )
            .is_err()
        {
            let _ = self.session.quarantine();
            return Err(ProtocolError::Quarantined);
        }
        self.guards.insert(*envelope.request_id(), guard.clone());
        self.pending.push_back(*envelope.request_id());
        Ok(guard)
    }

    /// Idempotently cancels one request and releases its in-flight slot.
    pub fn cancel(&mut self, target: &RequestId) -> CancelOutcome {
        let outcome = cancel_request(&mut self.inflight, &mut self.guards, target);
        if matches!(outcome, CancelOutcome::Cancelled { .. }) {
            self.pending.retain(|pending| pending != target);
        }
        outcome
    }

    /// Emits the single terminal response for the oldest pending request.
    ///
    /// Returns the response status plus the assigned provider sequence. A
    /// repeated terminal fails with `DuplicateTerminal`; completing a
    /// request that is pending but not oldest fails with `SequenceGap`;
    /// completing an unknown identity fails distinctly. The in-flight slot
    /// is released exactly once per admission.
    pub fn note_terminal(
        &mut self,
        request_id: &RequestId,
        terminal: TerminalKind,
    ) -> Result<(RequestStatus, u64), ProtocolError> {
        if self.completed.contains(request_id) {
            return Err(ProtocolError::DuplicateTerminal);
        }
        if self.pending_len() == 0 {
            return Err(ProtocolError::InvalidSessionTransition);
        }
        match self.pending.front() {
            Some(oldest) if oldest == request_id => {}
            Some(_) if self.pending.contains(request_id) => {
                return Err(ProtocolError::SequenceGap);
            }
            _ => return Err(ProtocolError::InvalidSessionTransition),
        }
        let guard = self
            .guards
            .get_mut(request_id)
            .ok_or(ProtocolError::InvalidSessionTransition)?;
        guard.finish(terminal, self.limits)?;
        self.pending.pop_front();
        self.completed.insert(*request_id);
        let _ = cancel_request(&mut self.inflight, &mut self.guards, request_id);
        let sequence = self
            .provider_sequence
            .next_expected()
            .ok_or(ProtocolError::SequenceExhausted)?;
        SequenceTracker::require_accepted(self.provider_sequence.observe(sequence))?;
        Ok((RequestStatus::from_terminal(terminal), sequence))
    }

    /// Disconnects deterministically: cancels every in-flight request,
    /// releases every guard and closes the session, reporting exact counts.
    /// Never fails; afterwards the connection admits nothing.
    pub fn disconnect(&mut self) -> DisconnectReceipt {
        let receipt = disconnect_all(&mut self.inflight, &mut self.guards);
        self.pending.clear();
        let _ = self.session.begin_drain();
        let _ = self.session.close();
        receipt
    }
}

// ---------------------------------------------------------------------------
// Small pure helpers.
// ---------------------------------------------------------------------------

/// Process-local millisecond clock for admission instants.
///
/// Values are meaningful only inside this process incarnation and are never
/// serialized; the T06 child-request budget (not this clock) bounds work.
#[must_use]
pub fn monotonic_millis() -> MonotonicMillis {
    use std::sync::OnceLock;
    use std::time::Instant;
    static EPOCH: OnceLock<Instant> = OnceLock::new();
    let epoch = EPOCH.get_or_init(Instant::now);
    MonotonicMillis::new(u64::try_from(epoch.elapsed().as_millis()).unwrap_or(u64::MAX))
}

fn decode_op_hex(arg: &str) -> Result<Vec<u8>, &'static str> {
    if arg.len() > MAX_OP_ARG_HEX {
        return Err(protocol_reason(ProtocolError::FrameTooLarge));
    }
    decode_hex(arg).ok_or(PROVIDER_UNKNOWN_COMMAND)
}

fn decode_hex_exact(arg: &str, expected_bytes: usize) -> Option<Vec<u8>> {
    if arg.len() != expected_bytes * 2 {
        return None;
    }
    decode_hex(arg)
}

fn decode_hex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) || text.is_empty() {
        return None;
    }
    let mut output = Vec::with_capacity(text.len() / 2);
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        let high = hex_value(bytes[index])?;
        let low = hex_value(bytes[index + 1])?;
        output.push((high << 4) | low);
        index += 2;
    }
    Some(output)
}

const fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

/// Lowercase hex encoding for non-secret framing bytes.
#[must_use]
pub fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0F)]));
    }
    output
}

fn json_string(value: &str) -> String {
    let mut output = String::with_capacity(value.len().saturating_add(2));
    output.push('"');
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character if character.is_control() => {
                use std::fmt::Write as _;
                let _ = write!(&mut output, "\\u{:04x}", u32::from(character));
            }
            character => output.push(character),
        }
    }
    output.push('"');
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use search_provider_protocol::request::{
        ControlCommand as Command, MonotonicMillis as At, seal_envelope,
    };

    const KEY: [u8; 32] = [0x2A; 32];
    const OTHER_KEY: [u8; 32] = [0x3B; 32];

    fn nonce(counter: u64) -> ServerNonce {
        derive_server_nonce(&KEY, counter).expect("nonce")
    }

    fn envelope(command: Command, request: [u8; 16], nonce: &ServerNonce) -> AuthenticatedEnvelope {
        let digest = ProofDigest::from_bytes([0x33; 32]);
        let stub = seal_envelope(
            PROVIDER_PROTOCOL_VERSION,
            *nonce,
            RequestId::from_bytes(request),
            command,
            digest,
            ProofDigest::from_bytes([0; 32]),
        );
        let proof = ProofDigest::from_bytes(
            *blake3::keyed_hash(&KEY, &envelope_transcript(&stub)).as_bytes(),
        );
        seal_envelope(
            PROVIDER_PROTOCOL_VERSION,
            *nonce,
            RequestId::from_bytes(request),
            command,
            digest,
            proof,
        )
    }

    fn router_with_nonce(counter: u64) -> (ProviderRouter, ServerNonce) {
        let nonce = nonce(counter);
        let router = ProviderRouter::open(
            &KEY,
            PROVIDER_PROTOCOL_VERSION,
            nonce,
            DEFAULT_PROTOCOL_LIMITS,
        )
        .expect("open");
        (router, nonce)
    }

    #[test]
    fn registries_are_closed() {
        assert_eq!(ProviderOperation::ALL.len(), 8);
        assert_eq!(
            ProviderOperation::parse("reboot"),
            Err(PROVIDER_UNKNOWN_COMMAND)
        );
        for operation in ProviderOperation::ALL {
            assert_eq!(ProviderOperation::parse(operation.as_str()), Ok(*operation));
        }
        assert!(ProviderOperation::Health.requires_envelope());
        assert!(ProviderOperation::Shutdown.requires_envelope());
        assert!(!ProviderOperation::Status.requires_envelope());
        assert!(!ProviderOperation::Query.requires_envelope());
    }

    #[test]
    fn hello_negotiates_exact_version_and_rejects_major_mismatch() {
        let range = ProtocolRange::new(PROVIDER_PROTOCOL_VERSION, PROVIDER_PROTOCOL_VERSION)
            .expect("range");
        assert_eq!(
            negotiate_connection_version(range),
            Ok(PROVIDER_PROTOCOL_VERSION)
        );
        let foreign = ProtocolRange::new(
            ProtocolVersion { major: 2, minor: 0 },
            ProtocolVersion { major: 2, minor: 0 },
        )
        .expect("range");
        assert_eq!(
            negotiate_connection_version(foreign),
            Err(ProtocolError::NoCompatibleVersion)
        );
        assert_eq!(
            parse_client_range("1.0-1.0").expect("range"),
            PROVIDER_PROTOCOL_RANGE
        );
        assert!(parse_client_range("2.0-2.0").is_ok());
        assert!(parse_client_range("1.1-1.0").is_err());
        assert!(parse_client_range("hello").is_err());
        assert!(parse_client_range("01.0-1.0").is_err());
    }

    #[test]
    fn open_rejects_zero_key_and_foreign_version() {
        assert!(matches!(
            ProviderRouter::open(
                &[0; 32],
                PROVIDER_PROTOCOL_VERSION,
                nonce(1),
                DEFAULT_PROTOCOL_LIMITS,
            ),
            Err(ProtocolError::InvalidBindingKey)
        ));
        assert!(matches!(
            ProviderRouter::open(
                &KEY,
                ProtocolVersion { major: 2, minor: 0 },
                ServerNonce::from_bytes([1; 16]).expect("nonce"),
                DEFAULT_PROTOCOL_LIMITS,
            ),
            Err(ProtocolError::NoCompatibleVersion)
        ));
    }

    #[test]
    fn admit_checks_version_nonce_proof_sequence_replay_and_ceiling() {
        let (mut router, nonce) = router_with_nonce(11);
        let now = At::new(1000);
        // Happy path.
        let first = envelope(Command::Health, [1; 16], &nonce);
        router.admit(&first, &KEY, 1, now, None).expect("admit");
        assert_eq!(router.in_flight_len(), 1);
        // Wrong key fails the proof, touching no session state.
        let second = envelope(Command::Health, [2; 16], &nonce);
        assert_eq!(
            router
                .admit(&second, &OTHER_KEY, 2, now, None)
                .expect_err("proof"),
            ProtocolError::AuthenticationFailed
        );
        // Wrong nonce fails before sequence or replay state moves.
        let mut foreign_nonce = *nonce.as_bytes();
        foreign_nonce[0] ^= 0xFF;
        let foreign = ServerNonce::from_bytes(foreign_nonce).expect("nonce");
        let third = envelope(Command::Health, [3; 16], &foreign);
        assert_eq!(
            router.admit(&third, &KEY, 2, now, None).expect_err("nonce"),
            ProtocolError::AuthenticationFailed
        );
        // Replay of an admitted identity fails even with a fresh sequence.
        assert_eq!(
            router
                .admit(&first, &KEY, 2, now, None)
                .expect_err("replay"),
            ProtocolError::ReplayDetected
        );
        // Sequence gap fails distinctly. Note the composed ordering owned
        // by `SessionMachine`: the replay attempt above already consumed
        // sequence 2 before its replay verdict, so history is [1, 2].
        let fourth = envelope(Command::Version, [4; 16], &nonce);
        assert_eq!(
            router.admit(&fourth, &KEY, 9, now, None).expect_err("gap"),
            ProtocolError::SequenceGap
        );
        // Repeating the last accepted sequence duplicates, even for a fresh
        // identity: sequence is checked before replay.
        let fifth = envelope(Command::Version, [5; 16], &nonce);
        assert_eq!(
            router
                .admit(&fifth, &KEY, 2, now, None)
                .expect_err("duplicate"),
            ProtocolError::DuplicateSequence
        );
        // Falling behind accepted history regresses distinctly.
        assert_eq!(
            router
                .admit(&fourth, &KEY, 1, now, None)
                .expect_err("regression"),
            ProtocolError::SequenceRegression
        );
        // Expired relative deadline fails before mutation.
        assert_eq!(
            router
                .admit(&fourth, &KEY, 2, now, Some(0))
                .expect_err("deadline"),
            ProtocolError::DeadlineExpired
        );
        assert_eq!(router.in_flight_len(), 1);
    }

    #[test]
    fn terminal_is_unique_ordered_and_releases_exactly_once() {
        let (mut router, nonce) = router_with_nonce(21);
        let now = At::new(50);
        let first = envelope(Command::Health, [0xA1; 16], &nonce);
        let second = envelope(Command::Version, [0xA2; 16], &nonce);
        router.admit(&first, &KEY, 1, now, None).expect("first");
        router.admit(&second, &KEY, 2, now, None).expect("second");
        // Out-of-order completion fails closed without releasing anything.
        assert_eq!(
            router.note_terminal(second.request_id(), TerminalKind::Success),
            Err(ProtocolError::SequenceGap)
        );
        assert_eq!(router.in_flight_len(), 2);
        // Oldest-first completion assigns provider sequences 1, 2.
        assert_eq!(
            router.note_terminal(first.request_id(), TerminalKind::Success),
            Ok((RequestStatus::Ok, 1))
        );
        assert_eq!(
            router.note_terminal(second.request_id(), TerminalKind::Failed),
            Ok((RequestStatus::Failed, 2))
        );
        assert_eq!(router.in_flight_len(), 0);
        // A repeated terminal is rejected, never double-released.
        assert_eq!(
            router.note_terminal(first.request_id(), TerminalKind::Success),
            Err(ProtocolError::DuplicateTerminal)
        );
        // An unknown identity is rejected distinctly.
        assert_eq!(
            router.note_terminal(&RequestId::from_bytes([0xFF; 16]), TerminalKind::Failed),
            Err(ProtocolError::InvalidSessionTransition)
        );
    }

    #[test]
    fn in_flight_ceiling_fails_closed_at_canonical_bound() {
        let mut router = ProviderRouter::open(
            &KEY,
            PROVIDER_PROTOCOL_VERSION,
            nonce(31),
            ProtocolLimits {
                max_in_flight_requests: 2,
                ..DEFAULT_PROTOCOL_LIMITS
            },
        )
        .expect("open");
        let now = At::new(7);
        let nonce = *router.server_nonce();
        for (index, sequence) in [(0xB1_u8, 1_u64), (0xB2, 2)] {
            let env = envelope(Command::Health, [index; 16], &nonce);
            router
                .admit(&env, &KEY, sequence, now, None)
                .expect("admit");
        }
        let overflow = envelope(Command::Health, [0xB3; 16], &nonce);
        assert_eq!(
            router
                .admit(&overflow, &KEY, 3, now, None)
                .expect_err("ceiling"),
            ProtocolError::ResourceExhausted
        );
    }

    #[test]
    fn cancel_is_idempotent_and_releases_once() {
        let (mut router, nonce) = router_with_nonce(41);
        let now = At::new(9);
        let env = envelope(Command::Health, [0xC1; 16], &nonce);
        router.admit(&env, &KEY, 1, now, None).expect("admit");
        assert!(matches!(
            router.cancel(env.request_id()),
            CancelOutcome::Cancelled { terminal: false }
        ));
        assert_eq!(router.in_flight_len(), 0);
        assert_eq!(
            router.cancel(env.request_id()),
            CancelOutcome::UnknownOrTerminal
        );
        assert_eq!(
            router.cancel(&RequestId::from_bytes([9; 16])),
            CancelOutcome::UnknownOrTerminal
        );
    }

    #[test]
    fn capabilities_gate_recipes_but_never_the_shell() {
        let shell = CapabilityEvidence::from_parts(
            false,
            false,
            false,
            vec!["SEARCH_NOT_ACCEPTED", "INDEXED_NOT_ACCEPTED"],
        )
        .expect("shell evidence");
        let caps = negotiate_capabilities(&shell);
        assert!(caps.health_available);
        assert!(caps.status_available);
        assert!(caps.cancel_available);
        assert!(!caps.query_available);
        assert!(!caps.ingest_available);
        assert!(!caps.expand_available);
        for operation in [
            ProviderOperation::Health,
            ProviderOperation::Status,
            ProviderOperation::Version,
            ProviderOperation::Cancel,
            ProviderOperation::Shutdown,
        ] {
            assert_eq!(gate_operation(operation, &caps), Ok(()));
        }
        for (operation, reason) in [
            (ProviderOperation::Ingest, PROVIDER_INGEST_UNAVAILABLE),
            (ProviderOperation::Query, PROVIDER_QUERY_UNAVAILABLE),
            (ProviderOperation::Expand, PROVIDER_EXPAND_UNAVAILABLE),
        ] {
            let denial = gate_operation(operation, &caps).expect_err("gated");
            assert_eq!(denial.reason, reason);
            assert!(!denial.blockers.is_empty());
        }
        // Accepted search evidence opens the recipes.
        let open = CapabilityEvidence::from_parts(true, true, false, vec![]).expect("evidence");
        let caps = negotiate_capabilities(&open);
        assert!(caps.query_available);
        assert_eq!(gate_operation(ProviderOperation::Query, &caps), Ok(()));
    }

    #[test]
    fn response_seal_binds_receipt_without_relabeling() {
        let nonce = nonce(51);
        let id = RequestId::from_bytes([0xD1; 16]);
        let response = seal_response_with_receipt(
            &KEY,
            PROVIDER_PROTOCOL_VERSION,
            nonce,
            id,
            RequestStatus::Ok,
            4,
        );
        assert_eq!(response.version(), PROVIDER_PROTOCOL_VERSION);
        assert_eq!(*response.request_id(), id);
        assert_eq!(response.status(), RequestStatus::Ok);
        // Receipt binding: a different provider sequence renders a different
        // body digest, so a swapped sequence fails instead of aliasing.
        let tampered_receipt =
            render_response_receipt(PROVIDER_PROTOCOL_VERSION, &id, RequestStatus::Ok, 5);
        let tampered_digest = ProofDigest::from_bytes(*blake3::hash(&tampered_receipt).as_bytes());
        assert_ne!(*response.body_digest(), tampered_digest);
        // Proof binding: a different key seals a different proof over the
        // same transcript, so a wrong key fails instead of verifying.
        let other = seal_response_with_receipt(
            &OTHER_KEY,
            PROVIDER_PROTOCOL_VERSION,
            nonce,
            id,
            RequestStatus::Ok,
            4,
        );
        assert_ne!(response.proof(), other.proof());
        // Frame round trip via the live response encoder plus the protocol
        // decoder preserves the sealed response.
        let frame = encode_response_frame(&response).expect("encode");
        let decoded = search_provider_protocol::decode_response(
            frame.as_slice(),
            DEFAULT_PROTOCOL_LIMITS,
            PROVIDER_PROTOCOL_RANGE,
        )
        .expect("decode");
        assert_eq!(decoded, response);
    }

    #[test]
    fn envelope_frames_round_trip_with_prefix_validation() {
        let nonce = nonce(61);
        let sealed = envelope(Command::Shutdown, [0xE1; 16], &nonce);
        let frame =
            search_provider_protocol::request::encode_envelope(&sealed, DEFAULT_PROTOCOL_LIMITS)
                .expect("encode");
        let frame = frame.as_slice().to_vec();
        assert_eq!(
            usize::try_from(u32::from_le_bytes(frame[..4].try_into().expect("prefix")))
                .expect("prefix")
                + 4,
            frame.len()
        );
        assert_eq!(decode_envelope_frame(&frame).expect("decode"), sealed);
        // Truncation and version tampering fail closed.
        assert!(decode_envelope_frame(&frame[..frame.len() - 1]).is_err());
        let mut oversize = vec![0xFF; DEFAULT_PROTOCOL_LIMITS.max_frame_bytes + 1];
        oversize[0..4].copy_from_slice(&10_u32.to_le_bytes());
        assert_eq!(
            decode_envelope_frame(&oversize),
            Err(ProtocolError::FrameTooLarge)
        );
    }

    #[test]
    fn line_parsing_is_strict_and_bounded() {
        assert!(parse_op_line("health").is_err());
        assert_eq!(parse_op_line("op\thealth"), Err(PROVIDER_ENVELOPE_REQUIRED));
        assert!(parse_op_line("op\tstatus").is_ok());
        assert!(parse_op_line("op\tstatus\textra").is_err());
        assert!(parse_op_line("op\treboot").is_err());
        assert!(parse_op_line("op\tcancel").is_err());
        let (op, arg) =
            parse_op_line("op\tcancel\t00112233445566778899aabbccddeeff").expect("cancel");
        assert_eq!(op, ProviderOperation::Cancel);
        assert!(matches!(arg, OpArgument::CancelTarget(_)));
        assert!(parse_op_line("op\tcancel\tZZ").is_err());
        assert!(parse_op_line("op\tquery").is_err());
        assert!(parse_op_line("op\tquery\tABCD").is_err());
        assert!(parse_op_line("op\tquery\tab").is_ok());
        let huge = format!("op\tquery\t{}", "ab".repeat(MAX_OP_ARG_HEX));
        assert_eq!(
            parse_op_line(&huge).expect_err("bounded"),
            protocol_reason(ProtocolError::FrameTooLarge)
        );
        assert_eq!(parse_hello_line("op\thello"), Ok(None));
        assert!(parse_hello_line("op\thello\t1.0-1.0").is_ok());
        assert!(parse_hello_line("op\thello\tnope").is_err());
        assert!(parse_hello_line("op\tstatus").is_err());
        let (sequence, _) = parse_envelope_line("envelope\t12\tabcd").expect("line");
        assert_eq!(sequence, 12);
        assert!(parse_envelope_line("envelope\tabcd").is_err());
        assert!(parse_envelope_line("envelope\t01\tabcd").is_err());
        assert!(parse_envelope_line("envelope\t1\tAB").is_err());
    }

    #[test]
    fn nonce_draws_are_fresh() {
        let first = derive_server_nonce(&KEY, 1).expect("nonce");
        let second = derive_server_nonce(&KEY, 2).expect("nonce");
        assert_ne!(first, second);
        // Request-ID minting lives in the client (`bins/eliot-search`);
        // the daemon never mints client identities.
    }

    #[test]
    fn child_reply_mapping_never_relabels_the_unknown() {
        assert_eq!(
            status_for_reply(ChildReply::Complete),
            (RequestStatus::Ok, TerminalKind::Success)
        );
        assert_eq!(
            status_for_reply(ChildReply::Rejected),
            (RequestStatus::Failed, TerminalKind::Failed)
        );
        assert_eq!(
            status_for_reply(ChildReply::Shutdown),
            (RequestStatus::Ok, TerminalKind::Success)
        );
        assert_eq!(
            status_for_reply(ChildReply::Fatal),
            (RequestStatus::OutcomeUnknown, TerminalKind::OutcomeUnknown)
        );
        assert_eq!(child_command_for_envelope(Command::Health), "health");
        assert_eq!(child_command_for_envelope(Command::Shutdown), "shutdown");
    }

    #[test]
    fn rendered_lines_are_bounded_json_with_blockers() {
        let shell = CapabilityEvidence::from_parts(
            false,
            false,
            false,
            vec!["SEARCH_NOT_ACCEPTED", "INDEXED_NOT_ACCEPTED"],
        )
        .expect("shell evidence");
        let caps = negotiate_capabilities(&shell);
        let hello = render_hello(PROVIDER_PROTOCOL_VERSION, &nonce(71), &caps, 0).expect("hello");
        assert!(hello.contains("\"event\":\"provider_hello\""));
        assert!(hello.contains("\"version\":\"1.0\""));
        assert!(hello.contains("SEARCH_NOT_ACCEPTED"));
        let denied = gate_operation(ProviderOperation::Query, &caps).expect_err("denied");
        let line = render_op_response(
            ProviderOperation::Query,
            OpStatus::Unavailable,
            denied.reason,
            &denied.blockers,
        )
        .expect("render");
        assert!(line.contains(PROVIDER_QUERY_UNAVAILABLE));
        assert!(line.contains("\"status\":\"unavailable\""));
        assert!(!line.contains("\"status\":\"ok\""));
    }

    #[test]
    fn shim_key_derivation_rejects_short_and_empty_material() {
        assert!(shim_key_from_bytes(&[]).is_err());
        assert!(shim_key_from_bytes(&[0x41; 31]).is_err());
        let first = shim_key_from_bytes(&[0x41; 32]).expect("key");
        let second = shim_key_from_bytes(&[0x41; 32]).expect("key");
        assert_eq!(first, second);
        assert_ne!(first, shim_key_from_bytes(&[0x42; 32]).expect("key"));
        assert_ne!(first, [0; 32]);
    }

    #[test]
    fn disconnect_reports_exact_counts_and_closes() {
        let (mut router, nonce) = router_with_nonce(81);
        let now = At::new(3);
        for index in [0xF1_u8, 0xF2] {
            let env = envelope(Command::Health, [index; 16], &nonce);
            router
                .admit(&env, &KEY, u64::from(index - 0xF0), now, None)
                .expect("admit");
        }
        let receipt = router.disconnect();
        assert_eq!(receipt.cancelled_requests(), 2);
        assert_eq!(receipt.released_guards(), 2);
        assert!(!router.is_active());
        let env = envelope(Command::Health, [0xF3; 16], &nonce);
        assert!(router.admit(&env, &KEY, 3, now, None).is_err());
        let again = router.disconnect();
        assert_eq!(again.cancelled_requests(), 0);
    }
}

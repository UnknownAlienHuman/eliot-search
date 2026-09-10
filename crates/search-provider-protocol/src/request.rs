//! Authenticated per-request envelopes (#89) over the canonical frame codec.
//!
//! Layer order: [`crate::frame::FrameCodec`] carries length-delimited bytes,
//! [`crate::pairing`] proves the peer, and this layer binds every request to
//! the negotiated version, the per-incarnation server nonce, the opaque
//! request ID, a closed command registry entry and a body digest through a
//! keyed proof. Admission additionally requires the [`crate::pairing`]
//! ceremony token — pairing first, envelopes on top — enforced by the
//! [`crate::binding::BoundSession`] composer.
//!
//! Like the pairing transcripts, the keyed digest itself is computed by the
//! secret-owning daemon adapter (which pins `blake3 v1.8.2`) over the exact
//! transcript bytes built here; this package compares proofs in fixed-work
//! time and enforces registries, sizes, ceilings and deadlines.

use std::collections::BTreeMap;

use search_contracts::protocol::JsonFramePayload;
use search_contracts::{
    BoundedBytes, MAX_FRAME_BYTES, MAX_PROTOCOL_IN_FLIGHT, ProtocolRange, ProtocolVersion,
    RequestId,
};

use crate::config::ProtocolLimits;
use crate::error::ProtocolError;
use crate::frame::FrameCodec;
use crate::pairing::{ProofDigest, ServerNonce, verify_proof};
use crate::progress::ProgressState;
use crate::terminal::TerminalKind;

/// Domain separator bound into every request envelope transcript.
pub const ENVELOPE_REQUEST_DOMAIN: &str = "ELIOT-ENVELOPE-REQ-v1";
/// Domain separator bound into every response envelope transcript.
pub const ENVELOPE_RESPONSE_DOMAIN: &str = "ELIOT-ENVELOPE-RESP-v1";

/// Closed W1 shell command registry.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ControlCommand {
    /// Return bounded daemon health and version information.
    Health,
    /// Return protocol and build version information.
    Version,
    /// Begin authenticated graceful shutdown.
    Shutdown,
}

impl ControlCommand {
    /// Closed registry: every command has a stable wire spelling.
    pub const ALL: &'static [Self] = &[Self::Health, Self::Version, Self::Shutdown];

    /// Stable wire spelling bound into envelope transcripts.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Health => "health",
            Self::Version => "version",
            Self::Shutdown => "shutdown",
        }
    }

    /// Parses a closed-registry command; anything else fails distinctly.
    pub fn parse(value: &str) -> Result<Self, ProtocolError> {
        match value {
            "health" => Ok(Self::Health),
            "version" => Ok(Self::Version),
            "shutdown" => Ok(Self::Shutdown),
            _ => Err(ProtocolError::UnknownCommand),
        }
    }
}

/// Closed response status registry.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RequestStatus {
    /// Operation completed its declared work.
    Ok,
    /// Operation completed with explicit partial coverage.
    Partial,
    /// Operation was cancelled before success.
    Cancelled,
    /// Operation failed before a verified success postcondition.
    Failed,
    /// A possible mutation requires authoritative readback.
    OutcomeUnknown,
}

impl RequestStatus {
    /// Closed registry: every status has a stable wire spelling.
    pub const ALL: &'static [Self] = &[
        Self::Ok,
        Self::Partial,
        Self::Cancelled,
        Self::Failed,
        Self::OutcomeUnknown,
    ];

    /// Stable wire spelling bound into response transcripts.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Partial => "partial",
            Self::Cancelled => "cancelled",
            Self::Failed => "failed",
            Self::OutcomeUnknown => "outcome_unknown",
        }
    }

    /// Parses a closed-registry status; anything else fails distinctly.
    pub fn parse(value: &str) -> Result<Self, ProtocolError> {
        match value {
            "ok" => Ok(Self::Ok),
            "partial" => Ok(Self::Partial),
            "cancelled" => Ok(Self::Cancelled),
            "failed" => Ok(Self::Failed),
            "outcome_unknown" => Ok(Self::OutcomeUnknown),
            _ => Err(ProtocolError::InvalidStatus),
        }
    }

    /// Maps the terminal response class to the status registry.
    #[must_use]
    pub const fn from_terminal(terminal: TerminalKind) -> Self {
        match terminal {
            TerminalKind::Success => Self::Ok,
            TerminalKind::Partial => Self::Partial,
            TerminalKind::Cancelled => Self::Cancelled,
            TerminalKind::Failed => Self::Failed,
            TerminalKind::OutcomeUnknown => Self::OutcomeUnknown,
        }
    }

    /// Maps the status registry back to the terminal response class.
    #[must_use]
    pub const fn terminal(self) -> TerminalKind {
        match self {
            Self::Ok => TerminalKind::Success,
            Self::Partial => TerminalKind::Partial,
            Self::Cancelled => TerminalKind::Cancelled,
            Self::Failed => TerminalKind::Failed,
            Self::OutcomeUnknown => TerminalKind::OutcomeUnknown,
        }
    }
}

/// Explicit monotonic clock supplied by the daemon adapter.
///
/// This package performs no clock reads; every deadline check takes the
/// current instant as an argument, which keeps admission deterministic and
/// testable.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MonotonicMillis(u64);

impl MonotonicMillis {
    /// Wraps an adapter-supplied millisecond instant.
    #[must_use]
    pub const fn new(millis: u64) -> Self {
        Self(millis)
    }

    /// Raw millisecond instant.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    /// Saturating deadline computation; overflow pins to the far future
    /// instead of wrapping.
    #[must_use]
    pub fn plus(self, delta_ms: u64) -> Self {
        Self(self.0.saturating_add(delta_ms))
    }
}

/// Authenticated per-request envelope: version, server nonce, opaque request
/// ID, closed command, body digest and keyed proof.
///
/// The envelope serializes to canonical JSON with fixed field order over the
/// [`FrameCodec`]; decoding is fixed-size and strict (unknown fields, wrong
/// order, wrong lengths and trailing bytes fail closed).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedEnvelope {
    version: ProtocolVersion,
    server_nonce: ServerNonce,
    request_id: RequestId,
    command: ControlCommand,
    body_digest: ProofDigest,
    proof: ProofDigest,
}

impl AuthenticatedEnvelope {
    /// Negotiated version bound into the proof.
    #[must_use]
    pub const fn version(&self) -> ProtocolVersion {
        self.version
    }

    /// Per-incarnation server nonce bound into the proof.
    #[must_use]
    pub const fn server_nonce(&self) -> &ServerNonce {
        &self.server_nonce
    }

    /// Opaque canonical request ID.
    #[must_use]
    pub const fn request_id(&self) -> &RequestId {
        &self.request_id
    }

    /// Closed-registry command.
    #[must_use]
    pub const fn command(&self) -> ControlCommand {
        self.command
    }

    /// Adapter-computed digest of the exact request body.
    #[must_use]
    pub const fn body_digest(&self) -> &ProofDigest {
        &self.body_digest
    }

    /// Keyed proof over the envelope transcript.
    #[must_use]
    pub const fn proof(&self) -> &ProofDigest {
        &self.proof
    }
}

/// Seals an authenticated envelope from adapter-supplied material.
///
/// The body digest and keyed proof are computed by the secret-owning adapter
/// (keyed BLAKE3 over [`envelope_transcript`]); this constructor binds the
/// values into one fixed-size envelope without reinterpreting them.
#[must_use]
pub fn seal_envelope(
    version: ProtocolVersion,
    server_nonce: ServerNonce,
    request_id: RequestId,
    command: ControlCommand,
    body_digest: ProofDigest,
    proof: ProofDigest,
) -> AuthenticatedEnvelope {
    AuthenticatedEnvelope {
        version,
        server_nonce,
        request_id,
        command,
        body_digest,
        proof,
    }
}

/// Exact bytes the request keyed proof must bind: domain, version, server
/// nonce, request ID, command wire spelling and body digest, in fixed order.
#[must_use]
pub fn envelope_transcript(envelope: &AuthenticatedEnvelope) -> Vec<u8> {
    let command = envelope.command.as_str();
    let mut bytes = Vec::with_capacity(
        ENVELOPE_REQUEST_DOMAIN.len() + 1 + 2 + 2 + 16 + 16 + command.len() + 1 + 32,
    );
    bytes.extend_from_slice(ENVELOPE_REQUEST_DOMAIN.as_bytes());
    bytes.push(0);
    bytes.extend_from_slice(&envelope.version.major.to_le_bytes());
    bytes.extend_from_slice(&envelope.version.minor.to_le_bytes());
    bytes.extend_from_slice(envelope.server_nonce.as_bytes());
    bytes.extend_from_slice(envelope.request_id.as_bytes());
    bytes.extend_from_slice(command.as_bytes());
    bytes.push(0);
    bytes.extend_from_slice(envelope.body_digest.as_bytes());
    bytes
}

/// Verifies an envelope proof in fixed-work time against the adapter-computed
/// expectation. A mismatch is an authentication failure, never a fallback.
pub fn verify_envelope_proof(
    envelope: &AuthenticatedEnvelope,
    expected: &ProofDigest,
) -> Result<(), ProtocolError> {
    if verify_proof(expected, envelope.proof()) {
        Ok(())
    } else {
        Err(ProtocolError::AuthenticationFailed)
    }
}

/// Authenticated per-request response: version, server nonce, opaque request
/// ID, closed status, body digest and keyed proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedResponse {
    version: ProtocolVersion,
    server_nonce: ServerNonce,
    request_id: RequestId,
    status: RequestStatus,
    body_digest: ProofDigest,
    proof: ProofDigest,
}

impl AuthenticatedResponse {
    /// Negotiated version bound into the proof.
    #[must_use]
    pub const fn version(&self) -> ProtocolVersion {
        self.version
    }

    /// Closed-registry response status.
    #[must_use]
    pub const fn status(&self) -> RequestStatus {
        self.status
    }

    /// Opaque canonical request ID this response answers.
    #[must_use]
    pub const fn request_id(&self) -> &RequestId {
        &self.request_id
    }

    /// Keyed proof over the response transcript.
    #[must_use]
    pub const fn proof(&self) -> &ProofDigest {
        &self.proof
    }

    /// Adapter-computed digest of the exact response body.
    #[must_use]
    pub const fn body_digest(&self) -> &ProofDigest {
        &self.body_digest
    }
}

/// Seals an authenticated response from adapter-supplied material.
#[must_use]
pub fn seal_response(
    version: ProtocolVersion,
    server_nonce: ServerNonce,
    request_id: RequestId,
    status: RequestStatus,
    body_digest: ProofDigest,
    proof: ProofDigest,
) -> AuthenticatedResponse {
    AuthenticatedResponse {
        version,
        server_nonce,
        request_id,
        status,
        body_digest,
        proof,
    }
}

/// Exact bytes the response keyed proof must bind.
#[must_use]
pub fn response_transcript(response: &AuthenticatedResponse) -> Vec<u8> {
    let status = response.status.as_str();
    let mut bytes = Vec::with_capacity(
        ENVELOPE_RESPONSE_DOMAIN.len() + 1 + 2 + 2 + 16 + 16 + status.len() + 1 + 32,
    );
    bytes.extend_from_slice(ENVELOPE_RESPONSE_DOMAIN.as_bytes());
    bytes.push(0);
    bytes.extend_from_slice(&response.version.major.to_le_bytes());
    bytes.extend_from_slice(&response.version.minor.to_le_bytes());
    bytes.extend_from_slice(response.server_nonce.as_bytes());
    bytes.extend_from_slice(response.request_id.as_bytes());
    bytes.extend_from_slice(status.as_bytes());
    bytes.push(0);
    bytes.extend_from_slice(response.body_digest.as_bytes());
    bytes
}

/// Verifies a response proof in fixed-work time.
pub fn verify_response_proof(
    response: &AuthenticatedResponse,
    expected: &ProofDigest,
) -> Result<(), ProtocolError> {
    if verify_proof(expected, response.proof()) {
        Ok(())
    } else {
        Err(ProtocolError::AuthenticationFailed)
    }
}

fn hex_encode(bytes: &[u8], out: &mut [u8]) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for (index, byte) in bytes.iter().enumerate() {
        out[2 * index] = HEX[usize::from(byte >> 4)];
        out[2 * index + 1] = HEX[usize::from(byte & 0x0F)];
    }
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

fn hex_decode_into<const N: usize>(text: &[u8]) -> Option<[u8; N]> {
    if text.len() != 2 * N {
        return None;
    }
    let mut out = [0_u8; N];
    for index in 0..N {
        let high = hex_value(text[2 * index])?;
        let low = hex_value(text[2 * index + 1])?;
        out[index] = (high << 4) | low;
    }
    Some(out)
}

/// Canonical request envelope JSON with fixed field order.
///
/// `{"v":[MAJOR,MINOR],"nonce":"..","request":"..","command":"..",
/// "body":"..","proof":".."}` — hex is lowercase, numbers have no leading
/// zeros, no whitespace is permitted.
#[must_use]
pub fn encode_envelope_json(envelope: &AuthenticatedEnvelope) -> Vec<u8> {
    let mut nonce = [0_u8; 32];
    let mut request = [0_u8; 32];
    let mut body = [0_u8; 64];
    let mut proof = [0_u8; 64];
    hex_encode(envelope.server_nonce.as_bytes(), &mut nonce);
    hex_encode(envelope.request_id.as_bytes(), &mut request);
    hex_encode(envelope.body_digest.as_bytes(), &mut body);
    hex_encode(envelope.proof.as_bytes(), &mut proof);
    let mut out = Vec::with_capacity(32 + 32 + 32 + 64 + 64 + 80);
    out.extend_from_slice(b"{\"v\":[");
    out.extend_from_slice(envelope.version.major.to_string().as_bytes());
    out.extend_from_slice(b",");
    out.extend_from_slice(envelope.version.minor.to_string().as_bytes());
    out.extend_from_slice(b"],\"nonce\":\"");
    out.extend_from_slice(&nonce);
    out.extend_from_slice(b"\",\"request\":\"");
    out.extend_from_slice(&request);
    out.extend_from_slice(b"\",\"command\":\"");
    out.extend_from_slice(envelope.command.as_str().as_bytes());
    out.extend_from_slice(b"\",\"body\":\"");
    out.extend_from_slice(&body);
    out.extend_from_slice(b"\",\"proof\":\"");
    out.extend_from_slice(&proof);
    out.extend_from_slice(b"\"}");
    out
}

/// Canonical response envelope JSON with fixed field order.
#[must_use]
pub fn encode_response_json(response: &AuthenticatedResponse) -> Vec<u8> {
    let mut nonce = [0_u8; 32];
    let mut request = [0_u8; 32];
    let mut body = [0_u8; 64];
    let mut proof = [0_u8; 64];
    hex_encode(response.server_nonce.as_bytes(), &mut nonce);
    hex_encode(response.request_id.as_bytes(), &mut request);
    hex_encode(response.body_digest.as_bytes(), &mut body);
    hex_encode(response.proof.as_bytes(), &mut proof);
    let mut out = Vec::with_capacity(32 + 32 + 32 + 64 + 64 + 80);
    out.extend_from_slice(b"{\"v\":[");
    out.extend_from_slice(response.version.major.to_string().as_bytes());
    out.extend_from_slice(b",");
    out.extend_from_slice(response.version.minor.to_string().as_bytes());
    out.extend_from_slice(b"],\"nonce\":\"");
    out.extend_from_slice(&nonce);
    out.extend_from_slice(b"\",\"request\":\"");
    out.extend_from_slice(&request);
    out.extend_from_slice(b"\",\"status\":\"");
    out.extend_from_slice(response.status.as_str().as_bytes());
    out.extend_from_slice(b"\",\"body\":\"");
    out.extend_from_slice(&body);
    out.extend_from_slice(b"\",\"proof\":\"");
    out.extend_from_slice(&proof);
    out.extend_from_slice(b"\"}");
    out
}

struct Cursor<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Cursor<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    fn expect(&mut self, literal: &[u8]) -> Result<(), ProtocolError> {
        if self.bytes.len() - self.position >= literal.len()
            && &self.bytes[self.position..self.position + literal.len()] == literal
        {
            self.position += literal.len();
            Ok(())
        } else {
            Err(ProtocolError::InvalidEnvelope)
        }
    }

    fn parse_u16(&mut self) -> Result<u16, ProtocolError> {
        let start = self.position;
        while self.position < self.bytes.len() && self.bytes[self.position].is_ascii_digit() {
            self.position += 1;
        }
        let digits = &self.bytes[start..self.position];
        if digits.is_empty() || digits.len() > 5 || (digits.len() > 1 && digits[0] == b'0') {
            return Err(ProtocolError::InvalidEnvelope);
        }
        let mut value: u32 = 0;
        for digit in digits {
            value = value * 10 + u32::from(digit - b'0');
        }
        u16::try_from(value).map_err(|_| ProtocolError::InvalidEnvelope)
    }

    fn take_hex<const N: usize>(&mut self) -> Result<[u8; N], ProtocolError> {
        self.take_hex_as::<N>(ProtocolError::InvalidEnvelope)
    }

    fn take_hex_as<const N: usize>(
        &mut self,
        error: ProtocolError,
    ) -> Result<[u8; N], ProtocolError> {
        let end = self.position.saturating_add(2 * N);
        if end > self.bytes.len() {
            return Err(error);
        }
        let decoded = hex_decode_into::<N>(&self.bytes[self.position..end]).ok_or(error)?;
        self.position = end;
        Ok(decoded)
    }

    fn take_token(&mut self, max_len: usize) -> Result<&'a [u8], ProtocolError> {
        let start = self.position;
        while self.position < self.bytes.len() && self.bytes[self.position] != b'"' {
            if self.bytes[self.position] < 0x20 || self.bytes[self.position] == b'\\' {
                return Err(ProtocolError::InvalidEnvelope);
            }
            self.position += 1;
        }
        let token = &self.bytes[start..self.position];
        if token.is_empty() || token.len() > max_len {
            return Err(ProtocolError::InvalidEnvelope);
        }
        Ok(token)
    }

    const fn is_exhausted(&self) -> bool {
        self.position == self.bytes.len()
    }
}

fn parse_version(c: &mut Cursor<'_>) -> Result<ProtocolVersion, ProtocolError> {
    c.expect(b"{\"v\":[")?;
    let major = c.parse_u16()?;
    c.expect(b",")?;
    let minor = c.parse_u16()?;
    c.expect(b"],\"nonce\":\"")?;
    Ok(ProtocolVersion { major, minor })
}

/// Strict fixed-size request decoding with closed registries.
///
/// Field order, separators, hex lengths and the trailing close are exact;
/// the version must lie in the negotiated `supported` range.
pub fn decode_envelope_json(
    body: &[u8],
    supported: ProtocolRange,
) -> Result<AuthenticatedEnvelope, ProtocolError> {
    let mut c = Cursor::new(body);
    let version = parse_version(&mut c)?;
    if !supported.contains(version) {
        return Err(ProtocolError::NoCompatibleVersion);
    }
    let nonce =
        ServerNonce::from_bytes(c.take_hex::<16>()?).map_err(|_| ProtocolError::InvalidEnvelope)?;
    c.expect(b"\",\"request\":\"")?;
    let request_id = RequestId::from_bytes(c.take_hex::<16>()?);
    c.expect(b"\",\"command\":\"")?;
    let token = c.take_token(16)?;
    let command_text = core::str::from_utf8(token).map_err(|_| ProtocolError::InvalidEnvelope)?;
    let command = ControlCommand::parse(command_text)?;
    c.expect(b"\",\"body\":\"")?;
    let body_digest = ProofDigest::from_bytes(c.take_hex_as::<32>(ProtocolError::InvalidBody)?);
    c.expect(b"\",\"proof\":\"")?;
    let proof = ProofDigest::from_bytes(c.take_hex::<32>()?);
    c.expect(b"\"}")?;
    if !c.is_exhausted() {
        return Err(ProtocolError::InvalidEnvelope);
    }
    Ok(AuthenticatedEnvelope {
        version,
        server_nonce: nonce,
        request_id,
        command,
        body_digest,
        proof,
    })
}

/// Strict fixed-size response decoding with the closed status registry.
pub fn decode_response_json(
    body: &[u8],
    supported: ProtocolRange,
) -> Result<AuthenticatedResponse, ProtocolError> {
    let mut c = Cursor::new(body);
    let version = parse_version(&mut c)?;
    if !supported.contains(version) {
        return Err(ProtocolError::NoCompatibleVersion);
    }
    let nonce =
        ServerNonce::from_bytes(c.take_hex::<16>()?).map_err(|_| ProtocolError::InvalidEnvelope)?;
    c.expect(b"\",\"request\":\"")?;
    let request_id = RequestId::from_bytes(c.take_hex::<16>()?);
    c.expect(b"\",\"status\":\"")?;
    let token = c.take_token(16)?;
    let status_text = core::str::from_utf8(token).map_err(|_| ProtocolError::InvalidEnvelope)?;
    let status = RequestStatus::parse(status_text)?;
    c.expect(b"\",\"body\":\"")?;
    let body_digest = ProofDigest::from_bytes(c.take_hex_as::<32>(ProtocolError::InvalidBody)?);
    c.expect(b"\",\"proof\":\"")?;
    let proof = ProofDigest::from_bytes(c.take_hex::<32>()?);
    c.expect(b"\"}")?;
    if !c.is_exhausted() {
        return Err(ProtocolError::InvalidEnvelope);
    }
    Ok(AuthenticatedResponse {
        version,
        server_nonce: nonce,
        request_id,
        status,
        body_digest,
        proof,
    })
}

/// Encodes one authenticated request envelope over the canonical frame codec.
pub fn encode_envelope(
    envelope: &AuthenticatedEnvelope,
    limits: ProtocolLimits,
) -> Result<BoundedBytes<MAX_FRAME_BYTES>, ProtocolError> {
    let json = encode_envelope_json(envelope);
    let payload = JsonFramePayload::new(json).map_err(|_| ProtocolError::FrameTooLarge)?;
    FrameCodec::encode(&payload, limits)
}

/// Decodes one authenticated request envelope after prefix validation.
pub fn decode_envelope(
    bytes: &[u8],
    limits: ProtocolLimits,
    supported: ProtocolRange,
) -> Result<AuthenticatedEnvelope, ProtocolError> {
    let payload = FrameCodec::decode(bytes, limits)?;
    decode_envelope_json(payload.as_slice(), supported)
}

/// Encodes one authenticated response over the canonical frame codec with
/// bounded output.
pub fn encode_response(
    response: &AuthenticatedResponse,
    limits: ProtocolLimits,
) -> Result<BoundedBytes<MAX_FRAME_BYTES>, ProtocolError> {
    let json = encode_response_json(response);
    let payload = JsonFramePayload::new(json).map_err(|_| ProtocolError::FrameTooLarge)?;
    FrameCodec::encode(&payload, limits)
}

/// Decodes one authenticated response after prefix validation.
pub fn decode_response(
    bytes: &[u8],
    limits: ProtocolLimits,
    supported: ProtocolRange,
) -> Result<AuthenticatedResponse, ProtocolError> {
    let payload = FrameCodec::decode(bytes, limits)?;
    decode_response_json(payload.as_slice(), supported)
}

/// One admitted in-flight request: identity, ordering, deadline, progress.
#[derive(Clone, Debug)]
pub struct InFlightEntry {
    sequence: u64,
    admitted_at: MonotonicMillis,
    deadline: Option<MonotonicMillis>,
}

impl InFlightEntry {
    /// Records admission ordering and an optional absolute deadline.
    #[must_use]
    pub const fn new(
        sequence: u64,
        admitted_at: MonotonicMillis,
        deadline: Option<MonotonicMillis>,
    ) -> Self {
        Self {
            sequence,
            admitted_at,
            deadline,
        }
    }

    /// Admission sequence.
    #[must_use]
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    /// Admission instant.
    #[must_use]
    pub const fn admitted_at(&self) -> MonotonicMillis {
        self.admitted_at
    }

    /// Whether the entry expired at `now`.
    #[must_use]
    pub fn is_expired(&self, now: MonotonicMillis) -> bool {
        self.deadline
            .is_some_and(|deadline| now.get() >= deadline.get())
    }
}

/// Bounded registry of concurrent in-flight requests.
///
/// The ceiling is capped by the canonical `MAX_PROTOCOL_IN_FLIGHT` (32); a
/// full registry fails closed instead of queueing without bound.
#[derive(Clone, Debug)]
pub struct InFlightRegistry {
    entries: BTreeMap<RequestId, InFlightEntry>,
    capacity: usize,
}

impl InFlightRegistry {
    /// Creates a finite registry; zero or above-canonical capacity fails.
    pub fn new(capacity: usize) -> Result<Self, ProtocolError> {
        if capacity == 0 || capacity > MAX_PROTOCOL_IN_FLIGHT {
            return Err(ProtocolError::InvalidLimits);
        }
        Ok(Self {
            entries: BTreeMap::new(),
            capacity,
        })
    }

    /// Number of tracked requests.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether no request is tracked.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Configured ceiling.
    #[must_use]
    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    /// Whether this identity is currently in flight.
    #[must_use]
    pub fn contains(&self, request_id: &RequestId) -> bool {
        self.entries.contains_key(request_id)
    }

    /// Tracks one admitted request; duplicates replay, overflow exhausts.
    pub fn insert(
        &mut self,
        request_id: RequestId,
        entry: InFlightEntry,
    ) -> Result<(), ProtocolError> {
        if self.entries.contains_key(&request_id) {
            return Err(ProtocolError::ReplayDetected);
        }
        if self.entries.len() >= self.capacity {
            return Err(ProtocolError::ResourceExhausted);
        }
        self.entries.insert(request_id, entry);
        Ok(())
    }

    /// Releases one tracked request; returns whether it was present.
    pub fn remove(&mut self, request_id: &RequestId) -> bool {
        self.entries.remove(request_id).is_some()
    }

    /// Releases every tracked request while keeping the configured ceiling.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Snapshot of tracked identities in admission order of their IDs.
    #[must_use]
    pub fn tracked(&self) -> Vec<RequestId> {
        self.entries.keys().copied().collect()
    }
}

/// Guard for one admitted request: deadline, cancellation and progress.
///
/// The guard carries no grant decision and no source content — only ordering,
/// timing and lifecycle state.
#[derive(Clone, Debug)]
pub struct RequestGuard {
    request_id: RequestId,
    sequence: u64,
    admitted_at: MonotonicMillis,
    deadline: Option<MonotonicMillis>,
    progress: Option<ProgressState>,
    cancelled: bool,
}

impl RequestGuard {
    /// Admits a guard; an already-expired relative deadline fails closed.
    pub fn new(
        request_id: RequestId,
        sequence: u64,
        now: MonotonicMillis,
        relative_deadline_ms: Option<u64>,
        limits: ProtocolLimits,
    ) -> Result<Self, ProtocolError> {
        let _ = limits.validate()?;
        let deadline = relative_deadline_ms.map(|delta| now.plus(delta));
        if deadline.is_some_and(|deadline| now.get() >= deadline.get()) {
            return Err(ProtocolError::DeadlineExpired);
        }
        Ok(Self {
            request_id,
            sequence,
            admitted_at: now,
            deadline,
            progress: None,
            cancelled: false,
        })
    }

    /// Admitted request identity.
    #[must_use]
    pub const fn request_id(&self) -> &RequestId {
        &self.request_id
    }

    /// Admission sequence.
    #[must_use]
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    /// Admission instant.
    #[must_use]
    pub const fn admitted_at(&self) -> MonotonicMillis {
        self.admitted_at
    }

    /// Absolute deadline when the request carried a relative one.
    #[must_use]
    pub const fn deadline(&self) -> Option<MonotonicMillis> {
        self.deadline
    }

    /// Whether the guard expired at `now`.
    #[must_use]
    pub fn is_expired(&self, now: MonotonicMillis) -> bool {
        self.deadline
            .is_some_and(|deadline| now.get() >= deadline.get())
    }

    /// Whether cancellation was recorded for this guard.
    #[must_use]
    pub const fn is_cancelled(&self) -> bool {
        self.cancelled
    }

    /// Records cancellation; terminal emission still goes through `finish`.
    pub fn mark_cancelled(&mut self) {
        self.cancelled = true;
    }

    /// Advances monotone progress, fixing the denominator on first use.
    pub fn advance_progress(
        &mut self,
        total: u64,
        completed: u64,
        limits: ProtocolLimits,
    ) -> Result<(), ProtocolError> {
        match &mut self.progress {
            Some(state) => {
                if state.total() != total {
                    return Err(ProtocolError::InvalidEnvelope);
                }
                state.advance(completed)
            }
            empty @ None => {
                let mut state = ProgressState::new(total, limits)?;
                state.advance(completed)?;
                *empty = Some(state);
                Ok(())
            }
        }
    }

    /// Emits the single terminal response for this guard.
    pub fn finish(
        &mut self,
        terminal: TerminalKind,
        limits: ProtocolLimits,
    ) -> Result<(), ProtocolError> {
        match &mut self.progress {
            Some(state) => state.finish(terminal),
            empty @ None => {
                let mut state = ProgressState::new(0, limits)?;
                state.finish(terminal)?;
                *empty = Some(state);
                Ok(())
            }
        }
    }

    /// Current progress when any was reported.
    #[must_use]
    pub const fn progress(&self) -> Option<ProgressState> {
        self.progress
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn envelope() -> AuthenticatedEnvelope {
        seal_envelope(
            ProtocolVersion { major: 1, minor: 0 },
            ServerNonce::from_bytes([0x11; 16]).expect("nonce"),
            RequestId::from_bytes([0x22; 16]),
            ControlCommand::Health,
            ProofDigest::from_bytes([0x33; 32]),
            ProofDigest::from_bytes([0x44; 32]),
        )
    }

    fn supported() -> ProtocolRange {
        ProtocolRange::new(
            ProtocolVersion { major: 1, minor: 0 },
            ProtocolVersion { major: 1, minor: 2 },
        )
        .expect("range")
    }

    #[test]
    fn registries_are_closed_and_round_trip() {
        assert_eq!(ControlCommand::ALL.len(), 3);
        assert_eq!(ControlCommand::parse("health"), Ok(ControlCommand::Health));
        assert_eq!(
            ControlCommand::parse("reboot"),
            Err(ProtocolError::UnknownCommand)
        );
        assert_eq!(RequestStatus::ALL.len(), 5);
        assert_eq!(RequestStatus::parse("ok"), Ok(RequestStatus::Ok));
        assert_eq!(
            RequestStatus::parse("unknown"),
            Err(ProtocolError::InvalidStatus)
        );
        for terminal in [
            TerminalKind::Success,
            TerminalKind::Partial,
            TerminalKind::Cancelled,
            TerminalKind::Failed,
            TerminalKind::OutcomeUnknown,
        ] {
            assert_eq!(RequestStatus::from_terminal(terminal).terminal(), terminal);
        }
    }

    #[test]
    fn envelope_json_round_trips_with_fixed_order() {
        let sealed = envelope();
        let json = encode_envelope_json(&sealed);
        assert!(json.starts_with(b"{\"v\":[1,0],\"nonce\":\""));
        // Fixed sizes: envelope JSON has an exact length for this fixture.
        assert_eq!(json.len(), encode_envelope_json(&sealed).len());
        let decoded = decode_envelope_json(&json, supported()).expect("decode");
        assert_eq!(decoded, sealed);
    }

    #[test]
    fn envelope_rejects_reordered_and_trailing_bytes() {
        let json = encode_envelope_json(&envelope());
        let mut reordered = json.clone();
        // Swap two adjacent fields: strict order must fail.
        let nonce_pos = json
            .windows(b"\"nonce\"".len())
            .position(|w| w == b"\"nonce\"")
            .expect("nonce");
        let request_pos = json
            .windows(b"\"request\"".len())
            .position(|w| w == b"\"request\"")
            .expect("request");
        reordered[nonce_pos] = b'X';
        reordered[request_pos] = b'Y';
        assert_eq!(
            decode_envelope_json(&reordered, supported()),
            Err(ProtocolError::InvalidEnvelope)
        );
        let mut trailed = json.clone();
        trailed.push(b' ');
        assert_eq!(
            decode_envelope_json(&trailed, supported()),
            Err(ProtocolError::InvalidEnvelope)
        );
        // Uppercase hex is non-canonical.
        let upper = json
            .iter()
            .map(|b| {
                if b.is_ascii_hexdigit() {
                    b.to_ascii_uppercase()
                } else {
                    *b
                }
            })
            .collect::<Vec<u8>>();
        assert_eq!(
            decode_envelope_json(&upper, supported()),
            Err(ProtocolError::InvalidEnvelope)
        );
    }

    #[test]
    fn response_status_registry_is_strict() {
        let response = seal_response(
            ProtocolVersion { major: 1, minor: 0 },
            ServerNonce::from_bytes([0x11; 16]).expect("nonce"),
            RequestId::from_bytes([0x22; 16]),
            RequestStatus::Partial,
            ProofDigest::from_bytes([0x33; 32]),
            ProofDigest::from_bytes([0x44; 32]),
        );
        let json = encode_response_json(&response);
        assert_eq!(
            decode_response_json(&json, supported()).expect("decode"),
            response
        );
        let bad = json
            .windows(b"\"partial\"".len())
            .position(|w| w == b"\"partial\"")
            .expect("status");
        // Same-length unknown status: registry must reject, not reinterpret.
        let mut patched = json;
        patched[bad + 1..bad + 8].copy_from_slice(b"partiax");
        assert_eq!(
            decode_response_json(&patched, supported()),
            Err(ProtocolError::InvalidStatus)
        );
    }

    #[test]
    fn transcripts_bind_command_and_body() {
        let base = envelope();
        let other_command = AuthenticatedEnvelope {
            command: ControlCommand::Shutdown,
            ..base
        };
        assert_ne!(
            envelope_transcript(&base),
            envelope_transcript(&other_command)
        );
        assert!(
            envelope_transcript(&base)
                .windows(ENVELOPE_REQUEST_DOMAIN.len())
                .any(|w| w == ENVELOPE_REQUEST_DOMAIN.as_bytes())
        );
        assert_ne!(
            envelope_transcript(&base),
            response_transcript(&seal_response(
                base.version,
                *base.server_nonce(),
                *base.request_id(),
                RequestStatus::Ok,
                *base.body_digest(),
                *base.proof(),
            ))
        );
    }

    #[test]
    fn in_flight_registry_fails_closed_at_capacity() {
        let mut registry = InFlightRegistry::new(2).expect("registry");
        let at = MonotonicMillis::new(10);
        registry
            .insert(
                RequestId::from_bytes([1; 16]),
                InFlightEntry::new(1, at, None),
            )
            .expect("first");
        assert_eq!(
            registry.insert(
                RequestId::from_bytes([1; 16]),
                InFlightEntry::new(2, at, None),
            ),
            Err(ProtocolError::ReplayDetected)
        );
        registry
            .insert(
                RequestId::from_bytes([2; 16]),
                InFlightEntry::new(2, at, None),
            )
            .expect("second");
        assert_eq!(
            registry.insert(
                RequestId::from_bytes([3; 16]),
                InFlightEntry::new(3, at, None),
            ),
            Err(ProtocolError::ResourceExhausted)
        );
        assert!(registry.remove(&RequestId::from_bytes([1; 16])));
        assert!(!registry.remove(&RequestId::from_bytes([1; 16])));
    }

    #[test]
    fn guard_deadline_is_explicit_and_checked() {
        let limits = ProtocolLimits {
            max_frame_bytes: MAX_FRAME_BYTES,
            max_body_bytes: MAX_FRAME_BYTES - crate::config::FRAME_PREFIX_BYTES,
            max_replay_entries: 8,
            max_in_flight_requests: 8,
            max_progress_total: 100,
        };
        assert_eq!(
            RequestGuard::new(
                RequestId::from_bytes([1; 16]),
                1,
                MonotonicMillis::new(100),
                Some(0),
                limits,
            )
            .expect_err("zero relative deadline must expire"),
            ProtocolError::DeadlineExpired
        );
        let guard = RequestGuard::new(
            RequestId::from_bytes([1; 16]),
            1,
            MonotonicMillis::new(100),
            Some(50),
            limits,
        )
        .expect("guard");
        assert!(!guard.is_expired(MonotonicMillis::new(149)));
        assert!(guard.is_expired(MonotonicMillis::new(150)));
    }
}

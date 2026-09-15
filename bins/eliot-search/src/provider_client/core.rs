//! Canonical provider-protocol client shared by the CLI transports.
//!
//! This module is protocol-only: it opens no redb/Qdrant/CAS store, acquires
//! no data-root ownership and mints no grants. It reads exactly two local
//! files — the namespaced endpoint descriptor and the pairing token file —
//! performs the `pairing_blake3_v1` ceremony, negotiates the exact `1.0`
//! version, seals one envelope or operation frame, and verifies the sealed
//! response. Unsupported recipes are reported as explicit unavailable with
//! their blockers, never as empty success.
//!
//! Key-derivation duplication is deliberate and fenced: the development
//! token-file derivation and the loopback binding digest must stay byte-equal
//! to `bins/eliot-searchd/src/endpoint.rs` (`DEV_KEY_DOMAIN`,
//! `BINDING_DOMAIN`/`BINDING_ROLE`). The live round-trip test in
//! `eliot-searchd/tests/provider_process.rs` proves the agreement; any drift
//! fails the handshake closed, never silently.

use std::fs::File;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::Path;
use std::time::Duration;

use search_contracts::{ProtocolRange, ProtocolVersion, RequestId};
use search_provider_protocol::negotiation::negotiate_hello;
use search_provider_protocol::pairing::{
    ClientNonce, PairingChallenge, ProofDigest, ServerNonce, SessionId, client_proof_transcript,
    server_proof_transcript, verify_proof,
};
use search_provider_protocol::request::{
    AuthenticatedResponse, ControlCommand, RequestStatus, decode_response_json,
    encode_envelope_json, envelope_transcript, response_transcript, seal_envelope,
    verify_response_proof,
};

/// Exact negotiated provider version.
pub const PROVIDER_VERSION: ProtocolVersion = ProtocolVersion { major: 1, minor: 0 };
/// Transport bounds for one CLI invocation (single request per connection).
pub const IO_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_TOKEN_FILE_BYTES: usize = 4096;
const MIN_TOKEN_BYTES: usize = 32;
const MAX_CHALLENGE_LINE_BYTES: usize = 512;
const MAX_PROVIDER_LINE_BYTES: usize = 256 * 1024;
const MAX_RESPONSE_LINES: usize = 1_000_000;
const MAX_QUERY_BYTES: usize = 64 * 1024;
const MAX_TARGET_BYTES: usize = 32 * 1024;

/// Must stay byte-equal to the daemon endpoint development derivation.
const DEV_KEY_DOMAIN: &[u8] = b"eliot-search/loopback-dev-key/v1\0";
/// Must stay byte-equal to the daemon endpoint binding derivation.
const BINDING_DOMAIN: &[u8] = b"eliot-search/loopback-binding/v1\0";
/// Must stay byte-equal to the daemon endpoint loopback role.
const BINDING_ROLE: &[u8] = b"loopback-operator";
const REQUEST_ID_DOMAIN: &[u8] = b"eliot-provider-request/v1\0";

/// Exact daemon protocol range the CLI accepts.
#[must_use]
pub fn provider_range() -> ProtocolRange {
    ProtocolRange::new(PROVIDER_VERSION, PROVIDER_VERSION).expect("provider range")
}

/// One validated provider request before sealing (sealing needs the hello nonce).
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UnsignedRequest {
    /// Sealed envelope: daemon health.
    Health,
    /// Sealed envelope: protocol and build version.
    Version,
    /// Sealed envelope: graceful shutdown.
    Shutdown,
    /// Local readiness/capability diagnostics.
    Status,
    /// Idempotent cancellation of one in-flight identity.
    Cancel {
        /// Raw 16-byte target identity.
        target: [u8; 16],
    },
    /// Validated recipe query (capability-gated by the daemon).
    Query {
        /// Whether ASCII-insensitive matching was requested.
        ascii_insensitive: bool,
        /// Raw query bytes (non-empty, bounded).
        query: Vec<u8>,
    },
    /// Validated admission target (capability-gated by the daemon).
    Ingest {
        /// Raw target bytes (non-empty, bounded).
        target: Vec<u8>,
    },
    /// Validated handle expansion (capability-gated by the daemon).
    Expand {
        /// Opaque handle token bytes.
        handle: Vec<u8>,
        /// Byte range start (must precede `end`).
        start: u64,
        /// Byte range end.
        end: u64,
    },
}

impl UnsignedRequest {
    /// Validates a query body without interpreting it.
    pub fn query(ascii_insensitive: bool, query: &[u8]) -> Result<Self, String> {
        if query.is_empty() || query.len() > MAX_QUERY_BYTES {
            return Err("REMOTE_QUERY_INVALID".to_owned());
        }
        Ok(Self::Query {
            ascii_insensitive,
            query: query.to_vec(),
        })
    }

    /// Validates an ingest target without interpreting it.
    pub fn ingest(target: &[u8]) -> Result<Self, String> {
        if target.is_empty() || target.len() > MAX_TARGET_BYTES {
            return Err("REMOTE_TARGET_INVALID".to_owned());
        }
        Ok(Self::Ingest {
            target: target.to_vec(),
        })
    }

    /// Validates an expansion descriptor without interpreting it.
    pub fn expand(handle: &[u8], start: u64, end: u64) -> Result<Self, String> {
        if handle.is_empty() || handle.len() > MAX_TARGET_BYTES || start >= end {
            return Err("REMOTE_EXPAND_INVALID".to_owned());
        }
        Ok(Self::Expand {
            handle: handle.to_vec(),
            start,
            end,
        })
    }

    /// Validates a cancellation target identity.
    pub fn cancel(target_hex: &str) -> Result<Self, String> {
        let raw =
            hex_decode(target_hex).ok_or_else(|| "REMOTE_CANCEL_TARGET_INVALID".to_owned())?;
        if raw.len() != 16 {
            return Err("REMOTE_CANCEL_TARGET_INVALID".to_owned());
        }
        let mut target = [0_u8; 16];
        target.copy_from_slice(&raw);
        Ok(Self::Cancel { target })
    }
}

/// Renders the daemon-bound line for a non-envelope request.
///
/// Envelope commands are sealed by the transport after hello; this renders
/// the `op\t...` lines with validated hex arguments.
#[must_use]
pub fn render_op_line(request: &UnsignedRequest) -> Option<String> {
    match request {
        UnsignedRequest::Status => Some("op\tstatus".to_owned()),
        UnsignedRequest::Cancel { target } => Some(format!("op\tcancel\t{}", hex_encode(target))),
        UnsignedRequest::Query {
            ascii_insensitive,
            query,
        } => {
            let mut blob = if *ascii_insensitive {
                b"i:".to_vec()
            } else {
                b"s:".to_vec()
            };
            blob.extend_from_slice(query);
            Some(format!("op\tquery\t{}", hex_encode(&blob)))
        }
        UnsignedRequest::Ingest { target } => Some(format!("op\tingest\t{}", hex_encode(target))),
        UnsignedRequest::Expand { handle, start, end } => {
            let blob = format!("{}:{start}:{end}", hex_encode(handle));
            Some(format!("op\texpand\t{}", hex_encode(blob.as_bytes())))
        }
        UnsignedRequest::Health | UnsignedRequest::Version | UnsignedRequest::Shutdown => None,
    }
}

/// Maps a failure code to the process exit code.
///
/// Authenticated provider rejections — typed `PROVIDER_*`/`PROTOCOL_*`
/// reasons, loopback dispatch failures and failed request acknowledgements —
/// exit 2. Local usage, endpoint, token, transport and framing errors exit 1.
#[must_use]
pub fn exit_for_error(code: &str) -> u8 {
    if code.starts_with("PROVIDER_")
        || code.starts_with("PROTOCOL_")
        || code.starts_with("LOOPBACK_")
        || code == "REMOTE_REQUEST_FAILED"
    {
        2
    } else {
        1
    }
}

/// Canonical namespaced endpoint descriptor.
///
/// ```text
/// ELIOT_SEARCH_ENDPOINT_V1
/// address=127.0.0.1:PORT
/// ```
///
/// Loopback only; a missing descriptor fails closed — there is no hidden
/// default address to fall back to.
#[derive(Debug)]
pub struct NamespacedEndpoint {
    /// Loopback socket address from the descriptor.
    pub address: SocketAddr,
}

/// Reads the canonical endpoint descriptor under `data_root/runtime/endpoint.v1`.
pub fn read_endpoint_descriptor(data_root: &Path) -> Result<NamespacedEndpoint, String> {
    let path = data_root.join("runtime").join("endpoint.v1");
    // Descriptor failures are endpoint errors, never token errors: the two
    // files have different owners and different failure budgets.
    let body = read_small_regular(&path, 16 * 1024)
        .map_err(|_| "ENDPOINT_DESCRIPTOR_INVALID".to_owned())?;
    let body = String::from_utf8(body).map_err(|_| "ENDPOINT_DESCRIPTOR_INVALID".to_owned())?;
    let mut lines = body.lines();
    if lines.next() != Some("ELIOT_SEARCH_ENDPOINT_V1") {
        return Err("ENDPOINT_DESCRIPTOR_INVALID".to_owned());
    }
    let mut address = None;
    for line in lines {
        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| "ENDPOINT_DESCRIPTOR_INVALID".to_owned())?;
        if key != "address" {
            return Err("ENDPOINT_DESCRIPTOR_INVALID".to_owned());
        }
        if address.is_some() {
            return Err("ENDPOINT_DESCRIPTOR_INVALID".to_owned());
        }
        let parsed: SocketAddr = value
            .parse()
            .map_err(|_| "ENDPOINT_DESCRIPTOR_INVALID".to_owned())?;
        if !parsed.ip().is_loopback() {
            return Err("ENDPOINT_NON_LOOPBACK_DENIED".to_owned());
        }
        address = Some(parsed);
    }
    address
        .map(|address| NamespacedEndpoint { address })
        .ok_or_else(|| "ENDPOINT_DESCRIPTOR_INVALID".to_owned())
}

/// Derives the pairing key from a token file (development-compat derivation).
///
/// Regular files only, bounded size, ASCII-trimmed, minimum length enforced;
/// the raw buffer is zeroed before return.
pub fn read_shim_key(path: &Path) -> Result<[u8; 32], String> {
    let bytes = read_small_regular(path, (MAX_TOKEN_FILE_BYTES + 1) as u64)?;
    if bytes.len() > MAX_TOKEN_FILE_BYTES {
        return Err("REMOTE_TOKEN_FILE_TOO_LARGE".to_owned());
    }
    let start = bytes
        .iter()
        .position(|byte| !byte.is_ascii_whitespace())
        .unwrap_or(bytes.len());
    let end = bytes
        .iter()
        .rposition(|byte| !byte.is_ascii_whitespace())
        .map_or(start, |index| index + 1);
    shim_key_from_bytes(&bytes[start..end])
}

fn read_small_regular(path: &Path, maximum: u64) -> Result<Vec<u8>, String> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|error| format!("REMOTE_TOKEN_OPEN_ERROR:{error}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("REMOTE_TOKEN_FILE_INVALID".to_owned());
    }
    if metadata.len() > maximum {
        return Err("REMOTE_TOKEN_FILE_TOO_LARGE".to_owned());
    }
    let mut file = File::open(path).map_err(|error| format!("REMOTE_TOKEN_OPEN_ERROR:{error}"))?;
    let mut bytes = Vec::new();
    Read::take(&mut file, maximum)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("REMOTE_TOKEN_READ_ERROR:{error}"))?;
    if bytes.len() > MAX_TOKEN_FILE_BYTES {
        bytes.fill(0);
        return Err("REMOTE_TOKEN_FILE_TOO_LARGE".to_owned());
    }
    Ok(bytes)
}

/// Pure development-key derivation over trimmed token bytes.
fn shim_key_from_bytes(trimmed: &[u8]) -> Result<[u8; 32], String> {
    if trimmed.len() < MIN_TOKEN_BYTES {
        return Err("REMOTE_TOKEN_TOO_SHORT".to_owned());
    }
    let mut hasher = blake3::Hasher::new();
    hasher.update(DEV_KEY_DOMAIN);
    hasher.update(trimmed);
    let key = *hasher.finalize().as_bytes();
    if key.iter().all(|byte| *byte == 0) {
        return Err("REMOTE_TOKEN_INVALID".to_owned());
    }
    Ok(key)
}

/// Role-bound binding digest; must equal the daemon endpoint derivation.
fn binding_digest(key: &[u8; 32]) -> ProofDigest {
    let mut hasher = blake3::Hasher::new();
    hasher.update(BINDING_DOMAIN);
    hasher.update(BINDING_ROLE);
    hasher.update(&[0]);
    hasher.update(key);
    ProofDigest::from_bytes(*hasher.finalize().as_bytes())
}

fn keyed(key: &[u8; 32], bytes: &[u8]) -> ProofDigest {
    ProofDigest::from_bytes(*blake3::keyed_hash(key, bytes).as_bytes())
}

/// One open provider session: pairing-authenticated stream plus the hello
/// nonce, version and connection sequence counters.
pub struct ProviderSession {
    stream: TcpStream,
    reader: BufReader<TcpStream>,
    key: [u8; 32],
    nonce: ServerNonce,
    version: ProtocolVersion,
    envelope_sequence: u64,
    request_counter: u64,
}

impl Drop for ProviderSession {
    fn drop(&mut self) {
        self.key.fill(0);
    }
}

/// Opens a pairing-authenticated provider session and negotiates `1.0`.
pub fn open_session(address: SocketAddr, key: [u8; 32]) -> Result<ProviderSession, String> {
    if !address.ip().is_loopback() {
        return Err("REMOTE_NON_LOOPBACK_DENIED".to_owned());
    }
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(10))
        .map_err(|error| format!("REMOTE_CONNECT_ERROR:{error}"))?;
    stream
        .set_read_timeout(Some(IO_TIMEOUT))
        .and_then(|()| stream.set_write_timeout(Some(IO_TIMEOUT)))
        .map_err(|error| format!("REMOTE_TIMEOUT_CONFIGURATION_ERROR:{error}"))?;
    let read_stream = stream
        .try_clone()
        .map_err(|error| format!("REMOTE_STREAM_CLONE_ERROR:{error}"))?;
    let mut reader = BufReader::new(read_stream);
    let challenge = read_challenge(&mut reader)?;
    if binding_digest(&key) != challenge.binding {
        return Err("REMOTE_BINDING_MISMATCH".to_owned());
    }
    let proof = keyed(
        &key,
        client_proof_transcript(
            challenge.version,
            &challenge.binding,
            challenge.session,
            &challenge.nonce,
            &challenge.challenge,
        )
        .as_bytes(),
    );
    write_line(
        &mut stream,
        &format!("PAIRING_AUTH\tproof={}", hex_encode(proof.as_bytes())),
    )?;
    let verified_line = read_bounded_line(&mut reader, MAX_CHALLENGE_LINE_BYTES)?
        .ok_or_else(|| "REMOTE_VERIFIED_MISSING".to_owned())?;
    let observed = parse_verified_line(&verified_line)?;
    let expected = keyed(
        &key,
        server_proof_transcript(
            challenge.version,
            &challenge.binding,
            challenge.session,
            &challenge.nonce,
            &challenge.challenge,
        )
        .as_bytes(),
    );
    if !verify_proof(&expected, &observed) {
        return Err("REMOTE_PROVIDER_PROOF_INVALID".to_owned());
    }
    let ready = read_bounded_line(&mut reader, MAX_PROVIDER_LINE_BYTES)?
        .ok_or_else(|| "REMOTE_READY_MISSING".to_owned())?;
    if !ready.contains("\"event\":\"authenticated\"") {
        return Err("REMOTE_AUTHENTICATION_FAILED".to_owned());
    }
    let mut session = ProviderSession {
        stream,
        reader,
        key,
        nonce: ServerNonce::from_bytes([1; 16]).map_err(|_| "REMOTE_NONCE_INVALID".to_owned())?,
        version: PROVIDER_VERSION,
        envelope_sequence: 0,
        request_counter: 0,
    };
    session.hello()?;
    Ok(session)
}

struct ServerChallenge {
    version: ProtocolVersion,
    session: SessionId,
    nonce: ClientNonce,
    challenge: PairingChallenge,
    binding: ProofDigest,
}

fn read_challenge(reader: &mut BufReader<TcpStream>) -> Result<ServerChallenge, String> {
    let line = read_bounded_line(reader, MAX_CHALLENGE_LINE_BYTES)?
        .ok_or_else(|| "REMOTE_CHALLENGE_MISSING".to_owned())?;
    let parts: Vec<&str> = line.split('\t').collect();
    if parts.len() != 6 || parts[0] != "PAIRING_CHALLENGE" {
        return Err("REMOTE_CHALLENGE_INVALID".to_owned());
    }
    let version_text = parts[1]
        .strip_prefix("v=")
        .ok_or_else(|| "REMOTE_CHALLENGE_INVALID".to_owned())?;
    let (major, minor) = version_text
        .split_once('.')
        .ok_or_else(|| "REMOTE_CHALLENGE_INVALID".to_owned())?;
    let version = ProtocolVersion {
        major: major
            .parse::<u16>()
            .map_err(|_| "REMOTE_CHALLENGE_INVALID".to_owned())?,
        minor: minor
            .parse::<u16>()
            .map_err(|_| "REMOTE_CHALLENGE_INVALID".to_owned())?,
    };
    let session = SessionId::from_bytes(decode_16(
        parts[2]
            .strip_prefix("session=")
            .ok_or_else(|| "REMOTE_CHALLENGE_INVALID".to_owned())?,
    )?)
    .map_err(|_| "REMOTE_CHALLENGE_INVALID".to_owned())?;
    let nonce = ClientNonce::from_bytes(decode_16(
        parts[3]
            .strip_prefix("nonce=")
            .ok_or_else(|| "REMOTE_CHALLENGE_INVALID".to_owned())?,
    )?)
    .map_err(|_| "REMOTE_CHALLENGE_INVALID".to_owned())?;
    let challenge = PairingChallenge::from_bytes(decode_32(
        parts[4]
            .strip_prefix("challenge=")
            .ok_or_else(|| "REMOTE_CHALLENGE_INVALID".to_owned())?,
    )?)
    .map_err(|_| "REMOTE_CHALLENGE_INVALID".to_owned())?;
    let binding = ProofDigest::from_bytes(decode_32(
        parts[5]
            .strip_prefix("binding=")
            .ok_or_else(|| "REMOTE_CHALLENGE_INVALID".to_owned())?,
    )?);
    Ok(ServerChallenge {
        version,
        session,
        nonce,
        challenge,
        binding,
    })
}

fn parse_verified_line(line: &str) -> Result<ProofDigest, String> {
    let parts: Vec<&str> = line.split('\t').collect();
    if parts.len() != 2 || parts[0] != "PAIRING_VERIFIED" {
        return Err("REMOTE_VERIFIED_INVALID".to_owned());
    }
    let proof = parts[1]
        .strip_prefix("proof=")
        .ok_or_else(|| "REMOTE_VERIFIED_INVALID".to_owned())?;
    decode_32(proof)
        .map(ProofDigest::from_bytes)
        .map_err(|_| "REMOTE_VERIFIED_INVALID".to_owned())
}

impl ProviderSession {
    /// Runs `op\thello` with the exact CLI range and stores nonce/version.
    fn hello(&mut self) -> Result<(), String> {
        write_line(&mut self.stream, "op\thello\t1.0-1.0")?;
        let started = self.recv()?;
        if !started.contains("request_started") {
            return Err("REMOTE_HELLO_INVALID".to_owned());
        }
        let hello = self.recv()?;
        if !hello.contains("\"event\":\"provider_hello\"") {
            return Err("REMOTE_HELLO_INVALID".to_owned());
        }
        let range = provider_range();
        let version = extract_version(&hello)?;
        negotiate_hello(
            range,
            ProtocolRange::new(version, version).map_err(|_| "REMOTE_HELLO_INVALID".to_owned())?,
        )
        .map_err(|_| "REMOTE_VERSION_MISMATCH".to_owned())?;
        let nonce_hex = extract_field(&hello, "\"nonce\":\"")
            .ok_or_else(|| "REMOTE_HELLO_INVALID".to_owned())?;
        let nonce_raw = hex_decode(&nonce_hex).ok_or_else(|| "REMOTE_HELLO_INVALID".to_owned())?;
        if nonce_raw.len() != 16 {
            return Err("REMOTE_HELLO_INVALID".to_owned());
        }
        let mut nonce = [0_u8; 16];
        nonce.copy_from_slice(&nonce_raw);
        self.nonce =
            ServerNonce::from_bytes(nonce).map_err(|_| "REMOTE_HELLO_INVALID".to_owned())?;
        self.version = version;
        let complete = self.recv()?;
        if !complete.contains("\"ok\":true") {
            return Err("REMOTE_HELLO_INVALID".to_owned());
        }
        Ok(())
    }

    fn send(&mut self, line: &str) -> Result<(), String> {
        write_line(&mut self.stream, line)
    }

    fn recv(&mut self) -> Result<String, String> {
        let line = read_bounded_line(&mut self.reader, MAX_PROVIDER_LINE_BYTES)?
            .ok_or_else(|| "REMOTE_RESPONSE_TRUNCATED".to_owned())?;
        Ok(line)
    }

    fn mint_id(&mut self) -> RequestId {
        use std::time::{SystemTime, UNIX_EPOCH};
        self.request_counter = self.request_counter.wrapping_add(1);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(1, |elapsed| elapsed.as_nanos());
        let mut input = Vec::with_capacity(REQUEST_ID_DOMAIN.len() + 24);
        input.extend_from_slice(REQUEST_ID_DOMAIN);
        input.extend_from_slice(&self.request_counter.to_le_bytes());
        input.extend_from_slice(&nanos.to_le_bytes());
        let digest = blake3::keyed_hash(&self.key, &input);
        let mut raw = [0_u8; 16];
        raw.copy_from_slice(&digest.as_bytes()[..16]);
        if raw.iter().all(|byte| *byte == 0) {
            raw[15] = 1;
        }
        RequestId::from_bytes(raw)
    }

    /// Sends one validated request; prints daemon payload lines to stdout.
    ///
    /// Envelope responses are proof- and receipt-verified; any non-ok
    /// terminal (including explicit unavailable with blockers) becomes a
    /// typed `Err` for exit-code mapping. Nothing is printed on failure
    /// besides the daemon payload already streamed.
    pub fn invoke(&mut self, request: &UnsignedRequest) -> Result<(), String> {
        match request {
            UnsignedRequest::Health => self.invoke_envelope(ControlCommand::Health),
            UnsignedRequest::Version => self.invoke_envelope(ControlCommand::Version),
            UnsignedRequest::Shutdown => self.invoke_envelope(ControlCommand::Shutdown),
            _ => self.invoke_op(request),
        }
    }

    fn invoke_envelope(&mut self, command: ControlCommand) -> Result<(), String> {
        self.envelope_sequence = self
            .envelope_sequence
            .checked_add(1)
            .ok_or_else(|| "REMOTE_SEQUENCE_EXHAUSTED".to_owned())?;
        let request_id = self.mint_id();
        let digest = ProofDigest::from_bytes(*blake3::hash(&[]).as_bytes());
        let stub = seal_envelope(
            self.version,
            self.nonce,
            request_id,
            command,
            digest,
            ProofDigest::from_bytes([0; 32]),
        );
        let proof = keyed(&self.key, &envelope_transcript(&stub));
        let sealed = seal_envelope(self.version, self.nonce, request_id, command, digest, proof);
        let frame = encode_envelope_json(&sealed);
        let length =
            u32::try_from(frame.len()).map_err(|_| "REMOTE_REQUEST_TOO_LARGE".to_owned())?;
        let mut framed = length.to_le_bytes().to_vec();
        framed.extend_from_slice(&frame);
        self.send(&format!(
            "envelope\t{}\t{}",
            self.envelope_sequence,
            hex_encode(&framed)
        ))?;
        let started = self.recv()?;
        if !started.contains("request_started") {
            return Err("REMOTE_RESPONSE_INVALID".to_owned());
        }
        let mut provider_sequence = 0_u64;
        let mut terminal: Option<AuthenticatedResponse> = None;
        for _ in 0..MAX_RESPONSE_LINES {
            let line = self.recv()?;
            if let Some(hex) = line.strip_prefix("response\t") {
                let frame = hex_decode(hex).ok_or_else(|| "REMOTE_RESPONSE_INVALID".to_owned())?;
                if frame.len() < 4 {
                    return Err("REMOTE_RESPONSE_INVALID".to_owned());
                }
                let declared = u32::from_le_bytes(
                    frame[..4]
                        .try_into()
                        .map_err(|_| "REMOTE_RESPONSE_INVALID".to_owned())?,
                ) as usize;
                if declared + 4 != frame.len() {
                    return Err("REMOTE_RESPONSE_INVALID".to_owned());
                }
                let range = provider_range();
                let decoded = decode_response_json(&frame[4..], range)
                    .map_err(|_| "REMOTE_RESPONSE_INVALID".to_owned())?;
                if decoded.request_id() != &request_id {
                    return Err("REMOTE_RESPONSE_MISMATCH".to_owned());
                }
                provider_sequence += 1;
                verify_sealed_response(&self.key, &decoded, &self.nonce, provider_sequence)?;
                terminal = Some(decoded);
                break;
            }
            if line.contains("\"event\":\"provider_error\"") {
                return Err(extract_reason(&line));
            }
            println!("{line}");
        }
        let response = terminal.ok_or_else(|| "REMOTE_RESPONSE_TRUNCATED".to_owned())?;
        let complete = self.recv()?;
        if !complete.contains("request_complete") {
            return Err("REMOTE_RESPONSE_TRUNCATED".to_owned());
        }
        if !complete.contains("\"ok\":true") {
            return Err(extract_reason(&complete));
        }
        match response.status() {
            RequestStatus::Ok => Ok(()),
            RequestStatus::Partial => Err("REMOTE_PARTIAL_RESULT".to_owned()),
            RequestStatus::Cancelled => Err("REMOTE_CANCELLED".to_owned()),
            RequestStatus::Failed => Err("REMOTE_REQUEST_FAILED".to_owned()),
            RequestStatus::OutcomeUnknown => Err("REMOTE_OUTCOME_UNKNOWN".to_owned()),
        }
    }

    fn invoke_op(&mut self, request: &UnsignedRequest) -> Result<(), String> {
        let line = render_op_line(request).ok_or_else(|| "REMOTE_REQUEST_INVALID".to_owned())?;
        self.send(&line)?;
        let started = self.recv()?;
        if !started.contains("request_started") {
            return Err("REMOTE_RESPONSE_INVALID".to_owned());
        }
        let mut outcome: Option<(String, String, String)> = None;
        for _ in 0..MAX_RESPONSE_LINES {
            let line = self.recv()?;
            if line.contains("request_complete") {
                if !line.contains("\"ok\":true") && outcome.is_none() {
                    return Err(extract_reason(&line));
                }
                break;
            }
            if line.contains("\"event\":\"provider_op\"") {
                let status = extract_field(&line, "\"status\":\"").unwrap_or_default();
                let reason = extract_field(&line, "\"reason\":\"").unwrap_or_default();
                println!("{line}");
                outcome = Some((status, reason, line));
            } else if line.contains("\"event\":\"provider_error\"") {
                return Err(extract_reason(&line));
            } else {
                println!("{line}");
            }
        }
        match outcome {
            Some((status, _, _)) if status == "ok" => Ok(()),
            Some((status, _, _)) if status == "cancelled" => Ok(()),
            Some((_, reason, _)) if reason.is_empty() => Err("REMOTE_REQUEST_FAILED".to_owned()),
            Some((_, reason, _)) => Err(reason),
            None => Err("REMOTE_RESPONSE_TRUNCATED".to_owned()),
        }
    }
}

/// Verifies a sealed response proof plus its receipt binding.
fn verify_sealed_response(
    key: &[u8; 32],
    response: &AuthenticatedResponse,
    nonce: &ServerNonce,
    expected_sequence: u64,
) -> Result<(), String> {
    let stub = seal_response_placeholder(response, nonce);
    let expected = keyed(key, &response_transcript(&stub));
    verify_response_proof(response, &expected).map_err(|_| "REMOTE_PROOF_INVALID".to_owned())?;
    let receipt = format!(
        "provider-response\t{}.{}\t{}\t{}\t{expected_sequence}",
        response.version().major,
        response.version().minor,
        hex_encode(response.request_id().as_bytes()),
        response.status().as_str(),
    );
    let digest = ProofDigest::from_bytes(*blake3::hash(receipt.as_bytes()).as_bytes());
    if digest != *response.body_digest() {
        return Err("REMOTE_RECEIPT_MISMATCH".to_owned());
    }
    Ok(())
}

fn seal_response_placeholder(
    response: &AuthenticatedResponse,
    nonce: &ServerNonce,
) -> AuthenticatedResponse {
    // Rebuilds the exact transcript input: only version, nonce, identity,
    // status and body digest are bound, never the proof itself.
    let stub_digest = *response.body_digest();
    let stub_status = response.status();
    let stub_id = *response.request_id();
    let stub_version = response.version();
    search_provider_protocol::request::seal_response(
        stub_version,
        *nonce,
        stub_id,
        stub_status,
        stub_digest,
        ProofDigest::from_bytes([0; 32]),
    )
}

fn extract_version(line: &str) -> Result<ProtocolVersion, String> {
    let text =
        extract_field(line, "\"version\":\"").ok_or_else(|| "REMOTE_HELLO_INVALID".to_owned())?;
    let (major, minor) = text
        .split_once('.')
        .ok_or_else(|| "REMOTE_HELLO_INVALID".to_owned())?;
    Ok(ProtocolVersion {
        major: major
            .parse::<u16>()
            .map_err(|_| "REMOTE_HELLO_INVALID".to_owned())?,
        minor: minor
            .parse::<u16>()
            .map_err(|_| "REMOTE_HELLO_INVALID".to_owned())?,
    })
}

fn extract_field(line: &str, marker: &str) -> Option<String> {
    let start = line.find(marker)? + marker.len();
    let end = line[start..].find('"')?;
    Some(line[start..start + end].to_owned())
}

fn extract_reason(line: &str) -> String {
    if let Some(reason) = extract_field(line, "\"reason\":\"").filter(|reason| !reason.is_empty()) {
        return reason;
    }
    if let Some(error) = extract_field(line, "\"error\":\"").filter(|error| !error.is_empty()) {
        return error;
    }
    "REMOTE_REQUEST_FAILED".to_owned()
}

fn write_line(stream: &mut TcpStream, value: &str) -> Result<(), String> {
    stream
        .write_all(value.as_bytes())
        .and_then(|()| stream.write_all(b"\n"))
        .and_then(|()| stream.flush())
        .map_err(|error| format!("REMOTE_WRITE_ERROR:{error}"))
}

fn read_bounded_line(
    reader: &mut BufReader<TcpStream>,
    maximum_bytes: usize,
) -> Result<Option<String>, String> {
    let mut bytes = Vec::new();
    let mut limited =
        reader.take(u64::try_from(maximum_bytes.saturating_add(1)).unwrap_or(u64::MAX));
    let read = limited
        .read_until(b'\n', &mut bytes)
        .map_err(|error| format!("REMOTE_READ_ERROR:{error}"))?;
    if read == 0 {
        return Ok(None);
    }
    if bytes.len() > maximum_bytes || !bytes.ends_with(b"\n") {
        return Err("REMOTE_FRAME_TOO_LARGE".to_owned());
    }
    bytes.pop();
    if bytes.last() == Some(&b'\r') {
        bytes.pop();
    }
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|_| "REMOTE_FRAME_INVALID_UTF8".to_owned())
}

fn decode_16(text: &str) -> Result<[u8; 16], String> {
    let raw = hex_decode(text).ok_or_else(|| "REMOTE_CHALLENGE_INVALID".to_owned())?;
    if raw.len() != 16 {
        return Err("REMOTE_CHALLENGE_INVALID".to_owned());
    }
    let mut output = [0_u8; 16];
    output.copy_from_slice(&raw);
    Ok(output)
}

fn decode_32(text: &str) -> Result<[u8; 32], String> {
    let raw = hex_decode(text).ok_or_else(|| "REMOTE_CHALLENGE_INVALID".to_owned())?;
    if raw.len() != 32 {
        return Err("REMOTE_CHALLENGE_INVALID".to_owned());
    }
    let mut output = [0_u8; 32];
    output.copy_from_slice(&raw);
    Ok(output)
}

/// Lowercase hex encoding for non-secret framing bytes.
#[must_use]
pub fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

const fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn hex_decode(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) || text.is_empty() || text.len() > MAX_PROVIDER_LINE_BYTES {
        return None;
    }
    let mut output = Vec::with_capacity(text.len() / 2);
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        output.push((hex_value(bytes[index])? << 4) | hex_value(bytes[index + 1])?);
        index += 2;
    }
    Some(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use search_provider_protocol::request::verify_envelope_proof;

    #[test]
    fn op_lines_use_the_canonical_wire_spelling() {
        assert_eq!(
            render_op_line(&UnsignedRequest::Status),
            Some("op\tstatus".to_owned())
        );
        assert_eq!(render_op_line(&UnsignedRequest::Health), None);
        let cancel = UnsignedRequest::cancel("00112233445566778899aabbccddeeff").unwrap();
        assert_eq!(
            render_op_line(&cancel),
            Some("op\tcancel\t00112233445566778899aabbccddeeff".to_owned())
        );
        let query = UnsignedRequest::query(false, b"needle").unwrap();
        assert_eq!(
            render_op_line(&query),
            Some(format!("op\tquery\t{}", hex_encode(b"s:needle")))
        );
        assert!(UnsignedRequest::query(false, &[]).is_err());
        assert!(UnsignedRequest::query(false, &vec![b'x'; MAX_QUERY_BYTES + 1]).is_err());
        assert!(UnsignedRequest::cancel("zz").is_err());
        assert!(UnsignedRequest::expand(b"handle", 5, 5).is_err());
        assert!(UnsignedRequest::expand(b"", 0, 8).is_err());
        assert!(UnsignedRequest::ingest(&[]).is_err());
    }

    #[test]
    fn exit_codes_separate_provider_rejection_from_local_errors() {
        for code in [
            "PROVIDER_QUERY_UNAVAILABLE",
            "PROVIDER_UNKNOWN_COMMAND",
            "PROTOCOL_REPLAY_DETECTED",
            "LOOPBACK_DIRECT_COMMAND_FAILED",
            "REMOTE_REQUEST_FAILED",
        ] {
            assert_eq!(exit_for_error(code), 2, "{code}");
        }
        for code in [
            "USAGE_ERROR",
            "REMOTE_CONNECT_ERROR:refused",
            "ENDPOINT_DESCRIPTOR_INVALID",
            "REMOTE_TOKEN_TOO_SHORT",
        ] {
            assert_eq!(exit_for_error(code), 1, "{code}");
        }
    }

    #[test]
    fn endpoint_descriptor_is_namespaced_loopback_only() {
        let dir = std::env::temp_dir().join(format!(
            "eliot-cli-endpoint-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(dir.join("runtime")).unwrap();
        // Missing descriptor fails closed (no hidden default dial).
        assert!(read_endpoint_descriptor(&dir).is_err());
        std::fs::write(
            dir.join("runtime").join("endpoint.v1"),
            "ELIOT_SEARCH_ENDPOINT_V1\naddress=127.0.0.1:39171\n",
        )
        .unwrap();
        let endpoint = read_endpoint_descriptor(&dir).unwrap();
        assert_eq!(endpoint.address.port(), 39171);
        std::fs::write(
            dir.join("runtime").join("endpoint.v1"),
            "ELIOT_SEARCH_ENDPOINT_V1\naddress=127.0.0.1:1\naddress=127.0.0.1:2\n",
        )
        .unwrap();
        assert!(read_endpoint_descriptor(&dir).is_err());
        std::fs::write(
            dir.join("runtime").join("endpoint.v1"),
            "ELIOT_SEARCH_ENDPOINT_V1\naddress=93.184.216.34:80\n",
        )
        .unwrap();
        assert_eq!(
            read_endpoint_descriptor(&dir).unwrap_err(),
            "ENDPOINT_NON_LOOPBACK_DENIED"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn response_receipt_verification_is_tamper_evident() {
        let key = [0x77; 32];
        let nonce = ServerNonce::from_bytes([0x11; 16]).unwrap();
        let id = RequestId::from_bytes([0x22; 16]);
        let digest = ProofDigest::from_bytes(*blake3::hash(b"receipt-input").as_bytes());
        let stub = seal_envelope(
            PROVIDER_VERSION,
            nonce,
            id,
            ControlCommand::Health,
            digest,
            ProofDigest::from_bytes([0; 32]),
        );
        let proof = keyed(&key, &envelope_transcript(&stub));
        let sealed = seal_envelope(
            PROVIDER_VERSION,
            nonce,
            id,
            ControlCommand::Health,
            digest,
            proof,
        );
        verify_envelope_proof(&sealed, &proof).unwrap();
        assert!(verify_envelope_proof(&sealed, &ProofDigest::from_bytes([9; 32])).is_err());
    }

    #[test]
    fn key_derivation_is_deterministic_and_bounded() {
        let first = shim_key_from_bytes(&[0x41; 48]).unwrap();
        let second = shim_key_from_bytes(&[0x41; 48]).unwrap();
        assert_eq!(first, second);
        assert_ne!(first, shim_key_from_bytes(&[0x42; 48]).unwrap());
        assert!(shim_key_from_bytes(&[0x41; 31]).is_err());
        assert_ne!(first, [0; 32]);
    }
}

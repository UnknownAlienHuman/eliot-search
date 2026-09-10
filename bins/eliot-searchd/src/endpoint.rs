//! Authenticated bounded loopback transport for the development daemon.
//!
//! `pairing_blake3_v1` is the only wire: every connection runs a mutual
//! keyed-BLAKE3 ceremony over the approved `search-provider-protocol`
//! pairing transcripts. The endpoint owns no secret storage and performs no
//! key derivation beyond the transport framing — the 32-byte key is supplied
//! per connection by an [`EndpointKeySource`] (a purpose-bound
//! [`SecretLease`] in the product path), ceremony material is drawn from a
//! keyed PRF, challenges are single-use through a bounded [`PairingLedger`],
//! and only 32-byte digests ever cross the socket.
//!
//! The product path is [`serve_loopback_with_source`] with a lease-bound
//! source. No home-grown hash authentication protocol is defined here:
//! transcripts, the pairing state machine and fixed-work proof comparison
//! all come from `search-provider-protocol`, and keyed BLAKE3 is computed
//! with the pinned `blake3 v1.8.2` dependency.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use search_contracts::ProtocolVersion;
use search_provider_protocol::pairing::{
    ClientNonce, PairingChallenge, PairingLedger, PairingMachine, PairingTranscript, ProofDigest,
    SessionId,
};
// Reference-client transcript builders and comparison (cfg(test)); the
// product server path only verifies through `PairingMachine`.
#[cfg(test)]
use search_provider_protocol::pairing::{
    client_proof_transcript, server_proof_transcript, verify_proof,
};

/// Negotiated loopback-pairing version bound into every proof transcript.
pub const PAIRING_PROTOCOL_VERSION: ProtocolVersion = ProtocolVersion { major: 1, minor: 0 };
/// Wire authentication identifier; the legacy `sha256_challenge_v1` is gone.
pub const PAIRING_AUTHENTICATION_ID: &str = "pairing_blake3_v1";

#[cfg(test)]
const MAX_CHALLENGE_LINE_BYTES: usize = 512;
const MAX_AUTH_LINE_BYTES: usize = 256;
#[cfg(test)]
const MAX_VERIFIED_LINE_BYTES: usize = 256;
const MAX_COMMAND_LINE_BYTES: usize = 128 * 1024;
const MAX_COMMANDS_PER_CONNECTION: usize = 4096;
const MAX_PAIRING_CHALLENGES: usize = 4096;
// Silent-client read bound and slow-reader write bound. They share a value
// but never a meaning: the read timeout is not the socket configuration, and
// neither is the proxy child request (120s) / startup (30s) / cleanup (5s)
// deadline owned by `proxy_child::ChildLimits`.
const READ_TIMEOUT: Duration = Duration::from_secs(30);
const WRITE_TIMEOUT: Duration = Duration::from_secs(30);

const BINDING_DOMAIN: &[u8] = b"eliot-search/loopback-binding/v1\0";
const BINDING_ROLE: &[u8] = b"loopback-operator";
const SESSION_PRF_DOMAIN: &[u8] = b"eliot-search/loopback-session/v1\0";
const NONCE_PRF_DOMAIN: &[u8] = b"eliot-search/loopback-client-nonce/v1\0";
const CHALLENGE_PRF_DOMAIN: &[u8] = b"eliot-search/loopback-challenge/v1\0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EndpointAction {
    Continue,
    Shutdown,
    /// Unusable handler/output channel; close listener without any further reply.
    Abort,
}

/// Per-connection key supply owned by the secret-owning side.
///
/// The key is exposed only for the duration of one callback so proofs are
/// computed inside the lease window without the key ever crossing the
/// socket, reaching logs, or escaping by value. A lease-bound
/// implementation returns `Err` once the lease expires or the reference is
/// revoked, which fails the connection closed without a challenge.
pub trait EndpointKeySource {
    /// Exposes the active 32-byte pairing key for one callback.
    fn with_endpoint_key<T>(&mut self, use_key: impl FnOnce(&[u8; 32]) -> T) -> Result<T, String>;
}

/// Lease-bound entry: mutual keyed proofs over pairing transcripts.
///
/// `source` is consulted once per connection inside the lease window; the
/// bounded challenge ledger is shared across all connections of this
/// listener and fails closed (never evicts) when full.
pub fn serve_loopback_with_source<F, S>(port: u16, source: &mut S, handler: F) -> Result<(), String>
where
    F: FnMut(&str, &mut TcpStream) -> Result<EndpointAction, String>,
    S: EndpointKeySource,
{
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, port))
        .map_err(|error| format!("ENDPOINT_BIND_ERROR:{error}"))?;
    let local = listener
        .local_addr()
        .map_err(|error| format!("ENDPOINT_LOCAL_ADDRESS_ERROR:{error}"))?;
    if !local.ip().is_loopback() {
        return Err("ENDPOINT_NON_LOOPBACK_BIND_DENIED".to_owned());
    }
    println!(
        "{{\"event\":\"loopback_ready\",\"address\":\"{local}\",\"protocol_version\":1,\"authentication\":\"{PAIRING_AUTHENTICATION_ID}\"}}",
    );

    let ledger = PairingLedger::new(MAX_PAIRING_CHALLENGES)
        .map_err(|_| "ENDPOINT_REPLAY_LEDGER_INVALID".to_owned())?;
    serve_listener(&listener, source, ledger, handler)
}

fn serve_listener<F, S>(
    listener: &TcpListener,
    source: &mut S,
    mut ledger: PairingLedger,
    mut handler: F,
) -> Result<(), String>
where
    F: FnMut(&str, &mut TcpStream) -> Result<EndpointAction, String>,
    S: EndpointKeySource,
{
    let mut connection_sequence = 0_u64;
    for incoming in listener.incoming() {
        let mut stream = match incoming {
            Ok(stream) => stream,
            Err(error) => {
                eprintln!(
                    "{{\"error\":\"ENDPOINT_ACCEPT_ERROR\",\"detail_class\":\"{}\"}}",
                    error.kind()
                );
                continue;
            }
        };
        connection_sequence = connection_sequence
            .checked_add(1)
            .ok_or_else(|| "ENDPOINT_CONNECTION_SEQUENCE_EXHAUSTED".to_owned())?;
        let peer = stream
            .peer_addr()
            .map_err(|error| format!("ENDPOINT_PEER_ADDRESS_ERROR:{error}"))?;
        if !peer.ip().is_loopback() {
            let _ = write_line(&mut stream, "{\"error\":\"ENDPOINT_LOOPBACK_REQUIRED\"}");
            continue;
        }
        stream
            .set_read_timeout(Some(READ_TIMEOUT))
            .and_then(|()| stream.set_write_timeout(Some(WRITE_TIMEOUT)))
            .map_err(|error| format!("ENDPOINT_TIMEOUT_CONFIGURATION_ERROR:{error}"))?;

        match serve_connection(
            stream,
            connection_sequence,
            source,
            &mut ledger,
            &mut handler,
        ) {
            Ok(EndpointAction::Continue) => {}
            Ok(EndpointAction::Shutdown) => return Ok(()),
            // Not a per-client validation failure: the sole child is no longer
            // reusable. Dropping the listener also refuses queued/new clients.
            Ok(EndpointAction::Abort) => return Err("ENDPOINT_HANDLER_ABORTED".to_owned()),
            Err(error) => {
                eprintln!(
                    "{{\"error\":\"{}\",\"connection_sequence\":{}}}",
                    sanitize_code(&error),
                    connection_sequence,
                );
            }
        }
    }
    Ok(())
}

fn serve_connection<F, S>(
    mut stream: TcpStream,
    connection_sequence: u64,
    source: &mut S,
    ledger: &mut PairingLedger,
    handler: &mut F,
) -> Result<EndpointAction, String>
where
    F: FnMut(&str, &mut TcpStream) -> Result<EndpointAction, String>,
    S: EndpointKeySource,
{
    let mut reader = authenticate_connection(&mut stream, connection_sequence, source, ledger)?;

    let mut request_sequence = 0_u64;
    loop {
        let Some(command) = read_bounded_line(&mut reader, MAX_COMMAND_LINE_BYTES)? else {
            return Ok(EndpointAction::Continue);
        };
        if command.is_empty() {
            return Err("ENDPOINT_EMPTY_COMMAND".to_owned());
        }
        if request_sequence >= u64::try_from(MAX_COMMANDS_PER_CONNECTION).unwrap_or(u64::MAX) {
            write_line(
                &mut stream,
                "{\"event\":\"request_complete\",\"ok\":false,\"error\":\"ENDPOINT_REQUEST_LIMIT_EXCEEDED\"}",
            )
            .map_err(|error| format!("ENDPOINT_WRITE_ERROR:{error}"))?;
            return Ok(EndpointAction::Continue);
        }
        write_line(
            &mut stream,
            &format!("{{\"event\":\"request_started\",\"sequence\":{request_sequence}}}"),
        )
        .map_err(|error| format!("ENDPOINT_WRITE_ERROR:{error}"))?;
        let outcome = handler(&command, &mut stream);
        match complete_request(&mut stream, outcome, request_sequence) {
            EndpointAction::Continue => {
                request_sequence = request_sequence
                    .checked_add(1)
                    .ok_or_else(|| "ENDPOINT_REQUEST_SEQUENCE_EXHAUSTED".to_owned())?;
            }
            action => return Ok(action),
        }
    }
}

/// No suffix may follow a failed handler exchange: a partial JSON frame may
/// already be on the socket. Failure of the outer acknowledgement is also a
/// post-dispatch failure, even if the child's own terminal frame was complete.
fn complete_request(
    writer: &mut impl Write,
    outcome: Result<EndpointAction, String>,
    sequence: u64,
) -> EndpointAction {
    let (action, status) = match outcome {
        Ok(EndpointAction::Abort) => return EndpointAction::Abort,
        Ok(action) => (action, "\"ok\":true".to_owned()),
        Err(error) => (
            EndpointAction::Continue,
            format!("\"ok\":false,\"error\":\"{}\"", sanitize_code(&error)),
        ),
    };
    let frame = format!("{{\"event\":\"request_complete\",\"sequence\":{sequence},{status}}}");
    if write_line(writer, &frame).is_err() {
        EndpointAction::Abort
    } else {
        action
    }
}

/// Role-bound binding digest for one pairing key.
///
/// Must equal `secret_composition::derive_binding_digest` on the same key;
/// the process test proves that agreement on fixed vectors. The digest binds
/// the fixed loopback role to the key and travels in the clear; the key
/// itself never does.
#[must_use]
pub fn pairing_binding_digest(key: &[u8; 32]) -> ProofDigest {
    let mut hasher = blake3::Hasher::new();
    hasher.update(BINDING_DOMAIN);
    hasher.update(BINDING_ROLE);
    hasher.update(&[0]);
    hasher.update(key);
    ProofDigest::from_bytes(*hasher.finalize().as_bytes())
}

/// Keyed proof over one exact pairing transcript (secret-owning side only).
#[must_use]
pub fn keyed_proof(key: &[u8; 32], transcript: &PairingTranscript) -> ProofDigest {
    ProofDigest::from_bytes(*blake3::keyed_hash(key, transcript.as_bytes()).as_bytes())
}

/// Runs the mutual pairing ceremony for one connection and returns the
/// authenticated command reader.
///
/// Key handling matches the module contract: each proof is computed inside
/// one lease-window callback, challenges are consumed exactly once, and only
/// digests cross the socket.
fn authenticate_connection<S>(
    stream: &mut TcpStream,
    connection_sequence: u64,
    source: &mut S,
    ledger: &mut PairingLedger,
) -> Result<BufReader<TcpStream>, String>
where
    S: EndpointKeySource,
{
    // One key callback derives the binding digest, the fresh ceremony
    // material, the ceremony machine and the expected client proof. The key
    // never leaves this callback; only digests and opaque nonces do.
    let prepared = source
        .with_endpoint_key(|key| {
            let binding = pairing_binding_digest(key);
            let (session, nonce, challenge) = derive_ceremony_material(key, connection_sequence)?;
            let mut machine = PairingMachine::new(PAIRING_PROTOCOL_VERSION, binding);
            machine
                .issue_challenge(session, nonce, challenge)
                .map_err(|_| "ENDPOINT_PAIRING_ISSUE_FAILED".to_owned())?;
            let transcript = machine
                .client_transcript()
                .map_err(|_| "ENDPOINT_PAIRING_TRANSCRIPT_FAILED".to_owned())?;
            let expected_client = keyed_proof(key, &transcript);
            Ok::<_, String>((machine, binding, session, nonce, challenge, expected_client))
        })
        .map_err(|_| "ENDPOINT_KEY_UNAVAILABLE".to_owned())??;
    let (mut machine, binding, session, nonce, challenge, expected_client) = prepared;

    // Single-use challenge: an exact replay fails closed here, before any
    // proof comparison. A full ledger fails closed as well, never evicting.
    ledger.consume(session, &challenge).map_err(|error| {
        machine.fail();
        error.to_string()
    })?;

    write_line(
        stream,
        &encode_challenge(binding, session, &nonce, &challenge),
    )
    .map_err(|error| format!("ENDPOINT_CHALLENGE_WRITE_ERROR:{error}"))?;
    let read_stream = stream
        .try_clone()
        .map_err(|error| format!("ENDPOINT_STREAM_CLONE_ERROR:{error}"))?;
    let mut reader = BufReader::new(read_stream);
    let authentication = read_bounded_line(&mut reader, MAX_AUTH_LINE_BYTES)?
        .ok_or_else(|| "ENDPOINT_AUTHENTICATION_MISSING".to_owned())?;
    let observed = parse_auth_line(&authentication)?;
    if machine
        .verify_client_proof(&expected_client, &observed)
        .is_err()
    {
        let _ = write_line(stream, "{\"error\":\"AUTHENTICATION_FAILED\"}");
        return Err("ENDPOINT_AUTHENTICATION_FAILED".to_owned());
    }

    // Mutual proof: the provider proves the same key over the server-domain
    // transcript. A second key callback keeps the key inside the lease
    // window; the machine state already binds the ceremony.
    let provider_proof = source
        .with_endpoint_key(|key| {
            machine
                .server_transcript()
                .map(|transcript| keyed_proof(key, &transcript))
        })
        .map_err(|_| "ENDPOINT_KEY_UNAVAILABLE".to_owned())?
        .map_err(|_| "ENDPOINT_PAIRING_TRANSCRIPT_FAILED".to_owned())?;
    machine
        .issue_provider_proof(provider_proof)
        .map_err(|_| "ENDPOINT_PAIRING_PROVIDER_FAILED".to_owned())?;
    let verified = machine
        .into_verified()
        .map_err(|_| "ENDPOINT_PAIRING_PROVIDER_FAILED".to_owned())?;
    debug_assert_eq!(verified.version(), PAIRING_PROTOCOL_VERSION);

    write_line(
        stream,
        &format!(
            "PAIRING_VERIFIED\tproof={}",
            hex_encode(verified.provider_proof().as_bytes())
        ),
    )
    .map_err(|error| format!("ENDPOINT_READY_WRITE_ERROR:{error}"))?;
    write_line(
        stream,
        concat!(
            "{\"event\":\"authenticated\",\"protocol_version\":1,",
            "\"transport\":\"loopback_tcp\",",
            "\"authentication\":\"pairing_blake3_v1\"}"
        ),
    )
    .map_err(|error| format!("ENDPOINT_READY_WRITE_ERROR:{error}"))?;
    Ok(reader)
}

/// Fresh per-connection ceremony material from a keyed PRF.
///
/// Domains separate session, nonce and challenge draws; the connection
/// sequence and wall-clock nanos make each draw unique per connection. An
/// all-zero draw (malformed ceremony material) fails the connection closed
/// instead of retrying without bound.
fn derive_ceremony_material(
    key: &[u8; 32],
    connection_sequence: u64,
) -> Result<(SessionId, ClientNonce, PairingChallenge), String> {
    fn draw(key: &[u8; 32], domain: &[u8], sequence: u64, nanos: u128) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new_keyed(key);
        hasher.update(domain);
        hasher.update(&sequence.to_be_bytes());
        hasher.update(&nanos.to_be_bytes());
        *hasher.finalize().as_bytes()
    }
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "ENDPOINT_CLOCK_INVALID".to_owned())?
        .as_nanos();
    let session_bytes = draw(key, SESSION_PRF_DOMAIN, connection_sequence, nanos);
    let nonce_bytes = draw(key, NONCE_PRF_DOMAIN, connection_sequence, nanos);
    let challenge_bytes = draw(key, CHALLENGE_PRF_DOMAIN, connection_sequence, nanos);
    let mut session_raw = [0_u8; 16];
    let mut nonce_raw = [0_u8; 16];
    session_raw.copy_from_slice(&session_bytes[..16]);
    nonce_raw.copy_from_slice(&nonce_bytes[..16]);
    let session =
        SessionId::from_bytes(session_raw).map_err(|_| "ENDPOINT_ENTROPY_INVALID".to_owned())?;
    let nonce =
        ClientNonce::from_bytes(nonce_raw).map_err(|_| "ENDPOINT_ENTROPY_INVALID".to_owned())?;
    let challenge = PairingChallenge::from_bytes(challenge_bytes)
        .map_err(|_| "ENDPOINT_ENTROPY_INVALID".to_owned())?;
    Ok((session, nonce, challenge))
}

fn encode_challenge(
    binding: ProofDigest,
    session: SessionId,
    nonce: &ClientNonce,
    challenge: &PairingChallenge,
) -> String {
    format!(
        "PAIRING_CHALLENGE\tv={}.{}\tsession={}\tnonce={}\tchallenge={}\tbinding={}",
        PAIRING_PROTOCOL_VERSION.major,
        PAIRING_PROTOCOL_VERSION.minor,
        hex_encode(session.as_bytes()),
        hex_encode(nonce.as_bytes()),
        hex_encode(challenge.as_bytes()),
        hex_encode(binding.as_bytes()),
    )
}

/// Parsed server challenge: version plus ceremony material and binding.
///
/// Reference-client helper (`cfg(test)`): the product client lives in
/// `bins/eliot-search` (T18 client slice); this type proves the wire from the
/// test side, alongside the unit and process tests below.
#[cfg(test)]
pub struct ParsedChallenge {
    /// Negotiated version stated by the server.
    pub version: ProtocolVersion,
    /// Pairing session identifier.
    pub session: SessionId,
    /// Client nonce bound into both transcripts.
    pub nonce: ClientNonce,
    /// Fresh single-use provider challenge.
    pub challenge: PairingChallenge,
    /// Role-bound binding digest for the pairing key.
    pub binding: ProofDigest,
}

/// Strict challenge-line parse: exact field count, order, prefixes and
/// lowercase hex. Anything else fails closed without a proof attempt.
///
/// Reference-client helper (`cfg(test)`); see [`ParsedChallenge`].
#[cfg(test)]
pub fn parse_challenge_line(line: &str) -> Result<ParsedChallenge, String> {
    let parts: Vec<&str> = line.split('\t').collect();
    if parts.len() != 6 || parts[0] != "PAIRING_CHALLENGE" {
        return Err("ENDPOINT_PAIRING_FORMAT_INVALID".to_owned());
    }
    let version_text = parts[1]
        .strip_prefix("v=")
        .ok_or_else(|| "ENDPOINT_PAIRING_FORMAT_INVALID".to_owned())?;
    let (major, minor) = version_text
        .split_once('.')
        .ok_or_else(|| "ENDPOINT_PAIRING_FORMAT_INVALID".to_owned())?;
    let version = ProtocolVersion {
        major: major
            .parse::<u16>()
            .map_err(|_| "ENDPOINT_PAIRING_FORMAT_INVALID".to_owned())?,
        minor: minor
            .parse::<u16>()
            .map_err(|_| "ENDPOINT_PAIRING_FORMAT_INVALID".to_owned())?,
    };
    if version != PAIRING_PROTOCOL_VERSION {
        return Err("ENDPOINT_PAIRING_VERSION_MISMATCH".to_owned());
    }
    let session = SessionId::from_bytes(
        parts[2]
            .strip_prefix("session=")
            .ok_or_else(|| "ENDPOINT_PAIRING_FORMAT_INVALID".to_owned())
            .and_then(hex_decode_16)?,
    )
    .map_err(|_| "ENDPOINT_PAIRING_FORMAT_INVALID".to_owned())?;
    let nonce = ClientNonce::from_bytes(
        parts[3]
            .strip_prefix("nonce=")
            .ok_or_else(|| "ENDPOINT_PAIRING_FORMAT_INVALID".to_owned())
            .and_then(hex_decode_16)?,
    )
    .map_err(|_| "ENDPOINT_PAIRING_FORMAT_INVALID".to_owned())?;
    let challenge = PairingChallenge::from_bytes(
        parts[4]
            .strip_prefix("challenge=")
            .ok_or_else(|| "ENDPOINT_PAIRING_FORMAT_INVALID".to_owned())
            .and_then(hex_decode_32)?,
    )
    .map_err(|_| "ENDPOINT_PAIRING_FORMAT_INVALID".to_owned())?;
    let binding = ProofDigest::from_bytes(
        parts[5]
            .strip_prefix("binding=")
            .ok_or_else(|| "ENDPOINT_PAIRING_FORMAT_INVALID".to_owned())
            .and_then(hex_decode_32)?,
    );
    Ok(ParsedChallenge {
        version,
        session,
        nonce,
        challenge,
        binding,
    })
}

fn parse_auth_line(line: &str) -> Result<ProofDigest, String> {
    let parts: Vec<&str> = line.split('\t').collect();
    if parts.len() != 2 || parts[0] != "PAIRING_AUTH" {
        return Err("ENDPOINT_AUTHENTICATION_INVALID".to_owned());
    }
    let proof = parts[1]
        .strip_prefix("proof=")
        .ok_or_else(|| "ENDPOINT_AUTHENTICATION_INVALID".to_owned())
        .and_then(hex_decode_32)
        .map_err(|_| "ENDPOINT_AUTHENTICATION_INVALID".to_owned())?;
    Ok(ProofDigest::from_bytes(proof))
}

/// Strict verified-line parse for the mutual provider proof.
///
/// Reference-client helper (`cfg(test)`); see [`ParsedChallenge`].
#[cfg(test)]
pub fn parse_verified_line(line: &str) -> Result<ProofDigest, String> {
    let parts: Vec<&str> = line.split('\t').collect();
    if parts.len() != 2 || parts[0] != "PAIRING_VERIFIED" {
        return Err("ENDPOINT_PAIRING_FORMAT_INVALID".to_owned());
    }
    parts[1]
        .strip_prefix("proof=")
        .ok_or_else(|| "ENDPOINT_PAIRING_FORMAT_INVALID".to_owned())
        .and_then(hex_decode_32)
        .map(ProofDigest::from_bytes)
        .map_err(|_| "ENDPOINT_PAIRING_FORMAT_INVALID".to_owned())
}

/// Client-side client-proof computation over the parsed challenge.
///
/// Reference-client helper (`cfg(test)`); see [`ParsedChallenge`].
#[cfg(test)]
#[must_use]
pub fn client_proof_for_challenge(key: &[u8; 32], challenge: &ParsedChallenge) -> ProofDigest {
    keyed_proof(
        key,
        &client_proof_transcript(
            challenge.version,
            &challenge.binding,
            challenge.session,
            &challenge.nonce,
            &challenge.challenge,
        ),
    )
}

/// Client-side provider-proof verification over the parsed challenge.
///
/// Reference-client helper (`cfg(test)`); see [`ParsedChallenge`].
#[cfg(test)]
#[must_use]
pub fn verify_provider_proof(
    key: &[u8; 32],
    challenge: &ParsedChallenge,
    observed: &ProofDigest,
) -> bool {
    let expected = keyed_proof(
        key,
        &server_proof_transcript(
            challenge.version,
            &challenge.binding,
            challenge.session,
            &challenge.nonce,
            &challenge.challenge,
        ),
    );
    verify_proof(&expected, observed)
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0F)]));
    }
    output
}

fn hex_value(byte: u8) -> Result<u8, String> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err("ENDPOINT_PAIRING_FORMAT_INVALID".to_owned()),
    }
}

/// Reference-client helper (`cfg(test)`); the server path only decodes
/// 32-byte digests through [`hex_decode_32`].
#[cfg(test)]
fn hex_decode_16(text: &str) -> Result<[u8; 16], String> {
    let bytes = text.as_bytes();
    if bytes.len() != 32 {
        return Err("ENDPOINT_PAIRING_FORMAT_INVALID".to_owned());
    }
    let mut output = [0_u8; 16];
    for (index, slot) in output.iter_mut().enumerate() {
        *slot = (hex_value(bytes[2 * index])? << 4) | hex_value(bytes[2 * index + 1])?;
    }
    Ok(output)
}

fn hex_decode_32(text: &str) -> Result<[u8; 32], String> {
    let bytes = text.as_bytes();
    if bytes.len() != 64 {
        return Err("ENDPOINT_PAIRING_FORMAT_INVALID".to_owned());
    }
    let mut output = [0_u8; 32];
    for (index, slot) in output.iter_mut().enumerate() {
        *slot = (hex_value(bytes[2 * index])? << 4) | hex_value(bytes[2 * index + 1])?;
    }
    Ok(output)
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
        .map_err(|error| match error.kind() {
            // A silent client holds the connection open without a frame. This
            // read timeout is distinct from a socket configuration failure and
            // from the proxy child request/cleanup deadlines.
            io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock => {
                "ENDPOINT_READ_TIMEOUT".to_owned()
            }
            _ => format!("ENDPOINT_READ_ERROR:{error}"),
        })?;
    if read == 0 {
        return Ok(None);
    }
    if bytes.len() > maximum_bytes || !bytes.ends_with(b"\n") {
        return Err("ENDPOINT_FRAME_TOO_LARGE".to_owned());
    }
    bytes.pop();
    if bytes.last() == Some(&b'\r') {
        bytes.pop();
    }
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|_| "ENDPOINT_FRAME_INVALID_UTF8".to_owned())
}

fn write_line(stream: &mut impl Write, value: &str) -> io::Result<()> {
    stream.write_all(value.as_bytes())?;
    stream.write_all(b"\n")?;
    stream.flush()
}

fn sanitize_code(error: &str) -> String {
    let code = error.split(':').next().unwrap_or("ENDPOINT_ERROR");
    let mut output = String::with_capacity(code.len().min(128));
    for character in code.chars().take(128) {
        if character.is_ascii_uppercase()
            || character.is_ascii_digit()
            || matches!(character, '_' | '-' | '.')
        {
            output.push(character);
        } else {
            output.push('_');
        }
    }
    if output.is_empty() {
        "ENDPOINT_ERROR".to_owned()
    } else {
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestKeySource {
        key: [u8; 32],
        failures_remaining: usize,
    }

    impl TestKeySource {
        fn new(key: [u8; 32]) -> Self {
            Self {
                key,
                failures_remaining: 0,
            }
        }

        fn failing_once() -> Self {
            Self {
                key: [0xA5; 32],
                failures_remaining: 1,
            }
        }
    }

    impl Drop for TestKeySource {
        fn drop(&mut self) {
            self.key.fill(0);
        }
    }

    impl EndpointKeySource for TestKeySource {
        fn with_endpoint_key<T>(
            &mut self,
            use_key: impl FnOnce(&[u8; 32]) -> T,
        ) -> Result<T, String> {
            if self.failures_remaining > 0 {
                self.failures_remaining -= 1;
                return Err("TEST_KEY_UNAVAILABLE".to_owned());
            }
            Ok(use_key(&self.key))
        }
    }

    /// Drives one full client handshake against a live listener connection.
    fn client_handshake(
        reader: &mut BufReader<TcpStream>,
        stream: &mut TcpStream,
        key: &[u8; 32],
    ) -> Result<ProofDigest, String> {
        let challenge_line = read_bounded_line(reader, MAX_CHALLENGE_LINE_BYTES)
            .map_err(|error| format!("TEST_CHALLENGE_READ:{error}"))?
            .ok_or_else(|| "TEST_CHALLENGE_MISSING".to_owned())?;
        let challenge =
            parse_challenge_line(&challenge_line).map_err(|error| format!("TEST_PARSE:{error}"))?;
        // The honest client re-derives the binding: a tampered binding fails
        // closed here, before any proof is attempted.
        if pairing_binding_digest(key) != challenge.binding {
            return Err("TEST_BINDING_MISMATCH".to_owned());
        }
        let proof = client_proof_for_challenge(key, &challenge);
        write_line(
            stream,
            &format!("PAIRING_AUTH\tproof={}", hex_encode(proof.as_bytes())),
        )
        .map_err(|error| format!("TEST_AUTH_WRITE:{error}"))?;
        let verified_line = read_bounded_line(reader, MAX_VERIFIED_LINE_BYTES)
            .map_err(|error| format!("TEST_VERIFIED_READ:{error}"))?
            .ok_or_else(|| "TEST_VERIFIED_MISSING".to_owned())?;
        let provider_proof =
            parse_verified_line(&verified_line).map_err(|error| format!("TEST_PARSE:{error}"))?;
        if !verify_provider_proof(key, &challenge, &provider_proof) {
            return Err("TEST_PROVIDER_PROOF_INVALID".to_owned());
        }
        let ready = read_bounded_line(reader, 1024)
            .map_err(|error| format!("TEST_READY_READ:{error}"))?
            .ok_or_else(|| "TEST_READY_MISSING".to_owned())?;
        if !ready.contains("\"event\":\"authenticated\"")
            || !ready.contains("\"authentication\":\"pairing_blake3_v1\"")
        {
            return Err("TEST_READY_INVALID".to_owned());
        }
        Ok(provider_proof)
    }

    /// Listener test handle: bound address, completion channel and thread.
    type SpawnedListener = (
        std::net::SocketAddr,
        std::sync::mpsc::Receiver<(Result<(), String>, usize)>,
        std::thread::JoinHandle<()>,
    );

    fn spawn_listener(
        source: TestKeySource,
        handler: impl FnMut(&str, &mut TcpStream) -> Result<EndpointAction, String> + Send + 'static,
    ) -> SpawnedListener {
        use std::sync::mpsc;
        let (done, result) = mpsc::channel();
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let mut source = source;
            let mut calls = 0_usize;
            let mut handler = handler;
            let ledger = PairingLedger::new(MAX_PAIRING_CHALLENGES).unwrap();
            let status = serve_listener(&listener, &mut source, ledger, |command, stream| {
                calls += 1;
                handler(command, stream)
            });
            let _ = done.send((status, calls));
        });
        (address, result, server)
    }

    #[test]
    fn binding_digest_is_deterministic_and_role_bound() {
        let first = pairing_binding_digest(&[0x42; 32]);
        let second = pairing_binding_digest(&[0x42; 32]);
        assert_eq!(first, second);
        assert_ne!(first, pairing_binding_digest(&[0x43; 32]));
        // Formatting is redacted: no key-derived hex ever reaches logs.
        assert_eq!(format!("{first:?}"), "ProofDigest(<redacted>)");
    }

    #[test]
    fn challenge_line_round_trips_with_strict_parse() {
        let key = [0x11; 32];
        let binding = pairing_binding_digest(&key);
        let (session, nonce, challenge) = derive_ceremony_material(&key, 7).unwrap();
        let line = encode_challenge(binding, session, &nonce, &challenge);
        assert!(line.len() < MAX_CHALLENGE_LINE_BYTES);
        let parsed = parse_challenge_line(&line).unwrap();
        assert_eq!(parsed.version, PAIRING_PROTOCOL_VERSION);
        assert_eq!(parsed.session, session);
        assert_eq!(parsed.nonce, nonce);
        assert_eq!(parsed.challenge, challenge);
        assert_eq!(parsed.binding, binding);
        // Reordered, truncated, uppercased and version-tampered lines fail.
        let mut reordered = line.clone();
        reordered = reordered.replacen("session=", "nonce=", 1);
        assert!(parse_challenge_line(&reordered).is_err());
        assert!(parse_challenge_line(&line[..line.len() - 1]).is_err());
        assert!(parse_challenge_line(&line.to_uppercase()).is_err());
        let tampered = line.replacen("v=1.0", "v=1.1", 1);
        assert!(matches!(
            parse_challenge_line(&tampered),
            Err(error) if error == "ENDPOINT_PAIRING_VERSION_MISMATCH"
        ));
    }

    #[test]
    fn full_handshake_proves_mutually_then_dispatches() {
        let key = [0x2A; 32];
        let (address, result, server) =
            spawn_listener(TestKeySource::new(key), |command, stream| {
                write_line(stream, &format!("{{\"echo\":\"{command}\"}}"))
                    .map_err(|_| "FIXTURE_WRITE_FAILED".to_owned())?;
                if command == "shutdown" {
                    Ok(EndpointAction::Shutdown)
                } else {
                    Ok(EndpointAction::Continue)
                }
            });
        let timeout = Duration::from_secs(5);
        let mut client = TcpStream::connect_timeout(&address, timeout).unwrap();
        client.set_read_timeout(Some(timeout)).unwrap();
        client.set_write_timeout(Some(timeout)).unwrap();
        let mut reader = BufReader::new(client.try_clone().unwrap());
        client_handshake(&mut reader, &mut client, &key).unwrap();
        write_line(&mut client, "hello").unwrap();
        let started = read_bounded_line(&mut reader, 1024).unwrap().unwrap();
        assert!(started.contains("\"event\":\"request_started\""));
        let echo = read_bounded_line(&mut reader, 1024).unwrap().unwrap();
        assert_eq!(echo, "{\"echo\":\"hello\"}");
        let complete = read_bounded_line(&mut reader, 1024).unwrap().unwrap();
        assert!(complete.contains("\"ok\":true"));
        write_line(&mut client, "shutdown").unwrap();
        let _ = read_bounded_line(&mut reader, 1024).unwrap().unwrap();
        let _ = read_bounded_line(&mut reader, 4096).unwrap();
        let _ = read_bounded_line(&mut reader, 4096).unwrap();
        let (status, calls) = result.recv_timeout(timeout).expect("bounded listener exit");
        server.join().unwrap();
        assert_eq!(status, Ok(()));
        assert_eq!(calls, 2);
    }

    #[test]
    fn wrong_key_tampered_proof_and_reused_ledger_entry_fail() {
        let key = [0x3B; 32];
        // Wrong key: binding check fails on the client before any proof.
        let (address, result, server) =
            spawn_listener(TestKeySource::new(key), |_, _| Ok(EndpointAction::Continue));
        let timeout = Duration::from_secs(5);
        let mut client = TcpStream::connect_timeout(&address, timeout).unwrap();
        client.set_read_timeout(Some(timeout)).unwrap();
        client.set_write_timeout(Some(timeout)).unwrap();
        let mut reader = BufReader::new(client.try_clone().unwrap());
        assert_eq!(
            client_handshake(&mut reader, &mut client, &[0x3C; 32]),
            Err("TEST_BINDING_MISMATCH".to_owned())
        );
        drop(client);
        drop(reader);
        // Tampered proof with the right key: the server rejects.
        let mut client = TcpStream::connect_timeout(&address, timeout).unwrap();
        client.set_read_timeout(Some(timeout)).unwrap();
        client.set_write_timeout(Some(timeout)).unwrap();
        let mut reader = BufReader::new(client.try_clone().unwrap());
        let challenge_line = read_bounded_line(&mut reader, MAX_CHALLENGE_LINE_BYTES)
            .unwrap()
            .unwrap();
        let challenge = parse_challenge_line(&challenge_line).unwrap();
        let mut proof = *client_proof_for_challenge(&key, &challenge).as_bytes();
        proof[0] ^= 1;
        write_line(
            &mut client,
            &format!("PAIRING_AUTH\tproof={}", hex_encode(&proof)),
        )
        .unwrap();
        let rejection = read_bounded_line(&mut reader, 1024).unwrap().unwrap();
        assert!(rejection.contains("AUTHENTICATION_FAILED"));
        drop(client);
        drop(reader);
        // Exact ledger reuse is rejected without touching the network.
        let mut ledger = PairingLedger::new(8).unwrap();
        let (session, _, challenge) = derive_ceremony_material(&key, 99).unwrap();
        ledger.consume(session, &challenge).unwrap();
        assert!(ledger.consume(session, &challenge).is_err());
        assert!(ledger.contains(session, &challenge));
        let _ = (result, server);
    }

    #[test]
    fn ledger_capacity_fails_closed_without_eviction() {
        let mut ledger = PairingLedger::new(2).unwrap();
        let key = [0x55; 32];
        for sequence in [1_u64, 2] {
            let (session, _, challenge) = derive_ceremony_material(&key, sequence).unwrap();
            ledger.consume(session, &challenge).unwrap();
        }
        let (session, _, challenge) = derive_ceremony_material(&key, 3).unwrap();
        assert!(ledger.consume(session, &challenge).is_err());
        assert_eq!(ledger.len(), 2);
    }

    #[test]
    fn key_source_failure_fails_the_connection_without_a_challenge() {
        // The source fails exactly once (expired lease window), then serves
        // the shutdown connection so the listener exits cleanly.
        let source_key = [0xA5; 32];
        let (address, result, server) =
            spawn_listener(TestKeySource::failing_once(), |command, _| {
                if command == "shutdown" {
                    Ok(EndpointAction::Shutdown)
                } else {
                    Ok(EndpointAction::Continue)
                }
            });
        let timeout = Duration::from_secs(5);
        let client = TcpStream::connect_timeout(&address, timeout).unwrap();
        client.set_read_timeout(Some(timeout)).unwrap();
        let mut reader = BufReader::new(client.try_clone().unwrap());
        // No challenge line is ever written when the key is unavailable: the
        // server fails the connection closed (clean EOF here).
        assert_eq!(
            read_bounded_line(&mut reader, MAX_CHALLENGE_LINE_BYTES).unwrap(),
            None
        );
        drop(client);
        drop(reader);
        // The listener is still alive: the retry handshakes cleanly.
        let mut client = TcpStream::connect_timeout(&address, timeout).unwrap();
        client.set_read_timeout(Some(timeout)).unwrap();
        client.set_write_timeout(Some(timeout)).unwrap();
        let mut reader = BufReader::new(client.try_clone().unwrap());
        client_handshake(&mut reader, &mut client, &source_key).unwrap();
        write_line(&mut client, "shutdown").unwrap();
        let (status, _) = result.recv_timeout(timeout).expect("bounded listener exit");
        server.join().unwrap();
        assert_eq!(status, Ok(()));
    }

    #[test]
    fn abort_does_not_append_a_terminal_to_partial_output() {
        let mut output = b"{\"partial\":".to_vec();
        let prior = output.clone();
        assert_eq!(
            complete_request(&mut output, Ok(EndpointAction::Abort), 3),
            EndpointAction::Abort
        );
        assert_eq!(output, prior);
    }

    #[test]
    fn complete_rejection_and_shutdown_keep_the_existing_wire_shape() {
        for (outcome, action, fields) in [
            (
                Ok(EndpointAction::Continue),
                EndpointAction::Continue,
                r#""ok":true"#,
            ),
            (
                Ok(EndpointAction::Shutdown),
                EndpointAction::Shutdown,
                r#""ok":true"#,
            ),
            (
                Err("SERVICE_HEX_INVALID:private detail".to_owned()),
                EndpointAction::Continue,
                r#""ok":false,"error":"SERVICE_HEX_INVALID""#,
            ),
        ] {
            let mut output = Vec::new();
            assert_eq!(complete_request(&mut output, outcome, 7), action);
            assert_eq!(
                output,
                format!("{{\"event\":\"request_complete\",\"sequence\":7,{fields}}}\n").as_bytes()
            );
        }
    }

    struct FailingOutput {
        bytes: Vec<u8>,
        calls: usize,
        fail_flush: bool,
    }
    impl Write for FailingOutput {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.calls += 1;
            if !self.fail_flush && self.calls == 2 {
                return Err(io::ErrorKind::BrokenPipe.into());
            }
            let count = if self.fail_flush {
                bytes.len()
            } else {
                bytes.len().min(5)
            };
            self.bytes.extend_from_slice(&bytes[..count]);
            Ok(count)
        }
        fn flush(&mut self) -> io::Result<()> {
            Err(io::ErrorKind::BrokenPipe.into())
        }
    }

    #[test]
    fn outer_ack_write_or_flush_failure_aborts_even_after_a_complete_child_reply() {
        for fail_flush in [false, true] {
            for outcome in [
                Ok(EndpointAction::Continue),
                Ok(EndpointAction::Shutdown),
                Err("REJECTED".to_owned()),
            ] {
                let mut writer = FailingOutput {
                    bytes: Vec::new(),
                    calls: 0,
                    fail_flush,
                };
                assert_eq!(
                    complete_request(&mut writer, outcome, 0),
                    EndpointAction::Abort
                );
                assert_eq!(writer.calls, 2); // One prefix + failure, or a frame + LF then failed flush.
                if !fail_flush {
                    assert_eq!(writer.bytes, b"{\"eve");
                }
            }
        }
    }

    #[test]
    fn fatal_handler_drops_listener_and_never_dispatches_the_next_queued_command() {
        use std::net::Shutdown;
        use std::sync::mpsc;
        use std::thread;
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let key = [0x6D; 32];
        let (done, result) = mpsc::channel();
        let server = thread::spawn(move || {
            let mut calls = 0;
            let mut source = TestKeySource::new(key);
            let ledger = PairingLedger::new(MAX_PAIRING_CHALLENGES).unwrap();
            let status = serve_listener(&listener, &mut source, ledger, |_, stream| {
                calls += 1;
                stream
                    .write_all(b"{\"partial\":")
                    .map_err(|_| "FIXTURE_WRITE_FAILED".to_owned())?;
                Ok(EndpointAction::Abort)
            });
            let _ = done.send((status, calls));
        });
        let timeout = Duration::from_secs(5);
        let mut client = TcpStream::connect_timeout(&address, timeout).unwrap();
        client.set_read_timeout(Some(timeout)).unwrap();
        client.set_write_timeout(Some(timeout)).unwrap();
        let mut reader = BufReader::new(client.try_clone().unwrap());
        client_handshake(&mut reader, &mut client, &key).unwrap();
        client.write_all(b"first\nsecond\nshutdown\n").unwrap();
        client.shutdown(Shutdown::Write).unwrap();
        let mut output = String::new();
        // Behavioral core first: the server delivered exactly the partial
        // bytes it wrote before aborting. The terminal read outcome itself
        // is platform-racy under load (clean EOF, RST, abortive close, or a
        // read timeout when the close is delayed by scheduling), so it is
        // intentionally not asserted on: the 5s read timeout above guarantees
        // termination, and the equality below proves full delivery.
        let _ = Read::take(&mut reader, 4096).read_to_string(&mut output);
        assert_eq!(
            output,
            "{\"event\":\"request_started\",\"sequence\":0}\n{\"partial\":"
        );
        let (status, calls) = result.recv_timeout(timeout).expect("bounded listener exit");
        server.join().unwrap();
        assert_eq!(status, Err("ENDPOINT_HANDLER_ABORTED".to_owned()));
        assert_eq!(calls, 1);
        assert!(TcpStream::connect_timeout(&address, timeout).is_err());
    }

    #[test]
    fn read_and_write_timeouts_are_finite_declared_bounds() {
        assert_eq!(READ_TIMEOUT, Duration::from_secs(30));
        assert_eq!(WRITE_TIMEOUT, Duration::from_secs(30));
        assert!(!READ_TIMEOUT.is_zero());
        assert!(!WRITE_TIMEOUT.is_zero());
    }

    #[test]
    fn silent_client_read_timeout_is_typed_and_bounded() {
        use std::sync::mpsc;
        use std::thread;
        use std::time::Instant;
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let (ready, accepted) = mpsc::channel();
        let server = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let _ = ready.send(());
            stream
                .set_read_timeout(Some(Duration::from_millis(200)))
                .unwrap();
            let mut reader = BufReader::new(stream);
            let start = Instant::now();
            let result = read_bounded_line(&mut reader, 1024);
            (result, start.elapsed())
        });
        let client = TcpStream::connect_timeout(&address, Duration::from_secs(5)).unwrap();
        client
            .set_write_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        accepted.recv_timeout(Duration::from_secs(5)).unwrap();
        // Silent client: hold the connection open without any frame.
        thread::sleep(Duration::from_millis(600));
        let (result, elapsed) = server.join().unwrap();
        assert_eq!(result, Err("ENDPOINT_READ_TIMEOUT".to_owned()));
        assert!(elapsed >= Duration::from_millis(150), "{elapsed:?}");
        assert!(elapsed < Duration::from_secs(5), "{elapsed:?}");
        drop(client);
    }

    #[test]
    fn real_socket_disconnect_mid_large_response_then_clean_health_has_no_contamination() {
        use std::sync::mpsc;
        use std::thread;
        use std::time::Instant;
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let large = vec![b'm'; 512 * 1024];
        let (done, finished) = mpsc::channel();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_write_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let start = Instant::now();
            let mut outcome: io::Result<()> = Ok(());
            // Stream a large response in bounded chunks; the client drops
            // mid-response, so a later write must fail instead of hanging.
            for chunk in large.chunks(16 * 1024) {
                if let Err(error) = stream.write_all(chunk) {
                    outcome = Err(error);
                    break;
                }
            }
            if outcome.is_ok() {
                outcome = stream.write_all(b"\n").and_then(|()| stream.flush());
            }
            let elapsed = start.elapsed();
            let _ = done.send(());
            (outcome, elapsed)
        });
        let timeout = Duration::from_secs(5);
        let client = TcpStream::connect_timeout(&address, timeout).unwrap();
        client.set_read_timeout(Some(timeout)).unwrap();
        let mut reader = BufReader::new(client.try_clone().unwrap());
        let mut prefix = vec![0_u8; 4096];
        Read::take(&mut reader, 4096)
            .read_exact(&mut prefix)
            .unwrap();
        assert!(prefix.iter().all(|byte| *byte == b'm'));
        // Disconnect mid-large-response: no further read, drop the socket.
        drop(reader);
        drop(client);
        finished.recv_timeout(timeout).expect("bounded server exit");
        let (outcome, elapsed) = server.join().unwrap();
        // The server must observe the disconnect instead of hanging; the exact
        // kind (reset/broken-pipe/timeout) is platform-specific and not part
        // of the contract.
        assert!(
            outcome.is_err(),
            "large response to a dropped client must fail"
        );
        assert!(elapsed < Duration::from_secs(5), "{elapsed:?}");
        // A fresh connection serves clean health with none of the old bytes.
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let (ready, health_ready) = mpsc::channel();
        let health = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream.set_write_timeout(Some(timeout)).unwrap();
            let _ = ready.send(());
            write_line(&mut stream, "{\"event\":\"health\",\"ok\":true}")
        });
        let client = TcpStream::connect_timeout(&address, timeout).unwrap();
        client.set_read_timeout(Some(timeout)).unwrap();
        health_ready.recv_timeout(timeout).unwrap();
        let mut reader = BufReader::new(client.try_clone().unwrap());
        let line = read_bounded_line(&mut reader, 1024).unwrap().unwrap();
        assert_eq!(line, "{\"event\":\"health\",\"ok\":true}");
        assert!(!line.contains('m'.to_string().repeat(16).as_str()));
        health.join().unwrap().unwrap();
    }

    #[test]
    fn slow_reader_write_is_bounded_by_write_timeout() {
        use std::sync::mpsc;
        use std::thread;
        use std::time::Instant;
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let (done, finished) = mpsc::channel();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_write_timeout(Some(Duration::from_millis(300)))
                .unwrap();
            let payload = vec![b's'; 2 * 1024 * 1024];
            let start = Instant::now();
            let mut outcome: io::Result<()> = Ok(());
            for chunk in payload.chunks(64 * 1024) {
                if let Err(error) = stream.write_all(chunk) {
                    outcome = Err(error);
                    break;
                }
            }
            if outcome.is_ok() {
                outcome = stream.flush();
            }
            let elapsed = start.elapsed();
            let _ = done.send(());
            (outcome.is_err(), elapsed)
        });
        let timeout = Duration::from_secs(5);
        // Slow reader: connect but never read, so the sender must time out
        // instead of blocking indefinitely on a full socket buffer.
        let client = TcpStream::connect_timeout(&address, timeout).unwrap();
        finished.recv_timeout(timeout).expect("bounded server exit");
        let (failed, elapsed) = server.join().unwrap();
        assert!(failed, "a never-reading client must bound the write");
        assert!(elapsed < Duration::from_secs(5), "{elapsed:?}");
        drop(client);
    }
}

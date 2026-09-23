//! Mutual keyed-BLAKE3 pairing and single-use ceremony state.

use std::io::BufReader;
use std::net::TcpStream;
use std::time::{SystemTime, UNIX_EPOCH};

use search_provider_protocol::pairing::{
    ClientNonce, PairingChallenge, PairingLedger, PairingMachine, PairingTranscript,
    ProofDigest, SessionId, verify_proof,
};

use super::codec::{encode_challenge, hex_encode, parse_auth_line};
use super::spec::{
    EndpointKeySource, MAX_AUTH_LINE_BYTES, PAIRING_PROTOCOL_VERSION, READ_TIMEOUT, WRITE_TIMEOUT,
};
use super::wire::{SocketDeadline, redacted_io_error};

const BINDING_DOMAIN: &[u8] = b"eliot-search/loopback-binding/v1\0";
const BINDING_ROLE: &[u8] = b"loopback-operator";
const SESSION_PRF_DOMAIN: &[u8] = b"eliot-search/loopback-session/v1\0";
const NONCE_PRF_DOMAIN: &[u8] = b"eliot-search/loopback-client-nonce/v1\0";
const CHALLENGE_PRF_DOMAIN: &[u8] = b"eliot-search/loopback-challenge/v1\0";

/// Role-bound binding digest for one pairing key.
///
/// Must equal `secret_composition::derive_binding_digest` on the same key.
#[must_use]
pub fn pairing_binding_digest(key: &[u8; 32]) -> ProofDigest {
    let mut hasher = blake3::Hasher::new();
    hasher.update(BINDING_DOMAIN);
    hasher.update(BINDING_ROLE);
    hasher.update(&[0]);
    hasher.update(key);
    ProofDigest::from_bytes(*hasher.finalize().as_bytes())
}

/// Keyed proof over one exact pairing transcript.
#[must_use]
pub fn keyed_proof(key: &[u8; 32], transcript: &PairingTranscript) -> ProofDigest {
    ProofDigest::from_bytes(*blake3::keyed_hash(key, transcript.as_bytes()).as_bytes())
}

/// Runs the mutual pairing ceremony under one non-renewable deadline.
/// Only complete proof/readiness output returns an authenticated reader. The
/// connection owner tears down every error/unwind without appending another frame.
pub(super) fn authenticate_connection<S>(
    stream: &mut TcpStream,
    connection_sequence: u64,
    source: &mut S,
    ledger: &mut PairingLedger,
) -> Result<BufReader<TcpStream>, String>
where
    S: EndpointKeySource,
{
    let deadline = SocketDeadline::new(READ_TIMEOUT)
        .map_err(|error| redacted_io_error("ENDPOINT_TIMEOUT_CONFIGURATION_ERROR", &error))?;
    let check_deadline = || {
        deadline.check().map_err(|_| "ENDPOINT_PAIRING_TIMEOUT".to_owned())
    };
    check_deadline()?;
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
    check_deadline()?;
    let (mut machine, binding, session, nonce, challenge, expected_client) = prepared;

    ledger.consume(session, &challenge).map_err(|error| {
        machine.fail();
        error.to_string()
    })?;

    deadline.write_line(
        stream,
        &encode_challenge(binding, session, &nonce, &challenge),
    )
    .map_err(|error| redacted_io_error("ENDPOINT_CHALLENGE_WRITE_ERROR", &error))?;
    let read_stream = stream
        .try_clone()
        .map_err(|error| redacted_io_error("ENDPOINT_STREAM_CLONE_ERROR", &error))?;
    let mut reader = BufReader::new(read_stream);
    let authentication = deadline.read_line(&mut reader, MAX_AUTH_LINE_BYTES)?
        .ok_or_else(|| "ENDPOINT_AUTHENTICATION_MISSING".to_owned())?;
    let observed = parse_auth_line(&authentication)?;
    if machine
        .verify_client_proof(&expected_client, &observed)
        .is_err()
    {
        let _ = deadline.write_line(stream, "{\"error\":\"AUTHENTICATION_FAILED\"}");
        return Err("ENDPOINT_AUTHENTICATION_FAILED".to_owned());
    }

    check_deadline()?;
    let provider_proof = source
        .with_endpoint_key(|key| {
            // The lease may have rotated while waiting for the client. Do not
            // authenticate with one key and acknowledge under another. Compare
            // the bound digest in constant work; raw key material stays leased.
            if !verify_proof(&binding, &pairing_binding_digest(key)) {
                return Err("ENDPOINT_KEY_CHANGED".to_owned());
            }
            machine
                .server_transcript()
                .map(|transcript| keyed_proof(key, &transcript))
                .map_err(|_| "ENDPOINT_PAIRING_TRANSCRIPT_FAILED".to_owned())
        })
        .map_err(|_| "ENDPOINT_KEY_UNAVAILABLE".to_owned())??;
    check_deadline()?;
    machine
        .issue_provider_proof(provider_proof)
        .map_err(|_| "ENDPOINT_PAIRING_PROVIDER_FAILED".to_owned())?;
    let verified = machine
        .into_verified()
        .map_err(|_| "ENDPOINT_PAIRING_PROVIDER_FAILED".to_owned())?;
    debug_assert_eq!(verified.version(), PAIRING_PROTOCOL_VERSION);

    deadline.write_line(
        stream,
        &format!(
            "PAIRING_VERIFIED\tproof={}",
            hex_encode(verified.provider_proof().as_bytes())
        ),
    )
    .map_err(|error| redacted_io_error("ENDPOINT_READY_WRITE_ERROR", &error))?;
    deadline.write_line(
        stream,
        concat!(
            "{\"event\":\"authenticated\",\"protocol_version\":1,",
            "\"transport\":\"loopback_tcp\",",
            "\"authentication\":\"pairing_blake3_v1\"}"
        ),
    )
    .map_err(|error| redacted_io_error("ENDPOINT_READY_WRITE_ERROR", &error))?;
    // Socket options are shared with the cloned read handle. Leave ordinary
    // command/child output with its configured bound, not the last few
    // milliseconds of the completed handshake. Resetting options does not
    // extend the handshake: its original deadline is checked again afterwards.
    stream
        .set_read_timeout(Some(READ_TIMEOUT))
        .and_then(|()| stream.set_write_timeout(Some(WRITE_TIMEOUT)))
        .map_err(|error| redacted_io_error("ENDPOINT_TIMEOUT_CONFIGURATION_ERROR", &error))?;
    check_deadline()?;
    Ok(reader)
}

/// Fresh per-connection ceremony material from a domain-separated keyed PRF.
pub(super) fn derive_ceremony_material(
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

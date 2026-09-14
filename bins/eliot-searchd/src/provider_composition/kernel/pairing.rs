//! Keyed envelope proofs, response sealing and development key acquisition.

use search_contracts::{ProtocolVersion, RequestId};
use search_provider_protocol::request::{
    AuthenticatedEnvelope, AuthenticatedResponse, RequestStatus, envelope_transcript,
    response_transcript, seal_response, verify_envelope_proof,
};
use search_provider_protocol::{ProofDigest, ProtocolError, ServerNonce};

use super::codec::hex_encode;
use super::spec::PROVIDER_TOKEN_INVALID;

/// Domain separating provider server-nonce draws from pairing material.
const SERVER_NONCE_DOMAIN: &[u8] = b"eliot-provider-server-nonce/v1\0";
/// Domain separating the development token-file key from pairing material.
///
/// Must stay byte-equal to the endpoint development-compat derivation; the
/// process test proves a real CLI/daemon round trip over this derivation.
const SHIM_KEY_DOMAIN: &[u8] = b"eliot-search/loopback-dev-key/v1\0";

/// Maximum token-file bytes read for the development-compat key.
pub const MAX_TOKEN_FILE_BYTES: usize = 4096;
/// Minimum trimmed token bytes accepted for the development-compat key.
pub const MIN_TOKEN_BYTES: usize = 32;

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

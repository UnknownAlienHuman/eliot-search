//! Exact pairing line codec and lowercase hex framing.

use search_contracts::ProtocolVersion;
use search_provider_protocol::pairing::{
    ClientNonce, PairingChallenge, ProofDigest, SessionId,
};

#[cfg(test)]
use search_provider_protocol::pairing::{
    client_proof_transcript, server_proof_transcript, verify_proof,
};

#[cfg(test)]
use super::pairing::keyed_proof;
use super::spec::PAIRING_PROTOCOL_VERSION;

pub(super) fn encode_challenge(
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

/// Parsed server challenge for reference-client and process tests.
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

/// Strict challenge-line parse: exact field count, order, prefixes and hex.
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

pub(super) fn parse_auth_line(line: &str) -> Result<ProofDigest, String> {
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

pub(super) fn hex_encode(bytes: &[u8]) -> String {
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

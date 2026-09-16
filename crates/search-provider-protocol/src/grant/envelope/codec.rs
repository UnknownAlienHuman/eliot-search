//! Strict canonical JSON and frame codec for standalone-grant envelopes.

use search_contracts::protocol::JsonFramePayload;
use search_contracts::{
    BoundedBytes, MAX_FRAME_BYTES, ProtocolRange, ProtocolVersion, RequestId,
};

use crate::config::ProtocolLimits;
use crate::error::ProtocolError;
use crate::frame::FrameCodec;
use crate::pairing::{ProofDigest, ServerNonce};

use super::{
    AuthenticatedStandaloneGrantEnvelope, MAX_STANDALONE_GRANT_ENVELOPE_JSON_BYTES,
    seal_standalone_grant_envelope,
};

/// Encodes one fixed-order canonical grant-envelope JSON object.
#[must_use]
pub fn encode_standalone_grant_envelope_json(
    envelope: &AuthenticatedStandaloneGrantEnvelope,
) -> Vec<u8> {
    let mut output = Vec::with_capacity(256);
    output.extend_from_slice(b"{\"v\":[");
    output.extend_from_slice(envelope.version().major.to_string().as_bytes());
    output.push(b',');
    output.extend_from_slice(envelope.version().minor.to_string().as_bytes());
    output.extend_from_slice(b"],\"nonce\":\"");
    push_hex(&mut output, envelope.server_nonce().as_bytes());
    output.extend_from_slice(b"\",\"request\":\"");
    push_hex(&mut output, envelope.request_id().as_bytes());
    output.extend_from_slice(b"\",\"body\":\"");
    push_hex(&mut output, envelope.body_digest().as_bytes());
    output.extend_from_slice(b"\",\"proof\":\"");
    push_hex(&mut output, envelope.proof().as_bytes());
    output.extend_from_slice(b"\"}");
    output
}

/// Decodes one exact canonical grant-envelope JSON object.
///
/// # Errors
///
/// Rejects oversize, whitespace, reordered/unknown fields, alternate number
/// or hex spelling, zero nonce, unsupported version and trailing bytes.
pub fn decode_standalone_grant_envelope_json(
    bytes: &[u8],
    supported: ProtocolRange,
) -> Result<AuthenticatedStandaloneGrantEnvelope, ProtocolError> {
    if bytes.is_empty() {
        return Err(ProtocolError::InvalidEnvelope);
    }
    if bytes.len() > MAX_STANDALONE_GRANT_ENVELOPE_JSON_BYTES {
        return Err(ProtocolError::FrameTooLarge);
    }

    let mut cursor = Cursor::new(bytes);
    cursor.expect(b"{\"v\":[")?;
    let major = cursor.parse_u16()?;
    cursor.expect(b",")?;
    let minor = cursor.parse_u16()?;
    cursor.expect(b"],\"nonce\":\"")?;
    let nonce = cursor.parse_hex::<16>()?;
    cursor.expect(b"\",\"request\":\"")?;
    let request = cursor.parse_hex::<16>()?;
    cursor.expect(b"\",\"body\":\"")?;
    let body = cursor.parse_hex::<32>()?;
    cursor.expect(b"\",\"proof\":\"")?;
    let proof = cursor.parse_hex::<32>()?;
    cursor.expect(b"\"}")?;
    if !cursor.exhausted() {
        return Err(ProtocolError::InvalidEnvelope);
    }

    let version = ProtocolVersion { major, minor };
    if !supported.contains(version) {
        return Err(ProtocolError::NoCompatibleVersion);
    }
    let envelope = seal_standalone_grant_envelope(
        version,
        ServerNonce::from_bytes(nonce).map_err(|_| ProtocolError::InvalidNonce)?,
        RequestId::from_bytes(request),
        ProofDigest::from_bytes(body),
        ProofDigest::from_bytes(proof),
    );
    if encode_standalone_grant_envelope_json(&envelope).as_slice() != bytes {
        return Err(ProtocolError::InvalidEnvelope);
    }
    Ok(envelope)
}

/// Encodes one grant envelope through the canonical length-prefixed frame.
///
/// # Errors
///
/// Returns a typed protocol error when protocol limits reject the frame.
pub fn encode_standalone_grant_envelope(
    envelope: &AuthenticatedStandaloneGrantEnvelope,
    limits: ProtocolLimits,
) -> Result<BoundedBytes<MAX_FRAME_BYTES>, ProtocolError> {
    let json = encode_standalone_grant_envelope_json(envelope);
    let payload = JsonFramePayload::new(json).map_err(|_| ProtocolError::FrameTooLarge)?;
    FrameCodec::encode(&payload, limits)
}

/// Decodes one canonical length-prefixed grant envelope.
///
/// # Errors
///
/// Returns a typed protocol error for malformed framing, JSON or version.
pub fn decode_standalone_grant_envelope(
    bytes: &[u8],
    limits: ProtocolLimits,
    supported: ProtocolRange,
) -> Result<AuthenticatedStandaloneGrantEnvelope, ProtocolError> {
    let payload = FrameCodec::decode(bytes, limits)?;
    decode_standalone_grant_envelope_json(payload.as_slice(), supported)
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
        let end = self.position.saturating_add(literal.len());
        if end <= self.bytes.len() && &self.bytes[self.position..end] == literal {
            self.position = end;
            Ok(())
        } else {
            Err(ProtocolError::InvalidEnvelope)
        }
    }

    fn parse_u16(&mut self) -> Result<u16, ProtocolError> {
        let start = self.position;
        while self
            .bytes
            .get(self.position)
            .is_some_and(u8::is_ascii_digit)
        {
            self.position += 1;
        }
        let digits = &self.bytes[start..self.position];
        if digits.is_empty() || (digits.len() > 1 && digits[0] == b'0') || digits.len() > 5 {
            return Err(ProtocolError::InvalidVersion);
        }
        let mut value = 0_u16;
        for digit in digits {
            value = value
                .checked_mul(10)
                .and_then(|current| current.checked_add(u16::from(*digit - b'0')))
                .ok_or(ProtocolError::InvalidVersion)?;
        }
        Ok(value)
    }

    fn parse_hex<const N: usize>(&mut self) -> Result<[u8; N], ProtocolError> {
        let encoded = N.checked_mul(2).ok_or(ProtocolError::InvalidEnvelope)?;
        let end = self
            .position
            .checked_add(encoded)
            .ok_or(ProtocolError::InvalidEnvelope)?;
        if end > self.bytes.len() {
            return Err(ProtocolError::InvalidEnvelope);
        }
        let input = &self.bytes[self.position..end];
        self.position = end;
        let mut output = [0_u8; N];
        for (index, pair) in input.chunks_exact(2).enumerate() {
            output[index] = (hex_value(pair[0]).ok_or(ProtocolError::InvalidEnvelope)? << 4)
                | hex_value(pair[1]).ok_or(ProtocolError::InvalidEnvelope)?;
        }
        Ok(output)
    }

    const fn exhausted(&self) -> bool {
        self.position == self.bytes.len()
    }
}

fn push_hex(output: &mut Vec<u8>, bytes: &[u8]) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in bytes {
        output.push(HEX[usize::from(byte >> 4)]);
        output.push(HEX[usize::from(byte & 0x0f)]);
    }
}

const fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

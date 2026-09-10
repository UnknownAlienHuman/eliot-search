//! Canonical `u32` little-endian length plus UTF-8 JSON framing.
//!
//! This is a thin limit-enforcing wrapper over
//! `search-contracts::protocol::{encode_json_frame, decode_json_frame}`.
//! Baseline performs no compression and no fragmented message assembly; the
//! 8 MiB ceiling includes the 4-byte prefix. Oversize input is rejected from
//! the declared prefix before the canonical decoder copies the body.

use search_contracts::protocol::{JsonFramePayload, decode_json_frame, encode_json_frame};
use search_contracts::{BoundedBytes, MAX_FRAME_BYTES, ProtocolErrorCode};

use crate::config::{FRAME_PREFIX_BYTES, ProtocolLimits};
use crate::error::ProtocolError;

/// Maps a canonical frame failure to the package failure registry.
const fn map_frame_error(code: ProtocolErrorCode) -> ProtocolError {
    match code {
        ProtocolErrorCode::FrameTooLarge => ProtocolError::FrameTooLarge,
        _ => ProtocolError::InvalidEnvelope,
    }
}

/// Canonical `u32` little-endian length plus UTF-8 JSON framing.
pub struct FrameCodec;

impl FrameCodec {
    /// Emits `u32` little-endian length plus canonical UTF-8 JSON.
    pub fn encode(
        payload: &JsonFramePayload,
        limits: ProtocolLimits,
    ) -> Result<BoundedBytes<MAX_FRAME_BYTES>, ProtocolError> {
        encode_frame(payload, limits)
    }

    /// Validates length before body allocation, then UTF-8/JSON shape.
    pub fn decode(bytes: &[u8], limits: ProtocolLimits) -> Result<JsonFramePayload, ProtocolError> {
        decode_frame(bytes, limits)
    }
}

/// Emits `u32` little-endian length plus canonical UTF-8 JSON.
pub fn encode_frame(
    payload: &JsonFramePayload,
    limits: ProtocolLimits,
) -> Result<BoundedBytes<MAX_FRAME_BYTES>, ProtocolError> {
    let limits = limits.validate()?;
    if payload.as_slice().len() > limits.max_body_bytes
        || payload.as_slice().len().saturating_add(FRAME_PREFIX_BYTES) > limits.max_frame_bytes
    {
        return Err(ProtocolError::FrameTooLarge);
    }
    encode_json_frame(payload).map_err(map_frame_error)
}

/// Validates length before body allocation, then UTF-8/JSON shape.
///
/// Oversize input is rejected without unbounded buffering: the configured
/// ceiling is enforced from the declared `u32` prefix before the canonical
/// decoder copies the body.
pub fn decode_frame(
    bytes: &[u8],
    limits: ProtocolLimits,
) -> Result<JsonFramePayload, ProtocolError> {
    let limits = limits.validate()?;
    if bytes.len() > limits.max_frame_bytes {
        return Err(ProtocolError::FrameTooLarge);
    }
    if bytes.len() < FRAME_PREFIX_BYTES {
        return Err(ProtocolError::InvalidEnvelope);
    }
    let declared = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    let declared = usize::try_from(declared).map_err(|_| ProtocolError::FrameTooLarge)?;
    if declared > limits.max_body_bytes
        || declared.saturating_add(FRAME_PREFIX_BYTES) > limits.max_frame_bytes
    {
        return Err(ProtocolError::FrameTooLarge);
    }
    decode_json_frame(bytes).map_err(map_frame_error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::DEFAULT_PROTOCOL_LIMITS;

    #[test]
    fn canonical_framing_round_trips_with_le_prefix() {
        let payload = JsonFramePayload::new(br#"{"ok":true}"#.to_vec()).expect("payload");
        let encoded = FrameCodec::encode(&payload, DEFAULT_PROTOCOL_LIMITS).expect("encode");
        assert_eq!(&encoded.as_slice()[..4], &11_u32.to_le_bytes());
        assert_eq!(
            FrameCodec::decode(encoded.as_slice(), DEFAULT_PROTOCOL_LIMITS).expect("decode"),
            payload
        );
    }

    #[test]
    fn framing_rejects_truncated_and_length_mismatch() {
        assert_eq!(
            FrameCodec::decode(&[1, 0, 0], DEFAULT_PROTOCOL_LIMITS),
            Err(ProtocolError::InvalidEnvelope)
        );
        assert_eq!(
            FrameCodec::decode(&[2, 0, 0, 0, b'{'], DEFAULT_PROTOCOL_LIMITS),
            Err(ProtocolError::InvalidEnvelope)
        );
        assert_eq!(
            FrameCodec::decode(&[11, 0, 0, 0, 0xff], DEFAULT_PROTOCOL_LIMITS),
            Err(ProtocolError::InvalidEnvelope)
        );
    }

    #[test]
    fn oversize_is_rejected_before_body_copy() {
        let bytes = vec![0; DEFAULT_PROTOCOL_LIMITS.max_frame_bytes + 1];
        assert_eq!(
            FrameCodec::decode(&bytes, DEFAULT_PROTOCOL_LIMITS),
            Err(ProtocolError::FrameTooLarge)
        );
    }
}

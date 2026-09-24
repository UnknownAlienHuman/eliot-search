//! Explicit authenticated transport profile for typed P00 frames.
//!
//! After mutual pairing, each peer sends PREFACE followed by its 32-byte keyed
//! proof over the corresponding transcript. The server verifies the offer before
//! acknowledging it. Data records are then [u32-LE JSON length][JSON][32-byte MAC].
//! The MAC covers the entire original length-prefixed frame using the existing
//! ProviderFrameTranscript, not a reserialized JSON body. No legacy op line,
//! request_complete, implicit mode detection or unsigned fallback is permitted.

use crate::config::{FRAME_PREFIX_BYTES, ProtocolLimits};
use crate::error::ProtocolError;
use crate::pairing::{ServerNonce, SessionId};

/// Closed transport profile for the typed 1.0 codecs, negotiated after pairing.
///
/// This is pure wire configuration, not a verified connection or source permit.
/// The adapter must check the actual pairing/session/nonce and keyed proofs.
/// Both handshake proofs are direction-separated and include the exact profile.
/// The configured frame ceiling bounds the WHOLE record, including its proof;
/// the JSON body is therefore at least 32 bytes smaller than an unsigned frame.
pub struct TypedTransportProfileV1;

impl TypedTransportProfileV1 {
    /// Exact profile offer and acknowledgement prefix; no alternate spellings.
    pub const PREFACE: &'static [u8] = b"ELIOT-PROVIDER-TYPED-BLAKE3/1.0\n";
    /// Fixed authentication trailer length. It is not part of the JSON length.
    pub const PROOF_BYTES: usize = 32;

    /// Borrowed parts to hash in order with the retained pairing key.
    #[must_use]
    pub fn offer_transcript<'a>(
        ceremony: &'a SessionId,
        nonce: &'a ServerNonce,
    ) -> [&'a [u8]; 4] {
        [b"ELIOT-PROVIDER-TRANSPORT-OFFER-v1\0", ceremony.as_bytes(), nonce.as_bytes(), Self::PREFACE]
    }

    /// Server acknowledgement proof; a reflected client offer cannot satisfy it.
    #[must_use]
    pub fn accept_transcript<'a>(
        ceremony: &'a SessionId,
        nonce: &'a ServerNonce,
    ) -> [&'a [u8]; 4] {
        [b"ELIOT-PROVIDER-TRANSPORT-ACCEPT-v1\0", ceremony.as_bytes(), nonce.as_bytes(), Self::PREFACE]
    }

    /// Validates the announced length before a transport allocates/reads a body.
    /// Returns the size of the canonical frame, excluding the fixed MAC trailer.
    /// Syntax, direction and authentication are checked by their existing owners.
    pub fn frame_length(prefix: [u8; 4], limits: ProtocolLimits) -> Result<usize, ProtocolError> {
        let limits = limits.validate()?;
        let body = usize::try_from(u32::from_le_bytes(prefix))
            .map_err(|_| ProtocolError::FrameTooLarge)?;
        if body == 0 { return Err(ProtocolError::InvalidEnvelope); }
        let frame = body.checked_add(FRAME_PREFIX_BYTES).ok_or(ProtocolError::FrameTooLarge)?;
        if body > limits.max_body_bytes
            || frame.checked_add(Self::PROOF_BYTES).is_none_or(|total| total > limits.max_frame_bytes)
        {
            return Err(ProtocolError::FrameTooLarge);
        }
        Ok(frame)
    }

    /// Checks output length before the first record byte is sent. The caller
    /// must pass the exact already-encoded typed frame; this does not sign it.
    pub fn validate_frame(frame: &[u8], limits: ProtocolLimits) -> Result<(), ProtocolError> {
        let prefix: [u8; 4] = frame.get(..FRAME_PREFIX_BYTES)
            .ok_or(ProtocolError::InvalidEnvelope)?.try_into()
            .map_err(|_| ProtocolError::InvalidEnvelope)?;
        if Self::frame_length(prefix, limits)? != frame.len() {
            return Err(ProtocolError::InvalidEnvelope);
        }
        Ok(())
    }
}

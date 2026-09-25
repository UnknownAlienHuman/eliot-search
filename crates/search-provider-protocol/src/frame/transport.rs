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
use crate::pairing::{ProofDigest, ServerNonce, SessionId};

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

/// Incremental storage for ONE typed record across bounded socket polls.
///
/// This is partial I/O of a single frame, not multi-message fragmentation. The
/// original prefix is retained and checked before body allocation. No second
/// frame can be prefetched. The adapter owns deadlines, EOF handling and proof
/// verification; completion alone proves neither JSON validity nor authenticity.
/// No Debug/Clone implementation exposes or duplicates retained payloads.
pub struct TypedRecordBuffer {
    limits: ProtocolLimits,
    prefix: [u8; FRAME_PREFIX_BYTES],
    prefix_filled: usize,
    frame: Vec<u8>,
    frame_filled: usize,
    proof: [u8; TypedTransportProfileV1::PROOF_BYTES],
    proof_filled: usize,
    offered: usize,
}

impl TypedRecordBuffer {
    /// Creates empty framing state with immutable validated size limits.
    /// The body is not allocated until all four prefix bytes are received.
    pub fn new(limits: ProtocolLimits) -> Result<Self, ProtocolError> {
        Ok(Self {
            limits: limits.validate()?, prefix: [0; FRAME_PREFIX_BYTES], prefix_filled: 0,
            frame: Vec::new(), frame_filled: 0,
            proof: [0; TypedTransportProfileV1::PROOF_BYTES], proof_filled: 0, offered: 0,
        })
    }

    /// Borrows at most `maximum` unfilled bytes for one Read call.
    /// Call `advance` with that call's successful byte count. An interrupted or
    /// timed-out read advances nothing; the same buffer can be borrowed again.
    /// Empty output means the record is complete. Zero maximum is invalid.
    pub fn read_buffer(&mut self, maximum: usize) -> Result<&mut [u8], ProtocolError> {
        self.offered = 0;
        if maximum == 0 { return Err(ProtocolError::InvalidLimits); }
        if self.prefix_filled < FRAME_PREFIX_BYTES {
            self.offered = maximum.min(FRAME_PREFIX_BYTES - self.prefix_filled);
            return Ok(&mut self.prefix[self.prefix_filled..self.prefix_filled + self.offered]);
        }
        if self.frame.is_empty() {
            let size = TypedTransportProfileV1::frame_length(self.prefix, self.limits)?;
            self.frame.try_reserve_exact(size).map_err(|_| ProtocolError::ResourceExhausted)?;
            self.frame.extend_from_slice(&self.prefix);
            self.frame.resize(size, 0);
            self.frame_filled = FRAME_PREFIX_BYTES;
        }
        if self.frame_filled < self.frame.len() {
            self.offered = maximum.min(self.frame.len() - self.frame_filled);
            return Ok(&mut self.frame[self.frame_filled..self.frame_filled + self.offered]);
        }
        self.offered = maximum.min(self.proof.len() - self.proof_filled);
        Ok(&mut self.proof[self.proof_filled..self.proof_filled + self.offered])
    }

    /// Records only bytes actually read into the last borrowed buffer.
    /// A count outside that buffer is refused without advancing framing state.
    /// Zero is accepted as no progress; the socket owner must classify EOF.
    pub fn advance(&mut self, count: usize) -> Result<(), ProtocolError> {
        if count > self.offered { return Err(ProtocolError::InvalidEnvelope); }
        self.offered = 0;
        if self.prefix_filled < FRAME_PREFIX_BYTES {
            self.prefix_filled += count;
        } else if self.frame_filled < self.frame.len() {
            self.frame_filled += count;
        } else {
            self.proof_filled += count;
        }
        Ok(())
    }

    /// Whether the entire prefixed frame AND authentication trailer arrived.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.prefix_filled == FRAME_PREFIX_BYTES && !self.frame.is_empty()
            && self.frame_filled == self.frame.len() && self.proof_filled == self.proof.len()
    }

    /// Moves a complete record out without copying its body. Partial records
    /// fail; the caller still verifies the MAC and typed schema before dispatch.
    pub fn finish(self) -> Result<(Vec<u8>, ProofDigest), ProtocolError> {
        if !self.is_complete() { return Err(ProtocolError::InvalidEnvelope); }
        Ok((self.frame, ProofDigest::from_bytes(self.proof)))
    }
}

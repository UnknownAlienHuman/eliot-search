//! Canonical bounded records for the mutual-pairing prelude.
//!
//! The initial hello uses the existing canonical [`crate::ClientEnvelopeCodec`]
//! and `ProviderBodyV1::Hello` schema. This module owns only the fixed-size
//! challenge and mutual-proof records that follow that hello and precede the
//! authenticated typed transport profile. It performs no I/O and retains no key.

use search_contracts::ProtocolVersion;

use crate::error::ProtocolError;
use crate::pairing::{
    ClientNonce, PairingChallenge, ProofDigest, ServerNonce, SessionId,
};

const CHALLENGE_MAGIC: &[u8; 8] = b"ELPCHL01";
const PROOF_MAGIC: &[u8; 8] = b"ELPPRF01";
const VERIFIED_MAGIC: &[u8; 8] = b"ELPVER01";

/// Exact encoded provider-challenge record bytes.
pub const PAIRING_CHALLENGE_BYTES: usize = 132;
/// Exact encoded client-proof record bytes.
pub const PAIRING_PROOF_BYTES: usize = 48;
/// Exact encoded provider-verification record bytes.
pub const PAIRING_VERIFIED_BYTES: usize = 48;

/// Provider challenge and session material for one accepted connection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PairingChallengeFrame {
    version: ProtocolVersion,
    connection_sequence: u64,
    session_id: SessionId,
    client_nonce: ClientNonce,
    challenge: PairingChallenge,
    binding_digest: ProofDigest,
    server_nonce: ServerNonce,
}

impl PairingChallengeFrame {
    /// Creates one nonzero-sequence challenge record.
    pub fn new(
        version: ProtocolVersion,
        connection_sequence: u64,
        session_id: SessionId,
        client_nonce: ClientNonce,
        challenge: PairingChallenge,
        binding_digest: ProofDigest,
        server_nonce: ServerNonce,
    ) -> Result<Self, ProtocolError> {
        if version.major == 0 || connection_sequence == 0 {
            return Err(ProtocolError::InvalidEnvelope);
        }
        Ok(Self {
            version,
            connection_sequence,
            session_id,
            client_nonce,
            challenge,
            binding_digest,
            server_nonce,
        })
    }

    /// Negotiated provider protocol version.
    #[must_use]
    pub const fn version(&self) -> ProtocolVersion {
        self.version
    }

    /// Listener-owned monotone connection sequence.
    #[must_use]
    pub const fn connection_sequence(&self) -> u64 {
        self.connection_sequence
    }

    /// Single-use pairing session identity.
    #[must_use]
    pub const fn session_id(&self) -> SessionId {
        self.session_id
    }

    /// Nonzero nonce bound into pairing proof transcripts.
    #[must_use]
    pub const fn client_nonce(&self) -> ClientNonce {
        self.client_nonce
    }

    /// Single-use provider challenge.
    #[must_use]
    pub const fn challenge(&self) -> PairingChallenge {
        self.challenge
    }

    /// Key-derived binding digest bound into both proof transcripts.
    #[must_use]
    pub const fn binding_digest(&self) -> ProofDigest {
        self.binding_digest
    }

    /// Per-connection nonce used by the authenticated request session.
    #[must_use]
    pub const fn server_nonce(&self) -> ServerNonce {
        self.server_nonce
    }
}

/// Client proof response for one exact challenge.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PairingProofFrame {
    connection_sequence: u64,
    proof: ProofDigest,
}

impl PairingProofFrame {
    /// Creates one proof response.
    pub fn new(
        connection_sequence: u64,
        proof: ProofDigest,
    ) -> Result<Self, ProtocolError> {
        if connection_sequence == 0 {
            return Err(ProtocolError::InvalidEnvelope);
        }
        Ok(Self {
            connection_sequence,
            proof,
        })
    }

    /// Listener-owned connection sequence echoed by the client.
    #[must_use]
    pub const fn connection_sequence(&self) -> u64 {
        self.connection_sequence
    }

    /// Client keyed proof.
    #[must_use]
    pub const fn proof(&self) -> ProofDigest {
        self.proof
    }
}

/// Provider proof acknowledgement for one exact challenge.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PairingVerifiedFrame {
    connection_sequence: u64,
    proof: ProofDigest,
}

impl PairingVerifiedFrame {
    /// Creates one provider-proof acknowledgement.
    pub fn new(
        connection_sequence: u64,
        proof: ProofDigest,
    ) -> Result<Self, ProtocolError> {
        if connection_sequence == 0 {
            return Err(ProtocolError::InvalidEnvelope);
        }
        Ok(Self {
            connection_sequence,
            proof,
        })
    }

    /// Listener-owned connection sequence.
    #[must_use]
    pub const fn connection_sequence(&self) -> u64 {
        self.connection_sequence
    }

    /// Provider keyed proof.
    #[must_use]
    pub const fn proof(&self) -> ProofDigest {
        self.proof
    }
}

/// Encodes one exact provider challenge record.
#[must_use]
pub fn encode_pairing_challenge(
    frame: PairingChallengeFrame,
) -> [u8; PAIRING_CHALLENGE_BYTES] {
    let mut output = [0_u8; PAIRING_CHALLENGE_BYTES];
    output[..8].copy_from_slice(CHALLENGE_MAGIC);
    output[8..10].copy_from_slice(&frame.version.major.to_le_bytes());
    output[10..12].copy_from_slice(&frame.version.minor.to_le_bytes());
    output[12..20].copy_from_slice(&frame.connection_sequence.to_le_bytes());
    output[20..36].copy_from_slice(frame.session_id.as_bytes());
    output[36..52].copy_from_slice(frame.client_nonce.as_bytes());
    output[52..84].copy_from_slice(frame.challenge.as_bytes());
    output[84..116].copy_from_slice(frame.binding_digest.as_bytes());
    output[116..132].copy_from_slice(frame.server_nonce.as_bytes());
    output
}

/// Decodes one exact provider challenge record.
pub fn decode_pairing_challenge(
    bytes: &[u8],
) -> Result<PairingChallengeFrame, ProtocolError> {
    if bytes.len() != PAIRING_CHALLENGE_BYTES || &bytes[..8] != CHALLENGE_MAGIC {
        return Err(ProtocolError::InvalidEnvelope);
    }
    PairingChallengeFrame::new(
        ProtocolVersion {
            major: u16::from_le_bytes([bytes[8], bytes[9]]),
            minor: u16::from_le_bytes([bytes[10], bytes[11]]),
        },
        u64::from_le_bytes(copy_array(&bytes[12..20])?),
        SessionId::from_bytes(copy_array(&bytes[20..36])?)?,
        ClientNonce::from_bytes(copy_array(&bytes[36..52])?)?,
        PairingChallenge::from_bytes(copy_array(&bytes[52..84])?)?,
        ProofDigest::from_bytes(copy_array(&bytes[84..116])?),
        ServerNonce::from_bytes(copy_array(&bytes[116..132])?)?,
    )
}

/// Encodes one exact client-proof record.
#[must_use]
pub fn encode_pairing_proof(
    frame: PairingProofFrame,
) -> [u8; PAIRING_PROOF_BYTES] {
    encode_proof_record(PROOF_MAGIC, frame.connection_sequence, frame.proof)
}

/// Decodes one exact client-proof record.
pub fn decode_pairing_proof(bytes: &[u8]) -> Result<PairingProofFrame, ProtocolError> {
    let (connection_sequence, proof) = decode_proof_record(PROOF_MAGIC, bytes)?;
    PairingProofFrame::new(connection_sequence, proof)
}

/// Encodes one exact provider-verification record.
#[must_use]
pub fn encode_pairing_verified(
    frame: PairingVerifiedFrame,
) -> [u8; PAIRING_VERIFIED_BYTES] {
    encode_proof_record(VERIFIED_MAGIC, frame.connection_sequence, frame.proof)
}

/// Decodes one exact provider-verification record.
pub fn decode_pairing_verified(
    bytes: &[u8],
) -> Result<PairingVerifiedFrame, ProtocolError> {
    let (connection_sequence, proof) = decode_proof_record(VERIFIED_MAGIC, bytes)?;
    PairingVerifiedFrame::new(connection_sequence, proof)
}

fn encode_proof_record(
    magic: &[u8; 8],
    connection_sequence: u64,
    proof: ProofDigest,
) -> [u8; PAIRING_PROOF_BYTES] {
    let mut output = [0_u8; PAIRING_PROOF_BYTES];
    output[..8].copy_from_slice(magic);
    output[8..16].copy_from_slice(&connection_sequence.to_le_bytes());
    output[16..48].copy_from_slice(proof.as_bytes());
    output
}

fn decode_proof_record(
    magic: &[u8; 8],
    bytes: &[u8],
) -> Result<(u64, ProofDigest), ProtocolError> {
    if bytes.len() != PAIRING_PROOF_BYTES || &bytes[..8] != magic {
        return Err(ProtocolError::InvalidEnvelope);
    }
    Ok((
        u64::from_le_bytes(copy_array(&bytes[8..16])?),
        ProofDigest::from_bytes(copy_array(&bytes[16..48])?),
    ))
}

fn copy_array<const N: usize>(bytes: &[u8]) -> Result<[u8; N], ProtocolError> {
    bytes.try_into().map_err(|_| ProtocolError::InvalidEnvelope)
}

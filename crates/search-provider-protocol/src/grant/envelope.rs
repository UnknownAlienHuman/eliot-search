//! Dedicated authenticated envelope for one standalone-grant request body.
//!
//! The domain is distinct from the W1 shell-command envelope. The envelope
//! authenticates only protocol/session identity and the exact canonical body
//! digest; it creates no grant and carries no binding or policy authority.

use search_contracts::{ProtocolVersion, RequestId};

use crate::error::ProtocolError;
use crate::pairing::{ProofDigest, ServerNonce, verify_proof};

/// Domain separator for standalone-grant request proofs.
pub const STANDALONE_GRANT_ENVELOPE_DOMAIN: &str = "ELIOT-STANDALONE-GRANT-REQ-v1";
/// Maximum canonical JSON bytes for the fixed-size grant envelope.
pub const MAX_STANDALONE_GRANT_ENVELOPE_JSON_BYTES: usize = 512;

/// Authenticated standalone-grant request header.
///
/// Exact body bytes travel separately and must hash to [`Self::body_digest`]
/// before the bound session mutates sequence, replay or in-flight state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedStandaloneGrantEnvelope {
    version: ProtocolVersion,
    server_nonce: ServerNonce,
    request_id: RequestId,
    body_digest: ProofDigest,
    proof: ProofDigest,
}

impl AuthenticatedStandaloneGrantEnvelope {
    /// Negotiated protocol version covered by the proof.
    #[must_use]
    pub const fn version(&self) -> ProtocolVersion {
        self.version
    }

    /// Per-incarnation server nonce covered by the proof.
    #[must_use]
    pub const fn server_nonce(&self) -> &ServerNonce {
        &self.server_nonce
    }

    /// Opaque request identity covered by the proof.
    #[must_use]
    pub const fn request_id(&self) -> &RequestId {
        &self.request_id
    }

    /// Digest of the exact canonical standalone-grant body bytes.
    #[must_use]
    pub const fn body_digest(&self) -> &ProofDigest {
        &self.body_digest
    }

    /// Keyed proof over [`standalone_grant_envelope_transcript`].
    #[must_use]
    pub const fn proof(&self) -> &ProofDigest {
        &self.proof
    }
}

/// Seals a standalone-grant envelope from adapter-supplied proof material.
#[must_use]
pub const fn seal_standalone_grant_envelope(
    version: ProtocolVersion,
    server_nonce: ServerNonce,
    request_id: RequestId,
    body_digest: ProofDigest,
    proof: ProofDigest,
) -> AuthenticatedStandaloneGrantEnvelope {
    AuthenticatedStandaloneGrantEnvelope {
        version,
        server_nonce,
        request_id,
        body_digest,
        proof,
    }
}

/// Exact bytes the keyed proof must cover.
///
/// The transcript binds domain, version, server nonce, request ID and exact
/// body digest in fixed order. The proof itself is not recursively included.
#[must_use]
pub fn standalone_grant_envelope_transcript(
    envelope: &AuthenticatedStandaloneGrantEnvelope,
) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(
        STANDALONE_GRANT_ENVELOPE_DOMAIN.len() + 1 + 2 + 2 + 16 + 16 + 32,
    );
    bytes.extend_from_slice(STANDALONE_GRANT_ENVELOPE_DOMAIN.as_bytes());
    bytes.push(0);
    bytes.extend_from_slice(&envelope.version.major.to_le_bytes());
    bytes.extend_from_slice(&envelope.version.minor.to_le_bytes());
    bytes.extend_from_slice(envelope.server_nonce.as_bytes());
    bytes.extend_from_slice(envelope.request_id.as_bytes());
    bytes.extend_from_slice(envelope.body_digest.as_bytes());
    bytes
}

/// Verifies the adapter-computed expected keyed proof in fixed work.
///
/// # Errors
///
/// Returns [`ProtocolError::AuthenticationFailed`] on mismatch.
pub fn verify_standalone_grant_envelope_proof(
    envelope: &AuthenticatedStandaloneGrantEnvelope,
    expected: &ProofDigest,
) -> Result<(), ProtocolError> {
    if verify_proof(expected, envelope.proof()) {
        Ok(())
    } else {
        Err(ProtocolError::AuthenticationFailed)
    }
}

mod codec;

pub use codec::{
    decode_standalone_grant_envelope, decode_standalone_grant_envelope_json,
    encode_standalone_grant_envelope, encode_standalone_grant_envelope_json,
};

#[cfg(test)]
mod tests;

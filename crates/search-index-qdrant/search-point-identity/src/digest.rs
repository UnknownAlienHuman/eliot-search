use search_contracts::Blake3Digest32;

use crate::{
    CanonicalPointKeyBytes, PointId128, PointIdentityError, PointIdentityLimits,
    ProjectionPointKey, derive_qdrant_uuid, encode_canonical_key,
};

const IDENTITY_DIGEST_DOMAIN: &[u8] = b"eliot-search/point-identity/v1\0";

/// Full BLAKE3-256 digest of one canonical point key.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PointIdentityDigest(Blake3Digest32);

impl PointIdentityDigest {
    /// Creates a full digest from exact bytes.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(Blake3Digest32::from_bytes(bytes))
    }

    /// Exact 32 digest bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        self.0.as_bytes()
    }

    /// Eliot contract digest used by S9.5 payload and projection manifests.
    #[must_use]
    pub const fn as_contract_digest(&self) -> Blake3Digest32 {
        self.0
    }
}

/// Complete derived point identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PointIdentity {
    /// Namespace-separated 128-bit Qdrant address.
    pub point_id: PointId128,
    /// Full BLAKE3-256 collision guard.
    pub full_digest: PointIdentityDigest,
    /// Exact canonical logical key.
    pub key: ProjectionPointKey,
}

/// Computes BLAKE3-256 over the identity domain prefix and canonical CBOR key.
#[must_use]
pub fn full_digest(canonical_bytes: &CanonicalPointKeyBytes) -> PointIdentityDigest {
    let mut hasher = blake3::Hasher::new();
    hasher.update(IDENTITY_DIGEST_DOMAIN);
    hasher.update(canonical_bytes.as_slice());
    PointIdentityDigest::from_bytes(*hasher.finalize().as_bytes())
}

/// Agent-contract spelling for [`full_digest`].
#[must_use]
pub fn point_identity_digest(
    canonical_bytes: &CanonicalPointKeyBytes,
) -> PointIdentityDigest {
    full_digest(canonical_bytes)
}

/// Derives canonical bytes, full digest and Qdrant UUID from one exact key.
pub fn derive_point_identity(
    key: ProjectionPointKey,
    limits: PointIdentityLimits,
) -> Result<PointIdentity, PointIdentityError> {
    let canonical = encode_canonical_key(&key, limits)?;
    let digest = full_digest(&canonical);
    let point_id = derive_qdrant_uuid(digest);
    Ok(PointIdentity {
        point_id,
        full_digest: digest,
        key,
    })
}

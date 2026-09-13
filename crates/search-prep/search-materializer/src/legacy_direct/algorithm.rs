//! Explicit persisted digest-algorithm tags for legacy DIRECT preparation.

/// BLAKE3-256 wire tag matching `search-contracts::DigestAlgorithm`.
pub const DIGEST_ALGORITHM_BLAKE3_256: u8 = 1;
/// SHA-256 wire tag matching `search-contracts::DigestAlgorithm`.
pub const DIGEST_ALGORITHM_SHA256: u8 = 2;
/// Legacy DIRECT source content uses SHA-256 identities.
pub const CONTENT_DIGEST_ALGORITHM: u8 = DIGEST_ALGORITHM_SHA256;
/// Legacy DIRECT representation identities use BLAKE3-256.
pub const REPRESENTATION_DIGEST_ALGORITHM: u8 = DIGEST_ALGORITHM_BLAKE3_256;
/// Legacy DIRECT preparation manifests use SHA-256 envelope digests.
pub const MANIFEST_DIGEST_ALGORITHM: u8 = DIGEST_ALGORITHM_SHA256;

//! Domain-separated legacy DIRECT representation identity.

use super::LegacyDirectPreparationError;

/// Domain bound into every legacy DIRECT preparation representation identity.
pub const LEGACY_DIRECT_REPRESENTATION_DOMAIN: &[u8] =
    b"eliot-searchd/preparation-representation/v1\x00";

/// Exact source, revision, content and profile inputs to representation identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LegacyDirectPreparationBinding {
    /// Data-root namespace digest.
    pub namespace: [u8; 32],
    /// Stable source identifier bytes.
    pub source_id: [u8; 32],
    /// Immutable revision identifier bytes.
    pub revision_id: [u8; 32],
    /// Exact source-content digest bytes.
    pub content_digest: [u8; 32],
    /// Exact retained source byte length.
    pub byte_length: u64,
    /// Canonical materializer profile digest.
    pub materializer_digest: [u8; 32],
    /// Canonical unitizer profile digest.
    pub unitizer_digest: [u8; 32],
}

/// Composition-supplied BLAKE3-compatible digest primitive.
///
/// Implementations absorb the domain followed by every ordered part exactly as
/// supplied, without adding implicit separators or changing algorithms.
pub trait LegacyDirectRepresentationDigest {
    /// Computes one digest over a domain and ordered raw parts.
    fn digest_parts(domain: &[u8], parts: &[&[u8]]) -> [u8; 32];
}

/// Derives the exact version-one legacy DIRECT representation identity.
#[must_use]
pub fn derive_legacy_direct_representation_id<D: LegacyDirectRepresentationDigest>(
    binding: &LegacyDirectPreparationBinding,
    canonical_or_gap: &[u8],
) -> [u8; 32] {
    let byte_length = binding.byte_length.to_be_bytes();
    let marker_length = u64::try_from(canonical_or_gap.len())
        .unwrap_or(u64::MAX)
        .to_be_bytes();
    D::digest_parts(
        LEGACY_DIRECT_REPRESENTATION_DOMAIN,
        &[
            &binding.namespace,
            &binding.source_id,
            &binding.revision_id,
            &binding.content_digest,
            &byte_length,
            &binding.materializer_digest,
            &binding.unitizer_digest,
            &marker_length,
            canonical_or_gap,
        ],
    )
}

/// Verifies an expected representation identity by exact recomputation.
pub fn verify_legacy_direct_representation<D: LegacyDirectRepresentationDigest>(
    expected: &[u8; 32],
    binding: &LegacyDirectPreparationBinding,
    canonical_or_gap: &[u8],
) -> Result<(), LegacyDirectPreparationError> {
    if derive_legacy_direct_representation_id::<D>(binding, canonical_or_gap) == *expected {
        Ok(())
    } else {
        Err(LegacyDirectPreparationError::InvalidFrame)
    }
}

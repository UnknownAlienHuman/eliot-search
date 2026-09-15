//! Canonical retained truth used as the read-only rebuild source.

use search_contracts::{
    Blake3Digest32, CollectionGenerationId, CollectionRouteRevision,
};

use super::error::RebuildError;

/// One canonically retained index point: identity plus content digests.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct RetainedPoint {
    /// Exact 128-bit point identifier.
    pub id: [u8; 16],
    /// Digest of the exact stored payload.
    pub payload_digest: Blake3Digest32,
    /// Digest of the exact point identity.
    pub identity_digest: Blake3Digest32,
}

/// Canonical retained manifest a rebuild replays read-only.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetainedManifest {
    /// Physical collection generation the points were projected for.
    pub generation: CollectionGenerationId,
    /// Logical route revision the points were projected for.
    pub route_revision: CollectionRouteRevision,
    /// Digest over the exact canonical point encoding.
    pub manifest_digest: Blake3Digest32,
    /// Canonically ordered retained points.
    pub points: Vec<RetainedPoint>,
}

/// Computes the exact canonical manifest digest with real BLAKE3.
///
/// Encoding: domain tag, point count as little-endian `u64`, then identifier,
/// payload digest and identity digest for each point in canonical order.
#[must_use]
pub fn retained_manifest_digest(points: &[RetainedPoint]) -> Blake3Digest32 {
    let mut input = Vec::with_capacity(
        32 + 8 + points.len().saturating_mul(16 + 32 + 32),
    );
    input.extend_from_slice(b"eliot-search/rebuild-manifest/v1\x00");
    input.extend_from_slice(&points.len().to_le_bytes());
    for point in points {
        input.extend_from_slice(&point.id);
        input.extend_from_slice(point.payload_digest.as_bytes());
        input.extend_from_slice(point.identity_digest.as_bytes());
    }
    Blake3Digest32::from_bytes(*blake3::hash(&input).as_bytes())
}

/// Validates canonical order, uniqueness and exact manifest digest.
pub fn validate_retained_manifest(
    manifest: &RetainedManifest,
) -> Result<(), RebuildError> {
    if manifest
        .points
        .windows(2)
        .any(|pair| pair[0].id >= pair[1].id)
    {
        return Err(RebuildError::ManifestNotCanonical);
    }
    if retained_manifest_digest(&manifest.points) != manifest.manifest_digest {
        return Err(RebuildError::DigestMismatch);
    }
    Ok(())
}

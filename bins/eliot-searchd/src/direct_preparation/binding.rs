//! Durable DIRECT preparation representation binding.

use super::layout::encode_preparation;
use super::profile::{
    canonical_materializer_digest, canonical_unitizer_digest,
};

/// Domain-separated BLAKE3 representation identity over the exact source
/// binding, canonical bytes (or explicit gap reason) and both profile digests.
/// Computed with the real `blake3` crate; never a SHA-256 relabel.
#[allow(clippy::too_many_arguments)]
pub fn representation_id(
    namespace: &[u8; 32],
    source_id: &[u8; 32],
    revision_id: &[u8; 32],
    content_digest: &[u8; 32],
    byte_length: u64,
    materializer_digest: &[u8; 32],
    unitizer_digest: &[u8; 32],
    canonical_or_gap: &[u8],
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"eliot-searchd/preparation-representation/v1\x00");
    hasher.update(namespace);
    hasher.update(source_id);
    hasher.update(revision_id);
    hasher.update(content_digest);
    hasher.update(&byte_length.to_be_bytes());
    hasher.update(materializer_digest);
    hasher.update(unitizer_digest);
    hasher.update(&(canonical_or_gap.len() as u64).to_be_bytes());
    hasher.update(canonical_or_gap);
    *hasher.finalize().as_bytes()
}

/// Canonical preparation body plus its representation identity.
///
/// For layout inputs the body equals the existing exact layout encoding, so
/// search coordinates stay source-accurate. A leading BOM is an explicit gap:
/// DIRECT has no coordinate reprojection, so stripping it would silently shift
/// every subsequent offset. No receipt is fabricated.
pub fn encode_canonical_preparation(
    bytes: &[u8],
    namespace: &[u8; 32],
    source_id: &[u8; 32],
    revision_id: &[u8; 32],
    content_digest: &[u8; 32],
    byte_length: u64,
) -> Result<([u8; 32], Vec<u8>), &'static str> {
    let materializer_digest = canonical_materializer_digest()?;
    let unitizer_digest = canonical_unitizer_digest()?;
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        let body = vec![6];
        let representation = representation_id(
            namespace,
            source_id,
            revision_id,
            content_digest,
            byte_length,
            &materializer_digest,
            &unitizer_digest,
            b"DIRECT_REVISION_HAS_BOM",
        );
        return Ok((representation, body));
    }
    let body = encode_preparation(bytes)?;
    let marker: &[u8] = match body.as_slice() {
        [0, layout @ ..] => layout,
        [1] => b"DIRECT_REVISION_NOT_UTF8",
        [2] => b"MATERIALIZATION_BINARY_CONTENT",
        [3] => b"MATERIALIZATION_TOO_MANY_LINES",
        [4] => b"UNITIZATION_TOO_MANY_UNITS",
        [5] => b"DIRECT_PREPARATION_LAYOUT_TOO_LARGE",
        [6] => b"DIRECT_REVISION_HAS_BOM",
        _ => return Err("DIRECT_PREPARATION_INVALID"),
    };
    let representation = representation_id(
        namespace,
        source_id,
        revision_id,
        content_digest,
        byte_length,
        &materializer_digest,
        &unitizer_digest,
        marker,
    );
    Ok((representation, body))
}

/// Canonical preparation receipt carrying real provenance for one durable object.
pub struct CanonicalPreparationReceipt {
    /// BLAKE3 representation identity bound to source, body and profiles.
    pub representation_id: [u8; 32],
    /// Canonical materializer profile digest bytes.
    pub materializer_digest: [u8; 32],
    /// Canonical unitizer profile digest bytes.
    pub unitizer_digest: [u8; 32],
    /// Closed preparation gap, if the source is not layout-searchable.
    pub gap: Option<&'static str>,
}

impl CanonicalPreparationReceipt {
    /// Representation identity as lowercase hexadecimal.
    pub fn representation_hex(&self) -> String {
        crate::sha256::hex(&self.representation_id)
    }
}

/// Verifies that a stored representation binds the live source bytes and the
/// current canonical profiles. Profile drift or tampering fails closed.
pub fn verify_canonical_representation(
    expected: &[u8; 32],
    bytes: &[u8],
    namespace: &[u8; 32],
    source_id: &[u8; 32],
    revision_id: &[u8; 32],
    content_digest: &[u8; 32],
    byte_length: u64,
) -> Result<(), &'static str> {
    let (recomputed, _) = encode_canonical_preparation(
        bytes,
        namespace,
        source_id,
        revision_id,
        content_digest,
        byte_length,
    )?;
    if &recomputed == expected {
        Ok(())
    } else {
        Err("DIRECT_PREPARATION_BINDING_MISMATCH")
    }
}

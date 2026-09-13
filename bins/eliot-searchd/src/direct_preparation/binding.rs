//! Durable DIRECT preparation representation binding.

use search_materializer::api::{
    LegacyDirectPreparationBinding, LegacyDirectPreparationGap,
    LegacyDirectRepresentationDigest, decode_legacy_direct_preparation,
    derive_legacy_direct_representation_id, encode_legacy_direct_gap,
};

use super::layout::encode_preparation;
use super::profile::{
    canonical_materializer_digest, canonical_unitizer_digest,
};

struct DirectRepresentationDigest;

impl LegacyDirectRepresentationDigest for DirectRepresentationDigest {
    fn digest_parts(domain: &[u8], parts: &[&[u8]]) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        hasher.update(domain);
        for part in parts {
            hasher.update(part);
        }
        *hasher.finalize().as_bytes()
    }
}

#[allow(clippy::too_many_arguments)]
fn binding(
    namespace: &[u8; 32],
    source_id: &[u8; 32],
    revision_id: &[u8; 32],
    content_digest: &[u8; 32],
    byte_length: u64,
    materializer_digest: &[u8; 32],
    unitizer_digest: &[u8; 32],
) -> LegacyDirectPreparationBinding {
    LegacyDirectPreparationBinding {
        namespace: *namespace,
        source_id: *source_id,
        revision_id: *revision_id,
        content_digest: *content_digest,
        byte_length,
        materializer_digest: *materializer_digest,
        unitizer_digest: *unitizer_digest,
    }
}

/// Domain-separated BLAKE3 representation identity over the exact source
/// binding, canonical bytes (or explicit gap reason) and both profile digests.
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
    derive_legacy_direct_representation_id::<DirectRepresentationDigest>(
        &binding(
            namespace,
            source_id,
            revision_id,
            content_digest,
            byte_length,
            materializer_digest,
            unitizer_digest,
        ),
        canonical_or_gap,
    )
}

/// Canonical preparation body plus its representation identity.
///
/// The materializer owner decodes the persisted frame and supplies the exact
/// representation marker. The daemon composes profiles and unitization only.
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
    let binding = binding(
        namespace,
        source_id,
        revision_id,
        content_digest,
        byte_length,
        &materializer_digest,
        &unitizer_digest,
    );
    let body = if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        encode_legacy_direct_gap(LegacyDirectPreparationGap::RevisionHasBom)
    } else {
        encode_preparation(bytes)?
    };
    let frame = decode_legacy_direct_preparation(&body).map_err(|error| error.code())?;
    let representation = derive_legacy_direct_representation_id::<DirectRepresentationDigest>(
        &binding,
        frame.identity_marker(),
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

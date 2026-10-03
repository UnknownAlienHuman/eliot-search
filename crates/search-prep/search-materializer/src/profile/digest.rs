//! Deterministic materializer profile identity digests.

use super::model::{MaterializerProfileId, ValidatedMaterializerProfile};

/// BLAKE3-256 over the versioned, unambiguous identity envelope.
///
/// The envelope is `u64_le(domain length) || domain || u64_le(chunk count) ||`
/// followed by `u64_le(chunk length) || chunk` for each chunk. Content digests
/// use [`digest_content_bytes`] instead and hash the exact content bytes
/// directly, without this identity envelope.
pub fn digest32(domain: &[u8], chunks: &[&[u8]]) -> [u8; 32] {
    fn update_length(hasher: &mut blake3::Hasher, length: usize) {
        // Rust slice lengths fit in u64 on the supported 32- and 64-bit
        // targets. This is the fixed-width representation used by v2.
        hasher.update(&(length as u64).to_le_bytes());
    }

    let mut hasher = blake3::Hasher::new();
    update_length(&mut hasher, domain.len());
    hasher.update(domain);
    update_length(&mut hasher, chunks.len());
    for chunk in chunks {
        update_length(&mut hasher, chunk.len());
        hasher.update(chunk);
    }
    *hasher.finalize().as_bytes()
}

/// Direct BLAKE3-256 content digest over the exact supplied bytes.
pub(crate) fn digest_content_bytes(bytes: &[u8]) -> [u8; 32] {
    *blake3::hash(bytes).as_bytes()
}

/// Domain-separated canonical digest over every load-bearing behavior and
/// bound. Any encoding, normalization, coordinate, loss, assurance or limit
/// change creates a different profile identity.
#[must_use]
pub fn profile_digest(profile: &ValidatedMaterializerProfile) -> MaterializerProfileId {
    let mut kind_tags = [0_u8; 2];
    for (index, kind) in profile.kinds.iter().enumerate() {
        if let Some(slot) = kind_tags.get_mut(index) {
            *slot = kind.tag();
        }
    }
    let mut encoding_tags = [0_u8; 3];
    for (index, encoding) in profile.encodings.iter().enumerate() {
        if let Some(slot) = encoding_tags.get_mut(index) {
            *slot = encoding.tag();
        }
    }
    let mut space_tags = [0_u8; 3];
    for (index, space) in profile.spaces.iter().enumerate() {
        if let Some(slot) = space_tags.get_mut(index) {
            *slot = space.tag();
        }
    }
    let limits = profile.limits;
    let chunks: &[&[u8]] = &[
        profile.name.as_bytes(),
        &profile.revision.to_le_bytes(),
        &kind_tags,
        &encoding_tags,
        &[profile.bom_policy.tag()],
        &[profile.invalid_sequence_policy.tag()],
        &[profile.newline_policy.tag()],
        &[profile.unicode_normalization.tag()],
        &[profile.loss_behavior.tag()],
        &limits.max_input_bytes.to_le_bytes(),
        &limits.max_output_bytes.to_le_bytes(),
        &limits.max_lines.to_le_bytes(),
        &limits.max_map_segments.to_le_bytes(),
        &limits.max_loss_records.to_le_bytes(),
        &limits.max_steps.to_le_bytes(),
        &space_tags,
        profile.golden.as_bytes(),
    ];
    MaterializerProfileId::from_bytes(digest32(b"eliot-search/materializer/profile/v2", chunks))
}

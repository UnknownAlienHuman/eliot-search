//! Deterministic materializer profile identity digests.

use super::model::{MaterializerProfileId, ValidatedMaterializerProfile};

/// Domain-separated 32-byte digest over an ordered byte preimage.
///
/// Four independent FNV-1a 64-bit lanes absorb the domain, a domain
/// separator, then each length-delimited chunk. Fixed little-endian lane
/// encoding keeps the digest byte-identical across runs and platforms. This
/// is an identity digest, not a cryptographic commitment over source bytes.
pub fn digest32(domain: &[u8], chunks: &[&[u8]]) -> [u8; 32] {
    const SEED: [u64; 4] = [
        0xcbf2_9ce4_8422_2325,
        0x8422_2325_cbf2_9ce4,
        0x4822_2325_cbf2_9ce4,
        0x2325_cbf2_9ce4_8422,
    ];
    const PRIME: u64 = 0x1_0000_0000_01B3;
    let mut lanes = SEED;
    let mut index = 0_usize;
    let mut absorb = |byte: u8| {
        let slot = index % 4;
        lanes[slot] ^= u64::from(byte);
        lanes[slot] = lanes[slot].wrapping_mul(PRIME);
        index = index.wrapping_add(1);
    };
    for byte in domain {
        absorb(*byte);
    }
    absorb(0xFF);
    for chunk in chunks {
        for byte in *chunk {
            absorb(*byte);
        }
        absorb(0xFE);
    }
    let mut out = [0_u8; 32];
    for (slot, lane) in lanes.iter().enumerate() {
        let start = slot * 8;
        out[start..start + 8].copy_from_slice(&lane.to_le_bytes());
    }
    out
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
    MaterializerProfileId::from_bytes(digest32(b"eliot-search/materializer/profile/v1", chunks))
}

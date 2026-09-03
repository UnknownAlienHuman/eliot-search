//! Canonical configuration fingerprints without external hashing dependencies.

#![allow(
    clippy::chunks_exact_to_as_chunks,
    clippy::many_single_char_names,
    clippy::unreadable_literal
)]

use std::num::NonZeroU64;

use search_contracts::{Blake3Digest32, ProfileId, Sha256Digest32};

use crate::{ConfigError, ConfigSectionName};

/// SHA-256 fingerprint over the canonical effective-configuration identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ConfigFingerprint(Sha256Digest32);

impl ConfigFingerprint {
    /// Creates a fingerprint from exact digest bytes.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(Sha256Digest32::from_bytes(bytes))
    }

    /// Exact digest bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        self.0.as_bytes()
    }

    /// Shared SHA-256 digest wrapper.
    #[must_use]
    pub const fn digest(self) -> Sha256Digest32 {
        self.0
    }
}

/// Canonical identity of one validated section.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SectionFingerprintInput<'a> {
    /// Canonical section name.
    pub section_name: &'a ConfigSectionName,
    /// Descriptor schema revision.
    pub schema_revision: NonZeroU64,
    /// Digest of the exact field registry.
    pub field_registry_digest: Blake3Digest32,
    /// Digest returned by the owning capability after validation.
    pub validated_section_digest: Blake3Digest32,
}

/// Computes the effective configuration fingerprint.
///
/// The preimage includes only schema/profile/descriptor identities and validated
/// section digests. Plaintext secrets cannot enter because they are structurally
/// forbidden before section validation.
///
/// # Errors
///
/// Returns [`ConfigError::CanonicalBytesExceeded`] when the canonical preimage
/// would exceed `max_bytes`.
pub fn fingerprint<'a>(
    config_schema_version: u32,
    selected_profile: &ProfileId,
    sections: impl IntoIterator<Item = SectionFingerprintInput<'a>>,
    max_bytes: usize,
) -> Result<ConfigFingerprint, ConfigError> {
    let mut writer = CanonicalWriter::new(max_bytes)?;
    writer.bytes(b"eliot-search/config-fingerprint/v1\0")?;
    writer.u32(config_schema_version)?;
    writer.text(selected_profile.as_str())?;
    let mut sections = sections.into_iter().collect::<Vec<_>>();
    sections.sort_by(|left, right| left.section_name.cmp(right.section_name));
    writer.usize(sections.len())?;
    for section in sections {
        writer.text(section.section_name.as_str())?;
        writer.u64(section.schema_revision.get())?;
        writer.bytes(section.field_registry_digest.as_bytes())?;
        writer.bytes(section.validated_section_digest.as_bytes())?;
    }
    Ok(ConfigFingerprint::from_bytes(sha256(writer.as_slice())))
}

/// Computes a SHA-256 digest over an arbitrary already-bounded byte slice.
#[must_use]
pub(crate) fn sha256_digest(bytes: &[u8]) -> Sha256Digest32 {
    Sha256Digest32::from_bytes(sha256(bytes))
}

pub(crate) struct CanonicalWriter {
    bytes: Vec<u8>,
    max_bytes: usize,
}

impl CanonicalWriter {
    pub(crate) const fn new(max_bytes: usize) -> Result<Self, ConfigError> {
        if max_bytes == 0 {
            return Err(ConfigError::CanonicalBytesExceeded);
        }
        Ok(Self {
            bytes: Vec::new(),
            max_bytes,
        })
    }

    pub(crate) fn as_slice(&self) -> &[u8] {
        &self.bytes
    }

    pub(crate) fn bytes(&mut self, bytes: &[u8]) -> Result<(), ConfigError> {
        let final_len = self
            .bytes
            .len()
            .checked_add(bytes.len())
            .ok_or(ConfigError::LengthOverflow)?;
        if final_len > self.max_bytes {
            return Err(ConfigError::CanonicalBytesExceeded);
        }
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }

    pub(crate) fn u32(&mut self, value: u32) -> Result<(), ConfigError> {
        self.bytes(&value.to_be_bytes())
    }

    pub(crate) fn u64(&mut self, value: u64) -> Result<(), ConfigError> {
        self.bytes(&value.to_be_bytes())
    }

    pub(crate) fn usize(&mut self, value: usize) -> Result<(), ConfigError> {
        let value = u64::try_from(value).map_err(|_| ConfigError::LengthOverflow)?;
        self.u64(value)
    }

    pub(crate) fn text(&mut self, value: &str) -> Result<(), ConfigError> {
        self.usize(value.len())?;
        self.bytes(value.as_bytes())
    }
}

const SHA256_INITIAL: [u32; 8] = [
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
];

const SHA256_K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

fn sha256(input: &[u8]) -> [u8; 32] {
    let bit_len = (input.len() as u64).wrapping_mul(8);
    let padded_len = input
        .len()
        .checked_add(9)
        .and_then(|value| value.checked_add(63))
        .map(|value| value / 64 * 64)
        .expect("slice lengths fit address space");
    let mut padded = Vec::with_capacity(padded_len);
    padded.extend_from_slice(input);
    padded.push(0x80);
    padded.resize(padded_len - 8, 0);
    padded.extend_from_slice(&bit_len.to_be_bytes());

    let mut state = SHA256_INITIAL;
    for chunk in padded.chunks_exact(64) {
        let mut schedule = [0_u32; 64];
        for (index, word) in chunk.chunks_exact(4).enumerate() {
            schedule[index] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for index in 16..64 {
            let left = schedule[index - 15];
            let right = schedule[index - 2];
            let sigma0 = left.rotate_right(7) ^ left.rotate_right(18) ^ (left >> 3);
            let sigma1 = right.rotate_right(17) ^ right.rotate_right(19) ^ (right >> 10);
            schedule[index] = schedule[index - 16]
                .wrapping_add(sigma0)
                .wrapping_add(schedule[index - 7])
                .wrapping_add(sigma1);
        }

        let mut a = state[0];
        let mut b = state[1];
        let mut c = state[2];
        let mut d = state[3];
        let mut e = state[4];
        let mut f = state[5];
        let mut g = state[6];
        let mut h = state[7];

        for index in 0..64 {
            let upper1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let choose = (e & f) ^ ((!e) & g);
            let temporary1 = h
                .wrapping_add(upper1)
                .wrapping_add(choose)
                .wrapping_add(SHA256_K[index])
                .wrapping_add(schedule[index]);
            let upper0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let majority = (a & b) ^ (a & c) ^ (b & c);
            let temporary2 = upper0.wrapping_add(majority);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temporary1);
            d = c;
            c = b;
            b = a;
            a = temporary1.wrapping_add(temporary2);
        }

        state[0] = state[0].wrapping_add(a);
        state[1] = state[1].wrapping_add(b);
        state[2] = state[2].wrapping_add(c);
        state[3] = state[3].wrapping_add(d);
        state[4] = state[4].wrapping_add(e);
        state[5] = state[5].wrapping_add(f);
        state[6] = state[6].wrapping_add(g);
        state[7] = state[7].wrapping_add(h);
    }

    let mut output = [0_u8; 32];
    for (chunk, word) in output.chunks_exact_mut(4).zip(state) {
        chunk.copy_from_slice(&word.to_be_bytes());
    }
    output
}

#[cfg(test)]
mod tests {
    use super::sha256;

    #[test]
    fn sha256_matches_known_vector() {
        assert_eq!(
            sha256(b"abc"),
            [
                0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae,
                0x22, 0x23, 0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61,
                0xf2, 0x00, 0x15, 0xad,
            ]
        );
    }
}

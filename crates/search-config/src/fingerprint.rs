//! Legacy v1 configuration preimages hashed by the shared exact-byte SHA owner.

use std::num::NonZeroU64;

use search_contracts::{
    Blake3Digest32, DigestInputLimit, MAX_CANONICAL_BYTES, ProfileId, Sha256Digest32,
    sha256_exact_bytes,
};

use crate::{ConfigError, ConfigSectionName};

/// SHA-256 over the legacy v1 effective-configuration preimage, never a v2 identity.
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
/// would exceed the caller's `max_bytes` or the shared hard 8 MiB ceiling.
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
    Ok(ConfigFingerprint(sha256_digest(writer.as_slice())?))
}

/// Exact-byte compatibility only; oversized inputs yield no digest.
pub(crate) fn sha256_digest(bytes: &[u8]) -> Result<Sha256Digest32, ConfigError> {
    let limit = DigestInputLimit::new(MAX_CANONICAL_BYTES)
        .map_err(|_| ConfigError::CanonicalBytesExceeded)?;
    sha256_exact_bytes(bytes, limit).map_err(|_| ConfigError::CanonicalBytesExceeded)
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
            max_bytes: if max_bytes > MAX_CANONICAL_BYTES {
                MAX_CANONICAL_BYTES
            } else {
                max_bytes
            },
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

#[cfg(test)]
mod tests {
    // #386 compatibility fixtures; AUTHORED_NOT_RUN.
    use super::{CanonicalWriter, SectionFingerprintInput, fingerprint, sha256_digest};
    use crate::{ConfigError, ConfigSectionName};
    use search_contracts::{Blake3Digest32, MAX_CANONICAL_BYTES, ProfileId};
    use std::num::NonZeroU64;

    #[test]
    fn sha256_matches_known_vector() {
        assert_eq!(
            *sha256_digest(b"abc").expect("bounded digest").as_bytes(),
            [
                0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae,
                0x22, 0x23, 0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61,
                0xf2, 0x00, 0x15, 0xad,
            ]
        );
    }

    #[test]
    fn legacy_empty_preimage_keeps_its_exact_v1_digest_and_byte_limit() {
        let profile = ProfileId::new("direct").expect("profile");
        assert_eq!(
            fingerprint(1, &profile, [], 61)
                .expect("exact 61-byte v1 preimage")
                .digest()
                .to_string(),
            "1509fa0539c54676a23eeefcba52575c411272408a559077a53ffbf20f102ec6",
        );
        assert_eq!(
            fingerprint(1, &profile, [], 60),
            Err(ConfigError::CanonicalBytesExceeded)
        );
        assert_eq!(
            fingerprint(1, &profile, [], 0),
            Err(ConfigError::CanonicalBytesExceeded)
        );
    }

    #[test]
    fn legacy_multi_section_preimage_keeps_its_exact_sorted_v1_digest() {
        let profile = ProfileId::new("direct").expect("profile");
        let alpha = ConfigSectionName::new("alpha", 128).expect("section");
        let zeta = ConfigSectionName::new("zeta", 128).expect("section");
        let sections = [
            SectionFingerprintInput {
                section_name: &zeta,
                schema_revision: NonZeroU64::new(7).expect("revision"),
                field_registry_digest: Blake3Digest32::from_bytes([0x33; 32]),
                validated_section_digest: Blake3Digest32::from_bytes([0x44; 32]),
            },
            SectionFingerprintInput {
                section_name: &alpha,
                schema_revision: NonZeroU64::new(2).expect("revision"),
                field_registry_digest: Blake3Digest32::from_bytes([0x11; 32]),
                validated_section_digest: Blake3Digest32::from_bytes([0x22; 32]),
            },
        ];
        assert_eq!(
            fingerprint(1, &profile, sections.clone(), 230)
                .expect("exact v1 preimage")
                .digest()
                .to_string(),
            "07856290f0e0a98e40b92739af37b9a1e7782dc20135190039b286b9f23d94f7",
        );
        assert_eq!(
            fingerprint(1, &profile, sections, 229),
            Err(ConfigError::CanonicalBytesExceeded)
        );
    }

    #[test]
    fn hard_digest_ceiling_rejects_before_writer_growth() {
        let mut writer = CanonicalWriter::new(usize::MAX).expect("nonzero policy ceiling");
        assert_eq!(writer.max_bytes, MAX_CANONICAL_BYTES);
        assert_eq!(
            writer.bytes(&vec![0; MAX_CANONICAL_BYTES + 1]),
            Err(ConfigError::CanonicalBytesExceeded)
        );
        assert!(writer.as_slice().is_empty());
    }
}

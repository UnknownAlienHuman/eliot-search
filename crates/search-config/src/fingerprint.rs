//! Versioned configuration equality identities; legacy restoration is decode-only.
use crate::ConfigError;
use search_contracts::{
    Blake3Digest32, DigestInputLimit, MAX_CANONICAL_BYTES, Sha256Digest32, sha256_exact_bytes,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
enum FingerprintDigest {
    LegacySha256V1(Sha256Digest32),
    Blake3CborV2(Blake3Digest32),
}

/// Tagged equality identity, never validation or runtime-application authority.
///
/// New effective snapshots compute canonical v2. Legacy bytes may be restored for
/// comparison/migration only; they cannot construct a current effective snapshot.
/// Equality includes the algorithm/profile/revision variant, even if raw bytes
/// coincide. Input TOML schema and fingerprint revision are distinct.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ConfigFingerprint(FingerprintDigest);

impl ConfigFingerprint {
    /// Restores exact legacy SHA-v1 bytes for decode-only migration/comparison.
    #[must_use]
    pub const fn restore_legacy_v1(bytes: [u8; 32]) -> Self {
        Self(FingerprintDigest::LegacySha256V1(
            Sha256Digest32::from_bytes(bytes),
        ))
    }

    pub(crate) const fn canonical_v2(digest: Blake3Digest32) -> Self {
        Self(FingerprintDigest::Blake3CborV2(digest))
    }

    /// Exact digest bytes; serialization must also preserve all three tags.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        match &self.0 {
            FingerprintDigest::LegacySha256V1(digest) => digest.as_bytes(),
            FingerprintDigest::Blake3CborV2(digest) => digest.as_bytes(),
        }
    }

    /// Closed algorithm spelling for serialization.
    #[must_use]
    pub const fn algorithm(self) -> &'static str {
        match self.0 {
            FingerprintDigest::LegacySha256V1(_) => "sha256",
            FingerprintDigest::Blake3CborV2(_) => "blake3",
        }
    }

    /// Exact preimage profile, independent of the input parser profile.
    #[must_use]
    pub const fn canonical_profile(self) -> &'static str {
        match self.0 {
            FingerprintDigest::LegacySha256V1(_) => "eliot-search/config-fingerprint/v1",
            FingerprintDigest::Blake3CborV2(_) => "eliot/cbor/config-snapshot/v2",
        }
    }

    /// Closed fingerprint revision for serialization.
    #[must_use]
    pub const fn revision(self) -> u32 {
        match self.0 {
            FingerprintDigest::LegacySha256V1(_) => 1,
            FingerprintDigest::Blake3CborV2(_) => 2,
        }
    }

    // Logical payload only, excluding serialization and allocator overhead.
    pub(crate) const fn logical_payload_bytes(self) -> usize {
        32 + self.algorithm().len() + self.canonical_profile().len() + 4
    }
}

/// Exact-byte compatibility for redacted paths only; oversized inputs yield no digest.
pub(crate) fn sha256_digest(bytes: &[u8]) -> Result<Sha256Digest32, ConfigError> {
    let limit = DigestInputLimit::new(MAX_CANONICAL_BYTES)
        .map_err(|_| ConfigError::CanonicalBytesExceeded)?;
    sha256_exact_bytes(bytes, limit).map_err(|_| ConfigError::CanonicalBytesExceeded)
}

#[cfg(test)]
mod tests {
    // Frozen legacy vectors, AUTHORED_NOT_RUN under #352.
    use super::{ConfigFingerprint, sha256_digest};

    #[test]
    fn sha256_matches_known_vector() {
        assert_eq!(
            sha256_digest(b"abc").expect("bounded digest").to_string(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        );
    }

    // Test-only exact v1 preimages; no production legacy encoder remains.
    fn legacy_header(count: u64) -> Vec<u8> {
        let mut bytes = b"eliot-search/config-fingerprint/v1\0".to_vec();
        bytes.extend_from_slice(&1_u32.to_be_bytes());
        bytes.extend_from_slice(&6_u64.to_be_bytes());
        bytes.extend_from_slice(b"direct");
        bytes.extend_from_slice(&count.to_be_bytes());
        bytes
    }

    #[test]
    fn legacy_empty_vector_is_restoration_only() {
        let bytes = legacy_header(0);
        assert_eq!(bytes.len(), 61);
        let digest = sha256_digest(&bytes).expect("exact legacy bytes");
        assert_eq!(
            digest.to_string(),
            "1509fa0539c54676a23eeefcba52575c411272408a559077a53ffbf20f102ec6"
        );
        let restored = ConfigFingerprint::restore_legacy_v1(*digest.as_bytes());
        assert_eq!(restored.algorithm(), "sha256");
        assert_eq!(restored.revision(), 1);
        assert_eq!(
            restored.canonical_profile(),
            "eliot-search/config-fingerprint/v1"
        );
    }

    #[test]
    fn legacy_multi_section_vector_keeps_exact_bytes() {
        let mut bytes = legacy_header(2);
        for (name, revision, registry, section) in
            [("alpha", 2_u64, 0x11_u8, 0x22_u8), ("zeta", 7, 0x33, 0x44)]
        {
            bytes.extend_from_slice(
                &u64::try_from(name.len())
                    .expect("name length")
                    .to_be_bytes(),
            );
            bytes.extend_from_slice(name.as_bytes());
            bytes.extend_from_slice(&revision.to_be_bytes());
            bytes.extend_from_slice(&[registry; 32]);
            bytes.extend_from_slice(&[section; 32]);
        }
        assert_eq!(bytes.len(), 230);
        assert_eq!(
            sha256_digest(&bytes)
                .expect("exact legacy bytes")
                .to_string(),
            "07856290f0e0a98e40b92739af37b9a1e7782dc20135190039b286b9f23d94f7"
        );
    }
}

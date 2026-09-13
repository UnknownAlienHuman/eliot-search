//! Closed durable unit-manifest format and identity constants.

use search_contracts::DigestAlgorithm;

/// Durable unit-manifest format identity. Changing boundary semantics,
/// serialization or digest domains requires a new identity; saved manifests
/// are never reinterpreted under a changed format.
pub const UNIT_MANIFEST_FORMAT: &str = "exact-unit-manifest/v1";
/// Durable unit-manifest codec version.
pub const UNIT_MANIFEST_VERSION: u16 = 1;
/// The sole digest algorithm a durable manifest may bind. Any other algorithm
/// is rejected instead of reinterpreted.
pub const UNIT_MANIFEST_DIGEST_ALGORITHM: DigestAlgorithm = DigestAlgorithm::Blake3_256;
/// Maximum unitizer profile-name length in bytes.
pub const MAX_UNITIZER_PROFILE_NAME_BYTES: usize = 128;

pub(super) const MAGIC: &[u8; 8] = b"ELSUMF01";
pub(super) const PROFILE_DOMAIN: &[u8] = b"eliot-search/unitizer/profile/v1";
pub(super) const UNIT_DOMAIN: &[u8] = b"eliot-search/unitizer/unit/v1";
pub(super) const MANIFEST_DOMAIN: &[u8] = b"eliot-search/unitizer/manifest/v1";
pub(super) const UNIT_CODEC_BYTES: usize = 73;
pub(super) const DIGEST_TRAILER_BYTES: usize = 32;

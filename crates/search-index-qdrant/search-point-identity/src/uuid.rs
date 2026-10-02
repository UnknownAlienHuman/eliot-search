use core::fmt;

use crate::PointIdentityDigest;

const UUID_PROJECTION_DOMAIN: &[u8] = b"eliot-search/point-uuid/v1\0";

/// Compact 128-bit provider-neutral point address.
///
/// The value is rendered in UUID form for Qdrant, but correctness never relies
/// on the 128-bit projection alone. Full-digest and identity-field validation
/// is mandatory before overwrite.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PointId128([u8; 16]);

impl PointId128 {
    /// Creates an address from exact bytes.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    /// Exact 16 projected bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }

    /// Deterministic lower-case hexadecimal representation.
    #[must_use]
    pub fn to_hex(self) -> String {
        let mut output = String::with_capacity(32);
        for byte in self.0 {
            use core::fmt::Write as _;
            write!(&mut output, "{byte:02x}")
                .expect("writing hexadecimal into String cannot fail");
        }
        output
    }

    /// Hyphenated UUID representation accepted by Qdrant point IDs.
    #[must_use]
    pub fn to_hyphenated(self) -> String {
        let hex = self.to_hex();
        format!(
            "{}-{}-{}-{}-{}",
            &hex[0..8],
            &hex[8..12],
            &hex[12..16],
            &hex[16..20],
            &hex[20..32]
        )
    }
}

impl fmt::Debug for PointId128 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("PointId128")
            .field(&self.to_hyphenated())
            .finish()
    }
}

impl fmt::Display for PointId128 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.to_hyphenated())
    }
}

/// Logical name for the Qdrant UUID projection.
pub type QdrantPointUuid = PointId128;

/// Derives the namespace-separated 128-bit Qdrant UUID projection.
///
/// A second BLAKE3 domain prevents the UUID address from being interpreted as
/// a raw prefix of the full identity digest.
#[must_use]
pub fn derive_qdrant_uuid(digest: PointIdentityDigest) -> QdrantPointUuid {
    let mut hasher = blake3::Hasher::new();
    hasher.update(UUID_PROJECTION_DOMAIN);
    hasher.update(digest.as_bytes());
    let projected = hasher.finalize();
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&projected.as_bytes()[..16]);
    PointId128::from_bytes(bytes)
}

/// Agent-contract spelling for [`derive_qdrant_uuid`].
#[must_use]
pub fn project_qdrant_uuid(digest: PointIdentityDigest) -> QdrantPointUuid {
    derive_qdrant_uuid(digest)
}

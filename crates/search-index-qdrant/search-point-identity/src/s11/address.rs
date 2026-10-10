//! Byte-exact 128-bit Qdrant point address.
//!
//! A [`PointId128`] stores the exact 16 bytes produced by the S11 address
//! projection. It preserves every delivered bit, including the version and
//! variant positions of a UUID rendering: no nibble is rewritten to imitate a
//! random UUID. The address locates a point inside one collection generation and
//! is never evidence of identity on its own.
//!
//! The projection is `project_address`. It binds the frozen raw-byte address
//! domain to the complete 256-bit full digest of a point key through the sole
//! shared digest owner, so the identity and address preimages stay disjoint.
//!
//! Rendering and parsing reuse the shared [`UuidBytes`] type; no local codec
//! and no vendor UUID type enter this module.
//!

use core::fmt;

use search_contracts::{
    Blake3Digest32, CanonicalDigestDomain, DigestInputLimit, UuidBytes, blake3_raw,
};

/// Number of address bytes carried by one Qdrant point address.
pub(super) const ADDRESS_BYTES: usize = 16;

/// Exact 128-bit point address for one Qdrant point.
///
/// Use the shared [`UuidBytes`] representation for rendering and parsing so that
/// a round trip reproduces the stored bytes bit for bit. Equality compares the
/// complete stored value, so two addresses differ when any byte differs.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct PointId128(UuidBytes);

impl PointId128 {
    /// Rebuilds an address from exact stored or received bytes.
    ///
    /// This is a decode path: it preserves the given bytes and runs no
    /// projection algorithm.
    #[must_use]
    pub const fn from_stored_bytes(bytes: [u8; ADDRESS_BYTES]) -> Self {
        Self(UuidBytes::from_bytes(bytes))
    }

    /// Exact 16 address bytes.
    pub const fn as_bytes(&self) -> &[u8; ADDRESS_BYTES] {
        self.0.as_bytes()
    }

    /// Parses a hyphenated lower-case UUID rendering into exact bytes.
    ///
    /// Any non-UUID shape is a malformed stored address and fails as
    /// [`PointIdError::InvalidUuid`](super::PointIdError::InvalidUuid).
    pub fn parse(value: &str) -> Result<Self, super::PointIdError> {
        UuidBytes::parse(value)
            .map(Self)
            .map_err(|_| super::PointIdError::InvalidUuid)
    }
}

impl fmt::Display for PointId128 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl From<[u8; ADDRESS_BYTES]> for PointId128 {
    fn from(bytes: [u8; ADDRESS_BYTES]) -> Self {
        Self::from_stored_bytes(bytes)
    }
}

impl From<UuidBytes> for PointId128 {
    fn from(bytes: UuidBytes) -> Self {
        Self(bytes)
    }
}

impl From<PointId128> for [u8; ADDRESS_BYTES] {
    fn from(address: PointId128) -> Self {
        *address.as_bytes()
    }
}

/// Projects the complete full identity digest into the 128-bit address.
///
/// The preimage is exactly `ADDRESS_DOMAIN || NUL || full digest bytes`,
/// which the shared raw-byte digest helper enforces together with the complete
/// preimage ceiling. The first 16 bytes of the result are the address; later
/// bytes of the digest stay in the full identity and are never truncated into
/// the address.
pub(super) fn project_address(
    full: &Blake3Digest32,
    limit: DigestInputLimit,
) -> Result<PointId128, super::PointIdError> {
    let domain = CanonicalDigestDomain::parse(super::ADDRESS_DOMAIN)
        .map_err(|_| super::PointIdError::CanonicalEncodingMismatch)?;
    let digest = blake3_raw(&domain, full.as_bytes(), limit)
        .map_err(|_| super::PointIdError::CanonicalEncodingMismatch)?;
    let bytes = digest.as_bytes();
    let mut address = [0_u8; ADDRESS_BYTES];
    address.copy_from_slice(
        bytes
            .get(..ADDRESS_BYTES)
            .ok_or(super::PointIdError::CanonicalEncodingMismatch)?,
    );
    Ok(PointId128::from_stored_bytes(address))
}

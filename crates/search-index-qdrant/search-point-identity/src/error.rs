use core::fmt;

/// Closed point-identity failure.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PointIdentityError {
    /// Finite limits are zero or internally inconsistent.
    InvalidLimits,
    /// The key schema version is not the exact admitted version.
    PointKeyVersionUnsupported,
    /// The projection-profile-set identifier exceeds the finite boundary.
    IdentifierTooLong,
    /// Deterministic CBOR encoding failed or overflowed a length conversion.
    CanonicalEncodingFailed,
    /// Canonical key bytes exceed the finite package ceiling.
    CanonicalBytesExceeded,
    /// One projected UUID names another full identity.
    DigestCollision,
    /// Full digest or canonical identity payload fields differ.
    IdentityMismatch,
}

impl PointIdentityError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidLimits => "POINT_ID_INVALID_LIMITS",
            Self::PointKeyVersionUnsupported => "POINT_KEY_VERSION_UNSUPPORTED",
            Self::IdentifierTooLong => "POINT_ID_IDENTIFIER_TOO_LONG",
            Self::CanonicalEncodingFailed => "CANONICAL_ENCODING_FAILED",
            Self::CanonicalBytesExceeded => "POINT_ID_CANONICAL_BYTES_EXCEEDED",
            Self::DigestCollision => "POINT_ID_COLLISION",
            Self::IdentityMismatch => "POINT_IDENTITY_MISMATCH",
        }
    }
}

impl fmt::Display for PointIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for PointIdentityError {}

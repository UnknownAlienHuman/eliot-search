use core::fmt;

use search_point_identity::PointIdentityError;

/// Closed projection-planning failure.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ProjectionError {
    /// A finite planning or canonical-encoding bound is invalid.
    InvalidLimits,
    /// The configured BLAKE3-256 digest provider failed closed.
    DigestUnavailable,
    /// The source/projection membership binding is incoherent.
    MembershipMismatch,
    /// Inputs in one exact plan span different collection or membership scopes.
    ScopeMismatch,
    /// The projection schema/profile set is incompatible with the membership.
    ProfileSetMismatch,
    /// The exact named-vector set differs from the accepted profile set.
    VectorSetMismatch,
    /// A vector name appears more than once in one point.
    DuplicateVectorName,
    /// A vector dimension or sparse/dense kind is incompatible.
    VectorDimensionMismatch,
    /// Vector values are empty, non-finite, unordered, or out of range.
    InvalidVector,
    /// A finite point, vector, or canonical-byte budget was exceeded.
    BudgetExceeded,
    /// Canonical point identity derivation or collision checking failed.
    PointIdentity,
    /// Two unit-role inputs resolve to the same logical role.
    DuplicateUnitRole,
    /// Two units carry the same scoring-document identity.
    DuplicateScoringDocument,
    /// Two exact point specifications use the same compact point address.
    DuplicatePointId,
    /// The exact point payload is internally contradictory.
    PointPayloadInvalid,
    /// Manifest entries are unsorted, duplicated, or inconsistent.
    InvalidManifest,
    /// Canonical manifest encoding exceeded its finite ceiling.
    ManifestTooLarge,
    /// A required collection named vector is absent.
    CollectionVectorMissing,
    /// A collection named-vector shape is incompatible.
    CollectionVectorMismatch,
    /// A mandatory S9.5 payload index is absent.
    PayloadIndexMissing,
    /// A mandatory S9.5 payload index has the wrong type.
    PayloadIndexTypeMismatch,
    /// The collection declares a payload index outside the exact S9.5 set.
    PayloadIndexUnexpected,
}

impl ProjectionError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidLimits => "PROJECTION_INVALID_LIMITS",
            Self::DigestUnavailable => "PROJECTION_DIGEST_UNAVAILABLE",
            Self::MembershipMismatch => "PROJECTION_MEMBERSHIP_MISMATCH",
            Self::ScopeMismatch => "PROJECTION_SCOPE_MISMATCH",
            Self::ProfileSetMismatch => "PROFILE_SET_MISMATCH",
            Self::VectorSetMismatch => "PROJECTION_VECTOR_SET_MISMATCH",
            Self::DuplicateVectorName => "PROJECTION_DUPLICATE_VECTOR_NAME",
            Self::VectorDimensionMismatch => "PROJECTION_VECTOR_DIMENSION_MISMATCH",
            Self::InvalidVector => "PROJECTION_INVALID_VECTOR",
            Self::BudgetExceeded => "PROJECTION_BUDGET_EXCEEDED",
            Self::PointIdentity => "PROJECTION_POINT_IDENTITY_INVALID",
            Self::DuplicateUnitRole => "PROJECTION_DUPLICATE_UNIT_ROLE",
            Self::DuplicateScoringDocument => "PROJECTION_DUPLICATE_SCORING_DOCUMENT",
            Self::DuplicatePointId => "PROJECTION_DUPLICATE_POINT_ID",
            Self::PointPayloadInvalid => "PROJECTION_POINT_PAYLOAD_INVALID",
            Self::InvalidManifest => "PROJECTION_MANIFEST_MISMATCH",
            Self::ManifestTooLarge => "PROJECTION_MANIFEST_TOO_LARGE",
            Self::CollectionVectorMissing => "PROJECTION_COLLECTION_VECTOR_MISSING",
            Self::CollectionVectorMismatch => "PROJECTION_COLLECTION_VECTOR_MISMATCH",
            Self::PayloadIndexMissing => "PROJECTION_PAYLOAD_INDEX_MISSING",
            Self::PayloadIndexTypeMismatch => "PROJECTION_PAYLOAD_INDEX_TYPE_MISMATCH",
            Self::PayloadIndexUnexpected => "PROJECTION_PAYLOAD_INDEX_UNEXPECTED",
        }
    }
}

impl fmt::Display for ProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for ProjectionError {}

impl From<PointIdentityError> for ProjectionError {
    fn from(_: PointIdentityError) -> Self {
        Self::PointIdentity
    }
}

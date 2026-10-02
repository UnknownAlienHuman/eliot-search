use core::fmt;

use search_point_identity::PointIdentityError;

/// Closed projection-planning failure.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ProjectionError {
    /// A finite planning limit is zero or internally inconsistent.
    InvalidLimits,
    /// One complete plan contains no unit points.
    EmptyPointSet,
    /// The membership binding and immutable profile set do not agree.
    ProfileSetMismatch,
    /// A point or manifest entry escaped the one-membership scope.
    MembershipIsolationViolation,
    /// One unit appears more than once for the same point role.
    DuplicateUnitRole,
    /// Two derived points resolve to the same compact UUID.
    DuplicatePointId,
    /// Point identity derivation or validation failed.
    PointIdentity,
    /// Named-vector names differ from the immutable profile set.
    VectorSetMismatch,
    /// One vector name appears more than once.
    DuplicateVectorName,
    /// Vector shape differs from the profile requirement.
    VectorDimensionMismatch,
    /// Vector indices or values are malformed.
    InvalidVector,
    /// The target publication epoch is the reserved empty epoch zero.
    InvalidEpoch,
    /// A finite point/vector/manifest budget was exceeded.
    BudgetExceeded,
    /// An immutable projection manifest is malformed or cannot be reconstructed.
    ManifestMismatch,
    /// Canonical manifest encoding exceeded its finite ceiling.
    ManifestTooLarge,
    /// The exact S9.5 payload index set is incomplete.
    PayloadIndexMissing,
    /// A payload index has the wrong immutable type.
    PayloadIndexTypeMismatch,
    /// The collection has an incompatible additional payload index.
    CollectionSchemaMismatch,
    /// A required named vector is absent from the collection schema.
    CollectionVectorMissing,
    /// A named vector has an incompatible kind, width or IDF modifier.
    CollectionVectorMismatch,
}

impl ProjectionError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidLimits => "PROJECTION_INVALID_LIMITS",
            Self::EmptyPointSet => "PROJECTION_POINT_SET_EMPTY",
            Self::ProfileSetMismatch => "PROFILE_SET_MISMATCH",
            Self::MembershipIsolationViolation => "MEMBERSHIP_ISOLATION_VIOLATION",
            Self::DuplicateUnitRole => "PROJECTION_DUPLICATE_UNIT_ROLE",
            Self::DuplicatePointId => "PROJECTION_DUPLICATE_POINT_ID",
            Self::PointIdentity => "PROJECTION_POINT_IDENTITY_INVALID",
            Self::VectorSetMismatch => "PROJECTION_VECTOR_SET_MISMATCH",
            Self::DuplicateVectorName => "PROJECTION_DUPLICATE_VECTOR_NAME",
            Self::VectorDimensionMismatch => "PROJECTION_VECTOR_DIMENSION_MISMATCH",
            Self::InvalidVector => "PROJECTION_VECTOR_INVALID",
            Self::InvalidEpoch => "PROJECTION_EPOCH_INVALID",
            Self::BudgetExceeded => "PROJECTION_BUDGET_EXCEEDED",
            Self::ManifestMismatch => "PROJECTION_MANIFEST_MISMATCH",
            Self::ManifestTooLarge => "PROJECTION_MANIFEST_TOO_LARGE",
            Self::PayloadIndexMissing => "PROJECTION_PAYLOAD_INDEX_MISSING",
            Self::PayloadIndexTypeMismatch => "PROJECTION_PAYLOAD_INDEX_TYPE_MISMATCH",
            Self::CollectionSchemaMismatch => "PROJECTION_COLLECTION_SCHEMA_MISMATCH",
            Self::CollectionVectorMissing => "PROJECTION_COLLECTION_VECTOR_MISSING",
            Self::CollectionVectorMismatch => "PROJECTION_COLLECTION_VECTOR_MISMATCH",
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

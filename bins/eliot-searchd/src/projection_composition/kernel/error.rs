//! Closed membership-scoped projection failure vocabulary.

use search_projection_planner::ProjectionError;

/// Closed membership-scoped projection composition failure.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ProjectionCompositionError {
    /// A finite limit is zero or internally inconsistent.
    InvalidLimits,
    /// Membership or immutable source identity is empty or mismatched.
    MembershipMismatch,
    /// More than one membership was supplied where exactly one is required.
    MembershipArrayForbidden,
    /// A raw public/vendor collection identifier reached a scope boundary.
    RawCollectionIdForbidden,
    /// Inputs span inequivalent scoring/security/generation domains.
    ScopeMismatch,
    /// Unit residency does not match the admitted residency binding.
    ResidencyMismatch,
    /// An expected unit has no admitted receipt in the exact plan.
    MissingUnitReceipt,
    /// An input unit is outside the declared complete unit set.
    UnexpectedUnit,
    /// Two point specs resolve to the same compact identity.
    DuplicatePoint,
    /// A compact point identifier maps to another complete key.
    PointCollision,
    /// Point identity input is invalid.
    PointIdentityInvalid,
    /// A manifest does not reconstruct exactly its point specs.
    ManifestInvalid,
    /// A plan exceeds its point, vector, or byte budget.
    BudgetExceeded,
    /// The scoped CAS or its references are unavailable or unreadable.
    CasUnavailable,
    /// An immutable CAS object already exists with different bytes.
    CasConflict,
    /// A control reference already exists with a different manifest binding.
    ReferenceConflict,
}

impl ProjectionCompositionError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidLimits => "PROJECTION_COMPOSITION_INVALID_LIMITS",
            Self::MembershipMismatch => "PROJECTION_COMPOSITION_MEMBERSHIP_MISMATCH",
            Self::MembershipArrayForbidden => "PROJECTION_COMPOSITION_MEMBERSHIP_ARRAY_FORBIDDEN",
            Self::RawCollectionIdForbidden => "PROJECTION_COMPOSITION_RAW_COLLECTION_ID_FORBIDDEN",
            Self::ScopeMismatch => "PROJECTION_COMPOSITION_SCOPE_MISMATCH",
            Self::ResidencyMismatch => "PROJECTION_COMPOSITION_RESIDENCY_MISMATCH",
            Self::MissingUnitReceipt => "PROJECTION_COMPOSITION_MISSING_UNIT_RECEIPT",
            Self::UnexpectedUnit => "PROJECTION_COMPOSITION_UNEXPECTED_UNIT",
            Self::DuplicatePoint => "PROJECTION_COMPOSITION_DUPLICATE_POINT",
            Self::PointCollision => "PROJECTION_COMPOSITION_POINT_COLLISION",
            Self::PointIdentityInvalid => "PROJECTION_COMPOSITION_POINT_IDENTITY_INVALID",
            Self::ManifestInvalid => "PROJECTION_COMPOSITION_MANIFEST_INVALID",
            Self::BudgetExceeded => "PROJECTION_COMPOSITION_BUDGET_EXCEEDED",
            Self::CasUnavailable => "PROJECTION_COMPOSITION_CAS_UNAVAILABLE",
            Self::CasConflict => "PROJECTION_COMPOSITION_CAS_CONFLICT",
            Self::ReferenceConflict => "PROJECTION_COMPOSITION_REFERENCE_CONFLICT",
        }
    }
}

impl core::fmt::Display for ProjectionCompositionError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for ProjectionCompositionError {}

impl From<ProjectionError> for ProjectionCompositionError {
    fn from(error: ProjectionError) -> Self {
        match error {
            ProjectionError::InvalidLimits => Self::InvalidLimits,
            ProjectionError::MembershipMismatch => Self::MembershipMismatch,
            ProjectionError::MembershipArrayForbidden => Self::MembershipArrayForbidden,
            ProjectionError::RawCollectionIdForbidden => Self::RawCollectionIdForbidden,
            ProjectionError::ResidencyMismatch => Self::ResidencyMismatch,
            ProjectionError::MissingUnitReceipt => Self::MissingUnitReceipt,
            ProjectionError::UnexpectedUnit => Self::UnexpectedUnit,
            ProjectionError::DuplicatePointId | ProjectionError::DuplicateUnitRole => {
                Self::DuplicatePoint
            }
            ProjectionError::PointIdentity => Self::PointIdentityInvalid,
            ProjectionError::InvalidManifest => Self::ManifestInvalid,
            ProjectionError::BudgetExceeded | ProjectionError::ManifestTooLarge => {
                Self::BudgetExceeded
            }
            ProjectionError::ScopeMismatch
            | ProjectionError::InvalidUnitRange
            | ProjectionError::VectorSetMismatch
            | ProjectionError::DuplicateVectorName
            | ProjectionError::VectorDimensionMismatch
            | ProjectionError::InvalidVector
            | ProjectionError::CollectionVectorMissing
            | ProjectionError::CollectionVectorMismatch
            | ProjectionError::PayloadIndexMissing => Self::ScopeMismatch,
        }
    }
}

impl From<search_point_identity::PointIdentityError> for ProjectionCompositionError {
    fn from(error: search_point_identity::PointIdentityError) -> Self {
        match error {
            search_point_identity::PointIdentityError::DigestCollision => Self::PointCollision,
            search_point_identity::PointIdentityError::RegistryCapacityExceeded => {
                Self::BudgetExceeded
            }
            _ => Self::PointIdentityInvalid,
        }
    }
}

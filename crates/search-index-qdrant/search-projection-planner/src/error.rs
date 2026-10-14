//! Closed projection-planning failure.

use core::fmt;

use search_contracts::ContractError;
use search_point_identity::s11::PointIdError;

/// Closed pure-planning refusal.
///
/// Every variant is a content-free reason: no source text, path, corpus name,
/// ACL subject or raw vendor identifier is representable here.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectionError {
    /// A finite limit is zero or internally inconsistent.
    InvalidLimits,
    /// The verified unit set carries no occurrence.
    EmptyUnitSet,
    /// The verified unit set exceeds the finite plan budget.
    UnitSetTooLarge,
    /// A verified unit binds another representation than the membership scope.
    RepresentationMismatch,
    /// More than one source/projection pair was supplied for one plan.
    MembershipArrayForbidden,
    /// The projection profile set disagrees with the membership scope.
    ProfileSetMismatch,
    /// A vector name is absent from, duplicated in or foreign to the schema.
    VectorNameMismatch,
    /// A vector representation disagrees with the accepted schema requirement.
    VectorShapeMismatch,
    /// A vector value is empty, non-finite or out of declared order.
    InvalidVector,
    /// One point carries more stored values than the accepted ceiling.
    StoredVectorValuesExceeded,
    /// A required named vector is absent for one unit.
    MissingVector,
    /// One unit identity appears more than once in the verified set.
    DuplicateUnit,
    /// A verified unit has no admitted planned unit in this plan.
    MissingUnitReceipt,
    /// Two derived points resolve to the same compact address.
    DuplicatePointId,
    /// Point identity derivation refused the exact key.
    PointIdentity,
    /// Canonical manifest encoding overflowed a finite ceiling.
    ManifestTooLarge,
    /// Manifest input is not canonically ordered or contains duplicates.
    InvalidManifest,
    /// A frozen manifest does not encode exactly its entries.
    ManifestMismatch,
    /// A readback index is absent from the qualified collection schema.
    PayloadIndexMissing,
    /// A readback payload index carries the wrong immutable kind.
    PayloadIndexTypeMismatch,
    /// A required named vector is absent from the qualified collection schema.
    CollectionVectorMissing,
    /// A named vector carries the wrong immutable requirement.
    CollectionVectorMismatch,
    /// The qualified schema carries an unexpected extra payload index.
    CollectionSchemaMismatch,
    /// A scope coordinate the unit set must bind is absent or mismatched.
    ScopeMismatch,
}

impl ProjectionError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidLimits => "PROJECTION_INVALID_LIMITS",
            Self::EmptyUnitSet => "PROJECTION_UNIT_SET_EMPTY",
            Self::UnitSetTooLarge => "PROJECTION_UNIT_SET_TOO_LARGE",
            Self::RepresentationMismatch => "PROJECTION_REPRESENTATION_MISMATCH",
            Self::MembershipArrayForbidden => "PROJECTION_MEMBERSHIP_ARRAY_FORBIDDEN",
            Self::ProfileSetMismatch => "PROJECTION_PROFILE_SET_MISMATCH",
            Self::VectorNameMismatch => "PROJECTION_VECTOR_NAME_MISMATCH",
            Self::VectorShapeMismatch => "PROJECTION_VECTOR_SHAPE_MISMATCH",
            Self::InvalidVector => "PROJECTION_VECTOR_INVALID",
            Self::StoredVectorValuesExceeded => "PROJECTION_STORED_VECTOR_VALUES_EXCEEDED",
            Self::MissingVector => "PROJECTION_MISSING_VECTOR",
            Self::DuplicateUnit => "PROJECTION_DUPLICATE_UNIT",
            Self::MissingUnitReceipt => "PROJECTION_MISSING_UNIT_RECEIPT",
            Self::DuplicatePointId => "PROJECTION_DUPLICATE_POINT_ID",
            Self::PointIdentity => "PROJECTION_POINT_IDENTITY_INVALID",
            Self::ManifestTooLarge => "PROJECTION_MANIFEST_TOO_LARGE",
            Self::InvalidManifest => "PROJECTION_INVALID_MANIFEST",
            Self::ManifestMismatch => "PROJECTION_MANIFEST_MISMATCH",
            Self::PayloadIndexMissing => "PROJECTION_PAYLOAD_INDEX_MISSING",
            Self::PayloadIndexTypeMismatch => "PROJECTION_PAYLOAD_INDEX_TYPE_MISMATCH",
            Self::CollectionVectorMissing => "PROJECTION_COLLECTION_VECTOR_MISSING",
            Self::CollectionVectorMismatch => "PROJECTION_COLLECTION_VECTOR_MISMATCH",
            Self::CollectionSchemaMismatch => "PROJECTION_COLLECTION_SCHEMA_MISMATCH",
            Self::ScopeMismatch => "PROJECTION_SCOPE_MISMATCH",
        }
    }
}

impl fmt::Display for ProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for ProjectionError {}

impl From<PointIdError> for ProjectionError {
    fn from(_: PointIdError) -> Self {
        Self::PointIdentity
    }
}

/// Maps one closed shared-contract refusal onto planning.
impl From<ContractError> for ProjectionError {
    fn from(_: ContractError) -> Self {
        Self::InvalidManifest
    }
}

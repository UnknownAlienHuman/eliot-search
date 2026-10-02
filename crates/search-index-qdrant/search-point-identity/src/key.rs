use search_contracts::{
    CollectionGenerationId, InstallationIncarnationId, ProjectionMembershipId,
    ProjectionProfileSetId, RepresentationId, UnitId,
};

use crate::PointIdentityError;

/// Exact admitted S11.1 key schema version.
pub const POINT_IDENTITY_SCHEMA_VERSION: u16 = 1;

/// Conservative finite point-identity limits.
pub const DEFAULT_POINT_IDENTITY_LIMITS: PointIdentityLimits = PointIdentityLimits {
    max_identifier_bytes: 4_096,
    max_canonical_bytes: 32_768,
};

/// Finite pure point-identity limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PointIdentityLimits {
    /// Maximum UTF-8 bytes in the projection-profile-set identifier.
    pub max_identifier_bytes: usize,
    /// Maximum deterministic CBOR bytes for one key.
    pub max_canonical_bytes: usize,
}

impl PointIdentityLimits {
    /// Validates every finite dimension as non-zero.
    pub const fn validate(self) -> Result<Self, PointIdentityError> {
        if self.max_identifier_bytes == 0 || self.max_canonical_bytes == 0 {
            Err(PointIdentityError::InvalidLimits)
        } else {
            Ok(self)
        }
    }
}

/// Closed point role from the S11.1 canonical key.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PointRole {
    /// Ordinary representation unit point.
    Unit,
    /// Relation point whose identity is still bound to one unit occurrence.
    Relation,
    /// Auxiliary point required by the immutable projection profile set.
    Auxiliary,
}

impl PointRole {
    /// Canonical S11.1 wire spelling.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Unit => "unit",
            Self::Relation => "relation",
            Self::Auxiliary => "auxiliary",
        }
    }
}

/// Exact immutable S11.1 `ProjectionPointKey`.
///
/// Source membership, source byte ranges, access/scoring state, vector names,
/// route names, insertion order and wall time are deliberately absent. Their
/// lifecycle is owned by other contracts; changing any identity-bearing
/// coordinate below creates another point identity.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProjectionPointKey {
    /// Exact key schema version; only version 1 is admitted.
    pub schema_version: u16,
    /// Current installation incarnation.
    pub installation_incarnation_id: InstallationIncarnationId,
    /// Immutable physical collection generation.
    pub collection_generation_id: CollectionGenerationId,
    /// Exactly one projection membership.
    pub projection_membership_id: ProjectionMembershipId,
    /// Exact canonical representation.
    pub representation_id: RepresentationId,
    /// Exact unit occurrence in that representation.
    pub unit_id: UnitId,
    /// Immutable required named-vector/profile set.
    pub projection_profile_set_id: ProjectionProfileSetId,
    /// Unit, relation or auxiliary point role.
    pub point_role: PointRole,
}

impl ProjectionPointKey {
    /// Validates the exact schema version and finite string boundary.
    pub fn validate(&self, limits: PointIdentityLimits) -> Result<(), PointIdentityError> {
        let limits = limits.validate()?;
        if self.schema_version != POINT_IDENTITY_SCHEMA_VERSION {
            return Err(PointIdentityError::PointKeyVersionUnsupported);
        }
        if self.projection_profile_set_id.as_str().len() > limits.max_identifier_bytes {
            return Err(PointIdentityError::IdentifierTooLong);
        }
        Ok(())
    }
}

/// Compatibility spelling for downstream packages consuming the point key.
pub type PointIdentityKey = ProjectionPointKey;

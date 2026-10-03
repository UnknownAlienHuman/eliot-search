use search_contracts::{
    Blake3Digest32, CollectionGenerationId, InstallationIncarnationId,
    ProjectionMembershipId, ProjectionProfileSetId, RepresentationId, UnitId,
};

use crate::{
    POINT_IDENTITY_SCHEMA_VERSION, PointId128, PointIdentity,
    PointIdentityError,
};

/// Exact independently represented S9.5 identity payload fields.
///
/// `point_role` is protected by the full digest but is not a separate S9.5
/// payload field. Every field that is represented separately in S9.5 is
/// compared in addition to the full digest before an existing UUID is accepted.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PointIdentityPayload {
    /// Qdrant point address being occupied.
    pub point_id: PointId128,
    /// Full S11.2 identity digest stored in payload and manifest.
    pub point_identity_digest_256: Blake3Digest32,
    /// Current installation incarnation.
    pub installation_incarnation_id: InstallationIncarnationId,
    /// Immutable collection generation.
    pub collection_generation_id: CollectionGenerationId,
    /// Exactly one projection membership.
    pub projection_membership_id: ProjectionMembershipId,
    /// Exact canonical representation.
    pub representation_id: RepresentationId,
    /// Exact unit occurrence.
    pub unit_id: UnitId,
    /// Immutable projection profile set.
    pub projection_profile_set_id: ProjectionProfileSetId,
}

impl PointIdentityPayload {
    /// Materializes the exact payload guard for a derived identity.
    #[must_use]
    pub fn from_identity(identity: &PointIdentity) -> Self {
        Self {
            point_id: identity.point_id,
            point_identity_digest_256: identity.full_digest.as_contract_digest(),
            installation_incarnation_id: identity.key.installation_incarnation_id,
            collection_generation_id: identity.key.collection_generation_id,
            projection_membership_id: identity.key.projection_membership_id,
            representation_id: identity.key.representation_id,
            unit_id: identity.key.unit_id,
            projection_profile_set_id: identity.key.projection_profile_set_id.clone(),
        }
    }
}

/// Logical name used when validating a point already present in Qdrant.
pub type ExistingPointIdentity = PointIdentityPayload;

/// Non-destructive decision for an existing projected UUID.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CollisionDecision {
    /// No point occupies the projected UUID.
    Vacant,
    /// Full digest and every independently represented identity field match.
    SameFullIdentity,
    /// Any mismatch blocks overwrite.
    CollisionBlock,
}

/// Validates the full digest and every independently represented identity field.
///
/// A payload for another UUID is an identity-routing mismatch. Once the UUID
/// matches, any full-digest or canonical field mismatch is the S11.2
/// `POINT_ID_COLLISION` condition and must block overwrite.
pub fn validate_identity_payload(
    expected: &PointIdentity,
    observed: &PointIdentityPayload,
) -> Result<(), PointIdentityError> {
    if expected.key.schema_version != POINT_IDENTITY_SCHEMA_VERSION {
        return Err(PointIdentityError::PointKeyVersionUnsupported);
    }
    if observed.point_id != expected.point_id {
        return Err(PointIdentityError::IdentityMismatch);
    }
    if observed.point_identity_digest_256
        != expected.full_digest.as_contract_digest()
        || observed.installation_incarnation_id
            != expected.key.installation_incarnation_id
        || observed.collection_generation_id
            != expected.key.collection_generation_id
        || observed.projection_membership_id
            != expected.key.projection_membership_id
        || observed.representation_id != expected.key.representation_id
        || observed.unit_id != expected.key.unit_id
        || observed.projection_profile_set_id
            != expected.key.projection_profile_set_id
    {
        return Err(PointIdentityError::DigestCollision);
    }
    Ok(())
}

/// Agent-contract spelling for [`validate_identity_payload`].
pub fn validate_identity_fields(
    expected: &PointIdentity,
    observed: &PointIdentityPayload,
) -> Result<(), PointIdentityError> {
    validate_identity_payload(expected, observed)
}

/// Compares an expected identity against an optional existing payload.
///
/// This function owns no registry or mutation state. Durable collision refusal
/// is performed by the Qdrant bridge after exact point readback; preparation
/// owners perform their own bounded set uniqueness checks before publication.
#[must_use]
pub fn compare_existing_identity(
    expected: &PointIdentity,
    observed: Option<&PointIdentityPayload>,
) -> CollisionDecision {
    let Some(observed) = observed else {
        return CollisionDecision::Vacant;
    };
    if validate_identity_payload(expected, observed).is_ok() {
        CollisionDecision::SameFullIdentity
    } else {
        CollisionDecision::CollisionBlock
    }
}

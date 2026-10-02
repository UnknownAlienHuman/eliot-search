//! Vendor-neutral eligibility, nominations and shared query validation.

mod oracle;
mod ranking;

use std::collections::BTreeSet;

use search_contracts::{
    AccessPartitionId, Blake3Digest32, CollectionGenerationId, Epoch,
    InstallationIncarnationId, ProjectionMembershipId, ProjectionProfileSetId,
    ScoringPartitionId,
};

use crate::{BridgeError, CollectionRoute, CollectionSchema, PointPayload, QdrantPointId};

/// Closed S10.3 base eligibility filter.
///
/// Access/scoring policy changes mint new immutable partition identities under
/// S8.2. Restrictive deny/shadow/purge/abandoned fences are compiled into the
/// currently allowed projection-membership set before this value reaches the
/// bridge. The same exact value is used for retrieval, `idf.corpus`, count,
/// scroll/facet/grouping adapters and response validation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EligibilityFilter {
    pub installation_incarnation_id: InstallationIncarnationId,
    pub collection_generation_id: CollectionGenerationId,
    pub allowed_projection_memberships: BTreeSet<ProjectionMembershipId>,
    pub access_partition_id: AccessPartitionId,
    pub scoring_partition_id: ScoringPartitionId,
    pub projection_profile_set_id: ProjectionProfileSetId,
    pub visible_epoch: Epoch,
}

impl EligibilityFilter {
    /// Exact payload indexes used by the mandatory base filter.
    pub const INDEXED_FIELDS: [&'static str; 8] = [
        PointPayload::INSTALLATION_INCARNATION_FIELD,
        PointPayload::COLLECTION_GENERATION_FIELD,
        PointPayload::PROJECTION_MEMBERSHIP_FIELD,
        PointPayload::ACCESS_PARTITION_FIELD,
        PointPayload::SCORING_PARTITION_FIELD,
        PointPayload::PROJECTION_PROFILE_SET_FIELD,
        PointPayload::VALID_FROM_FIELD,
        PointPayload::VALID_UNTIL_FIELD,
    ];

    pub(crate) fn matches(&self, payload: &PointPayload) -> bool {
        payload.installation_incarnation_id == self.installation_incarnation_id
            && payload.collection_generation_id == self.collection_generation_id
            && self
                .allowed_projection_memberships
                .contains(&payload.projection_membership_id)
            && payload.access_partition_id == self.access_partition_id
            && payload.scoring_partition_id == self.scoring_partition_id
            && payload.projection_profile_set_id == self.projection_profile_set_id
            && payload.valid_from_epoch <= self.visible_epoch
            && payload
                .valid_until_epoch_exclusive
                .is_none_or(|until| self.visible_epoch < until)
    }
}

/// One bounded nomination returned by filtered retrieval.
///
/// This is an untrusted hint. The full typed point payload and vectors must be
/// retrieved through exact readback before source/control validation.
#[derive(Clone, Debug, PartialEq)]
pub struct CandidateNomination {
    pub point_id: QdrantPointId,
    pub score: f32,
    pub point_identity_digest_256: Blake3Digest32,
}

pub(crate) fn validate_filter(filter: &EligibilityFilter) -> Result<(), BridgeError> {
    if filter.allowed_projection_memberships.is_empty() {
        return Err(BridgeError::InvalidFilter);
    }
    Ok(())
}

pub(crate) fn validate_filter_for_route(
    filter: &EligibilityFilter,
    route: &CollectionRoute,
) -> Result<(), BridgeError> {
    validate_filter(filter)?;
    if filter.collection_generation_id != route.generation {
        return Err(BridgeError::InvalidFilter);
    }
    Ok(())
}

pub(crate) fn ensure_filter_indexes(schema: &CollectionSchema) -> Result<(), BridgeError> {
    for field in EligibilityFilter::INDEXED_FIELDS {
        if !schema.indexed_payload_fields.contains(field) {
            return Err(BridgeError::UnindexedFilter);
        }
    }
    Ok(())
}

pub(crate) fn validate_query_vector(
    query: &[(u32, f32)],
    dimensions: u32,
) -> Result<(), BridgeError> {
    if query.is_empty()
        || query.iter().any(|(_, value)| !value.is_finite())
        || query.windows(2).any(|pair| pair[0].0 >= pair[1].0)
        || query
            .last()
            .is_some_and(|(index, _)| *index >= dimensions)
    {
        return Err(BridgeError::VectorDimensionMismatch);
    }
    Ok(())
}

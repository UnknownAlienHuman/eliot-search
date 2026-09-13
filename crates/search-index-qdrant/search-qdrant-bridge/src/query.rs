//! Vendor-neutral eligibility, nominations and shared query validation.

mod oracle;
mod ranking;

use std::collections::BTreeSet;

use search_contracts::{Blake3Digest32, Epoch, OpaqueId};

use crate::{
    BridgeError, CollectionSchema, PointPayload, QdrantPointId,
};

/// Closed indexed eligibility filter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EligibilityFilter {
    pub access_partition_digest: Blake3Digest32,
    pub allowed_source_memberships: BTreeSet<OpaqueId>,
    pub visible_epoch: Epoch,
}

impl EligibilityFilter {
    pub const INDEXED_FIELDS: [&'static str; 4] = [
        "access_partition_digest",
        "source_membership_id",
        "valid_from_epoch",
        "valid_until_epoch_exclusive",
    ];

    pub(crate) fn matches(&self, payload: &PointPayload) -> bool {
        payload.access_partition_digest == self.access_partition_digest
            && self
                .allowed_source_memberships
                .contains(&payload.source_membership_id)
            && payload.valid_from_epoch <= self.visible_epoch
            && payload
                .valid_until_epoch_exclusive
                .is_none_or(|until| self.visible_epoch < until)
    }
}

/// One bounded nomination returned by filtered retrieval.
#[derive(Clone, Debug, PartialEq)]
pub struct CandidateNomination {
    pub point_id: QdrantPointId,
    pub score: f32,
    pub payload_digest: Blake3Digest32,
    pub identity_digest: Blake3Digest32,
}

pub(crate) fn validate_filter(filter: &EligibilityFilter) -> Result<(), BridgeError> {
    if filter.allowed_source_memberships.is_empty() {
        return Err(BridgeError::InvalidFilter);
    }
    Ok(())
}

pub(crate) fn ensure_filter_indexes(
    schema: &CollectionSchema,
) -> Result<(), BridgeError> {
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

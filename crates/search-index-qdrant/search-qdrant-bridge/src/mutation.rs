//! Vendor-neutral mutation records and pure validation shared with the live adapter.

mod oracle;

use std::collections::BTreeMap;

use search_contracts::{Blake3Digest32, Epoch, OpaqueId};

use crate::{
    BridgeError, BridgeLimits, CollectionRoute, CollectionSchema, VectorSchema,
};

/// Provider-neutral exact 128-bit point ID.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct QdrantPointId(pub [u8; 16]);

/// Minimal filterable payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PointPayload {
    pub source_membership_id: OpaqueId,
    pub projection_membership_id: OpaqueId,
    pub access_partition_digest: Blake3Digest32,
    pub source_revision: u64,
    pub unit_ordinal: u64,
    pub valid_from_epoch: Epoch,
    pub valid_until_epoch_exclusive: Option<Epoch>,
    pub payload_digest: Blake3Digest32,
    pub identity_digest: Blake3Digest32,
}

/// Exact named vector values and digest.
#[derive(Clone, Debug, PartialEq)]
pub struct StoredVector {
    pub dimensions: u32,
    pub sparse: bool,
    pub values: Vec<(u32, f32)>,
    pub digest: Blake3Digest32,
}

impl StoredVector {
    pub(crate) fn validate(&self, schema: VectorSchema) -> Result<(), BridgeError> {
        if self.dimensions != schema.dimensions || self.sparse != schema.sparse {
            return Err(BridgeError::VectorDimensionMismatch);
        }
        if self.values.is_empty()
            || self.values.iter().any(|(_, value)| !value.is_finite())
            || self.values.windows(2).any(|pair| pair[0].0 >= pair[1].0)
            || self
                .values
                .last()
                .is_some_and(|(index, _)| *index >= self.dimensions)
        {
            return Err(BridgeError::VectorDimensionMismatch);
        }
        Ok(())
    }
}

/// Exact point record.
#[derive(Clone, Debug, PartialEq)]
pub struct PointRecord {
    pub point_id: QdrantPointId,
    pub payload: PointPayload,
    pub vectors: BTreeMap<String, StoredVector>,
}

/// Immutable exact mutation identity.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct BridgeMutation {
    pub operation_id: OpaqueId,
    pub canonical_input_digest: Blake3Digest32,
}

/// Exact mutation receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MutationReceipt {
    pub operation_id: OpaqueId,
    pub canonical_input_digest: Blake3Digest32,
    pub route: CollectionRoute,
    pub affected_ids: Vec<QdrantPointId>,
    pub replayed: bool,
}

pub(crate) fn validate_point(
    point: &PointRecord,
    schema: &CollectionSchema,
    limits: BridgeLimits,
) -> Result<(), BridgeError> {
    if point.vectors.len() != schema.named_vectors.len() {
        return Err(BridgeError::NamedVectorMissing);
    }
    let mut stored_values = 0_usize;
    for (name, vector_schema) in &schema.named_vectors {
        let vector = point
            .vectors
            .get(name)
            .ok_or(BridgeError::NamedVectorMissing)?;
        vector.validate(*vector_schema)?;
        stored_values = stored_values
            .checked_add(vector.values.len())
            .ok_or(BridgeError::MutationTooLarge)?;
    }
    if stored_values > limits.max_vector_values_per_point {
        return Err(BridgeError::MutationTooLarge);
    }
    Ok(())
}

pub(crate) fn validate_exact_ids(
    mut ids: Vec<QdrantPointId>,
    limit: usize,
) -> Result<Vec<QdrantPointId>, BridgeError> {
    if ids.is_empty() || ids.len() > limit {
        return Err(BridgeError::MutationTooLarge);
    }
    ids.sort();
    if ids.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(BridgeError::DuplicatePointId);
    }
    Ok(ids)
}

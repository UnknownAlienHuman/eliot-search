use std::collections::{BTreeMap, BTreeSet};

use search_contracts::{Blake3Digest32, Epoch, OpaqueId};

use crate::{
    BridgeError, BridgeLimits, CollectionRoute, CollectionSchema, QdrantBridge,
    VectorSchema,
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

impl QdrantBridge {
    /// Upserts only explicit point IDs with exact idempotency.
    pub fn upsert_exact(
        &mut self,
        route: &CollectionRoute,
        points: Vec<PointRecord>,
        mutation: BridgeMutation,
    ) -> Result<MutationReceipt, BridgeError> {
        if let Some(replay) = self.replay(&mutation)? {
            return Ok(replay);
        }
        if points.is_empty() || points.len() > self.limits.max_points_per_mutation {
            return Err(BridgeError::MutationTooLarge);
        }
        let collection = self
            .collections
            .get_mut(route)
            .ok_or(BridgeError::CollectionNotFound)?;
        let mut seen = BTreeSet::new();
        for point in &points {
            if !seen.insert(point.point_id) {
                return Err(BridgeError::DuplicatePointId);
            }
            validate_point(point, &collection.schema, self.limits)?;
        }
        let mut affected_ids = Vec::with_capacity(points.len());
        for point in points {
            affected_ids.push(point.point_id);
            collection.points.insert(point.point_id, point);
        }
        affected_ids.sort();
        self.record_mutation(route.clone(), mutation, affected_ids)
    }

    /// Sets the exact exclusive upper epoch on explicit point IDs.
    ///
    /// # Panics
    ///
    /// Never panics on valid input: the first loop returns `PointNotFound`
    /// before mutation, so the validated point exists under `&mut self` in the
    /// second loop and the `expect` is an unreachable invariant.
    pub fn close_exact(
        &mut self,
        route: &CollectionRoute,
        ids: Vec<QdrantPointId>,
        valid_until_epoch_exclusive: Epoch,
        mutation: BridgeMutation,
    ) -> Result<MutationReceipt, BridgeError> {
        if let Some(replay) = self.replay(&mutation)? {
            return Ok(replay);
        }
        let ids = validate_exact_ids(ids, self.limits.max_points_per_mutation)?;
        let collection = self
            .collections
            .get_mut(route)
            .ok_or(BridgeError::CollectionNotFound)?;
        for id in &ids {
            let point = collection.points.get(id).ok_or(BridgeError::PointNotFound)?;
            if valid_until_epoch_exclusive <= point.payload.valid_from_epoch {
                return Err(BridgeError::ExactReadbackMismatch);
            }
        }
        for id in &ids {
            collection
                .points
                .get_mut(id)
                .expect("validated point exists")
                .payload
                .valid_until_epoch_exclusive = Some(valid_until_epoch_exclusive);
        }
        self.record_mutation(route.clone(), mutation, ids)
    }

    /// Deletes only explicit exact point IDs.
    pub fn delete_exact(
        &mut self,
        route: &CollectionRoute,
        ids: Vec<QdrantPointId>,
        mutation: BridgeMutation,
    ) -> Result<MutationReceipt, BridgeError> {
        if let Some(replay) = self.replay(&mutation)? {
            return Ok(replay);
        }
        let ids = validate_exact_ids(ids, self.limits.max_points_per_mutation)?;
        let collection = self
            .collections
            .get_mut(route)
            .ok_or(BridgeError::CollectionNotFound)?;
        for id in &ids {
            collection.points.remove(id);
        }
        self.record_mutation(route.clone(), mutation, ids)
    }

    fn replay(
        &self,
        mutation: &BridgeMutation,
    ) -> Result<Option<MutationReceipt>, BridgeError> {
        let Some(existing) = self.operations.get(&mutation.operation_id) else {
            return Ok(None);
        };
        if existing.canonical_input_digest != mutation.canonical_input_digest {
            return Err(BridgeError::OperationConflict);
        }
        let mut replay = existing.clone();
        replay.replayed = true;
        Ok(Some(replay))
    }

    fn record_mutation(
        &mut self,
        route: CollectionRoute,
        mutation: BridgeMutation,
        affected_ids: Vec<QdrantPointId>,
    ) -> Result<MutationReceipt, BridgeError> {
        if self.operations.len() >= self.limits.max_operation_receipts {
            return Err(BridgeError::MutationTooLarge);
        }
        let receipt = MutationReceipt {
            operation_id: mutation.operation_id.clone(),
            canonical_input_digest: mutation.canonical_input_digest,
            route,
            affected_ids,
            replayed: false,
        };
        self.operations
            .insert(mutation.operation_id, receipt.clone());
        Ok(receipt)
    }
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

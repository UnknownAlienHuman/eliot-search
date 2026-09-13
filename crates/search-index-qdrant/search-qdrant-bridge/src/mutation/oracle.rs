//! In-memory exact mutations; production writes remain in `real`.

use std::collections::BTreeSet;

use search_contracts::Epoch;

use super::{
    BridgeMutation, MutationReceipt, PointRecord, QdrantPointId, validate_exact_ids,
    validate_point,
};
use crate::{BridgeError, CollectionRoute, QdrantBridge};

impl QdrantBridge {
    /// Upserts only explicit point IDs with exact idempotency.
    pub fn upsert_exact(
        &mut self,
        route: &CollectionRoute,
        points: Vec<PointRecord>,
        mutation: BridgeMutation,
    ) -> Result<MutationReceipt, BridgeError> {
        if let Some(replay) = self.admit_mutation(&mutation)? {
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
        Ok(self.record_mutation(route.clone(), mutation, affected_ids))
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
        if let Some(replay) = self.admit_mutation(&mutation)? {
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
        Ok(self.record_mutation(route.clone(), mutation, ids))
    }

    /// Deletes only explicit exact point IDs.
    pub fn delete_exact(
        &mut self,
        route: &CollectionRoute,
        ids: Vec<QdrantPointId>,
        mutation: BridgeMutation,
    ) -> Result<MutationReceipt, BridgeError> {
        if let Some(replay) = self.admit_mutation(&mutation)? {
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
        Ok(self.record_mutation(route.clone(), mutation, ids))
    }

    // Replay does not consume capacity. For a new operation the journal check
    // must precede all point mutations. The synchronous &mut self operation
    // retains exclusive ownership until its receipt is recorded.
    fn admit_mutation(
        &self,
        mutation: &BridgeMutation,
    ) -> Result<Option<MutationReceipt>, BridgeError> {
        if let Some(existing) = self.operations.get(&mutation.operation_id) {
            if existing.canonical_input_digest != mutation.canonical_input_digest {
                return Err(BridgeError::OperationConflict);
            }
            let mut replay = existing.clone();
            replay.replayed = true;
            return Ok(Some(replay));
        }
        if self.operations.len() >= self.limits.max_operation_receipts {
            return Err(BridgeError::MutationTooLarge);
        }
        Ok(None)
    }

    fn record_mutation(
        &mut self,
        route: CollectionRoute,
        mutation: BridgeMutation,
        affected_ids: Vec<QdrantPointId>,
    ) -> MutationReceipt {
        // Capacity was admitted before changing any points. No fallible step
        // may follow the first state change in this in-memory oracle.
        let receipt = MutationReceipt {
            operation_id: mutation.operation_id.clone(),
            canonical_input_digest: mutation.canonical_input_digest,
            route,
            affected_ids,
            replayed: false,
        };
        self.operations
            .insert(mutation.operation_id, receipt.clone());
        receipt
    }
}

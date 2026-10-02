use std::collections::{BTreeMap, BTreeSet};

use super::super::{PointStruct, UpsertPoints};

use super::check_acknowledgement;
use super::super::{
    BridgeError, BridgeMutation, CollectionRoute, MutationReceipt, OpContext,
    OperationBudget, PointRecord, QdrantPointId, RealDataPlane, collection_name,
    encode_payload, encode_vectors, map_mutation_error, same_point_identity,
    strong_ordering, update_completed, validate_point,
};

impl RealDataPlane {
    /// Upserts only explicit point IDs with collision refusal, `wait=true`,
    /// strong ordering and exact readback before success. Same mutation identity
    /// plus same canonical batch replays without a second write; same identity
    /// plus different input is [`BridgeError::OperationConflict`].
    pub async fn upsert_exact(
        &mut self,
        route: &CollectionRoute,
        points: Vec<PointRecord>,
        mutation: BridgeMutation,
        context: &OpContext,
    ) -> Result<MutationReceipt, BridgeError> {
        let budget = OperationBudget::begin(context)?;
        if let Some(replay) = self.replay(&mutation)? {
            return Ok(replay);
        }
        if self.operations.len() >= self.limits.max_operation_receipts {
            return Err(BridgeError::MutationTooLarge);
        }
        if points.is_empty() || points.len() > self.limits.max_points_per_mutation {
            return Err(BridgeError::MutationTooLarge);
        }
        let name = collection_name(route)?;
        let schema = self
            .schemas
            .get(&name)
            .ok_or(BridgeError::CollectionNotFound)?
            .clone();
        let mut seen = BTreeSet::new();
        for point in &points {
            if !seen.insert(point.point_id) {
                return Err(BridgeError::DuplicatePointId);
            }
            if point.payload.collection_generation_id != route.generation {
                return Err(BridgeError::PointPayloadInvalid);
            }
            validate_point(point, &schema, self.limits)?;
        }
        let mut vendor_points = Vec::with_capacity(points.len());
        for point in &points {
            vendor_points.push(PointStruct {
                id: Some(super::super::vendor_point_id(&point.point_id)),
                payload: encode_payload(point)?,
                vectors: Some(encode_vectors(point)),
            });
        }

        // S11.2 collision guard: before any upsert that may address an
        // existing UUID, retrieve it and compare the full identity digest plus
        // every canonical identity coordinate represented by this bridge.
        // The publication owner serializes mutation dispatch; this adapter
        // never treats a mismatched pre-existing point as overwriteable.
        let expected_by_id: BTreeMap<QdrantPointId, usize> = points
            .iter()
            .enumerate()
            .map(|(index, point)| (point.point_id, index))
            .collect();
        let ids: Vec<QdrantPointId> = expected_by_id.keys().copied().collect();
        let existing = self
            .fetch_points(
                &name,
                &ids,
                &schema,
                route.generation,
                budget.remaining(context)?,
            )
            .await?;
        for (id, existing_point) in &existing {
            let expected_index = expected_by_id
                .get(id)
                .ok_or(BridgeError::UnexpectedPoint)?;
            if !same_point_identity(existing_point, &points[*expected_index]) {
                return Err(BridgeError::PointIdCollision);
            }
        }

        let acked = tokio::time::timeout(
            budget.remaining(context)?,
            self.client.upsert_points(UpsertPoints {
                collection_name: name.clone(),
                wait: Some(true),
                ordering: Some(strong_ordering()),
                points: vendor_points,
                ..Default::default()
            }),
        )
        .await
        .map_err(|_| BridgeError::MutationOutcomeUnknown)?
        .map_err(map_mutation_error)?;
        check_acknowledgement(
            acked.result.as_ref().is_some_and(|result| update_completed(result.status)),
            context,
        )?;
        let mut affected: Vec<QdrantPointId> = points.iter().map(|point| point.point_id).collect();
        affected.sort();
        self.verify_upsert_readback(
            &name,
            &points,
            &schema,
            budget.remaining_after_dispatch(context)?,
        )
        .await?;
        self.record_mutation(route.clone(), mutation, affected)
    }
}

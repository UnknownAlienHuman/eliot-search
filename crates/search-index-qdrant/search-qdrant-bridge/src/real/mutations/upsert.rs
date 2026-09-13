use std::collections::BTreeSet;

use super::super::{PointStruct, UpsertPoints};

use super::check_acknowledgement;
use super::super::{
    BridgeError, BridgeMutation, CollectionRoute, MutationReceipt, OpContext,
    PointRecord, QdrantPointId, RealDataPlane, collection_name, encode_payload,
    encode_vectors, map_mutation_error, strong_ordering, update_completed, validate_point,
};

impl RealDataPlane {
    /// Upserts only explicit point IDs with `wait=true`, strong ordering and
    /// exact readback before success. Same identity plus same canonical batch
    /// replays without a second write; same identity plus different input is
    /// [`BridgeError::OperationConflict`].
    pub async fn upsert_exact(
        &mut self,
        route: &CollectionRoute,
        points: Vec<PointRecord>,
        mutation: BridgeMutation,
        context: &OpContext,
    ) -> Result<MutationReceipt, BridgeError> {
        context.check()?;
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
        // Validation/encoding may take time; cancellation still means no write here.
        context.check()?;
        let acked = tokio::time::timeout(
            context.deadline(),
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
        self.verify_upsert_readback(&name, &points, &schema, context).await?;
        self.record_mutation(route.clone(), mutation, affected)
    }
}

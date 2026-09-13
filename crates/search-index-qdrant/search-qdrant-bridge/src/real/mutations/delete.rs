use super::super::{DeletePoints, GetPoints, PointsIdsList, PointsSelector, points_selector};

use super::check_acknowledgement;
use super::super::{
    BridgeError, BridgeMutation, CollectionRoute, MutationReceipt, OpContext,
    QdrantPointId, RealDataPlane, collection_name, map_mutation_error,
    strong_ordering, update_completed, validate_exact_ids, vendor_point_id,
};

impl RealDataPlane {
    /// Deletes only explicit exact point IDs with `wait=true` and strong
    /// ordering, then proves absence through exact readback.
    pub async fn delete_exact(
        &mut self,
        route: &CollectionRoute,
        ids: Vec<QdrantPointId>,
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
        let ids = validate_exact_ids(ids, self.limits.max_points_per_mutation)?;
        let name = collection_name(route)?;
        context.check()?;
        let acked = tokio::time::timeout(
            context.deadline(),
            self.client.delete_points(DeletePoints {
                collection_name: name.clone(),
                wait: Some(true),
                points: Some(PointsSelector {
                    points_selector_one_of: Some(points_selector::PointsSelectorOneOf::Points(
                        PointsIdsList {
                            ids: ids.iter().map(vendor_point_id).collect(),
                        },
                    )),
                }),
                ordering: Some(strong_ordering()),
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
        let present = tokio::time::timeout(
            context.deadline(),
            self.client.get_points(GetPoints {
                collection_name: name,
                ids: ids.iter().map(vendor_point_id).collect(),
                with_payload: Some(false.into()),
                with_vectors: Some(false.into()),
                ..Default::default()
            }),
        )
        .await
        .map_err(|_| BridgeError::MutationOutcomeUnknown)?
        .map_err(|_| BridgeError::MutationOutcomeUnknown)?;
        if !present.result.is_empty() {
            return Err(BridgeError::ExactReadbackMismatch);
        }
        self.record_mutation(route.clone(), mutation, ids)
    }
}

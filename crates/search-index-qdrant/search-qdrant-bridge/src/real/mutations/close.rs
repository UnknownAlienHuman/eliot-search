use std::collections::HashMap;

use super::super::{PointsIdsList, PointsSelector, SetPayloadPoints, points_selector};

use super::check_acknowledgement;
use super::super::{
    BridgeError, BridgeMutation, CollectionRoute, EligibilityFilter, MutationReceipt,
    OpContext, QdrantPointId, RealDataPlane, collection_name, int_value,
    map_mutation_error, strong_ordering, update_completed, validate_exact_ids, vendor_point_id,
};

impl RealDataPlane {
    /// Sets the exact exclusive upper epoch on explicit point IDs via
    /// payload-only update (no broad-filter closure exists). The current
    /// upper bound is read first, so a stale close fails pre-dispatch with
    /// [`BridgeError::ExactReadbackMismatch`] and a missing ID with
    /// [`BridgeError::PointNotFound`].
    pub async fn close_exact(
        &mut self,
        route: &CollectionRoute,
        ids: Vec<QdrantPointId>,
        valid_until_epoch_exclusive: search_contracts::Epoch,
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
        let schema = self
            .schemas
            .get(&name)
            .ok_or(BridgeError::CollectionNotFound)?
            .clone();
        let current = self
            .fetch_points(&name, &ids, &schema, context)
            .await
            .map_err(|error| match error {
                BridgeError::TransportFailed | BridgeError::CollectionNotFound => error,
                _ => BridgeError::ExactReadbackMismatch,
            })?;
        for id in &ids {
            let point = current.get(id).ok_or(BridgeError::PointNotFound)?;
            if valid_until_epoch_exclusive <= point.payload.valid_from_epoch {
                return Err(BridgeError::ExactReadbackMismatch);
            }
        }
        let mut payload = HashMap::new();
        payload.insert(
            EligibilityFilter::INDEXED_FIELDS[3].to_owned(),
            int_value(valid_until_epoch_exclusive.get()),
        );
        // A cancellation during preflight readback must not dispatch a mutation.
        context.check()?;
        let acked = tokio::time::timeout(
            context.deadline(),
            self.client.set_payload(SetPayloadPoints {
                collection_name: name.clone(),
                wait: Some(true),
                payload,
                points_selector: Some(PointsSelector {
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
        let after = self
            .fetch_points(&name, &ids, &schema, context)
            .await
            .map_err(|_| BridgeError::MutationOutcomeUnknown)?;
        for id in &ids {
            let point = after.get(id).ok_or(BridgeError::MutationOutcomeUnknown)?;
            if point.payload.valid_until_epoch_exclusive != Some(valid_until_epoch_exclusive) {
                return Err(BridgeError::ExactReadbackMismatch);
            }
        }
        self.record_mutation(route.clone(), mutation, ids)
    }
}

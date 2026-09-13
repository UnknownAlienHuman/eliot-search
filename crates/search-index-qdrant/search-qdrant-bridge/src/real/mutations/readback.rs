use std::collections::{BTreeMap, BTreeSet};

use super::super::GetPoints;

use super::super::{
    BridgeError, CollectionSchema, OpContext, PointRecord, QdrantPointId,
    RealDataPlane, bridge_point_id, decode_point, map_read_error, vendor_point_id,
};

/// One bounded, unique response set. Missing points remain explicit to the
/// caller; duplicate or unrequested points cannot satisfy an exact mutation.
fn index_points<T>(
    ids: &[QdrantPointId],
    decoded: impl IntoIterator<Item = Result<(QdrantPointId, T), BridgeError>>,
) -> Result<BTreeMap<QdrantPointId, T>, BridgeError> {
    let requested: BTreeSet<_> = ids.iter().copied().collect();
    let mut points = BTreeMap::new();
    for point in decoded {
        let (id, point) = point?;
        if !requested.contains(&id) || points.insert(id, point).is_some() {
            return Err(BridgeError::MalformedResponse);
        }
    }
    Ok(points)
}

impl RealDataPlane {
    pub(super) async fn verify_upsert_readback(
        &self,
        name: &str,
        expected: &[PointRecord],
        schema: &CollectionSchema,
        context: &OpContext,
    ) -> Result<(), BridgeError> {
        let ids: Vec<_> = expected.iter().map(|point| point.point_id).collect();
        let readback = tokio::time::timeout(
            context.deadline(),
            self.client.get_points(GetPoints {
                collection_name: name.to_owned(),
                ids: ids.iter().map(vendor_point_id).collect(),
                with_payload: Some(true.into()),
                with_vectors: Some(true.into()),
                ..Default::default()
            }),
        )
        .await
        .map_err(|_| BridgeError::MutationOutcomeUnknown)?
        .map_err(|_| BridgeError::MutationOutcomeUnknown)?;
        if readback.result.len() != expected.len() {
            return Err(BridgeError::ExactReadbackMismatch);
        }
        // Index references, not decoded vector copies. Each returned ID is parsed once.
        let points = index_points(&ids, readback.result.iter().map(|retrieved| {
            let id = bridge_point_id(
                retrieved.id.as_ref().ok_or(BridgeError::MalformedResponse)?,
            )?;
            Ok((id, retrieved))
        }))
        .map_err(|_| BridgeError::ExactReadbackMismatch)?;
        for point in expected {
            let found = points.get(&point.point_id).ok_or(BridgeError::ExactReadbackMismatch)?;
            let decoded = decode_point(
                found.id.as_ref().ok_or(BridgeError::ExactReadbackMismatch)?,
                &found.payload,
                found.vectors.as_ref(),
                schema,
            )
            .map_err(|_| BridgeError::ExactReadbackMismatch)?;
            if decoded != *point {
                return Err(BridgeError::ExactReadbackMismatch);
            }
        }
        Ok(())
    }

    pub(super) async fn fetch_points(
        &self,
        name: &str,
        ids: &[QdrantPointId],
        schema: &CollectionSchema,
        context: &OpContext,
    ) -> Result<BTreeMap<QdrantPointId, PointRecord>, BridgeError> {
        let readback = tokio::time::timeout(
            context.deadline(),
            self.client.get_points(GetPoints {
                collection_name: name.to_owned(),
                ids: ids.iter().map(vendor_point_id).collect(),
                with_payload: Some(true.into()),
                with_vectors: Some(true.into()),
                ..Default::default()
            }),
        )
        .await
        .map_err(|_| BridgeError::TransportFailed)?
        .map_err(map_read_error)?;
        if readback.result.len() > ids.len() {
            return Err(BridgeError::MalformedResponse);
        }
        index_points(ids, readback.result.iter().map(|retrieved| {
            let point = decode_point(
                retrieved.id.as_ref().ok_or(BridgeError::MalformedResponse)?,
                &retrieved.payload,
                retrieved.vectors.as_ref(),
                schema,
            )?;
            Ok((point.point_id, point))
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn point(id: u8) -> (QdrantPointId, u8) {
        (QdrantPointId([id; 16]), id)
    }

    #[test]
    fn unordered_response_is_indexed_and_missing_ids_stay_missing() {
        let ids = [QdrantPointId([1; 16]), QdrantPointId([2; 16]), QdrantPointId([3; 16])];
        let points = index_points(&ids, [Ok(point(3)), Ok(point(1))]).expect("index");
        assert_eq!(points.keys().copied().collect::<Vec<_>>(), vec![ids[0], ids[2]]);
        assert!(!points.contains_key(&ids[1]));
    }

    #[test]
    fn duplicate_response_cannot_hide_a_missing_requested_id() {
        let ids = [QdrantPointId([1; 16]), QdrantPointId([2; 16])];
        assert_eq!(
            index_points(&ids, [Ok(point(1)), Ok(point(1))]),
            Err(BridgeError::MalformedResponse)
        );
    }

    #[test]
    fn unrequested_response_cannot_satisfy_an_exact_read() {
        assert_eq!(
            index_points(&[QdrantPointId([1; 16])], [Ok(point(2))]),
            Err(BridgeError::MalformedResponse)
        );
    }

    #[test]
    fn decoding_failure_never_returns_a_partial_success() {
        assert_eq!(
            index_points(
                &[QdrantPointId([1; 16]), QdrantPointId([2; 16])],
                [Ok(point(1)), Err(BridgeError::MalformedResponse)]
            ),
            Err(BridgeError::MalformedResponse)
        );
    }
}

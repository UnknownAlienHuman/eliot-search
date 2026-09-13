//! Exact count/readback, missing-ID and exact-delete qualification probe.

use qdrant_client::qdrant::{
    CountPoints, DeletePoints, GetPoints, PointId, PointsIdsList, PointsSelector,
    point_id, points_selector, value,
};

use super::super::super::fixtures::{
    FIELD_FROM, FIELD_TENANT, QUALIFICATION_COLLECTION, TENANT_A, UUID_POINT,
    base_filter, num_point_id, snapshot_id, strong_ordering, update_completed,
};
use super::super::super::suite::Suite;
use super::super::super::LiveError;

pub(super) async fn probe_count_and_readback(
    suite: &mut Suite,
) -> Result<(), LiveError> {
    let count = suite
        .client
        .count(CountPoints {
            collection_name: QUALIFICATION_COLLECTION.to_owned(),
            filter: Some(base_filter()?),
            exact: Some(true),
            ..Default::default()
        })
        .await
        .map_err(|_| LiveError::TransportFailed)?;
    let readback = suite
        .client
        .get_points(GetPoints {
            collection_name: QUALIFICATION_COLLECTION.to_owned(),
            ids: vec![
                num_point_id(1),
                num_point_id(2),
                num_point_id(9),
                PointId {
                    point_id_options: Some(point_id::PointIdOptions::Uuid(
                        UUID_POINT.to_owned(),
                    )),
                },
            ],
            with_payload: Some(true.into()),
            with_vectors: Some(true.into()),
            ..Default::default()
        })
        .await
        .map_err(|_| LiveError::TransportFailed)?;
    let mut found: Vec<String> = readback
        .result
        .iter()
        .map(|point| snapshot_id(point.id.as_ref()))
        .collect();
    found.sort();
    let payload_ok = readback.result.iter().all(|point| {
        point.payload.get(FIELD_TENANT).is_some_and(|tenant| {
            tenant.kind == Some(value::Kind::StringValue(TENANT_A.to_owned()))
        }) && point.payload.get(FIELD_FROM).is_some_and(|from| {
            matches!(from.kind, Some(value::Kind::IntegerValue(_)))
        })
    });
    let unknown = suite
        .client
        .get_points(GetPoints {
            collection_name: QUALIFICATION_COLLECTION.to_owned(),
            ids: vec![num_point_id(777)],
            with_payload: Some(false.into()),
            with_vectors: Some(false.into()),
            ..Default::default()
        })
        .await
        .map_err(|_| LiveError::TransportFailed)?;
    let passed = count.result.as_ref().map(|result| result.count) == Some(4)
        && found
            == [
                "1",
                "2",
                "550e8400-e29b-41d4-a716-446655440000",
                "9",
            ]
        && payload_ok
        && unknown.result.is_empty();
    suite.record(
        "exact_count_and_readback",
        passed,
        format!(
            "count={:?} ids={found:?} unknown_empty={}",
            count.result.as_ref().map(|result| result.count),
            unknown.result.is_empty()
        ),
    );

    let deleted = suite
        .client
        .delete_points(DeletePoints {
            collection_name: QUALIFICATION_COLLECTION.to_owned(),
            wait: Some(true),
            points: Some(PointsSelector {
                points_selector_one_of: Some(
                    points_selector::PointsSelectorOneOf::Points(PointsIdsList {
                        ids: vec![num_point_id(2)],
                    }),
                ),
            }),
            ordering: Some(strong_ordering()),
            ..Default::default()
        })
        .await
        .map_err(|_| LiveError::TransportFailed)?;
    if !deleted
        .result
        .as_ref()
        .is_some_and(|result| update_completed(result.status))
    {
        return Err(LiveError::ProbeFailed {
            probe: "exact_count_and_readback",
        });
    }
    let recount = suite
        .client
        .count(CountPoints {
            collection_name: QUALIFICATION_COLLECTION.to_owned(),
            filter: Some(base_filter()?),
            exact: Some(true),
            ..Default::default()
        })
        .await
        .map_err(|_| LiveError::TransportFailed)?;
    suite.log.push(format!(
        "RECLAIM point=2 post_count={:?}",
        recount.result.as_ref().map(|result| result.count)
    ));
    if recount.result.as_ref().map(|result| result.count) != Some(3) {
        return Err(LiveError::ProbeFailed {
            probe: "exact_count_and_readback",
        });
    }
    Ok(())
}

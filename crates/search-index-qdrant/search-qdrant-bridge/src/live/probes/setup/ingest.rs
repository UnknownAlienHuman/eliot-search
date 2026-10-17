//! Wait=true, strong-ordering ingest and immediate exact readback probe.

use qdrant_client::qdrant::{GetPoints, PointId, UpsertPoints, point_id, value};

use super::super::super::LiveError;
use super::super::super::fixtures::{
    FIELD_TENANT, TENANT_A, UUID_POINT, num_point_id, probe_fixture_points, strong_ordering,
    update_completed,
};
use super::super::super::suite::Suite;

pub(in crate::live) async fn probe_ingest_batch_a(suite: &mut Suite<'_>) -> Result<(), LiveError> {
    let points = probe_fixture_points();
    let response = suite
        .client
        .upsert_points(UpsertPoints {
            collection_name: suite.client.collection_name().to_owned(),
            wait: Some(true),
            ordering: Some(strong_ordering()),
            points,
            ..Default::default()
        })
        .await
        .map_err(|_| LiveError::TransportFailed)?;
    let acked = response
        .result
        .as_ref()
        .is_some_and(|result| update_completed(result.status));
    let readback = suite
        .client
        .get_points(GetPoints {
            collection_name: suite.client.collection_name().to_owned(),
            ids: vec![
                num_point_id(1),
                num_point_id(2),
                num_point_id(9),
                PointId {
                    point_id_options: Some(point_id::PointIdOptions::Uuid(UUID_POINT.to_owned())),
                },
            ],
            with_payload: Some(true.into()),
            with_vectors: Some(true.into()),
            ..Default::default()
        })
        .await
        .map_err(|_| LiveError::TransportFailed)?;
    let visible = readback.result.len() == 4
        && readback.result.iter().all(|point| {
            point.payload.get(FIELD_TENANT).is_some_and(|tenant| {
                tenant.kind == Some(value::Kind::StringValue(TENANT_A.to_owned()))
            })
        });
    suite.record(
        "wait_true_mutation_ack",
        acked && visible,
        format!("acked={acked} readback={}/4", readback.result.len()),
    );
    suite.record(
        "strong_write_ordering",
        acked && visible,
        "wait=true + WriteOrdering::Strong acknowledged and immediately readable".to_owned(),
    );
    Ok(())
}

//! Wait=true, strong-ordering ingest and immediate exact readback probe.

use std::collections::HashMap;

use qdrant_client::qdrant::{
    GetPoints, PointId, PointStruct, UpsertPoints, point_id, value,
};

use super::super::super::fixtures::{
    ACCESS_A, EPOCH_MAX, EPOCH_MIN, FIELD_ACCESS, FIELD_FROM, FIELD_TENANT,
    FIELD_UNINDEXED, QUALIFICATION_COLLECTION, TENANT_A, UUID_POINT,
    int_value, num_point_id, point, sparse_named, string_value,
    strong_ordering, update_completed,
};
use super::super::super::suite::Suite;
use super::super::super::LiveError;

pub(super) async fn probe_ingest_batch_a(
    suite: &mut Suite,
) -> Result<(), LiveError> {
    let mut uuid_payload = HashMap::new();
    uuid_payload.insert(FIELD_TENANT.to_owned(), string_value(TENANT_A));
    uuid_payload.insert(FIELD_ACCESS.to_owned(), string_value(ACCESS_A));
    uuid_payload.insert(FIELD_FROM.to_owned(), int_value(10));
    uuid_payload.insert(FIELD_UNINDEXED.to_owned(), string_value("code_unit"));
    let points = vec![
        point(
            1,
            TENANT_A,
            10,
            None,
            vec![(0, 1.0), (1, 1.0)],
            vec![(100, 1.0)],
        ),
        point(
            2,
            TENANT_A,
            10,
            Some(50),
            vec![(0, 1.0)],
            vec![(100, 1.0)],
        ),
        point(
            9,
            TENANT_A,
            EPOCH_MIN,
            Some(EPOCH_MAX),
            vec![(0, 1.0), (2, 1.0)],
            vec![(100, 1.0)],
        ),
        PointStruct {
            id: Some(PointId {
                point_id_options: Some(point_id::PointIdOptions::Uuid(
                    UUID_POINT.to_owned(),
                )),
            }),
            payload: uuid_payload,
            vectors: Some(sparse_named(vec![(1, 1.0)], vec![(101, 1.0)])),
        },
    ];
    let response = suite
        .client
        .upsert_points(UpsertPoints {
            collection_name: QUALIFICATION_COLLECTION.to_owned(),
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
        "wait=true + WriteOrdering::Strong acknowledged and immediately readable"
            .to_owned(),
    );
    Ok(())
}

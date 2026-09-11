use std::collections::HashMap;

use qdrant_client::qdrant::{
    CountPoints, CreateCollection, CreateFieldIndexCollection, DeletePoints,
    FieldType, Filter, GetPoints, Modifier, PointId, PointStruct,
    PointsSelector, Query, QueryPoints, SparseVectorConfig, SparseVectorParams,
    StrictModeConfig, UpsertPoints, VectorInput, point_id, points_selector,
    value,
};

use crate::qualified::{QUALIFIED_SERVER_BUILD, QUALIFIED_SERVER_VERSION};

use super::super::fixtures::{
    ACCESS_A, EPOCH_MAX, EPOCH_MIN, FIELD_ACCESS, FIELD_FROM, FIELD_TENANT,
    FIELD_UNINDEXED, FIELD_UNTIL, QUALIFICATION_COLLECTION, TENANT_A,
    UUID_POINT, VECTOR_CODE, VECTOR_TEXT, exact_f64, int_value,
    keyword_condition, num_point_id, point, range_condition, sparse_named,
    string_value, strong_ordering, update_completed,
};
use super::super::suite::Suite;
use super::super::LiveError;

pub(super) async fn probe_server_identity(
    suite: &mut Suite,
) -> Result<(), LiveError> {
    let reply = suite
        .client
        .health_check()
        .await
        .map_err(|_| LiveError::TransportFailed)?;
    let version_ok = reply.version == QUALIFIED_SERVER_VERSION;
    let commit_ok = reply
        .commit
        .as_deref()
        .is_some_and(|commit| commit.starts_with(QUALIFIED_SERVER_BUILD));
    suite.record(
        "live_server_identity",
        version_ok && commit_ok,
        format!(
            "health version={} commit={:?} want={QUALIFIED_SERVER_VERSION}/{QUALIFIED_SERVER_BUILD}",
            reply.version, reply.commit,
        ),
    );
    if !version_ok {
        return Err(LiveError::ServerVersionUnexpected);
    }
    if !commit_ok {
        return Err(LiveError::ServerBuildUnexpected);
    }
    Ok(())
}

pub(super) async fn probe_create_and_topology(
    suite: &mut Suite,
) -> Result<(), LiveError> {
    if suite
        .client
        .collection_exists(QUALIFICATION_COLLECTION)
        .await
        .map_err(|_| LiveError::TransportFailed)?
    {
        let deleted = suite
            .client
            .delete_collection(QUALIFICATION_COLLECTION)
            .await
            .map_err(|_| LiveError::TransportFailed)?;
        if !deleted.result {
            return Err(LiveError::ProbeFailed {
                probe: "one_shard_topology",
            });
        }
    }
    let mut sparse = HashMap::new();
    for name in [VECTOR_CODE, VECTOR_TEXT] {
        sparse.insert(
            name.to_owned(),
            SparseVectorParams {
                modifier: Some(Modifier::Idf as i32),
                ..Default::default()
            },
        );
    }
    let created = suite
        .client
        .create_collection(CreateCollection {
            collection_name: QUALIFICATION_COLLECTION.to_owned(),
            shard_number: Some(1),
            replication_factor: Some(1),
            write_consistency_factor: Some(1),
            sparse_vectors_config: Some(SparseVectorConfig { map: sparse }),
            strict_mode_config: Some(StrictModeConfig {
                enabled: Some(true),
                unindexed_filtering_retrieve: Some(false),
                unindexed_filtering_update: Some(false),
                ..Default::default()
            }),
            ..Default::default()
        })
        .await
        .map_err(|_| LiveError::TransportFailed)?;
    if !created.result {
        return Err(LiveError::ProbeFailed {
            probe: "one_shard_topology",
        });
    }
    let info = suite
        .client
        .collection_info(QUALIFICATION_COLLECTION)
        .await
        .map_err(|_| LiveError::TransportFailed)?
        .result
        .ok_or(LiveError::TransportFailed)?;
    let shards = info
        .config
        .as_ref()
        .and_then(|config| config.params.as_ref())
        .map(|params| params.shard_number);
    suite.record(
        "one_shard_topology",
        shards == Some(1),
        format!("shard_number={shards:?}"),
    );
    Ok(())
}

pub(super) async fn probe_payload_indexes(
    suite: &mut Suite,
) -> Result<(), LiveError> {
    for (field, field_type) in [
        (FIELD_TENANT, FieldType::Keyword),
        (FIELD_ACCESS, FieldType::Keyword),
        (FIELD_FROM, FieldType::Integer),
        (FIELD_UNTIL, FieldType::Integer),
    ] {
        let indexed = suite
            .client
            .create_field_index(CreateFieldIndexCollection {
                collection_name: QUALIFICATION_COLLECTION.to_owned(),
                field_name: field.to_owned(),
                field_type: Some(field_type as i32),
                wait: Some(true),
                ordering: Some(strong_ordering()),
                ..Default::default()
            })
            .await
            .map_err(|_| LiveError::TransportFailed)?;
        let index_ready = indexed
            .result
            .as_ref()
            .is_some_and(|result| update_completed(result.status));
        if !index_ready {
            return Err(LiveError::ProbeFailed {
                probe: "payload_index_completeness",
            });
        }
    }
    let info = suite
        .client
        .collection_info(QUALIFICATION_COLLECTION)
        .await
        .map_err(|_| LiveError::TransportFailed)?
        .result
        .ok_or(LiveError::TransportFailed)?;
    let missing: Vec<&str> = [FIELD_TENANT, FIELD_ACCESS, FIELD_FROM, FIELD_UNTIL]
        .into_iter()
        .filter(|field| !info.payload_schema.contains_key(*field))
        .collect();
    suite.record(
        "payload_index_completeness",
        missing.is_empty(),
        format!("missing={missing:?}"),
    );
    Ok(())
}

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

pub(super) async fn probe_strict_negatives(
    suite: &mut Suite,
) -> Result<(), LiveError> {
    let unindexed = Filter {
        must: vec![keyword_condition(FIELD_UNINDEXED, "code_unit")],
        ..Default::default()
    };
    let retrieve_rejected = suite
        .client
        .query(QueryPoints {
            collection_name: QUALIFICATION_COLLECTION.to_owned(),
            query: Some(Query::new_nearest(VectorInput::new_sparse(
                vec![0_u32],
                vec![1.0_f32],
            ))),
            using: Some(VECTOR_CODE.to_owned()),
            filter: Some(unindexed.clone()),
            limit: Some(10),
            ..Default::default()
        })
        .await
        .is_err();
    suite.record(
        "strict_unindexed_retrieve_rejected",
        retrieve_rejected,
        "filter on never-indexed unit_kind".to_owned(),
    );
    let update_rejected = suite
        .client
        .delete_points(DeletePoints {
            collection_name: QUALIFICATION_COLLECTION.to_owned(),
            wait: Some(true),
            points: Some(PointsSelector {
                points_selector_one_of: Some(
                    points_selector::PointsSelectorOneOf::Filter(unindexed),
                ),
            }),
            ordering: Some(strong_ordering()),
            ..Default::default()
        })
        .await
        .is_err();
    suite.record(
        "strict_unindexed_update_rejected",
        update_rejected,
        "delete-by-filter on never-indexed unit_kind".to_owned(),
    );
    Ok(())
}

pub(super) async fn probe_signed_range(
    suite: &mut Suite,
) -> Result<(), LiveError> {
    let lower = suite
        .client
        .count(CountPoints {
            collection_name: QUALIFICATION_COLLECTION.to_owned(),
            filter: Some(Filter {
                must: vec![range_condition(
                    FIELD_FROM,
                    None,
                    Some(exact_f64(EPOCH_MIN)?),
                )],
                ..Default::default()
            }),
            exact: Some(true),
            ..Default::default()
        })
        .await
        .map_err(|_| LiveError::TransportFailed)?;
    let upper = suite
        .client
        .count(CountPoints {
            collection_name: QUALIFICATION_COLLECTION.to_owned(),
            filter: Some(Filter {
                must: vec![range_condition(
                    FIELD_UNTIL,
                    Some(exact_f64(EPOCH_MAX)?),
                    None,
                )],
                ..Default::default()
            }),
            exact: Some(true),
            ..Default::default()
        })
        .await
        .map_err(|_| LiveError::TransportFailed)?;
    let lower_count = lower.result.as_ref().map(|result| result.count);
    let upper_count = upper.result.as_ref().map(|result| result.count);
    suite.record(
        "signed_i64_epoch_range",
        lower_count == Some(1) && upper_count == Some(1),
        format!(
            "from<=MIN count={lower_count:?} until>=MAX count={upper_count:?}"
        ),
    );
    Ok(())
}

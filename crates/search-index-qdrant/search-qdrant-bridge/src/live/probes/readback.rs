use qdrant_client::qdrant::{
    CollectionStatus, CountPoints, DeletePoints, GetPoints, Modifier, PointId,
    PointsIdsList, PointsSelector, point_id, points_selector, value,
};

use super::super::fixtures::{
    FIELD_ACCESS, FIELD_FROM, FIELD_TENANT, FIELD_UNTIL,
    QUALIFICATION_COLLECTION, TENANT_A, UUID_POINT, VECTOR_CODE, VECTOR_TEXT,
    base_filter, num_point_id, snapshot_id, strong_ordering, update_completed,
};
use super::super::suite::Suite;
use super::super::LiveError;

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

pub(super) async fn probe_schema_digest(
    suite: &mut Suite,
) -> Result<(), LiveError> {
    let info = suite
        .client
        .collection_info(QUALIFICATION_COLLECTION)
        .await
        .map_err(|_| LiveError::TransportFailed)?
        .result
        .ok_or(LiveError::TransportFailed)?;
    let mut digest_parts = Vec::new();
    let mut sparse_ok = false;
    let mut shard_ok = false;
    if let Some(params) = info
        .config
        .as_ref()
        .and_then(|config| config.params.as_ref())
    {
        digest_parts.push(format!("shards={}", params.shard_number));
        shard_ok = params.shard_number == 1;
        if let Some(sparse) = params.sparse_vectors_config.as_ref() {
            let mut names: Vec<(&str, i32)> = sparse
                .map
                .iter()
                .map(|(name, vector_params)| {
                    (name.as_str(), vector_params.modifier.unwrap_or_default())
                })
                .collect();
            names.sort_unstable();
            digest_parts.push(format!("sparse={names:?}"));
            sparse_ok = names
                == [
                    (VECTOR_CODE, Modifier::Idf as i32),
                    (VECTOR_TEXT, Modifier::Idf as i32),
                ];
        }
    }
    let mut payload_ok = true;
    for field in [FIELD_TENANT, FIELD_ACCESS, FIELD_FROM, FIELD_UNTIL] {
        if info.payload_schema.contains_key(field) {
            digest_parts.push(format!("index:{field}=present"));
        } else {
            payload_ok = false;
        }
    }
    let (strict_enabled, retrieve_open, update_open) = info
        .config
        .as_ref()
        .and_then(|config| config.strict_mode_config.as_ref())
        .map(|strict| {
            (
                strict.enabled.unwrap_or_default(),
                strict.unindexed_filtering_retrieve.unwrap_or(true),
                strict.unindexed_filtering_update.unwrap_or(true),
            )
        })
        .unwrap_or_default();
    digest_parts.push(format!(
        "strict={strict_enabled}/{retrieve_open}/{update_open}"
    ));
    let status = CollectionStatus::try_from(info.status)
        .map_or_else(|_| info.status.to_string(), |parsed| format!("{parsed:?}"));
    digest_parts.push(format!("status={status}"));
    let digest = digest_parts.join("|");
    suite.record(
        "schema_digest_equality",
        sparse_ok
            && payload_ok
            && shard_ok
            && strict_enabled
            && !retrieve_open
            && !update_open,
        digest,
    );
    Ok(())
}

//! Disposable collection topology and mandatory payload-index probes.

use std::collections::HashMap;

use qdrant_client::qdrant::{
    CreateCollection, CreateFieldIndexCollection, FieldType, Modifier,
    SparseVectorConfig, SparseVectorParams, StrictModeConfig,
};

use super::super::super::fixtures::{
    FIELD_ACCESS, FIELD_FROM, FIELD_TENANT, FIELD_UNTIL,
    QUALIFICATION_COLLECTION, VECTOR_CODE, VECTOR_TEXT, strong_ordering,
    update_completed,
};
use super::super::super::suite::Suite;
use super::super::super::LiveError;

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

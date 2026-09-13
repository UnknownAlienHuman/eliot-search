//! Exact collection-schema readback and equality probe.

use qdrant_client::qdrant::{CollectionStatus, Modifier};

use super::super::super::fixtures::{
    FIELD_ACCESS, FIELD_FROM, FIELD_TENANT, FIELD_UNTIL,
    QUALIFICATION_COLLECTION, VECTOR_CODE, VECTOR_TEXT,
};
use super::super::super::suite::Suite;
use super::super::super::LiveError;

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
    suite.record(
        "schema_digest_equality",
        sparse_ok
            && payload_ok
            && shard_ok
            && strict_enabled
            && !retrieve_open
            && !update_open,
        digest_parts.join("|"),
    );
    Ok(())
}

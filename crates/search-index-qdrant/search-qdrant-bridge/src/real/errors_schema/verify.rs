const COLLECTION_SCHEMA_DIGEST_METADATA_KEY: &str =
    "eliot_search_schema_identity_digest_v1";

fn schema_identity_metadata(
    digest: &search_contracts::Blake3Digest32,
) -> HashMap<String, Value> {
    HashMap::from([(
        COLLECTION_SCHEMA_DIGEST_METADATA_KEY.to_owned(),
        str_value(&digest.to_string()),
    )])
}

fn schema_identity_metadata_matches(
    metadata: &HashMap<String, Value>,
    digest: &search_contracts::Blake3Digest32,
) -> bool {
    if metadata.len() != 1 {
        return false;
    }
    metadata
        .get(COLLECTION_SCHEMA_DIGEST_METADATA_KEY)
        .and_then(|entry| entry.kind.as_ref())
        .is_some_and(|kind| {
            matches!(kind, value::Kind::StringValue(actual) if actual == &digest.to_string())
        })
}

pub(super) fn verify_server_schema(
    info: &CollectionInfo,
    schema: &CollectionSchema,
) -> Result<(), BridgeError> {
    let config = info
        .config
        .as_ref()
        .ok_or(BridgeError::CollectionSchemaMismatch)?;
    if !schema_identity_metadata_matches(&config.metadata, &schema.schema_digest) {
        return Err(BridgeError::CollectionSchemaMismatch);
    }
    let params = config
        .params
        .as_ref()
        .ok_or(BridgeError::CollectionSchemaMismatch)?;
    if params.shard_number != 1
        || params.replication_factor != Some(1)
        || params.write_consistency_factor != Some(1)
    {
        return Err(BridgeError::CollectionSchemaMismatch);
    }
    let sparse = params
        .sparse_vectors_config
        .as_ref()
        .ok_or(BridgeError::CollectionSchemaMismatch)?;
    if sparse.map.len() != schema.named_vectors.len() {
        return Err(BridgeError::CollectionSchemaMismatch);
    }
    for (name, vector_schema) in &schema.named_vectors {
        let remote = sparse
            .map
            .get(name)
            .ok_or(BridgeError::NamedVectorMissing)?;
        let want = if vector_schema.idf_enabled {
            Some(Modifier::Idf as i32)
        } else {
            None
        };
        if remote.modifier != want {
            return Err(BridgeError::CollectionSchemaMismatch);
        }
    }

    if schema
        .indexed_payload_fields
        .iter()
        .any(|field| !info.payload_schema.contains_key(field))
    {
        return Err(BridgeError::PayloadIndexMissing);
    }
    if info.payload_schema.len() != schema.indexed_payload_fields.len()
        || schema.indexed_payload_fields.len() != PointPayload::INDEXED_FIELDS.len()
    {
        return Err(BridgeError::CollectionSchemaMismatch);
    }
    for (field, kind) in payload_index_specs() {
        let remote = info
            .payload_schema
            .get(field)
            .ok_or(BridgeError::PayloadIndexMissing)?;
        if remote.data_type() != vendor_schema_type(kind) {
            return Err(BridgeError::CollectionSchemaMismatch);
        }
    }

    let strict = config
        .strict_mode_config
        .as_ref()
        .ok_or(BridgeError::StrictModeRequired)?;
    if !strict.enabled.unwrap_or_default()
        || strict.unindexed_filtering_retrieve.unwrap_or(true)
        || strict.unindexed_filtering_update.unwrap_or(true)
    {
        return Err(BridgeError::StrictModeRequired);
    }
    Ok(())
}

#[cfg(test)]
mod schema_identity_tests {
    use super::*;

    #[test]
    fn collection_metadata_binds_exact_schema_digest_only() {
        let digest = search_contracts::Blake3Digest32::from_bytes([0x31; 32]);
        let other = search_contracts::Blake3Digest32::from_bytes([0x32; 32]);
        let exact = schema_identity_metadata(&digest);
        assert!(schema_identity_metadata_matches(&exact, &digest));
        assert!(!schema_identity_metadata_matches(&exact, &other));

        let mut extra = exact.clone();
        extra.insert("unqualified".to_owned(), str_value("metadata"));
        assert!(!schema_identity_metadata_matches(&extra, &digest));

        let wrong_type = HashMap::from([(
            COLLECTION_SCHEMA_DIGEST_METADATA_KEY.to_owned(),
            int_value(1),
        )]);
        assert!(!schema_identity_metadata_matches(&wrong_type, &digest));
        assert!(!schema_identity_metadata_matches(&HashMap::new(), &digest));
    }
}

fn verify_server_schema(
    info: &CollectionInfo,
    schema: &CollectionSchema,
) -> Result<(), BridgeError> {
    let params = info
        .config
        .as_ref()
        .and_then(|config| config.params.as_ref())
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
    if info.payload_schema.len() != schema.indexed_payload_fields.len() {
        return Err(BridgeError::CollectionSchemaMismatch);
    }
    for (field, expected_type) in [
        (
            EligibilityFilter::INDEXED_FIELDS[0],
            PayloadSchemaType::Keyword,
        ),
        (
            EligibilityFilter::INDEXED_FIELDS[1],
            PayloadSchemaType::Keyword,
        ),
        (
            EligibilityFilter::INDEXED_FIELDS[2],
            PayloadSchemaType::Integer,
        ),
        (
            EligibilityFilter::INDEXED_FIELDS[3],
            PayloadSchemaType::Integer,
        ),
    ] {
        let remote = info
            .payload_schema
            .get(field)
            .ok_or(BridgeError::PayloadIndexMissing)?;
        if remote.data_type() != expected_type {
            return Err(BridgeError::CollectionSchemaMismatch);
        }
    }

    let strict = info
        .config
        .as_ref()
        .and_then(|config| config.strict_mode_config.as_ref())
        .ok_or(BridgeError::StrictModeRequired)?;
    if !strict.enabled.unwrap_or_default()
        || strict.unindexed_filtering_retrieve.unwrap_or(true)
        || strict.unindexed_filtering_update.unwrap_or(true)
    {
        return Err(BridgeError::StrictModeRequired);
    }
    Ok(())
}

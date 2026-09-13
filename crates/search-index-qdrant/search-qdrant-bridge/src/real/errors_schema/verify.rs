fn verify_server_schema(
    info: &CollectionInfo,
    schema: &CollectionSchema,
) -> Result<(), BridgeError> {
    let params = info
        .config
        .as_ref()
        .and_then(|config| config.params.as_ref())
        .ok_or(BridgeError::CollectionSchemaMismatch)?;
    if params.shard_number != 1 {
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
    for field in EligibilityFilter::INDEXED_FIELDS {
        if !info.payload_schema.contains_key(field) {
            return Err(BridgeError::PayloadIndexMissing);
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

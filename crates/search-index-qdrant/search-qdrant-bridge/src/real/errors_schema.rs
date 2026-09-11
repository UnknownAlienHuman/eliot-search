/// Maps a vendor failure on a read path (no commit is possible) to a stable
/// typed error. Status numbers are gRPC canonical codes; status text never
/// crosses into errors.
fn map_read_error(error: qdrant_client::QdrantError) -> BridgeError {
    match error {
        qdrant_client::QdrantError::ResponseError { status }
        | qdrant_client::QdrantError::ResourceExhaustedError { status, .. } => {
            map_read_status(status.code() as i32)
        }
        qdrant_client::QdrantError::ConversionError(_)
        | qdrant_client::QdrantError::InvalidUri(_)
        | qdrant_client::QdrantError::NoSnapshotFound(_) => BridgeError::MalformedResponse,
        qdrant_client::QdrantError::Io(_) => BridgeError::TransportFailed,
    }
}

const fn map_read_status(code: i32) -> BridgeError {
    match code {
        CODE_NOT_FOUND => BridgeError::CollectionNotFound,
        CODE_ALREADY_EXISTS => BridgeError::CollectionAlreadyExists,
        CODE_INVALID_ARGUMENT | CODE_FAILED_PRECONDITION | CODE_OUT_OF_RANGE => {
            BridgeError::UnindexedFilter
        }
        CODE_PERMISSION_DENIED | CODE_UNAUTHENTICATED => BridgeError::AuthenticationInvalid,
        CODE_RESOURCE_EXHAUSTED => BridgeError::QueryBudgetExceeded,
        _ => BridgeError::TransportFailed,
    }
}

/// Maps a vendor failure after a mutation dispatch. Definite server
/// rejections keep their typed codes; any loss, deadline or ambiguous status
/// becomes `MutationOutcomeUnknown` because the write may have committed.
fn map_mutation_error(error: qdrant_client::QdrantError) -> BridgeError {
    match error {
        qdrant_client::QdrantError::ResponseError { status }
        | qdrant_client::QdrantError::ResourceExhaustedError { status, .. } => {
            map_mutation_status(status.code() as i32)
        }
        qdrant_client::QdrantError::ConversionError(_)
        | qdrant_client::QdrantError::InvalidUri(_)
        | qdrant_client::QdrantError::NoSnapshotFound(_) => BridgeError::MalformedResponse,
        qdrant_client::QdrantError::Io(_) => BridgeError::MutationOutcomeUnknown,
    }
}

const fn map_mutation_status(code: i32) -> BridgeError {
    match code {
        CODE_NOT_FOUND => BridgeError::CollectionNotFound,
        CODE_ALREADY_EXISTS => BridgeError::CollectionAlreadyExists,
        CODE_INVALID_ARGUMENT | CODE_FAILED_PRECONDITION | CODE_OUT_OF_RANGE => {
            BridgeError::VectorDimensionMismatch
        }
        CODE_PERMISSION_DENIED | CODE_UNAUTHENTICATED => BridgeError::AuthenticationInvalid,
        CODE_RESOURCE_EXHAUSTED => BridgeError::MutationTooLarge,
        _ => BridgeError::MutationOutcomeUnknown,
    }
}

/// Maps a vendor failure on collection creation. Creation carries no mutation
/// identity, so loss reports `MutationOutcomeUnknown`; a retry then converges
/// through `CollectionAlreadyExists` plus [`RealDataPlane::verify_schema`].
fn map_create_error(error: qdrant_client::QdrantError) -> BridgeError {
    match error {
        qdrant_client::QdrantError::ResponseError { status }
        | qdrant_client::QdrantError::ResourceExhaustedError { status, .. } => {
            match status.code() as i32 {
                CODE_NOT_FOUND => BridgeError::CollectionNotFound,
                CODE_ALREADY_EXISTS => BridgeError::CollectionAlreadyExists,
                CODE_INVALID_ARGUMENT | CODE_FAILED_PRECONDITION | CODE_OUT_OF_RANGE => {
                    BridgeError::CollectionSchemaMismatch
                }
                CODE_PERMISSION_DENIED | CODE_UNAUTHENTICATED => BridgeError::AuthenticationInvalid,
                CODE_RESOURCE_EXHAUSTED => BridgeError::MutationTooLarge,
                _ => BridgeError::MutationOutcomeUnknown,
            }
        }
        qdrant_client::QdrantError::ConversionError(_)
        | qdrant_client::QdrantError::InvalidUri(_)
        | qdrant_client::QdrantError::NoSnapshotFound(_) => BridgeError::CollectionSchemaMismatch,
        qdrant_client::QdrantError::Io(_) => BridgeError::MutationOutcomeUnknown,
    }
}

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

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
        | qdrant_client::QdrantError::NoSnapshotFound(_) => {
            BridgeError::MalformedResponse
        }
        qdrant_client::QdrantError::Io(_) => {
            BridgeError::MutationOutcomeUnknown
        }
    }
}

const fn map_mutation_status(code: i32) -> BridgeError {
    match code {
        CODE_NOT_FOUND => BridgeError::CollectionNotFound,
        CODE_ALREADY_EXISTS => BridgeError::CollectionAlreadyExists,
        CODE_INVALID_ARGUMENT | CODE_FAILED_PRECONDITION | CODE_OUT_OF_RANGE => {
            BridgeError::VectorDimensionMismatch
        }
        CODE_PERMISSION_DENIED | CODE_UNAUTHENTICATED => {
            BridgeError::AuthenticationInvalid
        }
        CODE_RESOURCE_EXHAUSTED => BridgeError::MutationTooLarge,
        _ => BridgeError::MutationOutcomeUnknown,
    }
}

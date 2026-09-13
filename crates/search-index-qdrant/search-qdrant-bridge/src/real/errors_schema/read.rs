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
        | qdrant_client::QdrantError::NoSnapshotFound(_) => {
            BridgeError::MalformedResponse
        }
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
        CODE_PERMISSION_DENIED | CODE_UNAUTHENTICATED => {
            BridgeError::AuthenticationInvalid
        }
        CODE_RESOURCE_EXHAUSTED => BridgeError::QueryBudgetExceeded,
        _ => BridgeError::TransportFailed,
    }
}

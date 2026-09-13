/// Maps a vendor failure on collection or payload-index creation. These writes
/// carry no mutation identity, so transport loss reports
/// `MutationOutcomeUnknown`; callers must reconcile the exact server schema
/// before admitting the collection. Existing collections are never silently
/// adopted by this mapper.
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
                CODE_PERMISSION_DENIED | CODE_UNAUTHENTICATED => {
                    BridgeError::AuthenticationInvalid
                }
                CODE_RESOURCE_EXHAUSTED => BridgeError::MutationTooLarge,
                _ => BridgeError::MutationOutcomeUnknown,
            }
        }
        qdrant_client::QdrantError::ConversionError(_)
        | qdrant_client::QdrantError::InvalidUri(_)
        | qdrant_client::QdrantError::NoSnapshotFound(_) => {
            BridgeError::CollectionSchemaMismatch
        }
        qdrant_client::QdrantError::Io(_) => {
            BridgeError::MutationOutcomeUnknown
        }
    }
}

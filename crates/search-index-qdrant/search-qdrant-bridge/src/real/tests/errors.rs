use super::*;

#[test]
fn error_codes_are_stable_and_redacted() {
    for error in [
        BridgeError::Cancelled,
        BridgeError::TransportFailed,
        BridgeError::MalformedResponse,
        BridgeError::MutationOutcomeUnknown,
        BridgeError::CollectionNotFound,
        BridgeError::InvalidFilter,
    ] {
        let rendered = error.to_string();
        assert_eq!(rendered, error.code());
        assert!(!rendered.contains("127.0.0.1"));
        assert!(!rendered.contains("http"));
    }
    assert_eq!(BridgeError::Cancelled.code(), "QDRANT_OPERATION_CANCELLED");
    assert_eq!(
        BridgeError::TransportFailed.code(),
        "QDRANT_TRANSPORT_FAILED"
    );
    assert_eq!(
        BridgeError::MalformedResponse.code(),
        "QDRANT_MALFORMED_RESPONSE"
    );
}

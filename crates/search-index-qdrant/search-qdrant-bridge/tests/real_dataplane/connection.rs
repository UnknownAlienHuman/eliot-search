use super::support::*;

#[tokio::test]
async fn t24_real_dead_endpoint_is_typed_not_unknown() {
    // Network loss before any send is definite TransportFailed, never an
    // unknown mutation outcome and never carrying endpoint text.
    let endpoint = search_qdrant_bridge::live::LiveEndpoint::grpc(
        "127.0.0.1",
        1,
    )
    .expect("loopback");
    let gate = qualified_gate().await;
    let Err(error) = RealDataPlane::connect(&endpoint, gate, limits()).await
    else {
        panic!("dead port must fail")
    };
    assert_eq!(error, BridgeError::TransportFailed);
    assert_eq!(error.to_string(), "QDRANT_TRANSPORT_FAILED");
    assert!(!error.to_string().contains("127.0.0.1"));
}

use super::support::*;

#[tokio::test]
async fn t24_stale_endpoint_binding_fails_before_dispatch() {
    let (server, plane) = live_plane().await;
    let (_, wrong_grpc_port) = free_loopback_ports().expect("loopback ports");
    let endpoint = search_qdrant_bridge::live::LiveEndpoint::loopback(
        server.endpoint().host(),
        server.endpoint().http_port(),
        wrong_grpc_port,
    )
    .expect("loopback endpoint");
    let Err(error) = RealDataPlane::connect(
        &endpoint,
        server.fixture_connection_binding(),
        server.fixture_api_key_lease(),
        plane.gate().clone(),
        limits(),
    )
    .await
    else {
        panic!("a stale endpoint tuple must fail before network dispatch")
    };
    assert_eq!(error, BridgeError::SupervisorReceiptMismatch);
    assert!(!error.to_string().contains("127.0.0.1"));
}

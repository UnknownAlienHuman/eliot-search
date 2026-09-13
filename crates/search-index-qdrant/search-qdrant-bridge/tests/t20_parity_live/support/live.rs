use super::*;

fn exe_path() -> String {
    std::env::var("ELIOT_QDRANT_EXE")
        .unwrap_or_else(|_| NATIVE_EXE_PATH.to_owned())
}

pub(crate) async fn live_plane() -> (
    search_qdrant_bridge::live::DisposableServer,
    RealDataPlane,
) {
    let (http_port, grpc_port) =
        free_loopback_ports().expect("loopback ports");
    let server = spawn_disposable_server(&exe_path(), http_port, grpc_port)
        .await
        .expect("disposable server");
    let report = run_qualification_suite(&server)
        .await
        .expect("qualification suite");
    let gate = QualifiedGate::admit(&report.receipt).expect("gate");
    let plane = RealDataPlane::connect(
        server.endpoint(),
        gate,
        BridgeLimits::BASELINE,
    )
    .await
    .expect("real data plane connects");
    (server, plane)
}

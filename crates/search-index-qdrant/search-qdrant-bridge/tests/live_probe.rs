//! T22 live qualification probes against the native Windows server.
//!
//! Spawns `C:\Tools\Qdrant\1.19.0\qdrant.exe` on disposable storage with
//! OS-assigned loopback ports, executes every bridge-owned mandatory probe
//! through the pinned `qdrant-client` 1.19.0 gRPC transport, then kills the
//! server and removes the storage. Fail-closed: a missing binary, a readiness
//! timeout, or any failed probe fails the test — there is no in-memory
//! stand-in on this path (the oracle stays in `lib.rs`).
//!
//! Run: `cargo test -p search-qdrant-bridge --test live_probe`.
//! Override binary: `ELIOT_QDRANT_EXE=<path>`.

use search_qdrant_bridge::live::{
    NATIVE_EXE_PATH, free_loopback_ports, run_qualification_suite,
    spawn_disposable_server,
};
use search_qdrant_bridge::qualified::{
    MANDATORY_LIVE_PROBES, QualificationError, QualifiedGate,
};

fn exe_path() -> String {
    std::env::var("ELIOT_QDRANT_EXE")
        .unwrap_or_else(|_| NATIVE_EXE_PATH.to_owned())
}

#[tokio::test]
async fn t22_live_qualification_probes() {
    let outcome = tokio::time::timeout(
        std::time::Duration::from_secs(180),
        async {
            let (http_port, grpc_port) =
                free_loopback_ports().expect("loopback ports");
            let server =
                spawn_disposable_server(&exe_path(), http_port, grpc_port)
                    .await
                    .expect("disposable server");
            let report = run_qualification_suite(&server)
                .await
                .expect("qualification suite");
            for line in &report.log {
                println!("{line}");
            }
            report
        },
    )
    .await
    .expect("suite finishes before the 180s budget");

    for required in MANDATORY_LIVE_PROBES {
        let passed = outcome
            .receipt
            .outcomes
            .iter()
            .any(|result| result.probe_id == required && result.passed);
        assert!(passed, "mandatory live probe passed: {required}");
    }
    assert_eq!(
        outcome.receipt.outcomes.len(),
        MANDATORY_LIVE_PROBES.len()
    );
    let gate = QualifiedGate::admit(&outcome.receipt)
        .expect("live receipt admits gate");
    assert_eq!(gate.server_version(), "1.19.0");
    assert_eq!(gate.collection(), "t22_qual_probe");

    // The gate executes against the live receipt: a tampered copy that claims
    // a different server version must not admit, even with all probes passed.
    let mut tampered = outcome.receipt;
    tampered.server_version = "1.19.1".to_owned();
    assert_eq!(
        QualifiedGate::admit(&tampered)
            .expect_err("tampered receipt must reject"),
        QualificationError::LiveIdentityMismatch
    );
}

#[tokio::test]
async fn t22_live_wrong_executable_rejected_before_spawn() {
    // Create an existing, deterministic wrong-size artifact. Verification must
    // reject it before process creation, independent of checkout location or
    // ELIOT_QDRANT_EXE configuration.
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "eliot-qdrant-wrong-exe-{}-{stamp}.bin",
        std::process::id()
    ));
    std::fs::write(&path, b"not-the-qualified-qdrant-executable")
        .expect("write wrong executable fixture");
    let path_text = path.to_string_lossy().into_owned();
    let (http_port, grpc_port) =
        free_loopback_ports().expect("loopback ports");
    let result =
        spawn_disposable_server(&path_text, http_port, grpc_port).await;
    let _ = std::fs::remove_file(&path);
    let Err(error) = result else {
        panic!("wrong executable must be rejected before spawn")
    };
    assert_eq!(
        error.code(),
        QualificationError::ArtifactSizeMismatch.code()
    );
}

#[test]
fn t22_live_non_loopback_endpoint_rejected_without_network() {
    let error = search_qdrant_bridge::live::LiveEndpoint::grpc(
        "10.0.0.1",
        6334,
    )
    .expect_err("non-loopback endpoint must be rejected");
    assert_eq!(error.code(), "QDRANT_LIVE_ENDPOINT_NOT_LOOPBACK");
}

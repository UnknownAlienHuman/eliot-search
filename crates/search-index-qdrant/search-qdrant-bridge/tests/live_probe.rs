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

use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use search_contracts::{Blake3Digest32, CollectionGenerationId, Epoch, OpaqueId};
use search_qdrant_bridge::live::{
    NATIVE_EXE_PATH, free_loopback_ports, run_qualification_suite, spawn_disposable_server,
};
use search_qdrant_bridge::qualified::{MANDATORY_LIVE_PROBES, QualificationError, QualifiedGate};
use search_qdrant_bridge::real::{OpContext, RealDataPlane};
use search_qdrant_bridge::{
    BridgeError, BridgeLimits, BridgeMutation, CollectionRoute, CollectionSchema,
    EligibilityFilter, PointPayload, PointRecord, QdrantPointId, StoredVector, StrictnessFloors,
    VectorSchema,
};

fn exe_path() -> String {
    std::env::var("ELIOT_QDRANT_EXE").unwrap_or_else(|_| NATIVE_EXE_PATH.to_owned())
}

#[tokio::test]
async fn t22_live_qualification_probes() {
    let (server, outcome) = tokio::time::timeout(std::time::Duration::from_secs(180), async {
        let (http_port, grpc_port) = free_loopback_ports().expect("loopback ports");
        let server = spawn_disposable_server(&exe_path(), http_port, grpc_port)
            .await
            .expect("disposable server");
        server
            .verify_authentication_required()
            .await
            .expect("wrong and missing API keys are rejected");
        let report = run_qualification_suite(&server)
            .await
            .expect("qualification suite");
        for line in &report.log {
            println!("{line}");
        }
        (server, report)
    })
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
    assert_eq!(outcome.receipt.outcomes.len(), MANDATORY_LIVE_PROBES.len());
    let gate = QualifiedGate::admit(&outcome.receipt).expect("live receipt admits gate");
    assert_eq!(gate.server_version(), "1.19.0");
    assert_eq!(gate.collection(), "t22_qual_probe");
    let mut plane = RealDataPlane::connect(
        server.endpoint(),
        server.fixture_connection_binding(),
        server.fixture_api_key_lease(),
        gate.clone(),
        search_qdrant_bridge::BridgeLimits::BASELINE,
    )
    .await
    .expect("correct fixture API key is admitted by a protected RPC");
    assert_eq!(plane.gate().server_version(), "1.19.0");

    // A lease that becomes invalid after the upsert dispatch but before the
    // exact readback must preserve the unknown write outcome. The still-valid
    // plane then performs the explicit recovery readback.
    let route = CollectionRoute {
        generation: CollectionGenerationId::from_bytes([0x61; 16]),
        physical_name: OpaqueId::new("auth_fault_collection")
            .expect("valid fixture collection name"),
    };
    let mut named_vectors = BTreeMap::new();
    named_vectors.insert(
        "auth_sparse_v1".to_owned(),
        VectorSchema {
            dimensions: 4,
            sparse: true,
            idf_enabled: true,
        },
    );
    let schema = CollectionSchema {
        named_vectors,
        indexed_payload_fields: EligibilityFilter::INDEXED_FIELDS
            .into_iter()
            .map(str::to_owned)
            .collect::<BTreeSet<_>>(),
        one_shard: true,
        floors: StrictnessFloors {
            strict_mode: true,
            wait_for_mutations: true,
            strong_ordering: true,
        },
        schema_digest: Blake3Digest32::from_bytes([0x62; 32]),
    };
    let context = OpContext::new(Duration::from_secs(20));
    plane
        .create_collection(&route, &schema, &context)
        .await
        .expect("create auth-fault collection");
    let mut vectors = BTreeMap::new();
    vectors.insert(
        "auth_sparse_v1".to_owned(),
        StoredVector {
            dimensions: 4,
            sparse: true,
            values: vec![(0, 1.0)],
            digest: Blake3Digest32::from_bytes([0x63; 32]),
        },
    );
    let expected = PointRecord {
        point_id: QdrantPointId([0x64; 16]),
        payload: PointPayload {
            source_membership_id: OpaqueId::new("auth_source").expect("valid source membership"),
            projection_membership_id: OpaqueId::new("auth_projection")
                .expect("valid projection membership"),
            access_partition_digest: Blake3Digest32::from_bytes([0x65; 32]),
            source_revision: 1,
            unit_ordinal: 1,
            valid_from_epoch: Epoch::new(1).expect("positive epoch"),
            valid_until_epoch_exclusive: None,
            payload_digest: Blake3Digest32::from_bytes([0x66; 32]),
            identity_digest: Blake3Digest32::from_bytes([0x67; 32]),
        },
        vectors,
    };
    // Five fixture callbacks cover SDK construction, connect health/list,
    // schema verification and the upsert dispatch; the next callback is the
    // required post-write exact readback and must observe the expired lease.
    let mut expiring_plane = RealDataPlane::connect(
        server.endpoint(),
        server.fixture_connection_binding(),
        server.fixture_api_key_lease_expiring_after(5),
        gate.clone(),
        BridgeLimits::BASELINE,
    )
    .await
    .expect("second authenticated connection");
    expiring_plane
        .verify_schema(&route, &schema, &context)
        .await
        .expect("verify fixture schema before faulted mutation");
    assert_eq!(
        expiring_plane
            .upsert_exact(
                &route,
                vec![expected.clone()],
                BridgeMutation {
                    operation_id: OpaqueId::new("auth_expiry_upsert").expect("valid operation id"),
                    canonical_input_digest: Blake3Digest32::from_bytes([0x68; 32]),
                },
                &context,
            )
            .await
            .expect_err("expired lease blocks exact post-write readback"),
        BridgeError::MutationOutcomeUnknown
    );
    let recovered = plane
        .readback_exact(&route, vec![expected.point_id], &context)
        .await
        .expect("valid plane resolves the unknown write by exact readback");
    assert_eq!(recovered.points, vec![expected]);
    assert!(recovered.missing_ids.is_empty());
    assert!(recovered.unexpected_ids.is_empty());

    // The gate executes against the live receipt: a tampered copy that claims
    // a different server version must not admit, even with all probes passed.
    let mut tampered = outcome.receipt;
    tampered.server_version = "1.19.1".to_owned();
    assert_eq!(
        QualifiedGate::admit(&tampered).expect_err("tampered receipt must reject"),
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
    let (http_port, grpc_port) = free_loopback_ports().expect("loopback ports");
    let result = spawn_disposable_server(&path_text, http_port, grpc_port).await;
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
    let error = search_qdrant_bridge::live::LiveEndpoint::grpc("10.0.0.1", 6334)
        .expect_err("non-loopback endpoint must be rejected");
    assert_eq!(error.code(), "QDRANT_LIVE_ENDPOINT_NOT_LOOPBACK");
}

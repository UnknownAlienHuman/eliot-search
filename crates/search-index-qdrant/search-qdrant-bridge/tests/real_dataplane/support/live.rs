use super::*;

fn exe_path() -> String {
    std::env::var("ELIOT_QDRANT_EXE")
        .unwrap_or_else(|_| NATIVE_EXE_PATH.to_owned())
}

/// Spawns a disposable server, runs the complete T22 qualification suite,
/// admits the gate and connects the real data plane. The returned server must
/// stay alive for the whole scenario.
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
    assert_eq!(
        report.receipt.outcomes.len(),
        13,
        "all mandatory probes executed"
    );
    let gate =
        QualifiedGate::admit(&report.receipt).expect("live receipt admits gate");
    let plane = RealDataPlane::connect(server.endpoint(), gate, limits())
        .await
        .expect("real data plane connects");
    (server, plane)
}

pub(crate) fn oracle() -> QdrantBridge {
    let endpoint_digest = Blake3Digest32::from_bytes([0xE0; 32]);
    let process_digest = Blake3Digest32::from_bytes([0xE1; 32]);
    let artifact_digest = Blake3Digest32::from_bytes([0xE2; 32]);
    let supervisor = SupervisorReceipt {
        owner_epoch: OwnerEpoch::new(7).expect("owner epoch"),
        process_identity_digest: process_digest,
        artifact_digest,
        endpoint_digest,
    };
    let capability = probe_capabilities(
        supervisor,
        Blake3Digest32::from_bytes([0xE3; 32]),
        CapabilityProbeResults {
            topology: TopologyGates {
                authenticated_health: true,
                single_shard: true,
                signed_i64_ranges: true,
            },
            filters: FilterGates {
                missing_upper_bound_must_not: true,
                sparse_idf: true,
                independent_idf_corpus: true,
            },
            indexes: IndexGates {
                strict_mode: true,
                payload_indexes: true,
                wait_for_mutations: true,
            },
            consistency: ConsistencyGates {
                strong_ordering: true,
                exact_count_and_readback: true,
                named_sparse_vectors: true,
            },
        },
    )
    .expect("capability");
    QdrantBridge::connect(
        BridgeEndpoint {
            endpoint_digest,
            loopback_only: true,
        },
        AuthLeaseEvidence {
            reference_digest: Blake3Digest32::from_bytes([0xE4; 32]),
            purpose_digest: Blake3Digest32::from_bytes([0xE5; 32]),
            valid: true,
        },
        supervisor,
        capability,
        limits(),
    )
    .expect("oracle connects")
}

pub(crate) async fn qualified_gate() -> QualifiedGate {
    let (http_port, grpc_port) =
        free_loopback_ports().expect("loopback ports");
    let server = spawn_disposable_server(&exe_path(), http_port, grpc_port)
        .await
        .expect("server");
    let report = run_qualification_suite(&server)
        .await
        .expect("suite");
    QualifiedGate::admit(&report.receipt).expect("gate")
}

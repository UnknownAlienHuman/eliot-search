//! Daemon-level live gRPC parity for the qualified Qdrant adapter.
//!
//! This lane is Windows-only because the frozen qualified artifact is
//! `qdrant.exe`. It compiles only with the indexed feature graph and performs
//! no runtime capability publication: the test itself executes all mandatory
//! live probes before constructing `QualifiedGate` and `RealDataPlane`.

#![cfg(all(windows, feature = "wave3-index"))]
#![forbid(unsafe_code)]
#![allow(clippy::too_many_lines)]

use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use search_contracts::{
    Blake3Digest32, CollectionGenerationId, Epoch, OpaqueId,
};
use search_qdrant_bridge::live::{
    NATIVE_EXE_PATH, free_loopback_ports, run_qualification_suite,
    spawn_disposable_server,
};
use search_qdrant_bridge::qualified::{
    MANDATORY_LIVE_PROBES, QualifiedGate,
};
use search_qdrant_bridge::real::{IdfScope, OpContext, RealDataPlane};
use search_qdrant_bridge::{
    BridgeLimits, BridgeMutation, CollectionRoute, CollectionSchema,
    EligibilityFilter, PointPayload, PointRecord, QdrantPointId, StoredVector,
    StrictnessFloors, VectorSchema,
};

const VECTOR_NAME: &str = "lex_code_v1";
const VECTOR_DIMS: u32 = 256;

fn executable_path() -> String {
    std::env::var("ELIOT_QDRANT_EXE")
        .unwrap_or_else(|_| NATIVE_EXE_PATH.to_owned())
}

const fn digest(byte: u8) -> Blake3Digest32 {
    Blake3Digest32::from_bytes([byte; 32])
}

fn member(value: &str) -> OpaqueId {
    OpaqueId::new(value).expect("bounded member identifier")
}

const fn point_id(number: u8) -> QdrantPointId {
    let mut bytes = [0_u8; 16];
    bytes[15] = number;
    QdrantPointId(bytes)
}

fn schema() -> CollectionSchema {
    let mut named_vectors = BTreeMap::new();
    named_vectors.insert(
        VECTOR_NAME.to_owned(),
        VectorSchema {
            dimensions: VECTOR_DIMS,
            sparse: true,
            idf_enabled: true,
        },
    );
    let indexed_payload_fields: BTreeSet<String> =
        EligibilityFilter::INDEXED_FIELDS
            .iter()
            .copied()
            .map(str::to_owned)
            .collect();
    CollectionSchema {
        named_vectors,
        indexed_payload_fields,
        one_shard: true,
        floors: StrictnessFloors {
            strict_mode: true,
            wait_for_mutations: true,
            strong_ordering: true,
        },
        schema_digest: digest(0x51),
    }
}

fn route() -> CollectionRoute {
    CollectionRoute {
        generation: CollectionGenerationId::from_bytes([0x31; 16]),
        physical_name: OpaqueId::new("daemon_grpc_parity")
            .expect("bounded collection name"),
    }
}

fn filter() -> EligibilityFilter {
    let mut allowed_source_memberships = BTreeSet::new();
    allowed_source_memberships.insert(member("daemon-member-allowed"));
    EligibilityFilter {
        access_partition_digest: digest(0xA1),
        allowed_source_memberships,
        visible_epoch: Epoch::new(42).expect("visible epoch"),
    }
}

fn point(
    number: u8,
    partition: u8,
    membership: &str,
    weight: f32,
) -> PointRecord {
    let mut vectors = BTreeMap::new();
    vectors.insert(
        VECTOR_NAME.to_owned(),
        StoredVector {
            dimensions: VECTOR_DIMS,
            sparse: true,
            values: vec![(0, weight)],
            digest: digest(number),
        },
    );
    PointRecord {
        point_id: point_id(number),
        payload: PointPayload {
            source_membership_id: member(membership),
            projection_membership_id: member("daemon-projection-a"),
            access_partition_digest: digest(partition),
            source_revision: u64::from(number),
            unit_ordinal: u64::from(number),
            valid_from_epoch: Epoch::new(10).expect("start epoch"),
            valid_until_epoch_exclusive: None,
            payload_digest: digest(number.wrapping_add(0x40)),
            identity_digest: digest(number.wrapping_add(0x80)),
        },
        vectors,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn daemon_live_grpc_path_preserves_filter_query_count_and_readback() {
    let outcome = tokio::time::timeout(Duration::from_secs(300), async {
        let (http_port, grpc_port) =
            free_loopback_ports().expect("reserve loopback ports");
        let server = spawn_disposable_server(
            &executable_path(),
            http_port,
            grpc_port,
        )
        .await
        .expect("spawn exact qualified Qdrant");
        let qualification = run_qualification_suite(&server)
            .await
            .expect("execute mandatory live probes");
        assert_eq!(
            qualification.receipt.outcomes.len(),
            MANDATORY_LIVE_PROBES.len(),
            "every mandatory probe must execute"
        );
        assert!(
            qualification
                .receipt
                .outcomes
                .iter()
                .all(|probe| probe.passed),
            "a failed live probe must block the test gate"
        );
        let gate = QualifiedGate::admit(&qualification.receipt)
            .expect("executed receipt admits the live path");
        let context = OpContext::new(Duration::from_secs(20));
        let mut plane = RealDataPlane::connect(
            server.endpoint(),
            gate,
            BridgeLimits::BASELINE,
        )
        .await
        .expect("connect pinned qdrant-client gRPC transport");

        let route = route();
        let schema = schema();
        plane
            .create_collection(&route, &schema, &context)
            .await
            .expect("create one-shard strict collection");
        plane
            .verify_schema(&route, &schema, &context)
            .await
            .expect("read back exact schema");

        let points = vec![
            point(1, 0xA1, "daemon-member-allowed", 1.0),
            // Higher-scoring points must not enter retrieval or the scoped IDF
            // population when either partition or membership is denied.
            point(2, 0xB2, "daemon-member-allowed", 10.0),
            point(3, 0xA1, "daemon-member-denied", 20.0),
        ];
        let mutation = BridgeMutation {
            operation_id: OpaqueId::new("daemon-grpc-parity-upsert")
                .expect("operation identity"),
            canonical_input_digest: digest(0xD1),
        };
        let receipt = plane
            .upsert_exact(&route, points, mutation, &context)
            .await
            .expect("wait=true strongly ordered upsert");
        assert_eq!(receipt.affected_ids.len(), 3);
        assert!(!receipt.replayed);

        let filter = filter();
        let count = plane
            .count_exact(&route, &filter, &context)
            .await
            .expect("exact filtered count");
        assert_eq!(count.count, 1, "denied points are outside denominator");

        let hits = plane
            .query_filtered(
                &route,
                &filter,
                VECTOR_NAME,
                &[(0, 1.0)],
                10,
                IdfScope::ScopedToRetrieval,
                &context,
            )
            .await
            .expect("filtered sparse-IDF query over gRPC");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].point_id, point_id(1));
        assert!(hits[0].score.is_finite());
        assert!(hits[0].score > 0.0);

        let readback = plane
            .readback_exact(
                &route,
                vec![point_id(1), point_id(2), point_id(3)],
                &context,
            )
            .await
            .expect("exact source-independent point readback");
        assert_eq!(readback.points.len(), 3);
        assert!(readback.missing_ids.is_empty());
        assert!(readback.unexpected_ids.is_empty());
    })
    .await;

    outcome.expect("daemon gRPC parity finishes within 300 seconds");
}

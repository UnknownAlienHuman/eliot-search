//! T24 mandatory live T20 parity: denied documents cannot influence
//! permitted ranking.
//!
//! The T20 access compiler denies before scoring: retrieval proposes
//! candidates from one accepted contract, and both the retrieval filter and
//! the independent IDF corpus (`idf.corpus`) are rendered from that single
//! contract (invariant 5). This test proves the property against the real
//! Qdrant 1.19.0 server: after ingesting a forbidden population that shares
//! query terms, permitted counts, per-document scores and ranking are
//! bit-identical under the scoped IDF corpus, while the unscoped (global)
//! IDF demonstrably moves — proving the denied population exists on the
//! server but is excluded from every permitted denominator.
//!
//! Candidate (retrieval) and IDF filters are exercised separately (exact
//! count with the retrieval filter alone; global-IDF query) and jointly
//! (scoped query with `filter` + `idf.corpus` from one contract).
//!
//! Run: `cargo test -p search-qdrant-bridge --test t20_parity_live`.

use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use search_contracts::{Blake3Digest32, CollectionGenerationId, Epoch, OpaqueId};
use search_qdrant_bridge::live::{
    NATIVE_EXE_PATH, free_loopback_ports, run_qualification_suite, spawn_disposable_server,
};
use search_qdrant_bridge::qualified::QualifiedGate;
use search_qdrant_bridge::real::{IdfScope, OpContext, RealDataPlane};
use search_qdrant_bridge::{
    BridgeLimits, BridgeMutation, CandidateNomination, CollectionRoute, CollectionSchema,
    EligibilityFilter, PointPayload, PointRecord, QdrantPointId, StoredVector, StrictnessFloors,
    VectorSchema,
};

const VECTOR_NAME: &str = "lex_code_v1";
const VECTOR_DIMS: u32 = 256;
const PARTITION_PERMITTED: u8 = 0xA1;
const PARTITION_DENIED: u8 = 0xB2;

fn exe_path() -> String {
    std::env::var("ELIOT_QDRANT_EXE").unwrap_or_else(|_| NATIVE_EXE_PATH.to_owned())
}

const fn ctx() -> OpContext {
    OpContext::new(Duration::from_secs(20))
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
    let mut indexed = BTreeSet::new();
    for field in EligibilityFilter::INDEXED_FIELDS {
        indexed.insert((*field).to_owned());
    }
    CollectionSchema {
        named_vectors,
        indexed_payload_fields: indexed,
        one_shard: true,
        floors: StrictnessFloors {
            strict_mode: true,
            wait_for_mutations: true,
            strong_ordering: true,
        },
        schema_digest: Blake3Digest32::from_bytes([0x77; 32]),
    }
}

fn permitted_filter() -> EligibilityFilter {
    let mut allowed = BTreeSet::new();
    allowed.insert(OpaqueId::new("t20-member-a").expect("member"));
    EligibilityFilter {
        access_partition_digest: Blake3Digest32::from_bytes([PARTITION_PERMITTED; 32]),
        allowed_source_memberships: allowed,
        visible_epoch: Epoch::new(42).expect("epoch"),
    }
}

fn denied_filter() -> EligibilityFilter {
    let mut allowed = BTreeSet::new();
    allowed.insert(OpaqueId::new("t20-member-denied").expect("member"));
    EligibilityFilter {
        access_partition_digest: Blake3Digest32::from_bytes([PARTITION_DENIED; 32]),
        allowed_source_memberships: allowed,
        visible_epoch: Epoch::new(42).expect("epoch"),
    }
}

const fn point_id(n: u8) -> QdrantPointId {
    let mut bytes = [0_u8; 16];
    bytes[15] = n;
    QdrantPointId(bytes)
}

fn permitted_point(n: u8, terms: Vec<(u32, f32)>) -> PointRecord {
    let mut vectors = BTreeMap::new();
    vectors.insert(
        VECTOR_NAME.to_owned(),
        StoredVector {
            dimensions: VECTOR_DIMS,
            sparse: true,
            values: terms,
            digest: Blake3Digest32::from_bytes([n; 32]),
        },
    );
    PointRecord {
        point_id: point_id(n),
        payload: PointPayload {
            source_membership_id: OpaqueId::new("t20-member-a").expect("member"),
            projection_membership_id: OpaqueId::new("t20-proj-a").expect("proj"),
            access_partition_digest: Blake3Digest32::from_bytes([PARTITION_PERMITTED; 32]),
            source_revision: u64::from(n),
            unit_ordinal: u64::from(n),
            valid_from_epoch: Epoch::new(10).expect("from"),
            valid_until_epoch_exclusive: None,
            payload_digest: Blake3Digest32::from_bytes([n.wrapping_add(100); 32]),
            identity_digest: Blake3Digest32::from_bytes([n.wrapping_add(200); 32]),
        },
        vectors,
    }
}

fn denied_point(n: u8) -> PointRecord {
    let mut vectors = BTreeMap::new();
    vectors.insert(
        VECTOR_NAME.to_owned(),
        StoredVector {
            dimensions: VECTOR_DIMS,
            sparse: true,
            values: vec![(0, 1.0)],
            digest: Blake3Digest32::from_bytes([n; 32]),
        },
    );
    PointRecord {
        point_id: point_id(n),
        payload: PointPayload {
            source_membership_id: OpaqueId::new("t20-member-denied").expect("member"),
            projection_membership_id: OpaqueId::new("t20-proj-denied").expect("proj"),
            access_partition_digest: Blake3Digest32::from_bytes([PARTITION_DENIED; 32]),
            source_revision: u64::from(n),
            unit_ordinal: u64::from(n),
            valid_from_epoch: Epoch::new(10).expect("from"),
            valid_until_epoch_exclusive: None,
            payload_digest: Blake3Digest32::from_bytes([n.wrapping_add(100); 32]),
            identity_digest: Blake3Digest32::from_bytes([n.wrapping_add(200); 32]),
        },
        vectors,
    }
}

fn mutation(tag: &str, n: u8) -> BridgeMutation {
    BridgeMutation {
        operation_id: OpaqueId::new(tag).expect("operation id"),
        canonical_input_digest: Blake3Digest32::from_bytes([n; 32]),
    }
}

/// Permitted-contract snapshot: exact count (retrieval filter separately)
/// plus one global and two scoped queries (IDF separately and jointly).
async fn permitted_snapshot(
    plane: &RealDataPlane,
    route: &CollectionRoute,
    filter: &EligibilityFilter,
    context: &OpContext,
) -> (
    usize,
    Vec<CandidateNomination>,
    Vec<CandidateNomination>,
    Vec<CandidateNomination>,
) {
    let count = plane
        .count_exact(route, filter, context)
        .await
        .expect("exact count")
        .count;
    let scoped_t0 = plane
        .query_filtered(
            route,
            filter,
            VECTOR_NAME,
            &[(0, 1.0)],
            10,
            IdfScope::ScopedToRetrieval,
            context,
        )
        .await
        .expect("scoped t0");
    let scoped_t1 = plane
        .query_filtered(
            route,
            filter,
            VECTOR_NAME,
            &[(1, 1.0)],
            10,
            IdfScope::ScopedToRetrieval,
            context,
        )
        .await
        .expect("scoped t1");
    let global_t0 = plane
        .query_filtered(
            route,
            filter,
            VECTOR_NAME,
            &[(0, 1.0)],
            10,
            IdfScope::Global,
            context,
        )
        .await
        .expect("global t0");
    (count, scoped_t0, scoped_t1, global_t0)
}

#[tokio::test]
async fn t24_live_t20_parity_denied_cannot_move_permitted() {
    let outcome = tokio::time::timeout(Duration::from_secs(240), async {
        let (http_port, grpc_port) = free_loopback_ports().expect("loopback ports");
        let server = spawn_disposable_server(&exe_path(), http_port, grpc_port)
            .await
            .expect("disposable server");
        let report = run_qualification_suite(&server)
            .await
            .expect("qualification suite");
        let gate = QualifiedGate::admit(&report.receipt).expect("gate");
        let mut plane = RealDataPlane::connect(server.endpoint(), gate, BridgeLimits::BASELINE)
            .await
            .expect("real data plane connects");

        let context = ctx();
        let route = CollectionRoute {
            generation: CollectionGenerationId::from_bytes([0xC4; 16]),
            physical_name: OpaqueId::new("t24_t20_parity").expect("physical name"),
        };
        plane
            .create_collection(&route, &schema(), &context)
            .await
            .expect("create collection");

        // Permitted population: three documents, term 0 shared by two.
        let permitted = vec![
            permitted_point(1, vec![(0, 1.0), (1, 1.0)]),
            permitted_point(2, vec![(0, 1.0)]),
            permitted_point(3, vec![(2, 1.0)]),
        ];
        plane
            .upsert_exact(
                &route,
                permitted,
                mutation("t24-t20-permitted", 0xE1),
                &context,
            )
            .await
            .expect("permitted ingest");

        // Baseline under the permitted contract, retrieval filter exercised
        // separately (exact count) and jointly with each IDF scope.
        let permitted_filter = permitted_filter();
        let (base_count, scoped_t0_before, scoped_t1_before, global_t0_before) =
            permitted_snapshot(&plane, &route, &permitted_filter, &context).await;
        assert_eq!(base_count, 3);
        assert_eq!(scoped_t0_before.len(), 2);
        assert!(scoped_t0_before.iter().all(|hit| hit.score.is_finite()));

        // Forbidden population: six denied documents sharing term 0. They are
        // deniable by partition AND by membership (both differ), so no
        // permitted contract can name them.
        let denied: Vec<PointRecord> = (10..=15).map(denied_point).collect();
        plane
            .upsert_exact(&route, denied, mutation("t24-t20-denied", 0xE2), &context)
            .await
            .expect("denied ingest");

        // The denied population is really on the server (separate denied
        // contract sees all six): isolation below is exclusion, not absence.
        let denied_count = plane
            .count_exact(&route, &denied_filter(), &context)
            .await
            .expect("denied count");
        assert_eq!(denied_count.count, 6);

        // Retrieval filter separately: the permitted exact-proof denominator
        // is unchanged by denied documents. Jointly: retrieval filter +
        // idf.corpus from the same contract — scores, order and IDF
        // denominators are bit-identical.
        let (after_count, scoped_t0_after, scoped_t1_after, global_t0_after) =
            permitted_snapshot(&plane, &route, &permitted_filter, &context).await;
        assert_eq!(
            after_count, base_count,
            "denied docs cannot move permitted counts"
        );
        assert_eq!(
            scoped_t0_after, scoped_t0_before,
            "denied docs cannot move permitted scores/order/IDF"
        );
        assert_eq!(
            scoped_t1_after, scoped_t1_before,
            "denied docs cannot move the rare-term ranking either"
        );
        for hit in scoped_t0_after.iter().chain(&scoped_t1_after) {
            assert!(
                !denied_ids().contains(&hit.point_id),
                "denied identity leaked into permitted nominations: {:?}",
                hit.point_id
            );
        }

        // Discrimination: without the corpus scope the denied population DOES
        // move global IDF — proving the test had power and the scoped corpus
        // is what provides isolation.
        assert_ne!(
            global_t0_after, global_t0_before,
            "global IDF must observe the denied population (test power)"
        );
        assert_ne!(
            global_t0_after, scoped_t0_after,
            "scoped corpus must differ from the contaminated global IDF"
        );
    })
    .await;
    outcome.expect("T20 parity suite finishes before the 240s budget");
}

fn denied_ids() -> Vec<QdrantPointId> {
    (10..=15).map(point_id).collect()
}

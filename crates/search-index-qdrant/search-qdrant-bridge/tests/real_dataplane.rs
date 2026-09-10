//! T24 real data-plane adapter: live discriminating tests.
//!
//! Every live test spawns the exact qualified native server on disposable
//! storage with OS-assigned loopback ports, admits it through the executed
//! T22 qualification suite (`QualifiedGate`), then exercises the real
//! `qdrant-client` 1.19.0 transport in `search_qdrant_bridge::real`.
//! Fail-closed: any transport or probe failure fails the test. The in-memory
//! `QdrantBridge` oracle stays as a behavioral reference for parity only and
//! is never a production fallback.
//!
//! Run: `cargo test -p search-qdrant-bridge --test real_dataplane`.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

use search_contracts::{Blake3Digest32, CollectionGenerationId, Epoch, OpaqueId, OwnerEpoch};
use search_qdrant_bridge::live::{
    NATIVE_EXE_PATH, free_loopback_ports, run_qualification_suite, spawn_disposable_server,
};
use search_qdrant_bridge::qualified::QualifiedGate;
use search_qdrant_bridge::real::{IdfScope, OpContext, RealDataPlane, validate_collection_name};
use search_qdrant_bridge::{
    AuthLeaseEvidence, BoundedPointReadback, BridgeEndpoint, BridgeError, BridgeLimits,
    BridgeMutation, CapabilityProbeResults, CollectionRoute, CollectionSchema, ConsistencyGates,
    EligibilityFilter, FilterGates, IndexGates, PointPayload, PointRecord, QdrantBridge,
    QdrantPointId, StoredVector, StrictnessFloors, SupervisorReceipt, TopologyGates, VectorSchema,
    probe_capabilities,
};

const VECTOR_NAME: &str = "lex_code_v1";
const VECTOR_DIMS: u32 = 256;

fn exe_path() -> String {
    std::env::var("ELIOT_QDRANT_EXE").unwrap_or_else(|_| NATIVE_EXE_PATH.to_owned())
}

const fn limits() -> BridgeLimits {
    BridgeLimits::BASELINE
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
        schema_digest: Blake3Digest32::from_bytes([0x51; 32]),
    }
}

fn make_route(name: &str, gen_byte: u8) -> CollectionRoute {
    CollectionRoute {
        generation: CollectionGenerationId::from_bytes([gen_byte; 16]),
        physical_name: OpaqueId::new(name).expect("physical name"),
    }
}

const fn partition(byte: u8) -> Blake3Digest32 {
    Blake3Digest32::from_bytes([byte; 32])
}

fn member(name: &str) -> OpaqueId {
    OpaqueId::new(name).expect("member")
}

fn permitted_filter() -> EligibilityFilter {
    let mut allowed = BTreeSet::new();
    allowed.insert(member("t24-member-a"));
    EligibilityFilter {
        access_partition_digest: partition(0xA1),
        allowed_source_memberships: allowed,
        visible_epoch: Epoch::new(42).expect("epoch"),
    }
}

const fn point_id(n: u8) -> QdrantPointId {
    let mut bytes = [0_u8; 16];
    bytes[15] = n;
    QdrantPointId(bytes)
}

fn point(
    n: u8,
    partition_byte: u8,
    membership: &str,
    from: i64,
    until: Option<i64>,
    terms: Vec<(u32, f32)>,
) -> PointRecord {
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
            source_membership_id: member(membership),
            projection_membership_id: member("t24-proj-a"),
            access_partition_digest: partition(partition_byte),
            source_revision: u64::from(n),
            unit_ordinal: u64::from(n),
            valid_from_epoch: Epoch::new(from).expect("from"),
            valid_until_epoch_exclusive: until.map(|value| Epoch::new(value).expect("until")),
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

/// Spawns a disposable server, runs the full T22 qualification suite on it,
/// admits the live gate and connects the real data plane. The returned
/// server must stay alive for the whole test (`Drop` kills it).
async fn live_plane() -> (search_qdrant_bridge::live::DisposableServer, RealDataPlane) {
    let (http_port, grpc_port) = free_loopback_ports().expect("loopback ports");
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
    let gate = QualifiedGate::admit(&report.receipt).expect("live receipt admits gate");
    let plane = RealDataPlane::connect(server.endpoint(), gate, limits())
        .await
        .expect("real data plane connects");
    (server, plane)
}

fn parity_points() -> Vec<PointRecord> {
    // Distinct term-0 weights (3.0 / 2.0 / 1.0): single-term IDF scaling is
    // monotone, so the live ranking must equal the oracle TF order exactly.
    vec![
        point(1, 0xA1, "t24-member-a", 10, None, vec![(0, 2.0), (1, 1.0)]),
        point(2, 0xA1, "t24-member-a", 10, Some(50), vec![(0, 1.0)]),
        point(3, 0xA1, "t24-member-a", 10, None, vec![(2, 1.0)]),
        point(4, 0xA1, "t24-member-a", 10, None, vec![(0, 3.0), (2, 1.0)]),
    ]
}

fn oracle() -> QdrantBridge {
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

/// Walks the whole eligible set in bounded pages and returns sorted IDs.
async fn scroll_all_ids(
    plane: &RealDataPlane,
    route: &CollectionRoute,
    filter: &EligibilityFilter,
    context: &OpContext,
    page_size: usize,
) -> Vec<QdrantPointId> {
    let mut seen = Vec::new();
    let mut offset: Option<QdrantPointId> = None;
    for _ in 0..32 {
        let page = plane
            .scroll_exact(route, filter, offset, page_size, context)
            .await
            .expect("bounded page");
        assert!(page.points.len() <= page_size, "bounded page");
        if page.points.is_empty() {
            break;
        }
        seen.extend(page.points.iter().map(|point| point.point_id));
        offset = page.next_offset;
        if offset.is_none() {
            break;
        }
    }
    seen.sort();
    seen
}

/// `close_exact` and `delete_exact` parity tail: both planes narrow the
/// exact-proof denominator identically.
async fn parity_close_delete_tail(
    plane: &mut RealDataPlane,
    oracle: &mut QdrantBridge,
    route: &CollectionRoute,
    filter: &EligibilityFilter,
    context: &OpContext,
) {
    let close_mutation = mutation("t24-parity-close-1", 0x22);
    let close_epoch = Epoch::new(42).expect("close epoch");
    oracle
        .close_exact(
            route,
            vec![point_id(4)],
            close_epoch,
            close_mutation.clone(),
        )
        .expect("oracle close");
    plane
        .close_exact(
            route,
            vec![point_id(4)],
            close_epoch,
            close_mutation,
            context,
        )
        .await
        .expect("real close");
    assert_eq!(
        plane
            .count_exact(route, filter, context)
            .await
            .expect("count")
            .count,
        oracle
            .count_exact(route, filter)
            .expect("oracle count")
            .count
    );

    let delete_mutation = mutation("t24-parity-delete-1", 0x23);
    oracle
        .delete_exact(route, vec![point_id(3)], delete_mutation.clone())
        .expect("oracle delete");
    plane
        .delete_exact(route, vec![point_id(3)], delete_mutation, context)
        .await
        .expect("real delete");
    assert_eq!(
        plane
            .count_exact(route, filter, context)
            .await
            .expect("count")
            .count,
        2
    );
    assert_eq!(
        oracle
            .count_exact(route, filter)
            .expect("oracle count")
            .count,
        2
    );
}

/// A wrong namespace or generation never leaks data on any read path.
async fn assert_wrong_route_rejected(
    plane: &RealDataPlane,
    wrong: &CollectionRoute,
    filter: &EligibilityFilter,
    context: &OpContext,
) {
    assert_eq!(
        plane
            .count_exact(wrong, filter, context)
            .await
            .expect_err("wrong route count"),
        BridgeError::CollectionNotFound
    );
    assert_eq!(
        plane
            .readback_exact(wrong, vec![point_id(1)], context)
            .await
            .expect_err("wrong route readback"),
        BridgeError::CollectionNotFound
    );
    assert_eq!(
        plane
            .query_filtered(
                wrong,
                filter,
                VECTOR_NAME,
                &[(0, 1.0)],
                10,
                IdfScope::ScopedToRetrieval,
                context
            )
            .await
            .expect_err("wrong route query"),
        BridgeError::CollectionNotFound
    );
    assert_eq!(
        plane
            .scroll_exact(wrong, filter, None, 10, context)
            .await
            .expect_err("wrong route scroll"),
        BridgeError::CollectionNotFound
    );
}

fn oversize_batch() -> Vec<PointRecord> {
    let mut batch = Vec::new();
    for n in 0..=limits().max_points_per_mutation {
        let id_byte = u8::try_from(n % 251).expect("byte") + 1;
        batch.push(point(
            id_byte,
            0xA1,
            "t24-member-a",
            10,
            None,
            vec![(0, 1.0)],
        ));
    }
    batch
}

/// Asserts per-point digest/epoch parity between oracle and real readback.
fn assert_readback_parity(
    oracle_readback: &BoundedPointReadback,
    real_readback: &BoundedPointReadback,
) {
    assert_eq!(real_readback.missing_ids, oracle_readback.missing_ids);
    assert!(real_readback.missing_ids.is_empty());
    assert!(real_readback.unexpected_ids.is_empty());
    for expected in &oracle_readback.points {
        let actual = real_readback
            .points
            .iter()
            .find(|point| point.point_id == expected.point_id)
            .expect("parity point present");
        assert_eq!(
            actual.payload.payload_digest,
            expected.payload.payload_digest
        );
        assert_eq!(
            actual.payload.identity_digest,
            expected.payload.identity_digest
        );
        assert_eq!(
            actual.payload.valid_from_epoch,
            expected.payload.valid_from_epoch
        );
        assert_eq!(
            actual.payload.valid_until_epoch_exclusive,
            expected.payload.valid_until_epoch_exclusive
        );
    }
}

/// A poisoned batch fails whole-batch validation before dispatch and commits
/// nothing.
async fn assert_partial_batch_rejected(
    plane: &mut RealDataPlane,
    route: &CollectionRoute,
    context: &OpContext,
) {
    let partial = vec![
        point(24, 0xA1, "t24-member-a", 10, None, vec![(0, 1.0)]),
        point(25, 0xA1, "t24-member-a", 10, None, vec![(1, 1.0), (0, 1.0)]),
    ];
    assert_eq!(
        plane
            .upsert_exact(
                route,
                partial,
                mutation("t24-recovery-partial", 0x63),
                context
            )
            .await
            .expect_err("partial batch"),
        BridgeError::VectorDimensionMismatch
    );
    let missing = plane
        .readback_exact(route, vec![point_id(24), point_id(25)], context)
        .await
        .expect("readback");
    assert!(missing.points.is_empty());
    assert_eq!(missing.missing_ids.len(), 2);
}

/// Every read path checks cancellation before dispatch.
async fn assert_reads_cancelled(
    plane: &RealDataPlane,
    route: &CollectionRoute,
    cancelled: &OpContext,
) {
    assert_eq!(
        plane
            .count_exact(route, &permitted_filter(), cancelled)
            .await
            .expect_err("cancelled count"),
        BridgeError::Cancelled
    );
    assert_eq!(
        plane
            .query_filtered(
                route,
                &permitted_filter(),
                VECTOR_NAME,
                &[(0, 1.0)],
                10,
                IdfScope::ScopedToRetrieval,
                cancelled
            )
            .await
            .expect_err("cancelled query"),
        BridgeError::Cancelled
    );
    assert_eq!(
        plane
            .scroll_exact(route, &permitted_filter(), None, 2, cancelled)
            .await
            .expect_err("cancelled scroll"),
        BridgeError::Cancelled
    );
    assert_eq!(
        plane
            .readback_exact(route, vec![point_id(31)], cancelled)
            .await
            .expect_err("cancelled readback"),
        BridgeError::Cancelled
    );
}

/// Unknown point IDs surface as explicit missing entries, never errors.
async fn assert_explicit_missing(
    plane: &RealDataPlane,
    route: &CollectionRoute,
    context: &OpContext,
) {
    let unknown = plane
        .readback_exact(route, vec![point_id(22), point_id(77)], context)
        .await
        .expect("readback with unknown");
    assert_eq!(unknown.points.len(), 1);
    assert_eq!(unknown.missing_ids, vec![point_id(77)]);
    assert!(unknown.unexpected_ids.is_empty());
}

#[test]
fn collection_names_reject_non_qdrant_chars_without_network() {
    assert!(validate_collection_name("t24_dataplane_001").is_ok());
    assert!(validate_collection_name("a").is_ok());
    assert!(validate_collection_name("t24-parity_09.Z").is_ok());
    for bad in ["", "has space", "has/slash", "has:colon", "what?"] {
        assert!(
            validate_collection_name(bad).is_err(),
            "must reject without network: {bad:?}"
        );
    }
    let too_long = "a".repeat(200);
    assert!(
        validate_collection_name(&too_long).is_err(),
        "length bound enforced"
    );
}

#[tokio::test]
async fn t24_real_crud_query_parity_with_oracle() {
    let outcome = tokio::time::timeout(Duration::from_secs(240), async {
        let (_server, mut plane) = live_plane().await;
        let context = ctx();
        let route = make_route("t24_parity", 0x11);
        let schema = schema();
        let filter = permitted_filter();

        let mut oracle = oracle();
        oracle
            .create_candidate_collection(route.clone(), schema.clone())
            .expect("oracle create");
        plane
            .create_collection(&route, &schema, &context)
            .await
            .expect("real create");
        plane
            .verify_schema(&route, &schema, &context)
            .await
            .expect("real schema verifies");

        let batch = parity_points();
        let create_mutation = mutation("t24-parity-upsert-1", 0x21);
        let oracle_receipt = oracle
            .upsert_exact(&route, batch.clone(), create_mutation.clone())
            .expect("oracle upsert");
        assert!(!oracle_receipt.replayed);
        let real_receipt = plane
            .upsert_exact(&route, batch.clone(), create_mutation, &context)
            .await
            .expect("real upsert");
        assert!(!real_receipt.replayed);
        assert_eq!(real_receipt.affected_ids, oracle_receipt.affected_ids);

        let oracle_count = oracle.count_exact(&route, &filter).expect("oracle count");
        let real_count = plane
            .count_exact(&route, &filter, &context)
            .await
            .expect("real count");
        assert_eq!(real_count, oracle_count);
        assert_eq!(real_count.count, 4);

        let ids: Vec<QdrantPointId> = (1..=4).map(point_id).collect();
        let oracle_readback = oracle
            .readback_exact(&route, ids.clone())
            .expect("oracle readback");
        let real_readback = plane
            .readback_exact(&route, ids.clone(), &context)
            .await
            .expect("real readback");
        assert_readback_parity(&oracle_readback, &real_readback);

        // Single-term queries keep IDF-monotone order: the positively
        // matching IDs and ranking must equal the oracle TF order. Zero-match
        // documents (oracle score 0.0) are server-pruned nominations without
        // signal — a documented transport difference, not a ranking gap.
        let oracle_hits = oracle
            .query_filtered(&route, &filter, VECTOR_NAME, &[(0, 1.0)], 10)
            .expect("oracle query");
        let real_hits = plane
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
            .expect("real query");
        let oracle_ids: Vec<QdrantPointId> = oracle_hits
            .iter()
            .filter(|hit| hit.score > 0.0)
            .map(|hit| hit.point_id)
            .collect();
        let real_ids: Vec<QdrantPointId> = real_hits.iter().map(|hit| hit.point_id).collect();
        assert_eq!(real_ids, oracle_ids, "single-term ranking parity");
        for hit in &real_hits {
            assert!(hit.score.is_finite(), "finite scores only");
            assert!(
                hit.score > 0.0,
                "server returns positively-matching nominations"
            );
        }
        // Bounded scroll covers the same population as the oracle.
        let scrolled = scroll_all_ids(&plane, &route, &filter, &context, 2).await;
        assert_eq!(
            scrolled,
            vec![point_id(1), point_id(2), point_id(3), point_id(4)]
        );

        parity_close_delete_tail(&mut plane, &mut oracle, &route, &filter, &context).await;
    })
    .await;
    outcome.expect("parity suite finishes before the 240s budget");
}

#[tokio::test]
async fn t24_real_wrong_route_filter_and_bounds_rejected() {
    let outcome = tokio::time::timeout(Duration::from_secs(240), async {
        let (_server, mut plane) = live_plane().await;
        let context = ctx();
        let route = make_route("t24_reject", 0x31);
        plane
            .create_collection(&route, &schema(), &context)
            .await
            .expect("create");

        // Wrong namespace (unknown physical name) and wrong generation (same
        // physical, different generation) never leak data: CollectionNotFound.
        let filter = permitted_filter();
        assert_wrong_route_rejected(&plane, &make_route("t24_no_such", 0x31), &filter, &context)
            .await;
        assert_wrong_route_rejected(&plane, &make_route("t24_reject", 0x32), &filter, &context)
            .await;

        // Closed filter language: an empty membership set is InvalidFilter
        // before any network use.
        let mut empty = permitted_filter();
        empty.allowed_source_memberships.clear();
        assert_eq!(
            plane
                .count_exact(&route, &empty, &context)
                .await
                .expect_err("empty filter"),
            BridgeError::InvalidFilter
        );

        // Finite pagination floors: zero or over-max limits fail pre-dispatch.
        assert_eq!(
            plane
                .query_filtered(
                    &route,
                    &permitted_filter(),
                    VECTOR_NAME,
                    &[(0, 1.0)],
                    0,
                    IdfScope::ScopedToRetrieval,
                    &context
                )
                .await
                .expect_err("zero limit"),
            BridgeError::QueryBudgetExceeded
        );
        assert_eq!(
            plane
                .query_filtered(
                    &route,
                    &permitted_filter(),
                    VECTOR_NAME,
                    &[(0, 1.0)],
                    limits().max_query_candidates + 1,
                    IdfScope::ScopedToRetrieval,
                    &context
                )
                .await
                .expect_err("over-max limit"),
            BridgeError::QueryBudgetExceeded
        );
        assert_eq!(
            plane
                .scroll_exact(&route, &permitted_filter(), None, 0, &context)
                .await
                .expect_err("zero scroll"),
            BridgeError::QueryBudgetExceeded
        );

        // Duplicate IDs in one batch are rejected before dispatch and commit nothing.
        let dup = vec![
            point(11, 0xA1, "t24-member-a", 10, None, vec![(0, 1.0)]),
            point(11, 0xA1, "t24-member-a", 10, None, vec![(0, 1.0)]),
        ];
        assert_eq!(
            plane
                .upsert_exact(&route, dup, mutation("t24-reject-dup", 0x41), &context)
                .await
                .expect_err("duplicate batch"),
            BridgeError::DuplicatePointId
        );
        assert_eq!(
            plane
                .count_exact(&route, &permitted_filter(), &context)
                .await
                .expect("count")
                .count,
            0
        );

        // Oversize batches are rejected before dispatch.
        assert_eq!(
            plane
                .upsert_exact(
                    &route,
                    oversize_batch(),
                    mutation("t24-reject-oversize", 0x42),
                    &context
                )
                .await
                .expect_err("oversize batch"),
            BridgeError::MutationTooLarge
        );
    })
    .await;
    outcome.expect("rejection suite finishes before the 240s budget");
}

#[tokio::test]
async fn t24_real_unknown_write_recovery_and_replay() {
    let outcome = tokio::time::timeout(Duration::from_secs(240), async {
        let (_server, mut plane) = live_plane().await;
        let context = ctx();
        let route = make_route("t24_recovery", 0x51);
        plane
            .create_collection(&route, &schema(), &context)
            .await
            .expect("create");

        // Pre-dispatch cancellation is definite (Cancelled) and commits nothing.
        let flag = Arc::new(AtomicBool::new(true));
        let cancelled = OpContext::with_cancel(Duration::from_secs(20), Arc::clone(&flag));
        assert_eq!(
            plane
                .upsert_exact(
                    &route,
                    vec![point(21, 0xA1, "t24-member-a", 10, None, vec![(0, 1.0)])],
                    mutation("t24-recovery-cancel", 0x61),
                    &cancelled
                )
                .await
                .expect_err("cancelled upsert"),
            BridgeError::Cancelled
        );
        assert_eq!(
            plane
                .count_exact(&route, &permitted_filter(), &context)
                .await
                .expect("count")
                .count,
            0
        );

        // A zero deadline forces the timeout path: the write may have been
        // sent, so the outcome is UNKNOWN until readback reconciles it.
        let squeezed = OpContext::new(Duration::ZERO);
        let mutation_id = mutation("t24-recovery-unknown", 0x62);
        let batch = vec![point(22, 0xA1, "t24-member-a", 10, None, vec![(0, 1.0)])];
        assert_eq!(
            plane
                .upsert_exact(&route, batch.clone(), mutation_id.clone(), &squeezed)
                .await
                .expect_err("timeout is unknown"),
            BridgeError::MutationOutcomeUnknown
        );
        // Exact replay with the same identity never publishes twice: the
        // upsert is idempotent over exact IDs, so reconciliation converges to
        // exactly one effective write.
        let replay = plane
            .upsert_exact(&route, batch, mutation_id, &context)
            .await
            .expect("replay resolves");
        assert!(replay.affected_ids.contains(&point_id(22)));
        assert_eq!(
            plane
                .count_exact(&route, &permitted_filter(), &context)
                .await
                .expect("count")
                .count,
            1
        );

        // A second identical submission is a recorded replay, not a new write.
        let again = plane
            .upsert_exact(
                &route,
                vec![point(22, 0xA1, "t24-member-a", 10, None, vec![(0, 1.0)])],
                mutation("t24-recovery-unknown", 0x62),
                &context,
            )
            .await
            .expect("recorded replay");
        assert!(again.replayed);
        assert_eq!(
            plane
                .count_exact(&route, &permitted_filter(), &context)
                .await
                .expect("count")
                .count,
            1
        );

        // Same operation identity with different input is a conflict, never a silent overwrite.
        assert_eq!(
            plane
                .upsert_exact(
                    &route,
                    vec![point(23, 0xA1, "t24-member-a", 10, None, vec![(1, 1.0)])],
                    mutation("t24-recovery-unknown", 0x99),
                    &context
                )
                .await
                .expect_err("conflict"),
            BridgeError::OperationConflict
        );

        // Partial batches fail whole-batch validation before dispatch.
        assert_partial_batch_rejected(&mut plane, &route, &context).await;

        // Unknown point IDs are explicit missing entries, never errors.
        assert_explicit_missing(&plane, &route, &context).await;
    })
    .await;
    outcome.expect("recovery suite finishes before the 240s budget");
}

#[tokio::test]
async fn t24_real_pagination_cancellation_and_error_redaction() {
    let outcome = tokio::time::timeout(Duration::from_secs(240), async {
        let (_server, mut plane) = live_plane().await;
        let context = ctx();
        let route = make_route("t24_pages", 0x71);
        plane
            .create_collection(&route, &schema(), &context)
            .await
            .expect("create");
        let mut batch = Vec::new();
        for n in 31..=35 {
            batch.push(point(n, 0xA1, "t24-member-a", 10, None, vec![(0, 1.0)]));
        }
        plane
            .upsert_exact(&route, batch, mutation("t24-pages-upsert", 0x81), &context)
            .await
            .expect("upsert five");

        // Bounded pages walk the whole eligible set without oversize.
        let seen = scroll_all_ids(&plane, &route, &permitted_filter(), &context, 2).await;
        assert_eq!(
            seen,
            vec![
                point_id(31),
                point_id(32),
                point_id(33),
                point_id(34),
                point_id(35)
            ]
        );

        // Cancellation is checked before dispatch on every read path.
        let flag = Arc::new(AtomicBool::new(true));
        flag.store(true, Ordering::SeqCst);
        let cancelled = OpContext::with_cancel(Duration::from_secs(20), Arc::clone(&flag));
        assert_reads_cancelled(&plane, &route, &cancelled).await;

        // Redaction: typed errors carry stable codes only — no endpoint,
        // secret, token or payload content may escape into Display.
        let forbidden = [
            "http://",
            "127.0.0.1",
            "localhost",
            "Bearer",
            "api-key",
            "t24-member-a",
        ];
        let samples = [
            plane
                .count_exact(
                    &make_route("t24_pages_missing", 0x71),
                    &permitted_filter(),
                    &context,
                )
                .await
                .expect_err("sample"),
            BridgeError::InvalidFilter,
            BridgeError::QueryBudgetExceeded,
            BridgeError::Cancelled,
            BridgeError::MutationOutcomeUnknown,
            BridgeError::TransportFailed,
            BridgeError::MalformedResponse,
        ];
        for sample in samples {
            let rendered = sample.to_string();
            assert_eq!(rendered, sample.code(), "Display is the stable code");
            for needle in forbidden {
                assert!(
                    !rendered.contains(needle),
                    "redacted error {rendered:?} must not contain {needle:?}"
                );
            }
        }
    })
    .await;
    outcome.expect("pagination suite finishes before the 240s budget");
}

#[tokio::test]
async fn t24_real_dead_endpoint_is_typed_not_unknown() {
    // Network loss before any send (unreachable loopback port) is a definite
    // TransportFailed at connect time — never an unknown mutation outcome and
    // never carrying the endpoint URL in the error.
    let endpoint =
        search_qdrant_bridge::live::LiveEndpoint::grpc("127.0.0.1", 1).expect("loopback");
    let gate = {
        let (http_port, grpc_port) = free_loopback_ports().expect("ports");
        let server = spawn_disposable_server(&exe_path(), http_port, grpc_port)
            .await
            .expect("server");
        let report = run_qualification_suite(&server).await.expect("suite");
        QualifiedGate::admit(&report.receipt).expect("gate")
    };
    let Err(error) = RealDataPlane::connect(&endpoint, gate, limits()).await else {
        panic!("dead port must fail")
    };
    assert_eq!(error, BridgeError::TransportFailed);
    assert_eq!(error.to_string(), "QDRANT_TRANSPORT_FAILED");
    assert!(!error.to_string().contains("127.0.0.1"));
}

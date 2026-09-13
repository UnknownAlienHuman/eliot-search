//! Public-API regressions for the in-memory reference bridge only.
//! No live server is started and no qualification evidence is produced.

use std::collections::{BTreeMap, BTreeSet};

use search_contracts::{Blake3Digest32, CollectionGenerationId, Epoch, OpaqueId, OwnerEpoch};
use search_qdrant_bridge::{
    AuthLeaseEvidence, BridgeEndpoint, BridgeError, BridgeLimits, BridgeMutation,
    CandidateNomination, CapabilityProbeResults, CollectionRoute, CollectionSchema,
    ConsistencyGates, EligibilityFilter, FilterGates, IndexGates, MutationReceipt,
    PointPayload, PointRecord, QdrantBridge, QdrantPointId, StoredVector,
    StrictnessFloors, SupervisorReceipt, TopologyGates, VectorSchema, probe_capabilities,
};

const VECTOR: &str = "lexical";

const fn digest(byte: u8) -> Blake3Digest32 {
    Blake3Digest32::from_bytes([byte; 32])
}

fn opaque(text: &str) -> OpaqueId {
    OpaqueId::new(text).expect("fixture identifier")
}

fn epoch(value: i64) -> Epoch {
    Epoch::new(value).expect("fixture epoch")
}

const fn id(byte: u8) -> QdrantPointId {
    QdrantPointId([byte; 16])
}

fn mutation(tag: &str, byte: u8) -> BridgeMutation {
    BridgeMutation {
        operation_id: opaque(tag),
        canonical_input_digest: digest(byte),
    }
}

fn point(byte: u8, weight: f32) -> PointRecord {
    PointRecord {
        point_id: id(byte),
        payload: PointPayload {
            source_membership_id: opaque("member"),
            projection_membership_id: opaque("projection"),
            access_partition_digest: digest(1),
            source_revision: 1,
            unit_ordinal: u64::from(byte),
            valid_from_epoch: epoch(10),
            valid_until_epoch_exclusive: None,
            payload_digest: digest(byte),
            identity_digest: digest(byte),
        },
        vectors: BTreeMap::from([(
            VECTOR.to_owned(),
            StoredVector {
                dimensions: 8,
                sparse: true,
                values: vec![(0, weight)],
                digest: digest(byte),
            },
        )]),
    }
}

fn filter() -> EligibilityFilter {
    EligibilityFilter {
        access_partition_digest: digest(1),
        allowed_source_memberships: BTreeSet::from([opaque("member")]),
        visible_epoch: epoch(42),
    }
}

fn bridge(receipts: usize) -> (QdrantBridge, CollectionRoute) {
    let supervisor = SupervisorReceipt {
        owner_epoch: OwnerEpoch::new(1).expect("fixture owner"),
        process_identity_digest: digest(2),
        artifact_digest: digest(3),
        endpoint_digest: digest(4),
    };
    // These are test-double inputs for the reference bridge, not live probes.
    let capability = probe_capabilities(
        supervisor,
        digest(5),
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
    .expect("reference capability");
    let mut bridge = QdrantBridge::connect(
        BridgeEndpoint {
            endpoint_digest: digest(4),
            loopback_only: true,
        },
        AuthLeaseEvidence {
            reference_digest: digest(6),
            purpose_digest: digest(7),
            valid: true,
        },
        supervisor,
        capability,
        BridgeLimits {
            max_operation_receipts: receipts,
            ..BridgeLimits::BASELINE
        },
    )
    .expect("reference bridge");
    let route = CollectionRoute {
        generation: CollectionGenerationId::from_bytes([1; 16]),
        physical_name: opaque("oracle-contracts"),
    };
    let schema = CollectionSchema {
        named_vectors: BTreeMap::from([(
            VECTOR.to_owned(),
            VectorSchema {
                dimensions: 8,
                sparse: true,
                idf_enabled: true,
            },
        )]),
        indexed_payload_fields: EligibilityFilter::INDEXED_FIELDS
            .into_iter()
            .map(str::to_owned)
            .collect(),
        one_shard: true,
        floors: StrictnessFloors {
            strict_mode: true,
            wait_for_mutations: true,
            strong_ordering: true,
        },
        schema_digest: digest(8),
    };
    bridge.create_candidate_collection(route.clone(), schema).unwrap();
    (bridge, route)
}

fn seeded(receipts: usize) -> (QdrantBridge, CollectionRoute) {
    let (mut bridge, route) = bridge(receipts);
    bridge.upsert_exact(&route, vec![point(1, 1.0)], mutation("seed", 1)).unwrap();
    (bridge, route)
}

fn state(bridge: &QdrantBridge, route: &CollectionRoute) -> Vec<PointRecord> {
    let readback = bridge.readback_exact(route, vec![id(1), id(2)]).unwrap();
    assert!(readback.unexpected_ids.is_empty());
    readback.points
}

#[derive(Clone, Copy, Debug)]
enum Kind {
    Upsert,
    Close,
    Delete,
}

fn apply(
    bridge: &mut QdrantBridge,
    route: &CollectionRoute,
    kind: Kind,
    mutation: BridgeMutation,
) -> Result<MutationReceipt, BridgeError> {
    match kind {
        Kind::Upsert => bridge.upsert_exact(route, vec![point(2, 2.0)], mutation),
        Kind::Close => bridge.close_exact(route, vec![id(1)], epoch(20), mutation),
        Kind::Delete => bridge.delete_exact(route, vec![id(1)], mutation),
    }
}

#[test]
fn full_ledger_rejects_every_mutation_without_changing_points() {
    for kind in [Kind::Upsert, Kind::Close, Kind::Delete] {
        let (mut bridge, route) = seeded(1);
        let before = state(&bridge, &route);
        assert_eq!(
            apply(&mut bridge, &route, kind, mutation("rejected", 2)),
            Err(BridgeError::MutationTooLarge),
            "{kind:?}"
        );
        assert_eq!(state(&bridge, &route), before, "{kind:?}");
    }
}

#[test]
fn full_ledger_still_replays_and_reports_identity_conflicts() {
    for kind in [Kind::Upsert, Kind::Close, Kind::Delete] {
        let (mut bridge, route) = seeded(2);
        let mut expected = apply(&mut bridge, &route, kind, mutation("accepted", 2)).unwrap();
        let before = state(&bridge, &route);
        expected.replayed = true;
        assert_eq!(
            apply(&mut bridge, &route, kind, mutation("accepted", 2)).unwrap(),
            expected
        );
        assert_eq!(
            apply(&mut bridge, &route, kind, mutation("accepted", 3)),
            Err(BridgeError::OperationConflict)
        );
        assert_eq!(state(&bridge, &route), before);
    }
}

#[test]
fn invalid_upsert_batch_neither_writes_nor_consumes_a_receipt() {
    let (mut bridge, route) = seeded(2);
    let before = state(&bridge, &route);
    let mut invalid = point(2, 2.0);
    invalid.vectors.get_mut(VECTOR).unwrap().values = vec![(0, 1.0), (0, 2.0)];
    assert_eq!(
        bridge.upsert_exact(&route, vec![point(1, 9.0), invalid], mutation("retry", 2)),
        Err(BridgeError::VectorDimensionMismatch)
    );
    assert_eq!(state(&bridge, &route), before);
    let receipt = bridge
        .upsert_exact(&route, vec![point(2, 2.0)], mutation("retry", 3))
        .unwrap();
    assert!(!receipt.replayed);
}

#[test]
fn invalid_close_batch_neither_writes_nor_consumes_a_receipt() {
    let (mut bridge, route) = seeded(2);
    let before = state(&bridge, &route);
    assert_eq!(
        bridge.close_exact(&route, vec![id(1), id(2)], epoch(20), mutation("retry", 2)),
        Err(BridgeError::PointNotFound)
    );
    assert_eq!(state(&bridge, &route), before);
    let receipt = bridge
        .close_exact(&route, vec![id(1)], epoch(20), mutation("retry", 3))
        .unwrap();
    assert!(!receipt.replayed);
}

#[test]
fn duplicate_delete_neither_writes_nor_consumes_a_receipt() {
    let (mut bridge, route) = seeded(2);
    let before = state(&bridge, &route);
    assert_eq!(
        bridge.delete_exact(&route, vec![id(1), id(1)], mutation("retry", 2)),
        Err(BridgeError::DuplicatePointId)
    );
    assert_eq!(state(&bridge, &route), before);
    let receipt = bridge
        .delete_exact(&route, vec![id(1)], mutation("retry", 3))
        .unwrap();
    assert!(!receipt.replayed);
}

#[test]
fn public_query_matches_full_sort_with_ties_and_negative_scores() {
    let (mut bridge, route) = bridge(1);
    let points: Vec<_> = (1..=65_u8)
        .map(|byte| point(byte, f32::from(i16::from(byte % 13) - 6)))
        .collect();
    let mut expected: Vec<_> = points
        .iter()
        .map(|point| CandidateNomination {
            point_id: point.point_id,
            score: point.vectors[VECTOR].values[0].1,
            payload_digest: point.payload.payload_digest,
            identity_digest: point.payload.identity_digest,
        })
        .collect();
    expected.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap()
            .then_with(|| left.point_id.cmp(&right.point_id))
    });
    bridge.upsert_exact(&route, points, mutation("population", 1)).unwrap();
    for limit in [1, 2, 7, 31, 64, 65, 70] {
        let actual = bridge.query_filtered(&route, &filter(), VECTOR, &[(0, 1.0)], limit).unwrap();
        assert_eq!(actual, expected[..limit.min(expected.len())]);
    }
    for limit in [0, BridgeLimits::BASELINE.max_query_candidates + 1] {
        assert_eq!(
            bridge.query_filtered(&route, &filter(), VECTOR, &[(0, 1.0)], limit),
            Err(BridgeError::QueryBudgetExceeded)
        );
    }
}

#[test]
fn access_and_epoch_exclusions_happen_before_scoring() {
    let (mut bridge, route) = bridge(1);
    // Every excluded point would overflow if scoring happened before filtering.
    let mut partition = point(2, f32::MAX);
    partition.payload.access_partition_digest = digest(99);
    let mut membership = point(3, f32::MAX);
    membership.payload.source_membership_id = opaque("denied");
    let mut future = point(4, f32::MAX);
    future.payload.valid_from_epoch = epoch(43);
    let mut expired = point(5, f32::MAX);
    expired.payload.valid_until_epoch_exclusive = Some(epoch(42));
    let mut active = point(6, 0.5);
    active.payload.valid_until_epoch_exclusive = Some(epoch(43));
    bridge
        .upsert_exact(
            &route,
            vec![point(1, 1.0), partition, membership, future, expired, active],
            mutation("population", 1),
        )
        .unwrap();
    let actual = bridge.query_filtered(&route, &filter(), VECTOR, &[(0, 2.0)], 2).unwrap();
    let ids: Vec<_> = actual.iter().map(|candidate| candidate.point_id).collect();
    assert_eq!(ids, vec![id(1), id(6)]);
    assert_eq!(bridge.count_exact(&route, &filter()).unwrap().count, 2);
}

#[test]
fn eligible_score_overflow_is_not_hidden_by_a_full_top_k() {
    let (mut bridge, route) = bridge(1);
    bridge
        .upsert_exact(
            &route,
            vec![point(1, 1.0), point(2, f32::MAX)],
            mutation("population", 1),
        )
        .unwrap();
    assert_eq!(
        bridge.query_filtered(&route, &filter(), VECTOR, &[(0, 2.0)], 1),
        Err(BridgeError::InvalidScore)
    );
}

#[test]
fn shared_query_validation_rejects_malformed_vectors() {
    let (bridge, route) = seeded(1);
    let malformed = [
        vec![],
        vec![(0, f32::NAN)],
        vec![(0, f32::INFINITY)],
        vec![(0, f32::NEG_INFINITY)],
        vec![(0, 1.0), (0, 2.0)],
        vec![(1, 1.0), (0, 2.0)],
        vec![(8, 1.0)],
    ];
    for query in malformed {
        assert_eq!(
            bridge.query_filtered(&route, &filter(), VECTOR, &query, 1),
            Err(BridgeError::VectorDimensionMismatch)
        );
    }
}

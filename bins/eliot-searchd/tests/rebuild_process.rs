//! T29 rebuild/reclaim process test: route rebuild from retained manifests,
//! single verified cutover, bounded pins, and ordinary-only reclamation.
//!
//! This harness drives the real composition units over the synchronous
//! in-memory Qdrant oracle: [`rebuild_composition`] planning and gating, the
//! real [`PinRegistry`], and the real `search-index-reclaimer` execution
//! through a thin exact-ID admin adapter below. It never claims live-server
//! proof: live loss, interrupted-cutover restart, and scoring parity stay
//! covered by the reclaimer live suite plus the T24 acceptance obligation.
//! No purge path exists anywhere in this file: ordinary reclaim receipts are
//! checked with [`is_ordinary_reclaim_receipt`] and nothing else is constructible.

#![cfg(feature = "wave3-index")]
#![forbid(unsafe_code)]

#[path = "../src/rebuild_composition.rs"]
mod rebuild_composition;

use std::collections::BTreeMap;

use rebuild_composition::{
    IndexReadbackView, QuerySession, RebuildBudget, RebuildError, ReclaimTuning, RetainedManifest,
    RetainedPoint, authorize_reclaim, begin_pinned_query, commit_cutover,
    expire_continuation_pins_bounded, is_ordinary_reclaim_receipt, propose_rebuild,
    release_owner_pins_of, retained_manifest_digest, stage_cutover, validate_retained_manifest,
    verify_full_readback,
};
use search_contracts::{
    Blake3Digest32, CollectionGenerationId, CollectionRouteRevision, Epoch, OpaqueId, ReceiptRef,
};
use search_epoch_pins::{EpochPinPurpose, PinLimits, PinRegistry, RouteIdentity};
use search_index_reclaimer::{
    AdminDeleteAck, AdminError, AdminMutation, AdminReadback, IndexAdmin, PublicationCommitProof,
    ReclaimPointId, ReclaimReceiptKind, RetiredPointManifest, complete, execute_batch,
};
use search_qdrant_bridge::{
    AuthLeaseEvidence, BridgeEndpoint, BridgeError, BridgeLimits, BridgeMutation,
    CapabilityProbeResults, CollectionRoute, CollectionSchema, ConsistencyGates, EligibilityFilter,
    FilterGates, IndexGates, PointPayload, PointRecord, QdrantBridge, QdrantPointId, StoredVector,
    StrictnessFloors, SupervisorReceipt, TopologyGates, VectorSchema, probe_capabilities,
};

const VECTOR_NAME: &str = "lexical";
const VECTOR_DIMS: u32 = 8;

const fn digest(byte: u8) -> Blake3Digest32 {
    Blake3Digest32::from_bytes([byte; 32])
}

const fn old_route() -> RouteIdentity {
    RouteIdentity {
        collection_generation_id: CollectionGenerationId::from_bytes([0xA1; 16]),
        route_revision: CollectionRouteRevision::new(3),
    }
}

const fn new_generation() -> CollectionGenerationId {
    CollectionGenerationId::from_bytes([0xB2; 16])
}

fn epoch(value: i64) -> Epoch {
    Epoch::new(value).expect("fixture epoch is valid")
}

fn owner(tag: &str) -> OpaqueId {
    OpaqueId::new(tag).expect("fixture owner is valid")
}

fn registry() -> PinRegistry {
    PinRegistry::new(old_route(), epoch(7), PinLimits::BASELINE).expect("fixture registry")
}

const fn tuning() -> ReclaimTuning {
    ReclaimTuning {
        batch_size: 2,
        max_points: 16,
        max_batches: 8,
    }
}

const fn budget() -> RebuildBudget {
    RebuildBudget {
        max_points: 16,
        max_batches: 8,
    }
}

fn test_bridge() -> QdrantBridge {
    let supervisor = SupervisorReceipt {
        owner_epoch: search_contracts::OwnerEpoch::new(1).expect("fixture owner epoch"),
        process_identity_digest: digest(0xB1),
        artifact_digest: digest(0xA2),
        endpoint_digest: digest(0xE3),
    };
    let capability = probe_capabilities(
        supervisor,
        digest(0xC4),
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
    .expect("fixture capability");
    QdrantBridge::connect(
        BridgeEndpoint {
            endpoint_digest: digest(0xE3),
            loopback_only: true,
        },
        AuthLeaseEvidence {
            reference_digest: digest(0x11),
            purpose_digest: digest(0x22),
            valid: true,
        },
        supervisor,
        capability,
        BridgeLimits::BASELINE,
    )
    .expect("fixture bridge")
}

fn test_schema() -> CollectionSchema {
    CollectionSchema {
        named_vectors: BTreeMap::from([(
            VECTOR_NAME.to_owned(),
            VectorSchema {
                dimensions: VECTOR_DIMS,
                sparse: true,
                idf_enabled: true,
            },
        )]),
        indexed_payload_fields: EligibilityFilter::INDEXED_FIELDS
            .iter()
            .map(|field| (*field).to_owned())
            .collect(),
        one_shard: true,
        floors: StrictnessFloors {
            strict_mode: true,
            wait_for_mutations: true,
            strong_ordering: true,
        },
        schema_digest: digest(0xCC),
    }
}

fn bridge_route(name: &str, generation: u8) -> CollectionRoute {
    CollectionRoute {
        generation: CollectionGenerationId::from_bytes([generation; 16]),
        physical_name: OpaqueId::new(name).expect("fixture collection"),
    }
}

fn truth_point(n: u8) -> PointRecord {
    let mut id = [0_u8; 16];
    id[15] = n;
    PointRecord {
        point_id: QdrantPointId(id),
        payload: PointPayload {
            source_membership_id: owner("t29-member-a"),
            projection_membership_id: owner("t29-projection-a"),
            access_partition_digest: digest(0xA1),
            source_revision: u64::from(n),
            unit_ordinal: u64::from(n),
            valid_from_epoch: epoch(1),
            valid_until_epoch_exclusive: None,
            payload_digest: digest(n.wrapping_add(100)),
            identity_digest: digest(n.wrapping_add(200)),
        },
        vectors: BTreeMap::from([(
            VECTOR_NAME.to_owned(),
            StoredVector {
                dimensions: VECTOR_DIMS,
                sparse: true,
                values: vec![(0, 1.0), (u32::from(n) % VECTOR_DIMS, 2.0)],
                digest: digest(n),
            },
        )]),
    }
}

/// Immutable source truth: six projected points with their exact digests.
fn source_truth() -> Vec<PointRecord> {
    vec![
        truth_point(1),
        truth_point(2),
        truth_point(3),
        truth_point(4),
        truth_point(5),
        truth_point(6),
    ]
}

fn retained_manifest(truth: &[PointRecord]) -> RetainedManifest {
    let mut points: Vec<RetainedPoint> = truth
        .iter()
        .map(|point| RetainedPoint {
            id: point.point_id.0,
            payload_digest: point.payload.payload_digest,
            identity_digest: point.payload.identity_digest,
        })
        .collect();
    points.sort();
    let manifest_digest = retained_manifest_digest(&points);
    RetainedManifest {
        generation: CollectionGenerationId::from_bytes([0xA1; 16]),
        route_revision: CollectionRouteRevision::new(3),
        manifest_digest,
        points,
    }
}

fn bridge_mutation(tag: &str, n: u8) -> BridgeMutation {
    BridgeMutation {
        operation_id: owner(tag),
        canonical_input_digest: digest(n),
    }
}

fn upsert_all(
    bridge: &mut QdrantBridge,
    route: &CollectionRoute,
    truth: &[PointRecord],
    tag_prefix: &str,
) {
    for (index, point) in truth.iter().enumerate() {
        let tag = u8::try_from(index).expect("fixture batch fits");
        bridge
            .upsert_exact(
                route,
                vec![point.clone()],
                bridge_mutation(&format!("{tag_prefix}:{index}"), tag),
            )
            .expect("fixture upsert applies");
    }
}

fn view_of(bridge: &QdrantBridge, route: &CollectionRoute, ids: &[[u8; 16]]) -> IndexReadbackView {
    let readback = bridge
        .readback_exact(route, ids.iter().map(|id| QdrantPointId(*id)).collect())
        .expect("fixture readback reads");
    IndexReadbackView {
        present: readback
            .points
            .iter()
            .map(|point| RetainedPoint {
                id: point.point_id.0,
                payload_digest: point.payload.payload_digest,
                identity_digest: point.payload.identity_digest,
            })
            .collect(),
        missing: readback.missing_ids.iter().map(|id| id.0).collect(),
        unexpected: readback.unexpected_ids.iter().map(|id| id.0).collect(),
    }
}

/// Exact-ID admin adapter over the in-memory oracle.
struct BridgeAdmin<'bridge> {
    bridge: &'bridge mut QdrantBridge,
    route: CollectionRoute,
}

impl IndexAdmin for BridgeAdmin<'_> {
    fn delete_exact(
        &mut self,
        batch: &search_index_reclaimer::ReclaimBatch,
        mutation: &AdminMutation,
    ) -> Result<AdminDeleteAck, AdminError> {
        let ids: Vec<QdrantPointId> = batch
            .point_ids
            .iter()
            .map(|id| QdrantPointId(id.0))
            .collect();
        let bridge_mutation = BridgeMutation {
            operation_id: mutation.operation_id.clone(),
            canonical_input_digest: Blake3Digest32::from_bytes(mutation.input_digest),
        };
        match self.bridge.delete_exact(&self.route, ids, bridge_mutation) {
            Ok(receipt) => Ok(AdminDeleteAck {
                operation_id: receipt.operation_id,
                deleted_ids: receipt
                    .affected_ids
                    .into_iter()
                    .map(|id| ReclaimPointId(id.0))
                    .collect(),
                replayed: receipt.replayed,
            }),
            Err(BridgeError::OperationConflict) => Err(AdminError::Conflict),
            Err(
                BridgeError::MutationOutcomeUnknown
                | BridgeError::TransportFailed
                | BridgeError::Cancelled,
            ) => Err(AdminError::Unknown),
            Err(_) => Err(AdminError::Mismatch),
        }
    }

    fn readback_exact(&self, ids: &[ReclaimPointId]) -> Result<AdminReadback, AdminError> {
        let readback = self
            .bridge
            .readback_exact(
                &self.route,
                ids.iter().map(|id| QdrantPointId(id.0)).collect(),
            )
            .map_err(|_| AdminError::Unknown)?;
        Ok(AdminReadback {
            missing_ids: readback
                .missing_ids
                .iter()
                .map(|id| ReclaimPointId(id.0))
                .collect(),
            unexpected_ids: readback
                .unexpected_ids
                .iter()
                .map(|id| ReclaimPointId(id.0))
                .collect(),
        })
    }
}

fn retired_fixture(
    ids: &[[u8; 16]],
    route: RouteIdentity,
    retired_at: i64,
) -> (RetiredPointManifest, PublicationCommitProof) {
    let mut point_ids: Vec<ReclaimPointId> = ids.iter().map(|id| ReclaimPointId(*id)).collect();
    point_ids.sort();
    let manifest = RetiredPointManifest {
        collection_generation_id: route.collection_generation_id,
        route,
        retirement_epoch_exclusive: epoch(retired_at),
        manifest_digest: digest(0xD1),
        publication_receipt_ref: ReceiptRef::new("publication:t29:9").expect("fixture receipt"),
        point_ids,
    };
    let proof = PublicationCommitProof {
        collection_generation_id: route.collection_generation_id,
        route,
        retirement_epoch_exclusive: epoch(retired_at),
        retired_manifest_digest: digest(0xD1),
        publication_receipt_ref: ReceiptRef::new("publication:t29:9").expect("fixture receipt"),
        committed_visible_epoch: epoch(retired_at),
    };
    (manifest, proof)
}

fn manifest_ids(manifest: &RetainedManifest) -> Vec<[u8; 16]> {
    manifest.points.iter().map(|point| point.id).collect()
}

#[test]
fn rebuild_after_loss_cuts_over_once() {
    let truth = source_truth();
    let manifest = retained_manifest(&truth);
    validate_retained_manifest(&manifest).expect("retained manifest validates");
    let direct_before: Vec<Blake3Digest32> = truth
        .iter()
        .map(|point| point.payload.payload_digest)
        .collect();

    let registry = registry();
    let old_query = begin_pinned_query(
        &registry,
        &QuerySession {
            owner: owner("t29:query-old"),
            route: old_route(),
            epoch: epoch(7),
        },
        EpochPinPurpose::Query,
        1_000,
    )
    .expect("old route serves pinned queries before cutover");

    let mut bridge = test_bridge();
    let route_a = bridge_route("t29-gen-a", 0xA1);
    bridge
        .create_candidate_collection(route_a.clone(), test_schema())
        .expect("old generation exists");
    upsert_all(&mut bridge, &route_a, &truth, "t29:initial");

    // Index loss: every point is gone, source truth is untouched.
    let all_ids: Vec<QdrantPointId> = truth.iter().map(|point| point.point_id).collect();
    bridge
        .delete_exact(&route_a, all_ids, bridge_mutation("t29:loss", 0xA0))
        .expect("loss deletes index points only");
    let lost = view_of(&bridge, &route_a, &manifest_ids(&manifest));
    assert_eq!(lost.missing.len(), 6);

    // Rebuild replays the retained manifest into a fresh generation.
    let plan = propose_rebuild(
        old_route(),
        new_generation(),
        CollectionRouteRevision::new(4),
        &manifest,
        2,
        budget(),
    )
    .expect("rebuild plans from retained truth");
    assert_eq!(plan.batches.len(), 3);
    let route_b = bridge_route("t29-gen-b", 0xB2);
    bridge
        .create_candidate_collection(route_b.clone(), test_schema())
        .expect("new generation is created");
    upsert_all(&mut bridge, &route_b, &truth, "t29:rebuild");
    let readback = view_of(&bridge, &route_b, &manifest_ids(&manifest));
    let proof = verify_full_readback(&plan, &manifest, &readback).expect("full readback proves");
    assert_eq!(proof.verified_points, 6);
    let staged = stage_cutover(&plan, &proof).expect("cutover stages");
    let cutover = commit_cutover(&staged, &proof).expect("cutover commits once");
    assert_eq!(cutover.new_revision, CollectionRouteRevision::new(4));
    drop(old_query);

    // Source truth never moved: DIRECT evidence is byte-identical.
    let direct_after: Vec<Blake3Digest32> = truth
        .iter()
        .map(|point| point.payload.payload_digest)
        .collect();
    assert_eq!(direct_before, direct_after);
    validate_retained_manifest(&manifest).expect("retained manifest still validates");
}

#[test]
fn released_pins_unblock_ordinary_reclaim() {
    let truth = source_truth();
    let new_route = RouteIdentity {
        collection_generation_id: new_generation(),
        route_revision: CollectionRouteRevision::new(4),
    };
    let registry =
        PinRegistry::new(new_route, epoch(9), PinLimits::BASELINE).expect("fixture registry");
    let guard = begin_pinned_query(
        &registry,
        &QuerySession {
            owner: owner("t29:query-new"),
            route: new_route,
            epoch: epoch(9),
        },
        EpochPinPurpose::Query,
        1_000,
    )
    .expect("new route serves pinned queries");
    let (retired, proof_ref) =
        retired_fixture(&[truth[4].point_id.0, truth[5].point_id.0], new_route, 10);
    let blocked = authorize_reclaim(
        retired.clone(),
        &proof_ref,
        new_route,
        new_generation(),
        &registry,
        tuning(),
    );
    assert_eq!(blocked, Err(RebuildError::StillPinned));
    drop(guard);

    let reclaim_plan = authorize_reclaim(
        retired,
        &proof_ref,
        new_route,
        new_generation(),
        &registry,
        tuning(),
    )
    .expect("released pins unblock ordinary reclaim");
    assert_eq!(reclaim_plan.batches.len(), 1);
    let mut bridge = test_bridge();
    let route_b = bridge_route("t29-reclaim-b", 0xB2);
    bridge
        .create_candidate_collection(route_b.clone(), test_schema())
        .expect("generation exists");
    upsert_all(&mut bridge, &route_b, &truth[4..], "t29:reclaim");
    let mut admin = BridgeAdmin {
        bridge: &mut bridge,
        route: route_b,
    };
    let mutation = AdminMutation {
        operation_id: reclaim_plan.batches[0].operation_id.clone(),
        input_digest: reclaim_plan.plan_digest.0,
    };
    let batch_receipt =
        execute_batch(&reclaim_plan, 0, &mut admin, &mutation).expect("exact batch executes");
    let receipt = complete(&reclaim_plan, &[batch_receipt]).expect("reclaim completes");
    assert_eq!(
        receipt.kind,
        ReclaimReceiptKind::OrdinaryRetiredPointReclaim
    );
    assert!(is_ordinary_reclaim_receipt(&receipt));
    assert_eq!(receipt.reclaimed_points, 2);
}

#[test]
fn interrupted_cutover_never_commits_without_a_fresh_proof() {
    let truth = source_truth();
    let manifest = retained_manifest(&truth);
    let plan = propose_rebuild(
        old_route(),
        new_generation(),
        CollectionRouteRevision::new(4),
        &manifest,
        4,
        budget(),
    )
    .expect("rebuild plans");
    let mut bridge = test_bridge();
    let route_b = bridge_route("t29-restart-b", 0xB2);
    bridge
        .create_candidate_collection(route_b.clone(), test_schema())
        .expect("new generation is created");
    upsert_all(&mut bridge, &route_b, &truth, "t29:rebuild");
    let readback = view_of(
        &bridge,
        &route_b,
        &manifest
            .points
            .iter()
            .map(|point| point.id)
            .collect::<Vec<_>>(),
    );
    let proof = verify_full_readback(&plan, &manifest, &readback).expect("full readback proves");
    {
        let staged = stage_cutover(&plan, &proof).expect("cutover stages");
        // Interruption: the staged token is lost at the end of this block
        // and never committed. A foreign proof cannot commit it either.
        let foreign_proof = rebuild_composition::FullReadbackProof {
            plan_digest: digest(0xF0),
            verified_points: 6,
        };
        assert_eq!(
            commit_cutover(&staged, &foreign_proof),
            Err(RebuildError::CutoverMismatch)
        );
    }

    // Restart: the same retained truth replays deterministically and the fresh
    // proof commits exactly once.
    let fresh_ids: Vec<[u8; 16]> = manifest.points.iter().map(|point| point.id).collect();
    let manifest_again = retained_manifest(&truth);
    assert_eq!(manifest_again.manifest_digest, manifest.manifest_digest);
    let plan_again = propose_rebuild(
        old_route(),
        new_generation(),
        CollectionRouteRevision::new(4),
        &manifest_again,
        4,
        budget(),
    )
    .expect("restart replays the same plan");
    assert_eq!(plan_again.plan_digest, plan.plan_digest);
    let readback_again = view_of(&bridge, &route_b, &fresh_ids);
    let proof_again =
        verify_full_readback(&plan_again, &manifest_again, &readback_again).expect("re-verify");
    let staged_again = stage_cutover(&plan_again, &proof_again).expect("restage");
    commit_cutover(&staged_again, &proof_again).expect("fresh proof commits");
}

#[test]
fn restart_never_adopts_orphan_backend_state() {
    // A restarted backend is empty: no collection exists to adopt, so the
    // rebuild recreates the generation instead of inferring currentness.
    let truth = source_truth();
    let manifest = retained_manifest(&truth);
    let plan = propose_rebuild(
        old_route(),
        new_generation(),
        CollectionRouteRevision::new(4),
        &manifest,
        8,
        budget(),
    )
    .expect("rebuild plans");
    let mut bridge = test_bridge();
    let route_b = bridge_route("t29-orphan-b", 0xB2);
    let ids: Vec<QdrantPointId> = truth.iter().map(|point| point.point_id).collect();
    assert_eq!(
        bridge.readback_exact(&route_b, ids).map(|_| ()),
        Err(BridgeError::CollectionNotFound)
    );
    bridge
        .create_candidate_collection(route_b.clone(), test_schema())
        .expect("restart recreates the generation");
    upsert_all(&mut bridge, &route_b, &truth, "t29:restart");
    let readback = view_of(
        &bridge,
        &route_b,
        &manifest
            .points
            .iter()
            .map(|point| point.id)
            .collect::<Vec<_>>(),
    );
    let proof = verify_full_readback(&plan, &manifest, &readback).expect("full readback proves");
    assert_eq!(proof.verified_points, 6);
}

#[test]
fn pinned_old_epoch_blocks_reclaim_and_expiry_leaves_no_leak() {
    let registry = registry();
    let _guard = begin_pinned_query(
        &registry,
        &QuerySession {
            owner: owner("t29:continuation"),
            route: old_route(),
            epoch: epoch(6),
        },
        EpochPinPurpose::Continuation {
            expires_at_ms: 2_000,
        },
        1_000,
    )
    .expect("continuation pins within TTL");
    let (retired, proof_ref) = retired_fixture(&[truth_point(1).point_id.0], old_route(), 7);
    let blocked = authorize_reclaim(
        retired.clone(),
        &proof_ref,
        old_route(),
        CollectionGenerationId::from_bytes([0xA1; 16]),
        &registry,
        tuning(),
    );
    assert_eq!(blocked, Err(RebuildError::StillPinned));
    let early = expire_continuation_pins_bounded(&registry, 1_500, 64).expect("bounded sweep");
    assert_eq!(early.expired_pins, 0);
    let late = expire_continuation_pins_bounded(&registry, 2_000, 64).expect("bounded sweep");
    assert_eq!(late.expired_pins, 1);
    assert!(!late.more_expired);
    authorize_reclaim(
        retired,
        &proof_ref,
        old_route(),
        CollectionGenerationId::from_bytes([0xA1; 16]),
        &registry,
        tuning(),
    )
    .expect("expired pins unblock reclaim");
    let snapshot = registry.snapshot().expect("snapshot reads");
    assert_eq!(snapshot.total_pins, 0);
}

#[test]
fn cancelled_pins_release_exactly_their_owner() {
    let registry = registry();
    let guard = begin_pinned_query(
        &registry,
        &QuerySession {
            owner: owner("t29:cancel-me"),
            route: old_route(),
            epoch: epoch(7),
        },
        EpochPinPurpose::Query,
        1_000,
    )
    .expect("query pins");
    let _other = begin_pinned_query(
        &registry,
        &QuerySession {
            owner: owner("t29:stays"),
            route: old_route(),
            epoch: epoch(7),
        },
        EpochPinPurpose::Query,
        1_000,
    )
    .expect("second owner pins");
    let receipt =
        release_owner_pins_of(&registry, &owner("t29:cancel-me")).expect("cancel releases");
    assert_eq!(receipt.released_pins, 1);
    drop(guard);
    // The foreign release disturbed nothing: the surviving owner still pins.
    let snapshot = registry.snapshot().expect("snapshot reads");
    assert_eq!(snapshot.total_pins, 1);
    let again = release_owner_pins_of(&registry, &owner("t29:cancel-me")).expect("idempotent");
    assert_eq!(again.released_pins, 0);
}

#[test]
fn stale_revisions_generations_and_content_are_denied() {
    let truth = source_truth();
    let manifest = retained_manifest(&truth);

    // Skipped revision, recycled generation, and foreign manifest generation.
    assert_eq!(
        propose_rebuild(
            old_route(),
            new_generation(),
            CollectionRouteRevision::new(5),
            &manifest,
            2,
            budget(),
        ),
        Err(RebuildError::StaleRevision)
    );
    assert_eq!(
        propose_rebuild(
            old_route(),
            CollectionGenerationId::from_bytes([0xA1; 16]),
            CollectionRouteRevision::new(4),
            &manifest,
            2,
            budget(),
        ),
        Err(RebuildError::GenerationReuse)
    );
    let mut foreign = manifest.clone();
    foreign.generation = new_generation();
    assert_eq!(
        propose_rebuild(
            old_route(),
            CollectionGenerationId::from_bytes([0xC3; 16]),
            CollectionRouteRevision::new(4),
            &foreign,
            2,
            budget(),
        ),
        Err(RebuildError::GenerationMismatch)
    );

    // Tampered digest and unordered content never plan.
    let mut tampered = manifest.clone();
    tampered.manifest_digest = digest(0xEE);
    assert_eq!(
        propose_rebuild(
            old_route(),
            new_generation(),
            CollectionRouteRevision::new(4),
            &tampered,
            2,
            budget(),
        ),
        Err(RebuildError::DigestMismatch)
    );
    let mut unordered = manifest;
    unordered.points.swap(0, 1);
    assert_eq!(
        validate_retained_manifest(&unordered),
        Err(RebuildError::ManifestNotCanonical)
    );
}

#[test]
fn stale_route_and_foreign_reclaim_are_denied() {
    // A session presenting the pre-cutover route after rotation is stale.
    let registry = registry();
    registry
        .publish_active_route(
            RouteIdentity {
                collection_generation_id: new_generation(),
                route_revision: CollectionRouteRevision::new(4),
            },
            epoch(9),
        )
        .expect("cutover rotates the active route");
    let stale = begin_pinned_query(
        &registry,
        &QuerySession {
            owner: owner("t29:stale-owner"),
            route: old_route(),
            epoch: epoch(7),
        },
        EpochPinPurpose::Query,
        1_100,
    );
    assert_eq!(stale.map(|_| ()), Err(RebuildError::StaleRoute));

    // Reclaim with a foreign caller route or generation is denied.
    let (retired, proof_ref) = retired_fixture(&[truth_point(1).point_id.0], old_route(), 8);
    let foreign_route = RouteIdentity {
        collection_generation_id: new_generation(),
        route_revision: CollectionRouteRevision::new(4),
    };
    assert_eq!(
        authorize_reclaim(
            retired.clone(),
            &proof_ref,
            foreign_route,
            CollectionGenerationId::from_bytes([0xA1; 16]),
            &registry,
            tuning(),
        ),
        Err(RebuildError::StaleRoute)
    );
    assert_eq!(
        authorize_reclaim(
            retired,
            &proof_ref,
            old_route(),
            new_generation(),
            &registry,
            tuning(),
        ),
        Err(RebuildError::GenerationMismatch)
    );

    // A foreign operation identity executes nothing.
    let unpinned =
        PinRegistry::new(old_route(), epoch(7), PinLimits::BASELINE).expect("fixture registry");
    let (retired, proof_ref) = retired_fixture(&[truth_point(2).point_id.0], old_route(), 8);
    let reclaim_plan = authorize_reclaim(
        retired,
        &proof_ref,
        old_route(),
        CollectionGenerationId::from_bytes([0xA1; 16]),
        &unpinned,
        tuning(),
    )
    .expect("unpinned retired point plans");
    let mut bridge = test_bridge();
    let route_a = bridge_route("t29-denied-a", 0xA1);
    bridge
        .create_candidate_collection(route_a.clone(), test_schema())
        .expect("generation exists");
    upsert_all(&mut bridge, &route_a, &[truth_point(2)], "t29:denied");
    let mut admin = BridgeAdmin {
        bridge: &mut bridge,
        route: route_a,
    };
    let foreign = AdminMutation {
        operation_id: owner("reclaim:foreign:0"),
        input_digest: [0xFF; 32],
    };
    assert_eq!(
        execute_batch(&reclaim_plan, 0, &mut admin, &foreign).map(|_| ()),
        Err(search_index_reclaimer::ReclaimError::BatchReceiptMismatch)
    );
}

#[test]
fn partial_readback_never_cuts_over() {
    let truth = source_truth();
    let manifest = retained_manifest(&truth);
    let plan = propose_rebuild(
        old_route(),
        new_generation(),
        CollectionRouteRevision::new(4),
        &manifest,
        8,
        budget(),
    )
    .expect("rebuild plans");
    let mut bridge = test_bridge();
    let route_b = bridge_route("t29-partial-b", 0xB2);
    bridge
        .create_candidate_collection(route_b.clone(), test_schema())
        .expect("generation exists");
    // Only half the points land: the cutover gate must fail closed.
    upsert_all(&mut bridge, &route_b, &truth[..3], "t29:partial");
    let readback = view_of(&bridge, &route_b, &manifest_ids(&manifest));
    assert_eq!(readback.missing.len(), 3);
    assert_eq!(
        verify_full_readback(&plan, &manifest, &readback).map(|_| ()),
        Err(RebuildError::ReadbackMismatch)
    );
}

#[test]
fn ordinary_receipt_cannot_satisfy_purge() {
    let registry = registry();
    let (retired, proof_ref) = retired_fixture(&[truth_point(2).point_id.0], old_route(), 8);
    let reclaim_plan = authorize_reclaim(
        retired,
        &proof_ref,
        old_route(),
        CollectionGenerationId::from_bytes([0xA1; 16]),
        &registry,
        tuning(),
    )
    .expect("unpinned retired point plans");
    let mut bridge = test_bridge();
    let route_a = bridge_route("t29-purge-a", 0xA1);
    bridge
        .create_candidate_collection(route_a.clone(), test_schema())
        .expect("generation exists");
    upsert_all(&mut bridge, &route_a, &[truth_point(2)], "t29:purge");
    let mut admin = BridgeAdmin {
        bridge: &mut bridge,
        route: route_a,
    };
    let mutation = AdminMutation {
        operation_id: reclaim_plan.batches[0].operation_id.clone(),
        input_digest: reclaim_plan.plan_digest.0,
    };
    let batch_receipt =
        execute_batch(&reclaim_plan, 0, &mut admin, &mutation).expect("exact batch executes");
    let receipt = complete(&reclaim_plan, &[batch_receipt]).expect("reclaim completes");
    assert_eq!(
        receipt.kind,
        ReclaimReceiptKind::OrdinaryRetiredPointReclaim
    );
    assert!(is_ordinary_reclaim_receipt(&receipt));
}

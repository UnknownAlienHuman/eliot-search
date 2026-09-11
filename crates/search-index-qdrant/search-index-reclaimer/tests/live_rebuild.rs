//! T29 live rebuild/reclaim: disposable Qdrant, real transport, exact readback.
//!
//! Every test spawns the exact qualified native server on disposable storage
//! with OS-assigned loopback ports, admits it through the executed T22
//! qualification suite, then drives loss, rebuild, pin-gated planning, and
//! exact-ID execution through the real `qdrant-client` 1.19.0 transport in
//! `search_qdrant_bridge::real`. Fail-closed: any transport or probe failure
//! fails the test. The synchronous [`IndexAdmin`] sequencing is proven by the
//! `execute` unit suite plus the daemon `rebuild_process` oracle adapter; this
//! suite proves the same receipts against live bytes, including restart with
//! an empty backend and a genuine ledger identity conflict.
//!
//! Run: `cargo test -p search-index-reclaimer --test live_rebuild`.
//! The server child is killed and its storage removed on drop; no test leaves
//! an orphan `qdrant` process or temp storage behind.

#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::time::Duration;

use search_contracts::{Blake3Digest32, CollectionGenerationId, Epoch, OpaqueId, ReceiptRef};
use search_epoch_pins::{EpochPinPurpose, PinLimits, PinRegistry, RetiredVisibilityFence};
use search_index_reclaimer::{
    AdminDeleteAck, AdminError, AdminMutation, AdminReadback, IndexAdmin, PublicationCommitProof,
    ReclaimBatch, ReclaimBudget, ReclaimPointId, ReclaimReceiptKind, ReclaimSettings,
    RetiredPointManifest, checkpoint, complete, execute_batch, plan, resume,
    validate_retired_manifest,
};
use search_qdrant_bridge::live::{
    NATIVE_EXE_PATH, free_loopback_ports, run_qualification_suite, spawn_disposable_server,
};
use search_qdrant_bridge::qualified::QualifiedGate;
use search_qdrant_bridge::real::{OpContext, RealDataPlane};
use search_qdrant_bridge::{
    BridgeError, BridgeLimits, BridgeMutation, CollectionRoute, CollectionSchema,
    EligibilityFilter, PointPayload, PointRecord, QdrantPointId, StoredVector, StrictnessFloors,
    VectorSchema,
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

fn make_route(name: &str, generation: u8) -> CollectionRoute {
    CollectionRoute {
        generation: CollectionGenerationId::from_bytes([generation; 16]),
        physical_name: OpaqueId::new(name).expect("physical name"),
    }
}

const fn pin_route(generation: u8, rev: u64) -> search_epoch_pins::RouteIdentity {
    search_epoch_pins::RouteIdentity {
        collection_generation_id: CollectionGenerationId::from_bytes([generation; 16]),
        route_revision: search_contracts::CollectionRouteRevision::new(rev),
    }
}

const fn partition(byte: u8) -> Blake3Digest32 {
    Blake3Digest32::from_bytes([byte; 32])
}

fn member(name: &str) -> OpaqueId {
    OpaqueId::new(name).expect("member")
}

const fn point_id(n: u8) -> QdrantPointId {
    let mut bytes = [0_u8; 16];
    bytes[15] = n;
    QdrantPointId(bytes)
}

fn live_point(n: u8) -> PointRecord {
    let mut vectors = BTreeMap::new();
    vectors.insert(
        VECTOR_NAME.to_owned(),
        StoredVector {
            dimensions: VECTOR_DIMS,
            sparse: true,
            values: vec![(0, f32::from(n)), (u32::from(n) + 1, 1.0)],
            digest: Blake3Digest32::from_bytes([n; 32]),
        },
    );
    PointRecord {
        point_id: point_id(n),
        payload: PointPayload {
            source_membership_id: member("t29-member-a"),
            projection_membership_id: member("t29-proj-a"),
            access_partition_digest: partition(0xA1),
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

fn bridge_mutation(tag: &str, n: u8) -> BridgeMutation {
    BridgeMutation {
        operation_id: OpaqueId::new(tag).expect("operation id"),
        canonical_input_digest: Blake3Digest32::from_bytes([n; 32]),
    }
}

/// Spawns a disposable server, runs the full T22 qualification suite, admits
/// the live gate, and connects the real data plane. The returned server must
/// stay alive for the whole test (`Drop` kills it and removes storage).
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

/// Runs a future to completion from the synchronous [`IndexAdmin`] port.
///
/// The live tests run on the multi-thread runtime, so blocking the caller's
/// worker inside `block_in_place` and driving the transport future on it is
/// the documented pattern; no second runtime is created.
fn block<T>(future: impl Future<Output = T>) -> T {
    tokio::task::block_in_place(|| tokio::runtime::Handle::current().block_on(future))
}

/// Exact-ID admin adapter over the real data plane, bound to one route.
struct LiveAdmin {
    plane: RealDataPlane,
    route: CollectionRoute,
}

impl IndexAdmin for LiveAdmin {
    fn delete_exact(
        &mut self,
        batch: &ReclaimBatch,
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
        let plane = &mut self.plane;
        let route = &self.route;
        match block(plane.delete_exact(route, ids, bridge_mutation, &ctx())) {
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
        // `readback_exact` borrows immutably, but `block` needs no aliasing
        // guard beyond the shared borrow the trait already holds.
        let plane = &self.plane;
        let route = &self.route;
        let vendor: Vec<QdrantPointId> = ids.iter().map(|id| QdrantPointId(id.0)).collect();
        match block(plane.readback_exact(route, vendor, &ctx())) {
            Ok(readback) => Ok(AdminReadback {
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
            }),
            Err(_) => Err(AdminError::Unknown),
        }
    }
}

fn retired_fixture(
    ids: &[QdrantPointId],
    generation: u8,
    rev: u64,
    retired_at: i64,
) -> (RetiredPointManifest, PublicationCommitProof) {
    let mut point_ids: Vec<ReclaimPointId> = ids.iter().map(|id| ReclaimPointId(id.0)).collect();
    point_ids.sort();
    let route = pin_route(generation, rev);
    let manifest = RetiredPointManifest {
        collection_generation_id: CollectionGenerationId::from_bytes([generation; 16]),
        route,
        retirement_epoch_exclusive: Epoch::new(retired_at).expect("retirement"),
        manifest_digest: Blake3Digest32::from_bytes([0xD1; 32]),
        publication_receipt_ref: ReceiptRef::new("publication:t29:live").expect("receipt"),
        point_ids,
    };
    let proof = PublicationCommitProof {
        collection_generation_id: CollectionGenerationId::from_bytes([generation; 16]),
        route,
        retirement_epoch_exclusive: Epoch::new(retired_at).expect("retirement"),
        retired_manifest_digest: Blake3Digest32::from_bytes([0xD1; 32]),
        publication_receipt_ref: ReceiptRef::new("publication:t29:live").expect("receipt"),
        committed_visible_epoch: Epoch::new(retired_at).expect("visible"),
    };
    (manifest, proof)
}

const fn tuning() -> (ReclaimSettings, ReclaimBudget) {
    (
        ReclaimSettings { batch_size: 2 },
        ReclaimBudget {
            max_points: 16,
            max_batches: 8,
        },
    )
}

/// Upserts retained truth points one by one with distinct live identities.
async fn upsert_truth(
    plane: &mut RealDataPlane,
    route: &CollectionRoute,
    truth: &[PointRecord],
    prefix: &str,
    base: u8,
) {
    for (index, point) in truth.iter().enumerate() {
        let tag = format!("{prefix}:{index}");
        let tag_byte = base.wrapping_add(u8::try_from(index).expect("fixture index fits"));
        plane
            .upsert_exact(
                route,
                vec![point.clone()],
                bridge_mutation(&tag, tag_byte),
                &ctx(),
            )
            .await
            .expect("truth upsert applies");
    }
}

/// Asserts a live readback holds exactly the retained truth with matching
/// digests and nothing unexpected.
fn assert_full_match(readback: &search_qdrant_bridge::BoundedPointReadback, truth: &[PointRecord]) {
    assert!(readback.missing_ids.is_empty());
    assert!(readback.unexpected_ids.is_empty());
    assert_eq!(readback.points.len(), truth.len());
    for point in truth {
        let found = readback
            .points
            .iter()
            .find(|found| found.point_id == point.point_id)
            .expect("every retained point is back");
        assert_eq!(found.payload.payload_digest, point.payload.payload_digest);
        assert_eq!(found.payload.identity_digest, point.payload.identity_digest);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn live_loss_rebuild_full_readback_then_reclaim() {
    let (_server, mut plane) = live_plane().await;
    let route_a = make_route("t29-live-a", 0xA1);
    let route_b = make_route("t29-live-b", 0xB2);
    // Retained source truth lives outside the index: loss can never reach it.
    let truth: Vec<PointRecord> = vec![live_point(1), live_point(2), live_point(3), live_point(4)];
    let truth_digests: Vec<Blake3Digest32> = truth
        .iter()
        .map(|point| point.payload.payload_digest)
        .collect();

    plane
        .create_collection(&route_a, &schema(), &ctx())
        .await
        .expect("old generation is created");
    upsert_truth(&mut plane, &route_a, &truth, "t29:live:initial", 0).await;

    // Index loss: every point is gone from the backend only.
    let all_ids: Vec<QdrantPointId> = truth.iter().map(|point| point.point_id).collect();
    plane
        .delete_exact(
            &route_a,
            all_ids.clone(),
            bridge_mutation("t29:live:loss", 0xA0),
            &ctx(),
        )
        .await
        .expect("loss deletes index points only");
    let lost = plane
        .readback_exact(&route_a, all_ids.clone(), &ctx())
        .await
        .expect("loss readback reads");
    assert_eq!(lost.missing_ids.len(), 4);
    assert!(lost.unexpected_ids.is_empty());

    // Rebuild replays the retained truth into a fresh generation and proves
    // every point with matching digests before anything cuts over.
    plane
        .create_collection(&route_b, &schema(), &ctx())
        .await
        .expect("new generation is created");
    upsert_truth(&mut plane, &route_b, &truth, "t29:live:rebuild", 0x10).await;
    let rebuilt = plane
        .readback_exact(&route_b, all_ids.clone(), &ctx())
        .await
        .expect("rebuild readback reads");
    assert_full_match(&rebuilt, &truth);

    // Pin-gated ordinary reclaim of two retired points on the new route.
    let registry = PinRegistry::new(
        pin_route(0xB2, 4),
        Epoch::new(9).expect("visible"),
        PinLimits::BASELINE,
    )
    .expect("fixture registry");
    let (manifest, proof) = retired_fixture(&all_ids[..2], 0xB2, 4, 10);
    let committed = validate_retired_manifest(manifest, &proof).expect("committed manifest");
    let snapshot = registry.snapshot().expect("snapshot reads");
    let watermark = search_epoch_pins::compute_reclamation_watermark(
        RetiredVisibilityFence {
            route: pin_route(0xB2, 4),
            retirement_epoch_exclusive: Epoch::new(10).expect("retirement"),
        },
        &snapshot,
    );
    assert!(watermark.reclaimable);
    let (settings, budget) = tuning();
    let reclaim_plan = plan(committed, watermark, settings, budget).expect("reclaim plans");
    let mut admin = LiveAdmin {
        plane,
        route: route_b.clone(),
    };
    let mutation = AdminMutation {
        operation_id: reclaim_plan.batches[0].operation_id.clone(),
        input_digest: reclaim_plan.plan_digest.0,
    };
    let batch_receipt =
        execute_batch(&reclaim_plan, 0, &mut admin, &mutation).expect("live exact batch executes");
    let receipt = complete(&reclaim_plan, &[batch_receipt]).expect("reclaim completes");
    assert_eq!(
        receipt.kind,
        ReclaimReceiptKind::OrdinaryRetiredPointReclaim
    );
    assert_eq!(receipt.reclaimed_points, 2);

    // Source truth never moved.
    let after: Vec<Blake3Digest32> = truth
        .iter()
        .map(|point| point.payload.payload_digest)
        .collect();
    assert_eq!(truth_digests, after);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn live_pinned_old_epoch_blocks_reclaim_until_release() {
    let (_server, mut plane) = live_plane().await;
    let route_b = make_route("t29-live-pinned", 0xB2);
    plane
        .create_collection(&route_b, &schema(), &ctx())
        .await
        .expect("generation is created");
    let points = [live_point(11), live_point(12)];
    for (index, point) in points.iter().enumerate() {
        let tag = format!("t29:live:pinned:{index}");
        let tag_byte = u8::try_from(0x20 + index).expect("fixture index fits");
        plane
            .upsert_exact(
                &route_b,
                vec![point.clone()],
                bridge_mutation(&tag, tag_byte),
                &ctx(),
            )
            .await
            .expect("upsert applies");
    }
    let ids: Vec<QdrantPointId> = points.iter().map(|point| point.point_id).collect();

    let registry = PinRegistry::new(
        pin_route(0xB2, 4),
        Epoch::new(9).expect("visible"),
        PinLimits::BASELINE,
    )
    .expect("fixture registry");
    let guard = registry
        .acquire_epoch_pin(
            pin_route(0xB2, 4),
            Epoch::new(9).expect("visible"),
            OpaqueId::new("t29:live-reader").expect("owner"),
            EpochPinPurpose::Query,
            1_000,
        )
        .expect("live query pins the visible epoch");
    let (manifest, proof) = retired_fixture(&ids, 0xB2, 4, 10);
    let committed = validate_retired_manifest(manifest, &proof).expect("committed manifest");
    let (settings, budget) = tuning();
    let blocked_snapshot = registry.snapshot().expect("snapshot reads");
    let blocked = search_epoch_pins::compute_reclamation_watermark(
        RetiredVisibilityFence {
            route: pin_route(0xB2, 4),
            retirement_epoch_exclusive: Epoch::new(10).expect("retirement"),
        },
        &blocked_snapshot,
    );
    assert!(!blocked.reclaimable);
    assert_eq!(
        plan(committed.clone(), blocked, settings, budget).map(|_| ()),
        Err(search_index_reclaimer::ReclaimError::StillPinned)
    );
    drop(guard);

    let free_snapshot = registry.snapshot().expect("snapshot reads");
    let free = search_epoch_pins::compute_reclamation_watermark(
        RetiredVisibilityFence {
            route: pin_route(0xB2, 4),
            retirement_epoch_exclusive: Epoch::new(10).expect("retirement"),
        },
        &free_snapshot,
    );
    let reclaim_plan = plan(committed, free, settings, budget).expect("release unblocks");
    let mut admin = LiveAdmin {
        plane,
        route: route_b,
    };
    let mutation = AdminMutation {
        operation_id: reclaim_plan.batches[0].operation_id.clone(),
        input_digest: reclaim_plan.plan_digest.0,
    };
    let batch_receipt =
        execute_batch(&reclaim_plan, 0, &mut admin, &mutation).expect("live exact batch executes");
    let checkpoint = checkpoint(&reclaim_plan, vec![batch_receipt.clone()]).expect("checkpoint");
    let remaining = resume(&checkpoint, &reclaim_plan, free).expect("resume reads");
    assert!(
        remaining.is_empty(),
        "verified batch is skipped, not replayed"
    );
    complete(&reclaim_plan, &[batch_receipt]).expect("reclaim completes");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn live_interrupted_cutover_restart_rebuilds_from_truth() {
    let (server, mut plane) = live_plane().await;
    let route_b = make_route("t29-live-restart", 0xB2);
    let truth = vec![live_point(21), live_point(22)];
    plane
        .create_collection(&route_b, &schema(), &ctx())
        .await
        .expect("generation is created");
    plane
        .upsert_exact(
            &route_b,
            truth.clone(),
            bridge_mutation("t29:live:pre-crash", 0x30),
            &ctx(),
        )
        .await
        .expect("pre-crash upsert applies");

    // Crash: the whole backend is gone. Only the retained truth survives, in
    // the caller's memory, never in the dead server.
    drop(plane);
    drop(server);

    let (_server2, mut plane2) = live_plane().await;
    let ids: Vec<QdrantPointId> = truth.iter().map(|point| point.point_id).collect();
    // The restarted backend is empty: orphan state is never adopted.
    assert_eq!(
        plane2
            .readback_exact(&route_b, ids.clone(), &ctx())
            .await
            .map(|_| ()),
        Err(BridgeError::CollectionNotFound)
    );
    plane2
        .create_collection(&route_b, &schema(), &ctx())
        .await
        .expect("restart recreates the generation");
    plane2
        .upsert_exact(
            &route_b,
            truth.clone(),
            bridge_mutation("t29:live:rebuilt", 0x31),
            &ctx(),
        )
        .await
        .expect("rebuild replays retained truth");
    let rebuilt = plane2
        .readback_exact(&route_b, ids, &ctx())
        .await
        .expect("rebuild readback reads");
    assert!(rebuilt.missing_ids.is_empty());
    assert!(rebuilt.unexpected_ids.is_empty());
    for point in &truth {
        let found = rebuilt
            .points
            .iter()
            .find(|found| found.point_id == point.point_id)
            .expect("every retained point is back");
        assert_eq!(found.payload.payload_digest, point.payload.payload_digest);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn live_stale_generation_and_identity_conflict_are_denied() {
    let (_server, mut plane) = live_plane().await;
    let route_b = make_route("t29-live-denied", 0xB2);
    plane
        .create_collection(&route_b, &schema(), &ctx())
        .await
        .expect("generation is created");
    let point = live_point(31);
    plane
        .upsert_exact(
            &route_b,
            vec![point.clone()],
            bridge_mutation("t29:live:op", 0x40),
            &ctx(),
        )
        .await
        .expect("upsert applies");

    // Same operation identity with a different input digest is a genuine live
    // ledger conflict: denied, never applied.
    let conflicting = BridgeMutation {
        operation_id: OpaqueId::new("t29:live:op").expect("operation id"),
        canonical_input_digest: Blake3Digest32::from_bytes([0xEE; 32]),
    };
    assert_eq!(
        plane
            .upsert_exact(&route_b, vec![point.clone()], conflicting, &ctx())
            .await
            .map(|_| ()),
        Err(BridgeError::OperationConflict)
    );

    // An out-of-generation route names another generation's collection: the
    // live backend reports not-found instead of foreign data.
    let foreign = make_route("t29-live-denied", 0xC3);
    assert_eq!(
        plane
            .readback_exact(&foreign, vec![point.point_id], &ctx())
            .await
            .map(|_| ()),
        Err(BridgeError::CollectionNotFound)
    );
    assert_eq!(
        plane
            .delete_exact(
                &foreign,
                vec![point.point_id],
                bridge_mutation("t29:live:foreign-delete", 0x41),
                &ctx(),
            )
            .await
            .map(|_| ()),
        Err(BridgeError::CollectionNotFound)
    );

    // The admitted point itself is untouched by both denials.
    let intact = plane
        .readback_exact(&route_b, vec![point.point_id], &ctx())
        .await
        .expect("admitted point reads back");
    assert!(intact.missing_ids.is_empty());
    assert_eq!(
        intact.points[0].payload.payload_digest,
        point.payload.payload_digest
    );
}

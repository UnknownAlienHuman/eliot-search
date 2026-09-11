//! T29 exact-execution discriminating tests: only committed, unpinned retired
//! points are deleted by exact identifier, and every ambiguous outcome stays
//! unknown until an exact readback resolves it.

#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};

use search_contracts::{
    Blake3Digest32, CollectionGenerationId, CollectionRouteRevision, Epoch, OpaqueId, ReceiptRef,
};
use search_epoch_pins::{PinLimits, PinRegistry, RouteIdentity};
use search_index_reclaimer::{
    AdminDeleteAck, AdminError, AdminMutation, AdminReadback, CommittedRetiredManifest, IndexAdmin,
    PublicationCommitProof, ReclaimBatch, ReclaimBudget, ReclaimError, ReclaimPointId,
    ReclaimReceiptKind, ReclaimSettings, RetiredPointManifest, checkpoint, complete, execute_batch,
    plan, resume, validate_retired_manifest, verify_batch_receipt,
};

const fn route(generation: u8, rev: u64) -> RouteIdentity {
    RouteIdentity {
        collection_generation_id: CollectionGenerationId::from_bytes([generation; 16]),
        route_revision: CollectionRouteRevision::new(rev),
    }
}

fn epoch(value: i64) -> Epoch {
    Epoch::new(value).expect("fixture epoch is valid")
}

const fn point(n: u8) -> ReclaimPointId {
    let mut bytes = [0_u8; 16];
    bytes[15] = n;
    ReclaimPointId(bytes)
}

fn receipt_ref(tag: &str) -> ReceiptRef {
    ReceiptRef::new(tag).expect("fixture receipt ref is valid")
}

fn manifest(ids: Vec<ReclaimPointId>) -> (RetiredPointManifest, PublicationCommitProof) {
    let manifest = RetiredPointManifest {
        collection_generation_id: CollectionGenerationId::from_bytes([0xA1; 16]),
        route: route(0xA1, 3),
        retirement_epoch_exclusive: epoch(8),
        manifest_digest: Blake3Digest32::from_bytes([0xD1; 32]),
        publication_receipt_ref: receipt_ref("publication:t29:7"),
        point_ids: ids,
    };
    let proof = PublicationCommitProof {
        collection_generation_id: CollectionGenerationId::from_bytes([0xA1; 16]),
        route: route(0xA1, 3),
        retirement_epoch_exclusive: epoch(8),
        retired_manifest_digest: Blake3Digest32::from_bytes([0xD1; 32]),
        publication_receipt_ref: receipt_ref("publication:t29:7"),
        committed_visible_epoch: epoch(8),
    };
    (manifest, proof)
}

fn committed(ids: Vec<ReclaimPointId>) -> CommittedRetiredManifest {
    let (manifest, proof) = manifest(ids);
    validate_retired_manifest(manifest, &proof).expect("fixture manifest commits")
}

fn unpinned_watermark(manifest_route: RouteIdentity) -> search_epoch_pins::ReclamationWatermark {
    let registry =
        PinRegistry::new(manifest_route, epoch(8), PinLimits::BASELINE).expect("fixture registry");
    let snapshot = registry.snapshot().expect("snapshot reads");
    search_epoch_pins::compute_reclamation_watermark(
        search_epoch_pins::RetiredVisibilityFence {
            route: manifest_route,
            retirement_epoch_exclusive: epoch(8),
        },
        &snapshot,
    )
}

const fn settings() -> (ReclaimSettings, ReclaimBudget) {
    (
        ReclaimSettings { batch_size: 2 },
        ReclaimBudget {
            max_points: 16,
            max_batches: 8,
        },
    )
}

/// Vendor-neutral fake index admin: an exact-ID set plus a mutation-identity
/// ledger. Same identity with same IDs replays; same identity with different
/// IDs is a fail-closed conflict, mirroring the real data plane.
struct FakeAdmin {
    store: BTreeSet<ReclaimPointId>,
    ledger: BTreeMap<OpaqueId, Vec<ReclaimPointId>>,
    contradict_readback: bool,
}

impl FakeAdmin {
    fn seeded(ids: &[ReclaimPointId]) -> Self {
        Self {
            store: ids.iter().copied().collect(),
            ledger: BTreeMap::new(),
            contradict_readback: false,
        }
    }
}

impl IndexAdmin for FakeAdmin {
    fn delete_exact(
        &mut self,
        batch: &ReclaimBatch,
        mutation: &AdminMutation,
    ) -> Result<AdminDeleteAck, AdminError> {
        if let Some(previous) = self.ledger.get(&mutation.operation_id) {
            if *previous != batch.point_ids {
                return Err(AdminError::Conflict);
            }
            return Ok(AdminDeleteAck {
                operation_id: mutation.operation_id.clone(),
                deleted_ids: batch.point_ids.clone(),
                replayed: true,
            });
        }
        for id in &batch.point_ids {
            self.store.remove(id);
        }
        self.ledger
            .insert(mutation.operation_id.clone(), batch.point_ids.clone());
        Ok(AdminDeleteAck {
            operation_id: mutation.operation_id.clone(),
            deleted_ids: batch.point_ids.clone(),
            replayed: false,
        })
    }

    fn readback_exact(&self, ids: &[ReclaimPointId]) -> Result<AdminReadback, AdminError> {
        let mut missing: Vec<ReclaimPointId> = ids
            .iter()
            .copied()
            .filter(|id| !self.store.contains(id))
            .collect();
        if self.contradict_readback && !missing.is_empty() {
            missing.pop();
        }
        Ok(AdminReadback {
            missing_ids: missing,
            unexpected_ids: Vec::new(),
        })
    }
}

fn mutation_for(plan: &search_index_reclaimer::ReclaimPlan, batch: usize) -> AdminMutation {
    AdminMutation {
        operation_id: plan.batches[batch].operation_id.clone(),
        input_digest: plan.plan_digest.0,
    }
}

/// Admin that always reports an ambiguous delete. When `commit_before_loss`
/// is set, the write applied before the transport loss, so the resolving
/// readback observes absence; otherwise it observes a residual.
struct AmbiguousAdmin {
    inner: FakeAdmin,
    commit_before_loss: bool,
}

impl IndexAdmin for AmbiguousAdmin {
    fn delete_exact(
        &mut self,
        batch: &ReclaimBatch,
        _mutation: &AdminMutation,
    ) -> Result<AdminDeleteAck, AdminError> {
        if self.commit_before_loss {
            for id in &batch.point_ids {
                self.inner.store.remove(id);
            }
        }
        Err(AdminError::Unknown)
    }
    fn readback_exact(&self, ids: &[ReclaimPointId]) -> Result<AdminReadback, AdminError> {
        self.inner.readback_exact(ids)
    }
}

#[test]
fn executes_exact_ids_only_and_completes() {
    let committed = committed(vec![point(1), point(2), point(3), point(9)]);
    let (settings, budget) = settings();
    let plan = plan(
        committed,
        unpinned_watermark(route(0xA1, 3)),
        settings,
        budget,
    )
    .expect("unpinned manifest plans");
    assert_eq!(plan.batches.len(), 2);
    let mut admin = FakeAdmin::seeded(&[point(1), point(2), point(3), point(9)]);
    let first =
        execute_batch(&plan, 0, &mut admin, &mutation_for(&plan, 0)).expect("first batch executes");
    verify_batch_receipt(&plan, &first).expect("first receipt verifies");
    // Only the first batch left the store; the second batch is untouched.
    assert!(admin.store.contains(&point(3)));
    assert!(admin.store.contains(&point(9)));
    let second = execute_batch(&plan, 1, &mut admin, &mutation_for(&plan, 1))
        .expect("second batch executes");
    let receipt = complete(&plan, &[first, second]).expect("all absent completes");
    assert_eq!(
        receipt.kind,
        ReclaimReceiptKind::OrdinaryRetiredPointReclaim
    );
    assert_eq!(receipt.reclaimed_points, 4);
    assert!(admin.store.is_empty());
}

#[test]
fn unknown_delete_resolved_by_readback_when_absent() {
    let committed = committed(vec![point(1), point(2)]);
    let (settings, budget) = settings();
    let plan = plan(
        committed,
        unpinned_watermark(route(0xA1, 3)),
        settings,
        budget,
    )
    .expect("unpinned manifest plans");
    let mut admin = AmbiguousAdmin {
        inner: FakeAdmin::seeded(&[point(1), point(2)]),
        commit_before_loss: true,
    };
    let receipt = execute_batch(&plan, 0, &mut admin, &mutation_for(&plan, 0))
        .expect("unknown resolves to complete when readback proves absence");
    assert!(receipt.missing_ids.contains(&point(1)));
}

#[test]
fn unknown_delete_with_residual_points_stays_unknown() {
    let committed = committed(vec![point(1), point(2)]);
    let (settings, budget) = settings();
    let plan = plan(
        committed,
        unpinned_watermark(route(0xA1, 3)),
        settings,
        budget,
    )
    .expect("unpinned manifest plans");
    // The delete reports unknown while point 2 is still present: the outcome
    // must stay unknown, never success.
    let mut residual = AmbiguousAdmin {
        inner: FakeAdmin::seeded(&[point(1), point(2)]),
        commit_before_loss: false,
    };
    let outcome = execute_batch(&plan, 0, &mut residual, &mutation_for(&plan, 0));
    assert_eq!(outcome, Err(ReclaimError::BatchOutcomeUnknown));
}

#[test]
fn ack_contradicted_by_readback_is_rejected() {
    let committed = committed(vec![point(1), point(2)]);
    let (settings, budget) = settings();
    let plan = plan(
        committed,
        unpinned_watermark(route(0xA1, 3)),
        settings,
        budget,
    )
    .expect("unpinned manifest plans");
    let mut admin = FakeAdmin::seeded(&[point(1), point(2)]);
    admin.contradict_readback = true;
    let outcome = execute_batch(&plan, 0, &mut admin, &mutation_for(&plan, 0));
    assert_eq!(outcome, Err(ReclaimError::UnexpectedReadback));
}

#[test]
fn same_identity_with_different_ids_is_rejected() {
    let committed = committed(vec![point(1), point(2)]);
    let (settings, budget) = settings();
    let plan = plan(
        committed,
        unpinned_watermark(route(0xA1, 3)),
        settings,
        budget,
    )
    .expect("unpinned manifest plans");
    let mut admin = FakeAdmin::seeded(&[point(1), point(2)]);
    let mutation = mutation_for(&plan, 0);
    execute_batch(&plan, 0, &mut admin, &mutation).expect("first execution applies");
    // Replay the same identity against a tampered batch: the ledger conflict
    // must fail closed, never delete the tampered set.
    let mut tampered = plan.batches[0].clone();
    tampered.point_ids = vec![point(7)];
    let outcome = admin.delete_exact(&tampered, &mutation);
    assert_eq!(outcome, Err(AdminError::Conflict));
}

#[test]
fn foreign_operation_identity_is_rejected() {
    let committed = committed(vec![point(1), point(2)]);
    let (settings, budget) = settings();
    let plan = plan(
        committed,
        unpinned_watermark(route(0xA1, 3)),
        settings,
        budget,
    )
    .expect("unpinned manifest plans");
    let mut admin = FakeAdmin::seeded(&[point(1), point(2)]);
    let foreign = AdminMutation {
        operation_id: OpaqueId::new("reclaim:foreign:0").expect("fixture identity"),
        input_digest: [0xFF; 32],
    };
    let outcome = execute_batch(&plan, 0, &mut admin, &foreign);
    assert_eq!(outcome, Err(ReclaimError::BatchReceiptMismatch));
    assert!(admin.store.contains(&point(1)));
}

#[test]
fn unknown_batch_index_is_rejected() {
    let committed = committed(vec![point(1)]);
    let plan = plan(
        committed,
        unpinned_watermark(route(0xA1, 3)),
        ReclaimSettings { batch_size: 2 },
        ReclaimBudget {
            max_points: 16,
            max_batches: 8,
        },
    )
    .expect("unpinned manifest plans");
    let mut admin = FakeAdmin::seeded(&[point(1)]);
    let mutation = AdminMutation {
        operation_id: OpaqueId::new("reclaim:nope:9").expect("fixture identity"),
        input_digest: [0; 32],
    };
    let outcome = execute_batch(&plan, 9, &mut admin, &mutation);
    assert_eq!(outcome, Err(ReclaimError::BatchNotFound));
}

#[test]
fn pinned_manifest_never_plans() {
    let registry =
        PinRegistry::new(route(0xA1, 3), epoch(8), PinLimits::BASELINE).expect("fixture registry");
    let _pin = registry
        .acquire_route_pin(route(0xA1, 3), OpaqueId::new("t29:reader").expect("owner"))
        .expect("route pins");
    let snapshot = registry.snapshot().expect("snapshot reads");
    let watermark = search_epoch_pins::compute_reclamation_watermark(
        search_epoch_pins::RetiredVisibilityFence {
            route: route(0xA1, 3),
            retirement_epoch_exclusive: epoch(8),
        },
        &snapshot,
    );
    assert!(!watermark.reclaimable);
    let (settings, budget) = settings();
    let outcome = plan(committed(vec![point(1)]), watermark, settings, budget);
    assert_eq!(outcome, Err(ReclaimError::StillPinned));
}

#[test]
fn uncommitted_manifest_is_rejected() {
    let (mut tampered, _) = manifest(vec![point(1)]);
    tampered.manifest_digest = Blake3Digest32::from_bytes([0xEE; 32]);
    let (_, proof) = manifest(vec![point(1)]);
    let outcome = validate_retired_manifest(tampered, &proof);
    assert_eq!(outcome, Err(ReclaimError::PublicationMismatch));
}

#[test]
fn checkpoint_resume_skips_verified_batches() {
    let committed = committed(vec![point(1), point(2), point(3)]);
    let (settings, budget) = settings();
    let plan = plan(
        committed,
        unpinned_watermark(route(0xA1, 3)),
        settings,
        budget,
    )
    .expect("unpinned manifest plans");
    assert_eq!(plan.batches.len(), 2);
    let mut admin = FakeAdmin::seeded(&[point(1), point(2), point(3)]);
    let first =
        execute_batch(&plan, 0, &mut admin, &mutation_for(&plan, 0)).expect("first batch executes");
    let checkpoint = checkpoint(&plan, vec![first.clone()]).expect("checkpoint builds");
    // A newly pinned reader blocks the resume: the watermark is rechecked,
    // never trusted from the checkpoint.
    let blocking =
        PinRegistry::new(route(0xA1, 3), epoch(8), PinLimits::BASELINE).expect("fixture registry");
    let _pin = blocking
        .acquire_route_pin(route(0xA1, 3), OpaqueId::new("t29:late").expect("owner"))
        .expect("late pin acquires");
    let blocked_snapshot = blocking.snapshot().expect("snapshot reads");
    let blocked = search_epoch_pins::compute_reclamation_watermark(
        search_epoch_pins::RetiredVisibilityFence {
            route: route(0xA1, 3),
            retirement_epoch_exclusive: epoch(8),
        },
        &blocked_snapshot,
    );
    assert_eq!(
        resume(&checkpoint, &plan, blocked),
        Err(ReclaimError::StillPinned)
    );
    let remaining =
        resume(&checkpoint, &plan, unpinned_watermark(route(0xA1, 3))).expect("resume continues");
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].batch_index, 1);
    let second = execute_batch(&plan, 1, &mut admin, &mutation_for(&plan, 1))
        .expect("second batch executes");
    let receipt = complete(&plan, &[first, second]).expect("all absent completes");
    assert_eq!(
        receipt.kind,
        ReclaimReceiptKind::OrdinaryRetiredPointReclaim
    );
    assert_eq!(receipt.reclaimed_points, 3);
}

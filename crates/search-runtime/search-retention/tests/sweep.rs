//! T37 sweep decision tests: roots, pins, mark-sweep and safe CAS collection.
//!
//! Decisions live here; enforcement (revision-store tombstones, index-admin
//! deletion) is owned elsewhere. Purge and restore are out of scope.

#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};

use search_contracts::{Blake3Digest32, ObjectResidencyKeyDigest, OpaqueId};
use search_retention::sweep::{
    CasAdmin, CasAdminError, CasDeleteAck, CasReadback, PinEvidence, RetentionRootKind,
    SweepLimits, begin_sweep, collect_protection, complete_sweep, execute_sweep_batch,
    mark_reachable, plan_sweep,
};
use search_retention::{RetainedObjectKind, RetentionError, RetentionOperation};

fn oid(tag: &str) -> OpaqueId {
    OpaqueId::new(tag).expect("fixture id is valid")
}

const fn digest(byte: u8) -> Blake3Digest32 {
    Blake3Digest32::from_bytes([byte; 32])
}

const fn residency(byte: u8) -> ObjectResidencyKeyDigest {
    ObjectResidencyKeyDigest::from_bytes([byte; 32])
}

fn operation(id: &str, req: u8) -> RetentionOperation {
    RetentionOperation {
        operation_id: oid(id),
        request_digest: digest(req),
    }
}

fn root(id: &str, generation: u64) -> search_retention::sweep::DurableRoot {
    search_retention::sweep::DurableRoot {
        object_id: oid(id),
        kind: RetentionRootKind::ActiveProjectionManifest,
        residency_digest: residency(0x11),
        control_generation: generation,
    }
}

fn pins(ids: &[&str], generation: u64, control_generation: u64) -> PinEvidence {
    PinEvidence {
        pinned_ids: ids.iter().map(|s| oid(s)).collect(),
        pin_generation: generation,
        capture_time_ms: 1_000,
        fresh: true,
        control_generation,
        publication_generation: 7,
    }
}

fn graph(pairs: &[(&str, &[&str])]) -> BTreeMap<OpaqueId, Vec<OpaqueId>> {
    let mut out = BTreeMap::new();
    for (from, tos) in pairs {
        out.insert(oid(from), tos.iter().map(|s| oid(s)).collect::<Vec<_>>());
    }
    out
}

fn inventory(ids: &[&str]) -> Vec<OpaqueId> {
    ids.iter().map(|s| oid(s)).collect()
}

const fn limits() -> SweepLimits {
    SweepLimits {
        max_roots: 16,
        max_objects: 64,
        max_edges_per_object: 8,
        max_batches: 8,
        max_batch_objects: 4,
    }
}

struct MemoryCas {
    live: BTreeSet<OpaqueId>,
    fail_delete: Option<CasAdminError>,
    fail_readback: bool,
}

impl MemoryCas {
    fn new(live: &[&str]) -> Self {
        Self {
            live: live.iter().map(|s| oid(s)).collect(),
            fail_delete: None,
            fail_readback: false,
        }
    }
}

impl CasAdmin for MemoryCas {
    fn delete_exact(
        &mut self,
        batch: &search_retention::sweep::SweepBatch,
        mutation: &search_retention::sweep::CasMutation,
    ) -> Result<CasDeleteAck, CasAdminError> {
        if let Some(err) = self.fail_delete {
            return Err(err);
        }
        if mutation.operation_id != batch.operation_id {
            return Err(CasAdminError::Conflict);
        }
        for id in &batch.object_ids {
            self.live.remove(id);
        }
        Ok(CasDeleteAck {
            operation_id: batch.operation_id.clone(),
            deleted_ids: batch.object_ids.clone(),
            replayed: false,
        })
    }

    fn readback_exact(&self, ids: &[OpaqueId]) -> Result<CasReadback, CasAdminError> {
        if self.fail_readback {
            return Err(CasAdminError::Transport);
        }
        let mut missing = Vec::new();
        for id in ids {
            if !self.live.contains(id) {
                missing.push(id.clone());
            }
        }
        Ok(CasReadback {
            missing_ids: missing,
            unexpected_ids: Vec::new(),
        })
    }
}

struct StuckCas;

impl CasAdmin for StuckCas {
    fn delete_exact(
        &mut self,
        _batch: &search_retention::sweep::SweepBatch,
        _mutation: &search_retention::sweep::CasMutation,
    ) -> Result<CasDeleteAck, CasAdminError> {
        Err(CasAdminError::Transport)
    }

    fn readback_exact(&self, _ids: &[OpaqueId]) -> Result<CasReadback, CasAdminError> {
        Ok(CasReadback {
            missing_ids: Vec::new(),
            unexpected_ids: Vec::new(),
        })
    }
}

fn full_sweep_fixture() -> (
    search_retention::sweep::ProtectionSet,
    search_retention::sweep::SweepIntent,
    search_retention::sweep::MarkManifest,
    search_retention::sweep::SweepPlan,
) {
    // Roots: A -> B -> C chain; D is retired derived; H is pinned historical.
    let roots = vec![root("cas:A", 3)];
    let pin_ev = pins(&["cas:H"], 11, 3);
    let protection =
        collect_protection(roots, &pin_ev, &BTreeSet::new(), &BTreeSet::new(), limits())
            .expect("protection collects");
    let intent = begin_sweep(operation("sweep-op-1", 0x01), &protection).expect("intent begins");
    let g = graph(&[("cas:A", &["cas:B"]), ("cas:B", &["cas:C"]), ("cas:H", &[])]);
    let inv: BTreeSet<OpaqueId> = ["cas:A", "cas:B", "cas:C", "cas:H", "cas:D"]
        .iter()
        .map(|s| oid(s))
        .collect();
    let mark = mark_reachable(&intent, &protection, &g, &inv, limits()).expect("mark completes");
    let plan = plan_sweep(
        &intent,
        &mark,
        &inventory(&["cas:A", "cas:B", "cas:C", "cas:D", "cas:H"]),
        5,
        &protection,
        limits(),
    )
    .expect("plan builds");
    (protection, intent, mark, plan)
}

#[test]
fn pinned_historical_survives_while_expired_derived_is_reclaimable() {
    let (_protection, _intent, _mark, plan) = full_sweep_fixture();
    // Only D is unreachable and unprotected.
    assert_eq!(plan.candidates, vec![oid("cas:D")]);
    assert_eq!(plan.batches.len(), 1);
    assert_eq!(plan.batches[0].object_ids, vec![oid("cas:D")]);
}

#[test]
fn missing_roots_block_sweep() {
    let pin_ev = pins(&[], 11, 3);
    let err = collect_protection(
        vec![],
        &pin_ev,
        &BTreeSet::new(),
        &BTreeSet::new(),
        limits(),
    )
    .expect_err("empty roots must fail");
    assert_eq!(err, RetentionError::RootIncomplete);
}

#[test]
fn stale_pin_snapshot_blocks_sweep() {
    let roots = vec![root("cas:A", 3)];
    let mut pin_ev = pins(&[], 11, 3);
    pin_ev.fresh = false;
    let err = collect_protection(roots, &pin_ev, &BTreeSet::new(), &BTreeSet::new(), limits())
        .expect_err("stale pins must fail closed");
    assert_eq!(err, RetentionError::PinProtectionUnknown);
}

#[test]
fn control_generation_drift_blocks_sweep() {
    let roots = vec![root("cas:A", 3)];
    let pin_ev = pins(&[], 11, 9);
    let err = collect_protection(roots, &pin_ev, &BTreeSet::new(), &BTreeSet::new(), limits())
        .expect_err("generation drift must fail");
    assert_eq!(err, RetentionError::RootGenerationChanged);
}

#[test]
fn corrupt_edge_outside_inventory_blocks_mark() {
    let roots = vec![root("cas:A", 3)];
    let pin_ev = pins(&[], 11, 3);
    let protection =
        collect_protection(roots, &pin_ev, &BTreeSet::new(), &BTreeSet::new(), limits())
            .expect("protection");
    let intent = begin_sweep(operation("sweep-op-corrupt", 0x02), &protection).expect("intent");
    let g = graph(&[("cas:A", &["cas:FORGED"])]);
    let inv: BTreeSet<OpaqueId> = std::iter::once(oid("cas:A")).collect();
    let err = mark_reachable(&intent, &protection, &g, &inv, limits())
        .expect_err("forged edge must fail");
    assert_eq!(err, RetentionError::MarkIncomplete);
}

#[test]
fn forged_inventory_grammar_blocks_plan() {
    let (_protection, intent, mark, _plan) = full_sweep_fixture();
    // Unsorted inventory is a forged grammar.
    let protection2 = {
        let roots = vec![root("cas:A", 3)];
        let pin_ev = pins(&["cas:H"], 11, 3);
        collect_protection(roots, &pin_ev, &BTreeSet::new(), &BTreeSet::new(), limits())
            .expect("protection")
    };
    let mut unsorted = inventory(&["cas:D", "cas:A", "cas:B", "cas:C", "cas:H"]);
    unsorted.reverse();
    // Force unsorted by swapping first two (now D,B,... still unsorted).
    let err = plan_sweep(&intent, &mark, &unsorted, 5, &protection2, limits())
        .expect_err("unsorted inventory must fail");
    assert_eq!(err, RetentionError::SweepPlanInvalid);
}

#[test]
fn replaced_candidate_operation_conflict_blocks_execute() {
    let (protection, _intent, _mark, plan) = full_sweep_fixture();
    let mut cas = MemoryCas::new(&["cas:A", "cas:B", "cas:C", "cas:D", "cas:H"]);
    let foreign = search_retention::sweep::CasMutation {
        operation_id: oid("foreign-op"),
        input_digest: [0xFF; 32],
    };
    let err = execute_sweep_batch(&plan, 0, &mut cas, &foreign, &protection)
        .expect_err("foreign operation must conflict");
    assert_eq!(err, RetentionError::SweepGenerationMismatch);
    // Nothing deleted on conflict.
    assert!(cas.live.contains(&oid("cas:D")));
}

#[test]
fn new_pin_narrows_in_progress_sweep() {
    let (mut protection, _intent, _mark, plan) = full_sweep_fixture();
    // A fresh pin arrives for D before dispatch.
    protection.pinned_ids.insert(oid("cas:D"));
    let mut cas = MemoryCas::new(&["cas:A", "cas:B", "cas:C", "cas:D", "cas:H"]);
    let batch = &plan.batches[0];
    let mutation = search_retention::sweep::CasMutation {
        operation_id: batch.operation_id.clone(),
        input_digest: [0u8; 32],
    };
    let err = execute_sweep_batch(&plan, 0, &mut cas, &mutation, &protection)
        .expect_err("new pin must block");
    assert_eq!(err, RetentionError::SweepProtectedObjectConflict);
    assert!(cas.live.contains(&oid("cas:D")));
}

#[test]
fn concurrent_publication_generation_blocks_execute() {
    let (mut protection, _intent, _mark, plan) = full_sweep_fixture();
    protection.publication_generation = 8;
    let mut cas = MemoryCas::new(&["cas:A", "cas:B", "cas:C", "cas:D", "cas:H"]);
    let batch = &plan.batches[0];
    let mutation = search_retention::sweep::CasMutation {
        operation_id: batch.operation_id.clone(),
        input_digest: [0u8; 32],
    };
    let err = execute_sweep_batch(&plan, 0, &mut cas, &mutation, &protection)
        .expect_err("publication drift must block");
    assert_eq!(err, RetentionError::RootGenerationChanged);
}

#[test]
fn tombstoned_and_leased_objects_are_never_candidates() {
    let roots = vec![root("cas:A", 3)];
    let pin_ev = pins(&[], 11, 3);
    let mut tombstoned = BTreeSet::new();
    tombstoned.insert(oid("cas:D"));
    let mut leased = BTreeSet::new();
    leased.insert(oid("cas:E"));
    let protection =
        collect_protection(roots, &pin_ev, &tombstoned, &leased, limits()).expect("protection");
    let intent = begin_sweep(operation("sweep-op-tomb", 0x03), &protection).expect("intent");
    let g = graph(&[("cas:A", &["cas:B"]), ("cas:B", &[])]);
    let inv: BTreeSet<OpaqueId> = ["cas:A", "cas:B", "cas:D", "cas:E"]
        .iter()
        .map(|s| oid(s))
        .collect();
    let mark = mark_reachable(&intent, &protection, &g, &inv, limits()).expect("mark");
    // D/E are unreachable but protected: plan must be empty, not deletable.
    let plan = plan_sweep(
        &intent,
        &mark,
        &inventory(&["cas:A", "cas:B", "cas:D", "cas:E"]),
        6,
        &protection,
        limits(),
    )
    .expect("empty plan is valid");
    assert!(plan.candidates.is_empty());
    assert!(plan.batches.is_empty());
}

#[test]
fn preview_apply_restart_is_idempotent() {
    let (protection, intent, mark, plan) = full_sweep_fixture();
    // Same operation + same inputs reconstructs the identical plan.
    let intent2 = begin_sweep(operation("sweep-op-1", 0x01), &protection).expect("re-intent");
    assert_eq!(intent.protection_digest, intent2.protection_digest);
    let g = graph(&[("cas:A", &["cas:B"]), ("cas:B", &["cas:C"]), ("cas:H", &[])]);
    let inv: BTreeSet<OpaqueId> = ["cas:A", "cas:B", "cas:C", "cas:H", "cas:D"]
        .iter()
        .map(|s| oid(s))
        .collect();
    let mark2 = mark_reachable(&intent2, &protection, &g, &inv, limits()).expect("re-mark");
    assert_eq!(mark.mark_digest, mark2.mark_digest);
    let plan2 = plan_sweep(
        &intent2,
        &mark2,
        &inventory(&["cas:A", "cas:B", "cas:C", "cas:D", "cas:H"]),
        5,
        &protection,
        limits(),
    )
    .expect("re-plan");
    assert_eq!(plan.candidates, plan2.candidates);
    assert_eq!(plan.batches.len(), plan2.batches.len());

    // Apply once completes; restart sees absence and completes again.
    let mut cas = MemoryCas::new(&["cas:A", "cas:B", "cas:C", "cas:D", "cas:H"]);
    let batch = &plan.batches[0];
    let mutation = search_retention::sweep::CasMutation {
        operation_id: batch.operation_id.clone(),
        input_digest: [0u8; 32],
    };
    let receipt1 =
        execute_sweep_batch(&plan, 0, &mut cas, &mutation, &protection).expect("first apply");
    assert!(!cas.live.contains(&oid("cas:D")));
    // Restart: object already absent, same operation replays to the same receipt shape.
    let receipt2 =
        execute_sweep_batch(&plan, 0, &mut cas, &mutation, &protection).expect("restart replay");
    assert_eq!(receipt1.missing_ids, receipt2.missing_ids);
    let final_inv: BTreeSet<OpaqueId> = cas.live.iter().cloned().collect();
    let sweep_receipt = complete_sweep(&intent, &plan, &[receipt1], &final_inv).expect("complete");
    assert_eq!(sweep_receipt.deleted, vec![oid("cas:D")]);
    // Receipt never claims secure erase.
    assert!(!sweep_receipt.secure_erase_claimed);
}

#[test]
fn ambiguous_delete_with_residual_is_unknown_not_success() {
    let (_protection, _intent, _mark, plan) = full_sweep_fixture();
    let protection = {
        let roots = vec![root("cas:A", 3)];
        let pin_ev = pins(&["cas:H"], 11, 3);
        collect_protection(roots, &pin_ev, &BTreeSet::new(), &BTreeSet::new(), limits())
            .expect("protection")
    };
    // Simulate transport failure where the object remains: outcome unknown.
    let mut cas = StuckCas;
    let batch = &plan.batches[0];
    let mutation = search_retention::sweep::CasMutation {
        operation_id: batch.operation_id.clone(),
        input_digest: [0u8; 32],
    };
    let err = execute_sweep_batch(&plan, 0, &mut cas, &mutation, &protection)
        .expect_err("residual after ambiguous delete is unknown");
    assert_eq!(err, RetentionError::SweepDeleteOutcomeUnknown);
}

#[test]
fn retained_and_unknown_objects_are_untouched_by_complete() {
    let (protection, intent, _mark, plan) = full_sweep_fixture();
    let mut cas = MemoryCas::new(&["cas:A", "cas:B", "cas:C", "cas:D", "cas:H"]);
    let batch = &plan.batches[0];
    let mutation = search_retention::sweep::CasMutation {
        operation_id: batch.operation_id.clone(),
        input_digest: [0u8; 32],
    };
    let receipt = execute_sweep_batch(&plan, 0, &mut cas, &mutation, &protection).expect("apply");
    // Retained revisions survive.
    for id in ["cas:A", "cas:B", "cas:C", "cas:H"] {
        assert!(cas.live.contains(&oid(id)), "retained {id} must survive");
    }
    let final_inv: BTreeSet<OpaqueId> = cas.live.iter().cloned().collect();
    let sweep_receipt = complete_sweep(&intent, &plan, &[receipt], &final_inv).expect("complete");
    assert_eq!(sweep_receipt.deleted, vec![oid("cas:D")]);
}

#[test]
fn root_kinds_are_closed_and_retention_kinds_cover_policy() {
    assert_eq!(RetentionRootKind::ALL.len(), 11);
    for kind in RetentionRootKind::ALL {
        assert_eq!(RetentionRootKind::parse(kind.as_str()), Ok(*kind));
    }
    assert!(RetentionRootKind::parse("forged_root").is_err());
    // RetainedObjectKind already distinguishes source truth from derived artifacts.
    assert_ne!(
        RetainedObjectKind::SourceRevision,
        RetainedObjectKind::Materialization
    );
}

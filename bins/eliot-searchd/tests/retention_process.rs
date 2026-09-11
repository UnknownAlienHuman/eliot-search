//! T37 retention process test: ordinary sweep driven by canonical
//! reachability plus real pin evidence, with safe filesystem CAS collection.
//!
//! Decisions come from `search-retention` (roots, leases, mark-sweep); the
//! filesystem adapter below is the enforcement boundary (exact-ID delete plus
//! absence readback). Purge and restore do not exist in this file. Unknown
//! files block destructive work and are never deleted.

#![cfg(feature = "wave7-lifecycle")]
#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use search_contracts::{
    Blake3Digest32, CollectionGenerationId, CollectionRouteRevision, Epoch,
    ObjectResidencyKeyDigest, OpaqueId,
};
use search_epoch_pins::{
    EpochPinPurpose, PinLimits, PinRegistry, RetiredVisibilityFence, RouteIdentity,
    compute_reclamation_watermark,
};
use search_retention::sweep::{
    CasAdmin, CasAdminError, CasDeleteAck, CasMutation, CasReadback, DurableRoot, PinEvidence,
    ProtectionSet, RetentionRootKind, SweepBatch, SweepLimits, begin_sweep, collect_protection,
    complete_sweep, execute_sweep_batch, mark_reachable, plan_sweep,
};
use search_retention::{RetentionError, RetentionOperation};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Scratch {
    base: PathBuf,
}

impl Scratch {
    fn new() -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let base = std::env::temp_dir().join(format!(
            "eliot-t37-retention-{}-{stamp}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir_all(base.join("cas")).expect("scratch cas dir");
        Self { base }
    }

    fn cas_dir(&self) -> PathBuf {
        self.base.join("cas")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}

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

const fn limits() -> SweepLimits {
    SweepLimits {
        max_roots: 16,
        max_objects: 64,
        max_edges_per_object: 8,
        max_batches: 8,
        max_batch_objects: 4,
    }
}

fn epoch(value: i64) -> Epoch {
    Epoch::new(value).expect("fixture epoch is valid")
}

const fn route() -> RouteIdentity {
    RouteIdentity {
        collection_generation_id: CollectionGenerationId::from_bytes([0xA1; 16]),
        route_revision: CollectionRouteRevision::new(3),
    }
}

/// Filesystem CAS enforcement: exact-ID delete plus absence readback.
///
/// The inventory grammar is `<cas-id>.bin` where `<cas-id>` starts with
/// `cas-` followed by ASCII alphanumerics and hyphens. Any other regular
/// file in the directory is an unknown object: destructive work fails closed
/// and the file is never touched.
struct FileCas {
    dir: PathBuf,
}

impl FileCas {
    fn new(dir: &Path) -> Self {
        Self {
            dir: dir.to_owned(),
        }
    }

    fn object_path(&self, id: &OpaqueId) -> PathBuf {
        self.dir.join(format!("{id}.bin"))
    }

    fn scan(&self) -> Result<(BTreeSet<OpaqueId>, usize), CasAdminError> {
        let mut known = BTreeSet::new();
        let mut unknown = 0_usize;
        let entries = fs::read_dir(&self.dir).map_err(|_| CasAdminError::Transport)?;
        for entry in entries {
            let entry = entry.map_err(|_| CasAdminError::Transport)?;
            let path = entry.path();
            let meta = fs::symlink_metadata(&path).map_err(|_| CasAdminError::Transport)?;
            if !meta.is_file() {
                unknown += 1;
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            match name.strip_suffix(".bin") {
                Some(stem) if valid_cas_stem(stem) => {
                    let id = OpaqueId::new(stem).map_err(|_| CasAdminError::Mismatch)?;
                    if !known.insert(id) {
                        return Err(CasAdminError::Mismatch);
                    }
                }
                _ => {
                    unknown += 1;
                }
            }
        }
        Ok((known, unknown))
    }
}

fn valid_cas_stem(stem: &str) -> bool {
    let Some(body) = stem.strip_prefix("cas-") else {
        return false;
    };
    !body.is_empty()
        && body.len() <= 200
        && body.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

impl CasAdmin for FileCas {
    fn delete_exact(
        &mut self,
        batch: &SweepBatch,
        mutation: &CasMutation,
    ) -> Result<CasDeleteAck, CasAdminError> {
        if mutation.operation_id != batch.operation_id {
            return Err(CasAdminError::Conflict);
        }
        let (_, unknown) = self.scan()?;
        if unknown != 0 {
            return Err(CasAdminError::Mismatch);
        }
        // Already-absent batch members are tolerated: the deletion loop
        // below skips them and the readback still proves full absence,
        // which keeps preview/apply/restart idempotent.
        for id in &batch.object_ids {
            let path = self.object_path(id);
            if path.exists() {
                fs::remove_file(&path).map_err(|_| CasAdminError::Transport)?;
            }
        }
        Ok(CasDeleteAck {
            operation_id: batch.operation_id.clone(),
            deleted_ids: batch.object_ids.clone(),
            replayed: false,
        })
    }

    fn readback_exact(&self, ids: &[OpaqueId]) -> Result<CasReadback, CasAdminError> {
        let mut missing = Vec::new();
        for id in ids {
            if !self.object_path(id).exists() {
                missing.push(id.clone());
            }
        }
        Ok(CasReadback {
            missing_ids: missing,
            unexpected_ids: Vec::new(),
        })
    }
}

fn seed_objects(dir: &Path, ids: &[&str]) {
    for id in ids {
        fs::write(dir.join(format!("{id}.bin")), format!("bytes:{id}")).expect("seed object");
    }
}

fn protection_for(
    roots: Vec<DurableRoot>,
    pinned: &[&str],
    pin_generation: u64,
    control_generation: u64,
    publication_generation: u64,
) -> ProtectionSet {
    let pins = PinEvidence {
        pinned_ids: pinned.iter().map(|s| oid(s)).collect(),
        pin_generation,
        capture_time_ms: 1_000,
        fresh: true,
        control_generation,
        publication_generation,
    };
    collect_protection(roots, &pins, &BTreeSet::new(), &BTreeSet::new(), limits())
        .expect("protection collects")
}

fn root(id: &str, control_generation: u64) -> DurableRoot {
    DurableRoot {
        object_id: oid(id),
        kind: RetentionRootKind::ActiveProjectionManifest,
        residency_digest: residency(0x11),
        control_generation,
    }
}

#[test]
fn pinned_evidence_blocks_reclaim_while_sweep_collects_expired_derived() {
    // Real pin registry: an active epoch pin on the old route blocks ordinary
    // reclaim of the retired state (T29 watermark), so the historical object
    // must stay pinned in the sweep protection set too.
    let registry =
        PinRegistry::new(route(), epoch(7), PinLimits::BASELINE).expect("fixture registry");
    let owner = oid("t37-owner-pin");
    let _guard = registry
        .acquire_epoch_pin(route(), epoch(7), owner, EpochPinPurpose::Query, 1_000)
        .expect("epoch pin acquires");
    let snapshot = registry.snapshot().expect("snapshot reads");
    let watermark = compute_reclamation_watermark(
        RetiredVisibilityFence {
            route: route(),
            retirement_epoch_exclusive: epoch(8),
        },
        &snapshot,
    );
    assert!(!watermark.reclaimable, "active pin must block reclaim");

    let scratch = Scratch::new();
    let dir = scratch.cas_dir();
    seed_objects(&dir, &["cas-A", "cas-B", "cas-C", "cas-H", "cas-D"]);

    // Canonical reachability: A -> B -> C plus pinned historical H.
    let protection = protection_for(vec![root("cas-A", 3)], &["cas-H"], 11, 3, 7);
    let intent = begin_sweep(operation("t37-sweep-1", 0x01), &protection).expect("intent");
    let mut graph = BTreeMap::new();
    graph.insert(oid("cas-A"), vec![oid("cas-B")]);
    graph.insert(oid("cas-B"), vec![oid("cas-C")]);
    graph.insert(oid("cas-H"), Vec::new());
    let inventory_set: BTreeSet<OpaqueId> = ["cas-A", "cas-B", "cas-C", "cas-H", "cas-D"]
        .iter()
        .map(|s| oid(s))
        .collect();
    let mark = mark_reachable(&intent, &protection, &graph, &inventory_set, limits())
        .expect("mark completes");
    assert!(mark.reachable.contains(&oid("cas-H")));
    let plan = plan_sweep(
        &intent,
        &mark,
        &["cas-A", "cas-B", "cas-C", "cas-D", "cas-H"]
            .iter()
            .map(|s| oid(s))
            .collect::<Vec<_>>(),
        5,
        &protection,
        limits(),
    )
    .expect("plan builds");
    // Preview binds the exact candidate: only expired derived D.
    assert_eq!(plan.candidates, vec![oid("cas-D")]);

    // Apply deletes exactly D; retained and pinned objects survive.
    let mut cas = FileCas::new(&dir);
    let batch = &plan.batches[0];
    let mutation = CasMutation {
        operation_id: batch.operation_id.clone(),
        input_digest: [0u8; 32],
    };
    let receipt =
        execute_sweep_batch(&plan, 0, &mut cas, &mutation, &protection).expect("apply deletes D");
    assert!(!dir.join("cas-D.bin").exists());
    for id in ["cas-A", "cas-B", "cas-C", "cas-H"] {
        assert!(dir.join(format!("{id}.bin")).exists(), "{id} must survive");
    }
    // Restart is idempotent: same operation replays to the same absence proof.
    let replay =
        execute_sweep_batch(&plan, 0, &mut cas, &mutation, &protection).expect("restart replays");
    assert_eq!(receipt.missing_ids, replay.missing_ids);
    let (final_known, _) = cas.scan().expect("final scan reads");
    let sweep_receipt = complete_sweep(&intent, &plan, &[receipt], &final_known)
        .expect("complete accounts exactly");
    assert_eq!(sweep_receipt.deleted, vec![oid("cas-D")]);
    assert!(!sweep_receipt.secure_erase_claimed);
}

#[test]
fn corrupt_control_forged_grammar_and_replaced_candidate_block_deletion() {
    let scratch = Scratch::new();
    let dir = scratch.cas_dir();
    seed_objects(&dir, &["cas-A", "cas-B"]);

    // Missing roots block.
    let pins = PinEvidence {
        pinned_ids: BTreeSet::new(),
        pin_generation: 11,
        capture_time_ms: 1_000,
        fresh: true,
        control_generation: 3,
        publication_generation: 7,
    };
    assert_eq!(
        collect_protection(
            Vec::new(),
            &pins,
            &BTreeSet::new(),
            &BTreeSet::new(),
            limits()
        )
        .expect_err("empty roots must fail"),
        RetentionError::RootIncomplete
    );

    // Corrupt edge (outside inventory) blocks mark.
    let protection = protection_for(vec![root("cas-A", 3)], &[], 11, 3, 7);
    let intent = begin_sweep(operation("t37-sweep-corrupt", 0x02), &protection).expect("intent");
    let mut bad_graph = BTreeMap::new();
    bad_graph.insert(oid("cas-A"), vec![oid("cas-FORGED")]);
    let inventory_set: BTreeSet<OpaqueId> = [oid("cas-A"), oid("cas-B")].into_iter().collect();
    assert_eq!(
        mark_reachable(&intent, &protection, &bad_graph, &inventory_set, limits())
            .expect_err("forged edge must fail"),
        RetentionError::MarkIncomplete
    );

    // Forged inventory grammar (unsorted) blocks plan.
    let mut graph = BTreeMap::new();
    graph.insert(oid("cas-A"), Vec::new());
    let mark = mark_reachable(&intent, &protection, &graph, &inventory_set, limits())
        .expect("mark completes");
    let unsorted = vec![oid("cas-B"), oid("cas-A")];
    assert_eq!(
        plan_sweep(&intent, &mark, &unsorted, 5, &protection, limits())
            .expect_err("unsorted inventory must fail"),
        RetentionError::SweepPlanInvalid
    );

    // Replaced candidate (foreign operation identity) blocks execute.
    let plan = plan_sweep(
        &intent,
        &mark,
        &[oid("cas-A"), oid("cas-B")],
        5,
        &protection,
        limits(),
    )
    .expect("plan builds");
    if !plan.batches.is_empty() {
        let mut cas = FileCas::new(&dir);
        let foreign = CasMutation {
            operation_id: oid("foreign-op"),
            input_digest: [0xFF; 32],
        };
        assert_eq!(
            execute_sweep_batch(&plan, 0, &mut cas, &foreign, &protection)
                .expect_err("foreign op must conflict"),
            RetentionError::SweepGenerationMismatch
        );
        // Nothing deleted on conflict.
        assert!(dir.join("cas-A.bin").exists());
        assert!(dir.join("cas-B.bin").exists());
    }
}

#[test]
fn new_pin_and_concurrent_publication_block_in_progress_sweep() {
    let scratch = Scratch::new();
    let dir = scratch.cas_dir();
    seed_objects(&dir, &["cas-A", "cas-B", "cas-D"]);

    let protection = protection_for(vec![root("cas-A", 3)], &[], 11, 3, 7);
    let intent = begin_sweep(operation("t37-sweep-drift", 0x03), &protection).expect("intent");
    let mut graph = BTreeMap::new();
    graph.insert(oid("cas-A"), vec![oid("cas-B")]);
    let inventory_set: BTreeSet<OpaqueId> = [oid("cas-A"), oid("cas-B"), oid("cas-D")]
        .into_iter()
        .collect();
    let mark = mark_reachable(&intent, &protection, &graph, &inventory_set, limits())
        .expect("mark completes");
    let plan = plan_sweep(
        &intent,
        &mark,
        &[oid("cas-A"), oid("cas-B"), oid("cas-D")],
        5,
        &protection,
        limits(),
    )
    .expect("plan builds");
    assert_eq!(plan.candidates, vec![oid("cas-D")]);
    let batch = &plan.batches[0];
    let mutation = CasMutation {
        operation_id: batch.operation_id.clone(),
        input_digest: [0u8; 32],
    };

    // A newly arrived pin for D narrows the sweep: dispatch is refused.
    let mut with_pin = protection.clone();
    with_pin.pinned_ids.insert(oid("cas-D"));
    let mut cas = FileCas::new(&dir);
    assert_eq!(
        execute_sweep_batch(&plan, 0, &mut cas, &mutation, &with_pin)
            .expect_err("new pin must block"),
        RetentionError::SweepProtectedObjectConflict
    );
    assert!(dir.join("cas-D.bin").exists());

    // A concurrent publication generation also invalidates the sweep.
    let mut republished = protection;
    republished.publication_generation = 8;
    assert_eq!(
        execute_sweep_batch(&plan, 0, &mut cas, &mutation, &republished)
            .expect_err("publication drift must block"),
        RetentionError::RootGenerationChanged
    );
    assert!(dir.join("cas-D.bin").exists());
}

#[test]
fn unknown_files_block_apply_and_survive_unchanged() {
    let scratch = Scratch::new();
    let dir = scratch.cas_dir();
    seed_objects(&dir, &["cas-A", "cas-D"]);
    fs::write(dir.join("unexpected.dat"), b"not a cas object").expect("unknown file");

    let protection = protection_for(vec![root("cas-A", 3)], &[], 11, 3, 7);
    let intent = begin_sweep(operation("t37-sweep-unknown", 0x04), &protection).expect("intent");
    let mut graph = BTreeMap::new();
    graph.insert(oid("cas-A"), Vec::new());
    let inventory_set: BTreeSet<OpaqueId> = [oid("cas-A"), oid("cas-D")].into_iter().collect();
    let mark = mark_reachable(&intent, &protection, &graph, &inventory_set, limits())
        .expect("mark completes");
    let plan = plan_sweep(
        &intent,
        &mark,
        &[oid("cas-A"), oid("cas-D")],
        5,
        &protection,
        limits(),
    )
    .expect("plan previews D");
    assert_eq!(plan.candidates, vec![oid("cas-D")]);

    // Apply is refused while an unknown file is present; nothing is deleted.
    let mut cas = FileCas::new(&dir);
    let batch = &plan.batches[0];
    let mutation = CasMutation {
        operation_id: batch.operation_id.clone(),
        input_digest: [0u8; 32],
    };
    assert_eq!(
        execute_sweep_batch(&plan, 0, &mut cas, &mutation, &protection)
            .expect_err("unknown files must block"),
        RetentionError::SweepProtectedObjectConflict
    );
    assert!(dir.join("cas-D.bin").exists());
    assert!(dir.join("cas-A.bin").exists());
    assert!(dir.join("unexpected.dat").exists());

    // After the operator removes nothing but quarantines the unknown file out
    // of the CAS directory, the same preview reapplies idempotently.
    fs::remove_file(dir.join("unexpected.dat")).expect("quarantine removes unknown");
    let receipt = execute_sweep_batch(&plan, 0, &mut cas, &mutation, &protection)
        .expect("reapply after quarantine");
    assert!(!dir.join("cas-D.bin").exists());
    assert!(dir.join("cas-A.bin").exists());
    let (final_known, unknown) = cas.scan().expect("final scan");
    assert_eq!(unknown, 0);
    let sweep_receipt = complete_sweep(&intent, &plan, &[receipt], &final_known).expect("complete");
    assert_eq!(sweep_receipt.deleted, vec![oid("cas-D")]);
}

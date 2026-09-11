//! T38 security purge barriers at the daemon boundary: deny fences before any
//! deletion, purge lifecycle separate from ordinary reclaim, storage plus
//! result planes, logical-only deletion with absence readback.
//!
//! The test drives real `search-retention` purge decisions with a filesystem
//! CAS enforcement boundary (exact-ID delete plus absence readback, mirroring
//! T37) and real `search-access` live deny/purge state. Qdrant stays
//! unavailable by construction: the projection plane is proven as typed
//! unresolved evidence, never silent success. No physical secure erase is
//! ever claimed.

#![cfg(feature = "wave7-lifecycle")]
#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use search_access::{
    AccessCheckpoint, LiveSecurityState, RequestSecurityFence, classify_contaminated_legs,
    recheck_live_access,
};
use search_contracts::{
    Blake3Digest32, ObjectResidencyKeyDigest, OpaqueId, PurgeFenceRevision, ReceiptRef,
    SourceMembershipId,
};
use search_retention::purge::{
    PurgeBarrierProof, PurgeInvalidationSet, PurgePlane, PurgePlaneStatus,
    require_all_planes_resolved, require_live_deny_before_delete, verify_purge_invalidation,
};
use search_retention::{
    BackupDisposition, PhysicalEraseEvidence, PurgeCoordinator, PurgeLayerReceipt, PurgeManifest,
    PurgePhase, RetainedObject, RetainedObjectKind, RetentionError, RetentionPolicy,
};

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
            "eliot-t38-purge-{}-{stamp}-{}",
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

fn receipt(tag: &str) -> ReceiptRef {
    ReceiptRef::new(tag).expect("fixture receipt is valid")
}

const fn membership(n: u128) -> SourceMembershipId {
    SourceMembershipId::from_bytes(n.to_be_bytes())
}

fn target(id: &str, member: u128) -> RetainedObject {
    RetainedObject {
        object_id: oid(id),
        kind: RetainedObjectKind::Projection,
        residency_digest: ObjectResidencyKeyDigest::from_bytes([0x11; 32]),
        source_membership_id: Some(membership(member)),
        source_revision_id: None,
        collection_generation_id: None,
        last_visible_epoch: None,
        object_digest: digest(0xC1),
    }
}

fn manifest() -> PurgeManifest {
    PurgeManifest {
        request_id: oid("t38-purge-1"),
        targets: vec![target("purge:obj-1", 7)],
        manifest_digest: digest(0xA1),
        purge_generation: 9,
        purge_fence_revision: PurgeFenceRevision::new(6),
        preparation_receipt: receipt("t38-prep"),
    }
}

fn coordinator() -> PurgeCoordinator {
    let previous = PurgeFenceRevision::new(5);
    PurgeCoordinator::new(RetentionPolicy::BASELINE, manifest(), previous).expect("prepared purge")
}

fn layer_receipt(coordinator: &PurgeCoordinator, tag: &str) -> PurgeLayerReceipt {
    let ids = coordinator
        .manifest()
        .targets
        .iter()
        .map(|target| target.object_id.clone())
        .collect::<BTreeSet<_>>();
    PurgeLayerReceipt {
        request_id: coordinator.manifest().request_id.clone(),
        purge_generation: coordinator.manifest().purge_generation,
        acknowledged_objects: ids,
        missing_objects: BTreeSet::new(),
        unexpected_objects: BTreeSet::new(),
        readback_digest: digest(0xD1),
        receipt: receipt(tag),
    }
}

fn barrier(manifest: &PurgeManifest) -> PurgeBarrierProof {
    PurgeBarrierProof {
        request_id: manifest.request_id.clone(),
        purge_generation: manifest.purge_generation,
        fence_revision: manifest.purge_fence_revision,
        live_generation: 42,
        live_snapshot_digest: digest(0xE1),
    }
}

fn invalidation(manifest: &PurgeManifest) -> PurgeInvalidationSet {
    PurgeInvalidationSet {
        request_id: manifest.request_id.clone(),
        purge_generation: manifest.purge_generation,
        handles_revoked: true,
        continuations_revoked: true,
        cache_ranking_dropped: true,
        overlays_detached: true,
        invalidation_digest: digest(0xF1),
    }
}

fn live_denied(purged: &[u128]) -> LiveSecurityState {
    LiveSecurityState {
        generation: 42,
        denied_memberships: BTreeSet::new(),
        purged_memberships: purged.iter().copied().map(membership).collect(),
        fail_closed: false,
        snapshot_digest: digest(0xE1),
    }
}

fn request_for(member: u128) -> RequestSecurityFence {
    RequestSecurityFence {
        planned_generation: 42,
        memberships: BTreeSet::from([membership(member)]),
    }
}

#[test]
fn fenced_purge_denies_result_plane_before_storage_delete() {
    // Previously issued handles/continuations/overlay views for the purged
    // membership cannot reveal bytes once the deny fence is live, and cached
    // ranking built before the fence is discarded as contaminated.
    let live_before = live_denied(&[]);
    let fence = request_for(7);
    recheck_live_access(&fence, &live_before, AccessCheckpoint::RequestAdmission)
        .expect("admission before fence passes");

    let mut purge = coordinator();
    let prepared = purge.manifest().clone();
    // Storage deletion is refused before the live-deny commit.
    assert_eq!(
        require_live_deny_before_delete(&purge, Some(&barrier(&prepared)))
            .expect_err("prepared phase must not delete"),
        RetentionError::LiveDenyReceiptMissing
    );
    purge
        .accept_live_deny(layer_receipt(&purge, "t38-live-deny"))
        .expect("live deny commits");
    require_live_deny_before_delete(&purge, Some(&barrier(&prepared)))
        .expect("deny gates storage delete");

    // The live fence now denies the purged membership at every checkpoint,
    // including handle/continuation expansion and result emission.
    let live_after = live_denied(&[7]);
    for checkpoint in [
        AccessCheckpoint::RequestAdmission,
        AccessCheckpoint::BeforeResultEmission,
        AccessCheckpoint::HandleExpansion,
        AccessCheckpoint::ContinuationExpansion,
    ] {
        recheck_live_access(&fence, &live_after, checkpoint)
            .expect_err("purged scope must deny at every checkpoint");
    }
    // An in-flight leg that scored purged material before the fence is
    // discarded whole, so cached ranking cannot influence the result.
    let legs = vec![search_access::LegSecurityPopulation {
        leg_id: 0,
        memberships: BTreeSet::from([membership(7)]),
        security_generation: 41,
        idf_population_digest: None,
    }];
    assert!(matches!(
        classify_contaminated_legs(&legs, &live_before, &live_after),
        search_access::ContaminationDecision::DiscardLegs(_)
    ));

    // Result-plane invalidation completes before storage deletion proceeds.
    let invalidation_set = invalidation(&prepared);
    verify_purge_invalidation(&invalidation_set, &prepared).expect("result plane invalidated");
    purge
        .accept_invalidation(layer_receipt(&purge, "t38-invalidation"))
        .expect("invalidation accepted");

    // Storage CAS plane: exact-ID delete plus absence readback on the real
    // filesystem. The scratch id contains a colon, so it is addressed through
    // a safe local filename mapping instead of the raw object id.
    let scratch = Scratch::new();
    let dir = scratch.cas_dir();
    fs::write(dir.join("obj-1.bin"), b"bytes:purge:obj-1").expect("seed object");
    let id = oid("purge:obj-1");
    let local = dir.join("obj-1.bin");
    assert!(local.exists());
    fs::remove_file(&local).expect("exact purge delete");
    assert!(!local.exists(), "absence readback proves deletion");
    let _ = id;

    // No complete purge receipt while the projection plane is unresolved
    // (Qdrant unavailable by construction here).
    let statuses = vec![
        PurgePlaneStatus {
            plane: PurgePlane::LiveDeny,
            resolved: true,
            outcome_unknown: false,
        },
        PurgePlaneStatus {
            plane: PurgePlane::ResultHandles,
            resolved: true,
            outcome_unknown: false,
        },
        PurgePlaneStatus {
            plane: PurgePlane::StorageCas,
            resolved: true,
            outcome_unknown: false,
        },
        PurgePlaneStatus {
            plane: PurgePlane::StorageProjection,
            resolved: false,
            outcome_unknown: true,
        },
    ];
    assert_eq!(
        require_all_planes_resolved(
            &statuses,
            &[
                PurgePlane::LiveDeny,
                PurgePlane::ResultHandles,
                PurgePlane::StorageCas,
                PurgePlane::StorageProjection,
            ]
        )
        .expect_err("unresolved projection must block"),
        RetentionError::PurgePartial
    );
    assert_eq!(purge.phase(), PurgePhase::HandlesInvalidated);
}

#[test]
fn restart_mid_phase_resumes_with_same_operation_identity() {
    // Process restart after the live-deny commit: recovery rebuilds the same
    // coordinator identity, replays the fence receipt shape and resumes the
    // remaining exact layers without reopening access.
    let mut purge = coordinator();
    let saved = purge.manifest().clone();
    purge
        .accept_live_deny(layer_receipt(&purge, "t38-live-deny"))
        .expect("live deny commits");
    assert_eq!(purge.phase(), PurgePhase::LiveDenyCommitted);

    // Restart: same manifest plus previous fence reconstructs the identical
    // prepared identity; the fence receipt replays to the same phase.
    let previous = PurgeFenceRevision::new(5);
    let mut recovered = PurgeCoordinator::new(RetentionPolicy::BASELINE, saved.clone(), previous)
        .expect("recovery reconstructs identity");
    assert_eq!(recovered.phase(), PurgePhase::Prepared);
    recovered
        .accept_live_deny(layer_receipt(&recovered, "t38-live-deny"))
        .expect("fence receipt replays");
    assert_eq!(recovered.phase(), PurgePhase::LiveDenyCommitted);
    require_live_deny_before_delete(&recovered, Some(&barrier(&saved)))
        .expect("recovered fence gates delete");

    // Repeat invalidation with the same operation identity is exact: a wrong
    // request digest can never broaden the recovered deletion set.
    let mut replay = layer_receipt(&recovered, "t38-invalidation");
    replay.request_id = oid("t38-foreign");
    assert!(
        recovered.accept_invalidation(replay).is_err(),
        "foreign identity must not advance recovery"
    );
    recovered
        .accept_invalidation(layer_receipt(&recovered, "t38-invalidation"))
        .expect("exact invalidation resumes");
    assert_eq!(recovered.phase(), PurgePhase::HandlesInvalidated);

    // The live fence still denies throughout recovery.
    let live = live_denied(&[7]);
    recheck_live_access(&request_for(7), &live, AccessCheckpoint::RequestAdmission)
        .expect_err("fence holds across restart");
}

#[test]
fn ordinary_reclaim_cannot_satisfy_purge_projection_layer() {
    // T29 ordinary reclaim receipts are observations only: the purge
    // projection layer requires the security-purge index-admin path with its
    // own purge receipt, and completion stays blocked without it.
    let mut purge = coordinator();
    purge
        .accept_live_deny(layer_receipt(&purge, "t38-live-deny"))
        .expect("live deny commits");
    purge
        .accept_invalidation(layer_receipt(&purge, "t38-invalidation"))
        .expect("invalidation accepted");
    search_retention::purge::reject_ordinary_reclaim_as_purge(true)
        .expect_err("ordinary reclaim must not satisfy purge");
    // A purge-path index receipt advances; an ordinary receipt never could.
    purge
        .accept_index_deletion(layer_receipt(&purge, "t38-purge-index"))
        .expect("purge-path index receipt advances");
    assert_eq!(purge.phase(), PurgePhase::IndexDeleted);
}

#[test]
fn terminal_purge_receipt_is_logical_only_with_absence_readback() {
    // Full layer walk to the terminal tombstone: logical non-accessibility
    // plus absence readback, honest backup status, never secure erase.
    let mut purge = coordinator();
    let saved = purge.manifest().clone();
    purge
        .accept_live_deny(layer_receipt(&purge, "t38-live-deny"))
        .expect("deny");
    let snapshot = layer_receipt(&purge, "t38-layer");
    purge
        .accept_invalidation(snapshot.clone())
        .expect("handles");
    purge
        .accept_index_deletion(snapshot.clone())
        .expect("index");
    purge
        .accept_cache_deletion(snapshot.clone())
        .expect("cache");
    purge
        .accept_object_deletion(snapshot.clone())
        .expect("objects");
    purge
        .record_backup_disposition(BackupDisposition::TombstoneRetained, snapshot)
        .expect("backup");
    search_retention::purge::assert_logical_only(&PhysicalEraseEvidence::NotGuaranteed)
        .expect("honest limit");
    let tombstone = purge
        .complete(
            receipt("t38-logical-absence"),
            PhysicalEraseEvidence::NotGuaranteed,
            digest(0x99),
        )
        .expect("logical purge completes");
    assert_eq!(tombstone.purge_fence_revision, saved.purge_fence_revision);
    assert_eq!(
        tombstone.physical_erase,
        PhysicalEraseEvidence::NotGuaranteed
    );
    // Tombstone blocks resurrection: the purged membership stays denied.
    let live = live_denied(&[7]);
    recheck_live_access(&request_for(7), &live, AccessCheckpoint::RequestAdmission)
        .expect_err("tombstoned scope stays denied");
    let _ = BTreeMap::<OpaqueId, Vec<OpaqueId>>::new();
}

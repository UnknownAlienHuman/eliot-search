//! T38 security purge barrier decisions: deny fences before any deletion,
//! purge lifecycle separate from ordinary reclaim, storage + result planes,
//! logical-only deletion with absence readback (never physical secure erase).
//!
//! Decisions live here; enforcement (revision-store tombstones, index-admin
//! purge path, handle/continuation invalidation) is owned elsewhere. T37
//! sweep decisions are reused, never duplicated. T29 ordinary reclaim is
//! read-only context: its receipts can never satisfy a purge layer.

#![forbid(unsafe_code)]

use std::collections::BTreeSet;

use search_contracts::{Blake3Digest32, OpaqueId, PurgeFenceRevision, ReceiptRef};
use search_retention::purge::{
    PurgeAuthority, PurgeBarrierProof, PurgeInvalidationSet, PurgePlane, PurgePlaneStatus,
    PurgeResumeDirective, assert_logical_only, check_purge_cas_targets, direct_purge_resume,
    reject_ordinary_reclaim_as_purge, require_all_planes_resolved, require_live_deny_before_delete,
    validate_purge_authority, verify_purge_invalidation,
};
use search_retention::{
    BackupDisposition, PhysicalEraseEvidence, PurgeCoordinator, PurgeLayerReceipt, PurgeManifest,
    PurgePhase, RetentionError, RetentionOperation, RetentionPolicy,
};

fn oid(tag: &str) -> OpaqueId {
    OpaqueId::new(tag).expect("fixture id is valid")
}

const fn digest(byte: u8) -> Blake3Digest32 {
    Blake3Digest32::from_bytes([byte; 32])
}

fn receipt(tag: &str) -> ReceiptRef {
    ReceiptRef::new(tag).expect("fixture receipt is valid")
}

fn operation(id: &str, req: u8) -> RetentionOperation {
    RetentionOperation {
        operation_id: oid(id),
        request_digest: digest(req),
    }
}

fn coordinator_with_targets() -> (PurgeCoordinator, PurgeManifest) {
    use search_contracts::{ObjectResidencyKeyDigest, SourceMembershipId};
    use search_retention::{RetainedObject, RetainedObjectKind};
    let target = RetainedObject {
        object_id: oid("purge:obj-1"),
        kind: RetainedObjectKind::Projection,
        residency_digest: ObjectResidencyKeyDigest::from_bytes([0x11; 32]),
        source_membership_id: Some(SourceMembershipId::from_bytes(1_u128.to_be_bytes())),
        source_revision_id: None,
        collection_generation_id: None,
        last_visible_epoch: None,
        object_digest: digest(0xC1),
    };
    let manifest = PurgeManifest {
        request_id: oid("purge-req-1"),
        targets: vec![target],
        manifest_digest: digest(0xA1),
        purge_generation: 9,
        purge_fence_revision: PurgeFenceRevision::new(6),
        preparation_receipt: receipt("prep-1"),
    };
    let previous = PurgeFenceRevision::new(5);
    let coordinator = PurgeCoordinator::new(RetentionPolicy::BASELINE, manifest.clone(), previous)
        .expect("prepared purge");
    (coordinator, manifest)
}

fn layer_receipt(coordinator: &PurgeCoordinator) -> PurgeLayerReceipt {
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
        receipt: receipt("layer-1"),
    }
}

fn barrier_for(manifest: &PurgeManifest) -> PurgeBarrierProof {
    PurgeBarrierProof {
        request_id: manifest.request_id.clone(),
        purge_generation: manifest.purge_generation,
        fence_revision: manifest.purge_fence_revision,
        live_generation: 42,
        live_snapshot_digest: digest(0xE1),
    }
}

fn full_invalidation(manifest: &PurgeManifest) -> PurgeInvalidationSet {
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

#[test]
fn delete_without_live_deny_fails_closed() {
    let (coordinator, manifest) = coordinator_with_targets();
    assert_eq!(coordinator.phase(), PurgePhase::Prepared);
    // No barrier proof: every destructive layer is refused before any delete.
    let err = require_live_deny_before_delete(&coordinator, None)
        .expect_err("delete without deny must fail");
    assert_eq!(err, RetentionError::LiveDenyReceiptMissing);
    let _ = manifest;
}

#[test]
fn live_deny_barrier_gates_every_delete_layer() {
    let (mut coordinator, manifest) = coordinator_with_targets();
    let barrier = barrier_for(&manifest);
    // Prepared phase cannot delete even with a well-formed barrier proof.
    let err = require_live_deny_before_delete(&coordinator, Some(&barrier))
        .expect_err("prepared phase must not delete");
    assert_eq!(err, RetentionError::LiveDenyReceiptMissing);
    // After the live-deny commit the same barrier authorizes delete layers.
    coordinator
        .accept_live_deny(layer_receipt(&coordinator))
        .expect("live deny commits");
    require_live_deny_before_delete(&coordinator, Some(&barrier)).expect("deny gates delete");
    // A barrier bound to another request/generation/fence never authorizes.
    let mut foreign = barrier_for(&manifest);
    foreign.purge_generation = manifest.purge_generation + 1;
    assert_eq!(
        require_live_deny_before_delete(&coordinator, Some(&foreign))
            .expect_err("foreign barrier must fail"),
        RetentionError::LiveDenyReceiptMissing
    );
}

#[test]
fn ordinary_reclaim_receipt_never_satisfies_purge_index() {
    // The ordinary retired-point path (T29) is a separate owner with separate
    // receipts; presenting one as purge index evidence is refused.
    assert_eq!(
        reject_ordinary_reclaim_as_purge(true).expect_err("ordinary receipt must fail"),
        RetentionError::IndexDeletionIncomplete
    );
    reject_ordinary_reclaim_as_purge(false).expect("purge-path marker passes");
}

#[test]
fn result_plane_invalidation_must_be_complete() {
    let (_, manifest) = coordinator_with_targets();
    verify_purge_invalidation(&full_invalidation(&manifest), &manifest)
        .expect("complete result-plane invalidation passes");
    for mutate in [
        |set: &mut PurgeInvalidationSet| set.handles_revoked = false,
        |set: &mut PurgeInvalidationSet| set.continuations_revoked = false,
        |set: &mut PurgeInvalidationSet| set.cache_ranking_dropped = false,
        |set: &mut PurgeInvalidationSet| set.overlays_detached = false,
    ] {
        let mut partial = full_invalidation(&manifest);
        mutate(&mut partial);
        assert_eq!(
            verify_purge_invalidation(&partial, &manifest)
                .expect_err("partial result plane must fail"),
            RetentionError::InvalidationIncomplete
        );
    }
    // Foreign request identity never validates.
    let mut foreign = full_invalidation(&manifest);
    foreign.request_id = oid("purge-foreign");
    assert_eq!(
        verify_purge_invalidation(&foreign, &manifest).expect_err("foreign set must fail"),
        RetentionError::InvalidationIncomplete
    );
}

#[test]
fn restart_mid_phase_resumes_without_reopening_access() {
    // Crash after live-deny commit with the fence still live resumes the same
    // phase; the fence is never dropped by recovery.
    let directive = direct_purge_resume(PurgePhase::LiveDenyCommitted, true, true);
    assert_eq!(
        directive,
        PurgeResumeDirective::ResumeFrom(PurgePhase::LiveDenyCommitted)
    );
    // Lost fence requires re-fencing before any further deletion.
    assert_eq!(
        direct_purge_resume(PurgePhase::HandlesInvalidated, false, true),
        PurgeResumeDirective::RefenceRequired
    );
    // Generation drift quarantines instead of resuming.
    assert_eq!(
        direct_purge_resume(PurgePhase::IndexDeleted, true, false),
        PurgeResumeDirective::Quarantine
    );
    // Terminal phases never resume.
    assert_eq!(
        direct_purge_resume(PurgePhase::Complete, true, true),
        PurgeResumeDirective::Quarantine
    );
    assert_eq!(
        direct_purge_resume(PurgePhase::Quarantined, true, true),
        PurgeResumeDirective::Quarantine
    );
}

#[test]
fn unavailable_plane_is_typed_unknown_and_blocks_completion() {
    let (mut coordinator, manifest) = coordinator_with_targets();
    coordinator
        .accept_live_deny(layer_receipt(&coordinator))
        .expect("live deny commits");
    // Qdrant/credential unavailable after dispatch: unknown outcome, fence
    // holds, completion stays blocked.
    coordinator
        .mark_outcome_unknown()
        .expect("unknown outcome marks");
    assert_eq!(coordinator.phase(), PurgePhase::OutcomeUnknown);
    require_live_deny_before_delete(&coordinator, Some(&barrier_for(&manifest)))
        .expect_err("unknown phase must not authorize delete");
    let statuses = vec![
        PurgePlaneStatus {
            plane: PurgePlane::LiveDeny,
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
            &[PurgePlane::LiveDeny, PurgePlane::StorageProjection]
        )
        .expect_err("unresolved plane must block"),
        RetentionError::PurgePartial
    );
}

#[test]
fn wrong_domain_and_missing_authority_never_broaden_deletion() {
    let scope = digest(0x51);
    let domain = digest(0x52);
    let op = operation("purge-op-1", 0x01);
    let authority = PurgeAuthority {
        operation: op.clone(),
        scope_digest: scope,
        domain_digest: domain,
        authorized: true,
    };
    validate_purge_authority(&authority, &op, &scope, &domain).expect("valid authority passes");
    // Missing authority never authorizes.
    let denied = PurgeAuthority {
        operation: op.clone(),
        scope_digest: scope,
        domain_digest: domain,
        authorized: false,
    };
    assert_eq!(
        validate_purge_authority(&denied, &op, &scope, &domain)
            .expect_err("missing authority must fail"),
        RetentionError::PurgeNotAuthorized
    );
    // Wrong domain never broadens deletion to another residency scope.
    let foreign_domain = PurgeAuthority {
        operation: op.clone(),
        scope_digest: scope,
        domain_digest: digest(0xFF),
        authorized: true,
    };
    assert_eq!(
        validate_purge_authority(&foreign_domain, &op, &scope, &domain)
            .expect_err("wrong domain must fail"),
        RetentionError::PurgeScopeStale
    );
    // Reused operation identity with a different request digest is a conflict.
    let replayed = PurgeAuthority {
        operation: operation("purge-op-1", 0x02),
        scope_digest: scope,
        domain_digest: domain,
        authorized: true,
    };
    assert_eq!(
        validate_purge_authority(&replayed, &op, &scope, &domain)
            .expect_err("replayed identity must fail"),
        RetentionError::PurgeScopeStale
    );
}

#[test]
fn shared_cas_object_stays_protected_while_purged_scope_is_denied() {
    // T37 sweep protection reused: a CAS object reachable from an unaffected
    // membership/hold is never a purge CAS target, even though the purged
    // scope itself is logically denied.
    let purged = BTreeSet::from([oid("purge:obj-1")]);
    let shared = BTreeSet::from([oid("shared:obj-9")]);
    check_purge_cas_targets(&purged, &shared).expect("disjoint targets pass");
    let overlapping = BTreeSet::from([oid("purge:obj-1"), oid("shared:obj-9")]);
    assert_eq!(
        check_purge_cas_targets(&overlapping, &shared)
            .expect_err("shared object must stay protected"),
        RetentionError::SweepProtectedObjectConflict
    );
}

#[test]
fn completion_gate_blocks_while_any_required_plane_is_unresolved() {
    let required = [
        PurgePlane::LiveDeny,
        PurgePlane::ResultHandles,
        PurgePlane::StorageProjection,
        PurgePlane::StorageCas,
        PurgePlane::BackupDisposition,
    ];
    let resolved = required
        .iter()
        .map(|plane| PurgePlaneStatus {
            plane: *plane,
            resolved: true,
            outcome_unknown: false,
        })
        .collect::<Vec<_>>();
    require_all_planes_resolved(&resolved, &required).expect("all resolved passes");
    // One unresolved plane blocks the terminal receipt.
    let mut partial = resolved.clone();
    partial[2].resolved = false;
    assert_eq!(
        require_all_planes_resolved(&partial, &required).expect_err("partial planes must block"),
        RetentionError::PurgePartial
    );
    // Unknown outcome also blocks, even when marked resolved.
    let mut unknown = resolved;
    unknown[3].outcome_unknown = true;
    assert_eq!(
        require_all_planes_resolved(&unknown, &required).expect_err("unknown outcome must block"),
        RetentionError::PurgePartial
    );
}

#[test]
fn no_physical_secure_erase_claim_from_unlink_or_delete() {
    // Logical non-accessibility plus absence readback is the whole claim;
    // physical secure erasure is never promised from unlink/delete.
    assert_logical_only(&PhysicalEraseEvidence::NotGuaranteed).expect("honest limit passes");
    assert_eq!(
        assert_logical_only(&PhysicalEraseEvidence::EvidenceAvailable {
            evidence_receipt: receipt("erase-claim"),
        })
        .expect_err("erase claim must fail"),
        RetentionError::SecureEraseEvidenceMissing
    );
    // The terminal tombstone carries the honest limitation.
    let (mut coordinator, _) = coordinator_with_targets();
    coordinator
        .accept_live_deny(layer_receipt(&coordinator))
        .expect("deny");
    let receipt_snapshot = layer_receipt(&coordinator);
    coordinator
        .accept_invalidation(receipt_snapshot.clone())
        .expect("handles");
    coordinator
        .accept_index_deletion(receipt_snapshot.clone())
        .expect("index");
    coordinator
        .accept_cache_deletion(receipt_snapshot.clone())
        .expect("cache");
    coordinator
        .accept_object_deletion(receipt_snapshot.clone())
        .expect("objects");
    coordinator
        .record_backup_disposition(BackupDisposition::TombstoneRetained, receipt_snapshot)
        .expect("backup");
    let tombstone = coordinator
        .complete(
            receipt("logical-absence"),
            PhysicalEraseEvidence::NotGuaranteed,
            digest(0x99),
        )
        .expect("logical purge completes");
    assert_eq!(
        tombstone.physical_erase,
        PhysicalEraseEvidence::NotGuaranteed
    );
}

#[test]
fn purge_planes_are_closed() {
    assert_eq!(PurgePlane::ALL.len(), 10);
    for plane in PurgePlane::ALL {
        assert_eq!(PurgePlane::parse(plane.as_str()), Ok(*plane));
    }
    assert!(PurgePlane::parse("forged_plane").is_err());
}

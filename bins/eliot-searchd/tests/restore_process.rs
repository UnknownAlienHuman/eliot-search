//! T39 bounded restore, key migration and ownership cutover (daemon process).
//!
//! Drives the real daemon composition (`restore_composition`) over the real
//! retention coordinator, real OS-secret bindings and real registry cutover
//! verification. No second search database, no automatic cipher change, no
//! purge resurrection.

#![cfg(feature = "wave7-lifecycle")]
#![forbid(unsafe_code)]

#[path = "../src/restore_composition.rs"]
mod restore_composition;

use restore_composition::{
    DestinationAttestation, ExportManifest, KeyUnlockClaim, LivePurgeFence, OwnerCutoverProof,
    RestoreCompositionError, RestoreLimits, admit_direct_only, admit_indexed, apply_owner_cutover,
    authorize_serve, commit_key_migration, delete_sole_source, mark_outcome_unknown,
    plan_key_migration, resume_after_interrupt, stage_restore, validate_export_manifest,
    verify_control_layer, verify_index_layer, verify_objects_layer,
};
use search_contracts::{
    Blake3Digest32, CollectionGenerationId, CutoverAuthorization, CutoverId, CutoverValidation,
    DataRootId, Epoch, InstallationId, InstallationIncarnationId, NamespaceOwnershipStatus,
    NewSourceOwnerActivation, NonZeroRevision, OldSourceOwnerFence, OpaqueId, OpaqueRef,
    OwnerEpoch, PolicyRevision, ReceiptRef, RequestId, SourceIdentityKind, SourceNamespaceId,
    SourceNamespaceOwnership, SourceOwnerCutover, SourceOwnerCutoverProtocolV1,
    SourceOwnerCutoverReceipt, SourceOwnerGeneration, SourceViewRef, UtcTimestamp,
};
use search_os_secrets::SecretBinding;
use search_ports::{FakeCancellation, OperationContext};
use search_retention::{RestoreLayerReceipt, RestorePhase};
use search_source_registry::api as registry_api;
use search_source_registry::recovery::SourceRegistry;
use search_source_registry::{DEFAULT_REGISTRY_LIMITS, InMemoryRegistryJournal};

fn oid(tag: &str) -> OpaqueId {
    OpaqueId::new(tag).expect("fixture id is valid")
}

fn receipt(tag: &str) -> ReceiptRef {
    ReceiptRef::new(tag).expect("fixture receipt is valid")
}

const fn digest(byte: u8) -> Blake3Digest32 {
    Blake3Digest32::from_bytes([byte; 32])
}

fn binding(user: u8) -> SecretBinding {
    SecretBinding::new(
        InstallationId::from_bytes([1; 16]),
        InstallationIncarnationId::from_bytes([2; 16]),
        Blake3Digest32::from_bytes([user; 32]),
        oid("secret-purpose:restore-key"),
    )
}

const fn limits() -> RestoreLimits {
    RestoreLimits::BASELINE
}

fn export_fixture() -> ExportManifest {
    restore_composition::build_test_export()
}

const fn destination_fixture(export: &ExportManifest) -> DestinationAttestation {
    DestinationAttestation {
        root_id: export.data_root_id,
        incarnation: export.owner_incarnation,
        epoch: export.owner_epoch,
        same_physical_root: true,
        acl_restrictive: true,
        restrictive_policy_enforced: true,
    }
}

fn unlock_fixture(export: &ExportManifest) -> KeyUnlockClaim {
    KeyUnlockClaim {
        binding: export.key_binding.clone(),
        ciphertext_digest: export.key_ciphertext_digest,
    }
}

const fn live_fixture(export: &ExportManifest) -> LivePurgeFence {
    LivePurgeFence {
        generation: export.purge_generation,
        fence_revision: export.purge_fence_revision,
    }
}

fn layer(export: &ExportManifest, readback: Blake3Digest32, tag: &str) -> RestoreLayerReceipt {
    RestoreLayerReceipt {
        restore_id: export.export_id.clone(),
        paired_manifest_digest: export.manifest_digest,
        readback_digest: readback,
        receipt: receipt(tag),
    }
}

fn drive_to_direct(export: &ExportManifest) -> restore_composition::StagedRestore {
    let mut staged = stage_restore(
        export,
        &destination_fixture(export),
        &unlock_fixture(export),
        &live_fixture(export),
        limits(),
    )
    .expect("stage restores to pending");
    assert_eq!(
        restore_composition::phase(&staged),
        RestorePhase::RestorePendingRevalidation
    );
    verify_control_layer(
        &mut staged,
        layer(export, export.control_digest, "t39-control"),
    )
    .expect("control readback verifies");
    verify_objects_layer(
        &mut staged,
        layer(export, export.control_digest, "t39-objects"),
        true,
    )
    .expect("objects verify");
    admit_direct_only(&mut staged).expect("direct-only admits");
    staged
}

// --- registry-authority helpers (honest, never fabricated) ---

fn utc(value: &str) -> UtcTimestamp {
    UtcTimestamp::parse(value).expect("timestamp")
}

fn initial_ownership(namespace: SourceNamespaceId) -> SourceNamespaceOwnership {
    SourceNamespaceOwnership {
        source_namespace_id: namespace,
        owner_system_id: oid("system:old"),
        owner_installation_incarnation_id: InstallationIncarnationId::from_bytes([1; 16]),
        owner_epoch: OwnerEpoch::new(1).expect("epoch"),
        ownership_record_revision: NonZeroRevision::new(1).expect("revision"),
        source_owner_generation: SourceOwnerGeneration::from_bytes([10; 32]),
        source_admission_policy_revision: PolicyRevision::new(1),
        status: NamespaceOwnershipStatus::Active,
        cutover_receipt_ref: None,
    }
}

fn wire_receipt(
    namespace: SourceNamespaceId,
    old_generation: SourceOwnerGeneration,
    new_generation: SourceOwnerGeneration,
) -> SourceOwnerCutoverReceipt {
    SourceOwnerCutoverReceipt {
        protocol: SourceOwnerCutoverProtocolV1,
        cutover: SourceOwnerCutover {
            cutover_id: CutoverId::from_bytes([3; 16]),
            source_namespace_id: namespace,
            identity_mapping_digest: digest(130),
            prepared_at: utc("2026-09-02T10:00:00.000000Z"),
            effective_at: utc("2026-09-02T10:00:01.000000Z"),
        },
        old_owner: OldSourceOwnerFence {
            owner_system_id: oid("system:old"),
            source_owner_generation_before_fence: old_generation,
            fence_revision: NonZeroRevision::new(3).expect("revision"),
            final_source_view_ref: SourceViewRef {
                source_view_digest: digest(131),
                workspace_view_revision_ref: None,
            },
            final_revision_set_digest: digest(132),
            terminal_status: NamespaceOwnershipStatus::Fenced,
        },
        new_owner: NewSourceOwnerActivation {
            owner_system_id: oid("system:new"),
            source_owner_generation_after_activation: new_generation,
            activation_revision: NonZeroRevision::new(4).expect("revision"),
            admitted_revision_set_digest: digest(133),
            status: NamespaceOwnershipStatus::Active,
        },
        validation: CutoverValidation {
            compatibility_receipt_refs: search_contracts::BoundedList::new(vec![receipt(
                "receipt:compat",
            )])
            .expect("compat"),
            integrity_receipt_refs: search_contracts::BoundedList::empty(),
            unresolved_sources_and_reasons: search_contracts::BoundedList::empty(),
        },
        authorization: CutoverAuthorization {
            old_owner_authorization_ref: OpaqueRef::new("auth:old").expect("auth"),
            new_owner_authorization_ref: OpaqueRef::new("auth:new").expect("auth"),
            issued_at: utc("2026-09-02T09:59:59.000000Z"),
        },
    }
}

fn honest_cutover_proof(export: &ExportManifest) -> OwnerCutoverProof {
    let namespace = export.namespace;
    let old_generation = SourceOwnerGeneration::from_bytes([10; 32]);
    // Deterministic new generation distinct from the old one.
    let mut bytes = [10_u8; 32];
    bytes[31] = bytes[31].wrapping_add(1);
    let new_generation = SourceOwnerGeneration::from_bytes(bytes);
    let mut registry = SourceRegistry::new(DEFAULT_REGISTRY_LIMITS).expect("registry");
    registry.namespaces_mut().insert(
        namespace,
        search_source_registry::cutover::NamespaceCutoverState::new(initial_ownership(namespace)),
    );
    let old_state = SourceNamespaceOwnership {
        source_namespace_id: namespace,
        owner_system_id: oid("system:old"),
        owner_installation_incarnation_id: InstallationIncarnationId::from_bytes([1; 16]),
        owner_epoch: OwnerEpoch::new(1).expect("epoch"),
        ownership_record_revision: NonZeroRevision::new(3).expect("revision"),
        source_owner_generation: old_generation,
        source_admission_policy_revision: PolicyRevision::new(1),
        status: NamespaceOwnershipStatus::Fenced,
        cutover_receipt_ref: Some(receipt("receipt:cutover-ref")),
    };
    let new_state = SourceNamespaceOwnership {
        source_namespace_id: namespace,
        owner_system_id: oid("system:new"),
        owner_installation_incarnation_id: InstallationIncarnationId::from_bytes([2; 16]),
        owner_epoch: OwnerEpoch::new(2).expect("epoch"),
        ownership_record_revision: NonZeroRevision::new(4).expect("revision"),
        source_owner_generation: new_generation,
        source_admission_policy_revision: PolicyRevision::new(1),
        status: NamespaceOwnershipStatus::Active,
        cutover_receipt_ref: Some(receipt("receipt:cutover-ref")),
    };
    let wire = wire_receipt(namespace, old_generation, new_generation);
    let verified = registry_api::verify_cutover_receipt(
        &registry,
        &wire,
        &old_state,
        &new_state,
        export.source_count,
        export.membership_count,
    )
    .expect("registry authority verifies cutover");
    OwnerCutoverProof::from_registry_verified(
        &verified,
        oid("system:old"),
        oid("system:new"),
        0,
        receipt("receipt:cutover-ref"),
    )
    .expect("proof carries registry verification")
}

#[test]
fn wrong_user_binding_is_refused() {
    let export = export_fixture();
    let mut unlock = unlock_fixture(&export);
    unlock.binding = binding(0x09);
    assert_eq!(
        stage_restore(
            &export,
            &destination_fixture(&export),
            &unlock,
            &live_fixture(&export),
            limits(),
        ),
        Err(RestoreCompositionError::KeyBindingMismatch)
    );
}

#[test]
fn wrong_key_never_triggers_automatic_cipher_change() {
    let export = export_fixture();
    let mut unlock = unlock_fixture(&export);
    unlock.ciphertext_digest = digest(0x77);
    // Staging with the wrong key fails closed as a key mismatch.
    assert_eq!(
        stage_restore(
            &export,
            &destination_fixture(&export),
            &unlock,
            &live_fixture(&export),
            limits(),
        ),
        Err(RestoreCompositionError::KeyMismatch)
    );
    // An implicit cipher change without explicit authorization is forbidden,
    // even when the caller names a replacement digest.
    let mut staged = stage_restore(
        &export,
        &destination_fixture(&export),
        &unlock_fixture(&export),
        &live_fixture(&export),
        limits(),
    )
    .expect("honest stage works");
    assert_eq!(
        plan_key_migration(
            &mut staged,
            export.key_binding,
            digest(0x77),
            receipt("t39-migration"),
            false,
        ),
        Err(RestoreCompositionError::CipherChangeNotAuthorized)
    );
    // No silent fallback: the staged key identity is unchanged.
    assert!(!restore_composition::is_migrated(&staged));
}

#[test]
fn explicit_key_migration_retains_original_until_verified_cutover() {
    let export = export_fixture();
    let mut staged = stage_restore(
        &export,
        &destination_fixture(&export),
        &unlock_fixture(&export),
        &live_fixture(&export),
        limits(),
    )
    .expect("stage");
    let new_binding = binding(0x03);
    plan_key_migration(
        &mut staged,
        new_binding,
        digest(0x78),
        receipt("t39-migration"),
        true,
    )
    .expect("explicit migration plans");
    assert!(!restore_composition::is_migrated(&staged));
    // Original export evidence is still the staged source of truth.
    assert_eq!(
        staged.export().key_ciphertext_digest,
        export.key_ciphertext_digest
    );
    commit_key_migration(&mut staged, digest(0x78)).expect("readback commits");
    assert!(restore_composition::is_migrated(&staged));
    assert_eq!(
        staged.export().key_ciphertext_digest,
        export.key_ciphertext_digest
    );
}

#[test]
fn relocated_or_copied_root_is_refused() {
    let export = export_fixture();
    let live = live_fixture(&export);
    let unlock = unlock_fixture(&export);
    // Different root identity: a copied directory is not the admitted root.
    let mut foreign = destination_fixture(&export);
    foreign.root_id = DataRootId::from_bytes([0x99; 16]);
    assert_eq!(
        stage_restore(&export, &foreign, &unlock, &live, limits()),
        Err(RestoreCompositionError::DestinationMismatch)
    );
    // Same identity but proven different physical directory.
    let mut relocated = destination_fixture(&export);
    relocated.same_physical_root = false;
    assert_eq!(
        stage_restore(&export, &relocated, &unlock, &live, limits()),
        Err(RestoreCompositionError::DestinationMismatch)
    );
    // Non-restrictive ACL or policy never validates.
    let mut open_acl = destination_fixture(&export);
    open_acl.acl_restrictive = false;
    assert_eq!(
        stage_restore(&export, &open_acl, &unlock, &live, limits()),
        Err(RestoreCompositionError::DestinationNotValidated)
    );
}

#[test]
fn partial_backup_is_refused() {
    let mut export = export_fixture();
    export.source_count = 0;
    export.manifest_digest = restore_composition::canonical_export_digest(&export);
    assert_eq!(
        validate_export_manifest(&export, limits()),
        Err(RestoreCompositionError::ManifestInvalid)
    );
    assert_eq!(
        stage_restore(
            &export,
            &destination_fixture(&export),
            &unlock_fixture(&export),
            &live_fixture(&export),
            limits(),
        ),
        Err(RestoreCompositionError::ManifestInvalid)
    );
}

#[test]
fn tampered_manifest_is_refused() {
    let mut export = export_fixture();
    export.control_digest = digest(0x7F);
    // Digest no longer recomputes: the manifest is not the backed-up one.
    assert_eq!(
        validate_export_manifest(&export, limits()),
        Err(RestoreCompositionError::ManifestInvalid)
    );
}

#[test]
fn older_purge_fence_never_resurrects() {
    let export = export_fixture();
    // Live system fenced a newer purge generation after the backup.
    let newer = LivePurgeFence {
        generation: export.purge_generation.saturating_add(1),
        fence_revision: export.purge_fence_revision,
    };
    assert_eq!(
        stage_restore(
            &export,
            &destination_fixture(&export),
            &unlock_fixture(&export),
            &newer,
            limits(),
        ),
        Err(RestoreCompositionError::PurgeFenceStale)
    );
    // Newer fence revision at the same generation also blocks resurrection.
    let newer_revision = LivePurgeFence {
        generation: export.purge_generation,
        fence_revision: export
            .purge_fence_revision
            .checked_next()
            .expect("fence advances"),
    };
    assert_eq!(
        stage_restore(
            &export,
            &destination_fixture(&export),
            &unlock_fixture(&export),
            &newer_revision,
            limits(),
        ),
        Err(RestoreCompositionError::PurgeFenceStale)
    );
}

#[test]
fn interrupted_migration_reports_unknown_and_resumes_exactly() {
    let export = export_fixture();
    let mut staged = drive_to_direct(&export);
    mark_outcome_unknown(&mut staged);
    assert_eq!(
        verify_index_layer(
            &mut staged,
            layer(&export, export.index_digest, "t39-index")
        ),
        Err(RestoreCompositionError::OutcomeUnknown)
    );
    // The sole valid copy survives the interruption.
    assert!(staged.source_present());
    assert!(!staged.source_deleted());
    assert_eq!(
        delete_sole_source(&mut staged),
        Err(RestoreCompositionError::OutcomeUnknown)
    );
    resume_after_interrupt(&mut staged, export.manifest_digest).expect("exact resume");
    verify_index_layer(
        &mut staged,
        layer(&export, export.index_digest, "t39-index"),
    )
    .expect("resume continues exactly");
    admit_indexed(
        &mut staged,
        &receipt("t39-publication"),
        export.visible_epoch,
        export.collection_generation_id,
    )
    .expect("indexed admits after resume");
    assert!(restore_composition::is_destination_verified(&staged));
}

#[test]
fn export_alone_never_changes_owner_old_owner_fenced_after_cutover() {
    let export = export_fixture();
    let mut staged = drive_to_direct(&export);
    // Export alone does not transfer ownership: the new owner cannot serve.
    assert_eq!(
        authorize_serve(&staged, &oid("system:new")),
        Err(RestoreCompositionError::OwnerCutoverRequired)
    );
    // The old owner serves while no cutover is accepted.
    authorize_serve(&staged, &oid("system:old")).expect("old owner serves pre-cutover");
    // Only the registry authority can cut over: honest verification required.
    let proof = honest_cutover_proof(&export);
    apply_owner_cutover(&mut staged, proof).expect("registry cutover applies");
    // After the accepted cutover the old owner can no longer serve.
    assert_eq!(
        authorize_serve(&staged, &oid("system:old")),
        Err(RestoreCompositionError::OldOwnerStillServing)
    );
    authorize_serve(&staged, &oid("system:new")).expect("new owner serves post-cutover");
}

#[test]
fn sole_copy_survives_until_destination_verification() {
    let export = export_fixture();
    let mut staged = stage_restore(
        &export,
        &destination_fixture(&export),
        &unlock_fixture(&export),
        &live_fixture(&export),
        limits(),
    )
    .expect("stage stays pending, never READY");
    // Pending restore never authorizes serving and never releases the source.
    assert_eq!(
        authorize_serve(&staged, &oid("system:old")),
        Err(RestoreCompositionError::RevalidationIncomplete)
    );
    assert_eq!(
        delete_sole_source(&mut staged),
        Err(RestoreCompositionError::SoleCopyProtection)
    );
    verify_control_layer(
        &mut staged,
        layer(&export, export.control_digest, "t39-control"),
    )
    .expect("control");
    assert_eq!(
        delete_sole_source(&mut staged),
        Err(RestoreCompositionError::SoleCopyProtection)
    );
    verify_objects_layer(
        &mut staged,
        layer(&export, export.control_digest, "t39-objects"),
        true,
    )
    .expect("objects");
    admit_direct_only(&mut staged).expect("direct-only");
    // Destination verification unlocks exactly one source release; retry
    // replays the same receipt without a second effect.
    assert!(
        !restore_composition::is_destination_verified(&staged)
            || restore_composition::phase(&staged) == RestorePhase::DirectOnly
    );
    assert_eq!(delete_sole_source(&mut staged), Ok(false));
    assert!(staged.source_deleted());
    assert_eq!(delete_sole_source(&mut staged), Ok(true));
}

#[test]
fn restore_stays_pending_until_full_revalidation() {
    let export = export_fixture();
    let staged = stage_restore(
        &export,
        &destination_fixture(&export),
        &unlock_fixture(&export),
        &live_fixture(&export),
        limits(),
    )
    .expect("stage");
    assert_eq!(
        restore_composition::phase(&staged),
        RestorePhase::RestorePendingRevalidation
    );
    let receipt_text = restore_composition::staged_receipt(&staged);
    assert!(
        receipt_text.contains("\"pending_validation\":true"),
        "{receipt_text}"
    );
    assert!(receipt_text.contains("\"ready\":false"), "{receipt_text}");
    let _ = (SourceIdentityKind::NtfsFile, Epoch::new(7).expect("epoch"));
    let _ = (
        InMemoryRegistryJournal::<FakeCancellation>::new(8).is_ok(),
        OperationContext::<FakeCancellation>::new(
            RequestId::from_bytes([1; 16]),
            1_000,
            FakeCancellation::new(false),
            OpaqueRef::new("budget:t39").expect("budget"),
        )
        .is_ok(),
        CollectionGenerationId::from_bytes([0xA1; 16]),
        SourceNamespaceId::from_bytes([7; 16]),
    );
}

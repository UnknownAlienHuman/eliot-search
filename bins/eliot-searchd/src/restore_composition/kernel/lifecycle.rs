//! Restore staging, explicit key migration and retention-owned revalidation.

use search_contracts::{
    Blake3Digest32, CollectionGenerationId, Epoch, ReceiptRef,
};
use search_os_secrets::SecretBinding;
use search_retention::{
    RestoreCoordinator, RestoreLayerReceipt, RestoreManifest,
};

use super::manifest::{ExportManifest, validate_export_manifest};
use super::model::{
    DestinationAttestation, KeyMigrationPlan, KeyUnlockClaim, LivePurgeFence,
    StagedRestore, is_destination_verified,
};
use super::spec::{RestoreCompositionError, RestoreLimits};

/// Stages a bounded restore as pending validation, never `READY`.
///
/// # Errors
///
/// Returns [`RestoreCompositionError::ManifestInvalid`] for a partial or
/// tampered export, [`RestoreCompositionError::DestinationMismatch`] for a
/// relocated or copied root, [`RestoreCompositionError::DestinationNotValidated`]
/// for an open ACL or unenforced policy,
/// [`RestoreCompositionError::KeyBindingMismatch`] for a wrong user binding,
/// [`RestoreCompositionError::KeyMismatch`] for a wrong key with no automatic
/// cipher change, or [`RestoreCompositionError::PurgeFenceStale`] when the
/// backup predates the live purge fence.
pub fn stage_restore(
    export: &ExportManifest,
    destination: &DestinationAttestation,
    unlock: &KeyUnlockClaim,
    live: &LivePurgeFence,
    limits: RestoreLimits,
) -> Result<StagedRestore, RestoreCompositionError> {
    validate_export_manifest(export, limits)?;
    if destination.root_id != export.data_root_id
        || destination.incarnation != export.owner_incarnation
        || destination.epoch != export.owner_epoch
        || !destination.same_physical_root
    {
        return Err(RestoreCompositionError::DestinationMismatch);
    }
    if !destination.acl_restrictive || !destination.restrictive_policy_enforced {
        return Err(RestoreCompositionError::DestinationNotValidated);
    }
    if unlock.binding != export.key_binding {
        return Err(RestoreCompositionError::KeyBindingMismatch);
    }
    if unlock.ciphertext_digest != export.key_ciphertext_digest {
        return Err(RestoreCompositionError::KeyMismatch);
    }
    if export.purge_generation < live.generation
        || (export.purge_generation == live.generation
            && export.purge_fence_revision.get() < live.fence_revision.get())
    {
        return Err(RestoreCompositionError::PurgeFenceStale);
    }
    let manifest = RestoreManifest {
        restore_id: export.export_id.clone(),
        control_checkpoint_digest: export.control_digest,
        index_snapshot_digest: export.index_digest,
        collection_generation_id: export.collection_generation_id,
        visible_epoch: export.visible_epoch,
        purge_tombstone_generation: export.purge_generation,
        paired_manifest_digest: export.manifest_digest,
        backup_receipt: export.backup_receipt.clone(),
    };
    let coordinator =
        RestoreCoordinator::new(manifest).map_err(|_| RestoreCompositionError::ManifestInvalid)?;
    Ok(StagedRestore {
        export: export.clone(),
        destination: *destination,
        coordinator,
        migration: None,
        migrated: false,
        cutover: None,
        source_present: true,
        source_deleted: false,
        interrupted: false,
    })
}

/// Plans an explicit key migration without touching cipher bytes implicitly.
///
/// # Errors
///
/// Returns [`RestoreCompositionError::OutcomeUnknown`] after an interruption,
/// or [`RestoreCompositionError::CipherChangeNotAuthorized`] without an
/// explicit authorization.
pub fn plan_key_migration(
    staged: &mut StagedRestore,
    new_binding: SecretBinding,
    new_ciphertext_digest: Blake3Digest32,
    evidence_receipt: ReceiptRef,
    explicit_authorization: bool,
) -> Result<(), RestoreCompositionError> {
    if staged.interrupted {
        return Err(RestoreCompositionError::OutcomeUnknown);
    }
    if !explicit_authorization {
        return Err(RestoreCompositionError::CipherChangeNotAuthorized);
    }
    staged.migration = Some(KeyMigrationPlan {
        new_binding,
        new_ciphertext_digest,
        evidence_receipt,
    });
    Ok(())
}

/// Commits a planned migration by exact new-key readback.
///
/// The staged export keeps the original evidence unchanged.
///
/// # Errors
///
/// Returns [`RestoreCompositionError::OutcomeUnknown`] after an interruption,
/// [`RestoreCompositionError::CipherChangeNotAuthorized`] with no plan, or
/// [`RestoreCompositionError::KeyMismatch`] when the readback differs.
pub fn commit_key_migration(
    staged: &mut StagedRestore,
    new_readback_digest: Blake3Digest32,
) -> Result<(), RestoreCompositionError> {
    if staged.interrupted {
        return Err(RestoreCompositionError::OutcomeUnknown);
    }
    let plan = staged
        .migration
        .as_ref()
        .ok_or(RestoreCompositionError::CipherChangeNotAuthorized)?;
    if new_readback_digest != plan.new_ciphertext_digest {
        return Err(RestoreCompositionError::KeyMismatch);
    }
    staged.migrated = true;
    Ok(())
}

const fn map_retention(error: search_retention::RetentionError) -> RestoreCompositionError {
    use search_retention::RetentionError as E;
    match error {
        E::RestoreManifestInvalid => RestoreCompositionError::ManifestInvalid,
        E::RestoreRevalidationIncomplete | E::IndexedRestoreNotAdmitted => {
            RestoreCompositionError::RevalidationIncomplete
        }
        E::OutcomeUnknown => RestoreCompositionError::OutcomeUnknown,
        _ => RestoreCompositionError::Quarantined,
    }
}

/// Verifies the exact control checkpoint readback.
pub fn verify_control_layer(
    staged: &mut StagedRestore,
    receipt: RestoreLayerReceipt,
) -> Result<(), RestoreCompositionError> {
    if staged.interrupted {
        return Err(RestoreCompositionError::OutcomeUnknown);
    }
    staged
        .coordinator
        .verify_control(receipt)
        .map_err(map_retention)
}

/// Verifies the exact restored source/object readback.
pub fn verify_objects_layer(
    staged: &mut StagedRestore,
    receipt: RestoreLayerReceipt,
    all_objects_valid: bool,
) -> Result<(), RestoreCompositionError> {
    if staged.interrupted {
        return Err(RestoreCompositionError::OutcomeUnknown);
    }
    staged
        .coordinator
        .verify_objects(receipt, all_objects_valid)
        .map_err(map_retention)
}

/// Admits direct-only serving after control and object verification.
pub fn admit_direct_only(staged: &mut StagedRestore) -> Result<(), RestoreCompositionError> {
    if staged.interrupted {
        return Err(RestoreCompositionError::OutcomeUnknown);
    }
    staged
        .coordinator
        .admit_direct_only()
        .map_err(map_retention)
}

/// Verifies the exact index snapshot readback while staying direct-only.
pub fn verify_index_layer(
    staged: &mut StagedRestore,
    receipt: RestoreLayerReceipt,
) -> Result<(), RestoreCompositionError> {
    if staged.interrupted {
        return Err(RestoreCompositionError::OutcomeUnknown);
    }
    staged
        .coordinator
        .verify_index(receipt)
        .map_err(map_retention)
}

/// Admits indexed serving only after a current publication receipt.
pub fn admit_indexed(
    staged: &mut StagedRestore,
    publication_receipt: &ReceiptRef,
    current_epoch: Epoch,
    current_generation: CollectionGenerationId,
) -> Result<(), RestoreCompositionError> {
    if staged.interrupted {
        return Err(RestoreCompositionError::OutcomeUnknown);
    }
    staged
        .coordinator
        .admit_indexed(publication_receipt, current_epoch, current_generation)
        .map_err(map_retention)
}

/// Releases the sole valid copy only after destination verification.
///
/// The first call deletes exactly once; a retry replays the same receipt.
pub const fn delete_sole_source(
    staged: &mut StagedRestore,
) -> Result<bool, RestoreCompositionError> {
    if staged.interrupted {
        return Err(RestoreCompositionError::OutcomeUnknown);
    }
    if staged.source_deleted {
        return Ok(true);
    }
    if !is_destination_verified(staged) {
        return Err(RestoreCompositionError::SoleCopyProtection);
    }
    staged.source_deleted = true;
    staged.source_present = false;
    Ok(false)
}

/// Marks an ambiguous mutation without advancing success.
pub const fn mark_outcome_unknown(staged: &mut StagedRestore) {
    staged.interrupted = true;
}

/// Resumes exactly after an interruption without touching the sole copy.
pub fn resume_after_interrupt(
    staged: &mut StagedRestore,
    expected_digest: Blake3Digest32,
) -> Result<(), RestoreCompositionError> {
    if !staged.interrupted {
        return Ok(());
    }
    if expected_digest != staged.export.manifest_digest {
        return Err(RestoreCompositionError::Quarantined);
    }
    staged.interrupted = false;
    Ok(())
}

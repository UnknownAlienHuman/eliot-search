//! Bounded restore, explicit key migration and registry-owned cutover (T39).
//!
//! Daemon-side composition over three accepted authorities, owned nowhere
//! else here:
//!
//! - [`search_retention::RestoreCoordinator`] owns the restore revalidation
//!   phases. A staged restore enters pending validation, never `READY`.
//! - [`search_os_secrets::SecretBinding`] owns key identity. A wrong user or
//!   wrong key fails closed; a changed cipher without an explicit migration
//!   plan is forbidden, never silently re-encrypted.
//! - `search-source-registry` owns namespace-owner transitions through
//!   [`transition_namespace_owner`](search_source_registry::cutover::transition_namespace_owner)
//!   and [`VerifiedCutoverReceipt`]. This module never reimplements that
//!   automaton: it only consumes a registry-verified receipt, requires
//!   old-owner fencing, exact source accounting and zero unresolved sources.
//!
//! Additional daemon gates composed here:
//!
//! - destination root/ACL/policy revalidation (a relocated or copied root is
//!   a different root, never the admitted one);
//! - purge-fence staleness (a backup older than the live fence never
//!   resurrects purged state; restore after purge is a typed refusal);
//! - sole-copy protection (recovery or retry never destroys the sole valid
//!   retained copy before destination verification).
//!
//! Wiring (integration owner): add to `entry.rs`
//!
//! ```text
//! #[cfg(feature = "wave7-lifecycle")]
//! mod restore_composition;
//! ```
//!
//! What this module never does:
//!
//! - No automatic cipher change or provider switching.
//! - No purge resurrection and no reuse of a purged generation.
//! - No second search database and no unbounded inventory.
//! - No silent fallback: every contradiction is a typed error.

#![forbid(unsafe_code)]

use search_contracts::{
    Blake3Digest32, CollectionGenerationId, DataRootId, Epoch, InstallationIncarnationId, OpaqueId,
    OwnerEpoch, PurgeFenceRevision, ReceiptRef, SourceNamespaceId, SourceOwnerGeneration,
};
use search_os_secrets::SecretBinding;
use search_retention::{RestoreCoordinator, RestoreLayerReceipt, RestoreManifest, RestorePhase};
use search_source_registry::cutover::VerifiedCutoverReceipt;

/// Finite restore inventory limits.
#[allow(clippy::struct_field_names)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RestoreLimits {
    /// Maximum sources carried by one export.
    pub max_sources: usize,
    /// Maximum memberships carried by one export.
    pub max_memberships: usize,
    /// Maximum canonical manifest bytes hashed for the digest.
    pub max_manifest_bytes: usize,
}

impl RestoreLimits {
    /// Conservative local baseline.
    pub const BASELINE: Self = Self {
        max_sources: 1_024,
        max_memberships: 1_024,
        max_manifest_bytes: 64 * 1_024,
    };

    /// Validates finite non-zero limits.
    ///
    /// # Errors
    ///
    /// Returns [`RestoreCompositionError::CapacityExceeded`] when any limit is zero.
    pub const fn validate(self) -> Result<Self, RestoreCompositionError> {
        if self.max_sources == 0 || self.max_memberships == 0 || self.max_manifest_bytes == 0 {
            Err(RestoreCompositionError::CapacityExceeded)
        } else {
            Ok(self)
        }
    }
}

/// Closed restore-composition failure with a stable machine code.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RestoreCompositionError {
    /// Export manifest is partial, tampered or unpaired.
    ManifestInvalid,
    /// Destination names a different or relocated root.
    DestinationMismatch,
    /// Destination ACL or restrictive policy is not enforced.
    DestinationNotValidated,
    /// Key user, installation, incarnation or purpose binding differs.
    KeyBindingMismatch,
    /// Key bytes differ without an explicit migration.
    KeyMismatch,
    /// Cipher bytes would change without explicit authorization.
    CipherChangeNotAuthorized,
    /// Backup predates the live purge fence and must not resurrect.
    PurgeFenceStale,
    /// Ownership change requires a registry-verified cutover.
    OwnerCutoverRequired,
    /// Old owner attempted to serve after an accepted cutover.
    OldOwnerStillServing,
    /// Source deletion would destroy the sole valid copy.
    SoleCopyProtection,
    /// Restore has not completed exact readback.
    RevalidationIncomplete,
    /// External mutation outcome requires exact recovery.
    OutcomeUnknown,
    /// Contradictory state requires quarantine.
    Quarantined,
    /// Finite capacity was exhausted.
    CapacityExceeded,
}

impl RestoreCompositionError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::ManifestInvalid => "RESTORE_MANIFEST_INVALID",
            Self::DestinationMismatch => "RESTORE_DESTINATION_MISMATCH",
            Self::DestinationNotValidated => "RESTORE_DESTINATION_NOT_VALIDATED",
            Self::KeyBindingMismatch => "RESTORE_KEY_BINDING_MISMATCH",
            Self::KeyMismatch => "RESTORE_KEY_MISMATCH",
            Self::CipherChangeNotAuthorized => "RESTORE_CIPHER_CHANGE_NOT_AUTHORIZED",
            Self::PurgeFenceStale => "RESTORE_PURGE_FENCE_STALE",
            Self::OwnerCutoverRequired => "RESTORE_OWNER_CUTOVER_REQUIRED",
            Self::OldOwnerStillServing => "RESTORE_OLD_OWNER_STILL_SERVING",
            Self::SoleCopyProtection => "RESTORE_SOLE_COPY_PROTECTION",
            Self::RevalidationIncomplete => "RESTORE_REVALIDATION_INCOMPLETE",
            Self::OutcomeUnknown => "RESTORE_OUTCOME_UNKNOWN",
            Self::Quarantined => "RESTORE_QUARANTINED",
            Self::CapacityExceeded => "RESTORE_CAPACITY_EXCEEDED",
        }
    }
}

impl core::fmt::Display for RestoreCompositionError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for RestoreCompositionError {}

/// Content-safe immutable export manifest with lineage.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExportManifest {
    /// Stable export identity.
    pub export_id: OpaqueId,
    /// Source namespace exported.
    pub namespace: SourceNamespaceId,
    /// Namespace owner at export time (old owner for cutover).
    pub old_owner: OpaqueId,
    /// Data-root owner incarnation at export time.
    pub owner_incarnation: InstallationIncarnationId,
    /// Data-root owner epoch at export time.
    pub owner_epoch: OwnerEpoch,
    /// Admitted data root at export time.
    pub data_root_id: DataRootId,
    /// Collection generation restored.
    pub collection_generation_id: CollectionGenerationId,
    /// Visible epoch claimed by the backup.
    pub visible_epoch: Epoch,
    /// Control checkpoint digest.
    pub control_digest: Blake3Digest32,
    /// Index snapshot digest.
    pub index_digest: Blake3Digest32,
    /// Opaque key reference at export time.
    pub key_reference_id: OpaqueId,
    /// Exact key binding at export time.
    pub key_binding: SecretBinding,
    /// Ciphertext digest at export time (cipher identity).
    pub key_ciphertext_digest: Blake3Digest32,
    /// Residency/authority domain digest.
    pub domain_digest: Blake3Digest32,
    /// Purge tombstone generation included in the backup.
    pub purge_generation: u64,
    /// Purge fence revision included in the backup.
    pub purge_fence_revision: PurgeFenceRevision,
    /// Backup provenance receipt.
    pub backup_receipt: ReceiptRef,
    /// Exact exported source count.
    pub source_count: usize,
    /// Exact exported membership count.
    pub membership_count: usize,
    /// Digest over the exact canonical export encoding.
    pub manifest_digest: Blake3Digest32,
}

/// Computes the exact canonical export digest with real BLAKE3.
///
/// Canonical encoding: domain tag, then every field except `manifest_digest`
/// in declaration order with fixed-width integers and length-prefixed text.
/// The digest never covers itself, so tampering with any covered byte breaks
/// revalidation.
///
/// # Panics
///
/// Panics when a `usize` count does not fit in `u64`, which indicates a
/// platform that cannot represent the bounded test inventory.
#[must_use]
pub fn canonical_export_digest(manifest: &ExportManifest) -> Blake3Digest32 {
    let mut input = Vec::with_capacity(256);
    input.extend_from_slice(b"eliot-search/restore-export/v1\x00");
    push_str(&mut input, manifest.export_id.as_str());
    input.extend_from_slice(manifest.namespace.as_bytes());
    push_str(&mut input, manifest.old_owner.as_str());
    input.extend_from_slice(manifest.owner_incarnation.as_bytes());
    input.extend_from_slice(&manifest.owner_epoch.get().to_le_bytes());
    input.extend_from_slice(manifest.data_root_id.as_bytes());
    input.extend_from_slice(manifest.collection_generation_id.as_bytes());
    input.extend_from_slice(&manifest.visible_epoch.get().to_le_bytes());
    input.extend_from_slice(manifest.control_digest.as_bytes());
    input.extend_from_slice(manifest.index_digest.as_bytes());
    push_str(&mut input, manifest.key_reference_id.as_str());
    input.extend_from_slice(manifest.key_binding.installation_id().as_bytes());
    input.extend_from_slice(
        manifest
            .key_binding
            .installation_incarnation_id()
            .as_bytes(),
    );
    input.extend_from_slice(manifest.key_binding.user_scope_digest().as_bytes());
    push_str(&mut input, manifest.key_binding.purpose().as_str());
    input.extend_from_slice(manifest.key_ciphertext_digest.as_bytes());
    input.extend_from_slice(manifest.domain_digest.as_bytes());
    input.extend_from_slice(&manifest.purge_generation.to_le_bytes());
    input.extend_from_slice(&manifest.purge_fence_revision.get().to_le_bytes());
    push_str(&mut input, manifest.backup_receipt.as_str());
    input.extend_from_slice(&u64::try_from(manifest.source_count).unwrap().to_le_bytes());
    input.extend_from_slice(
        &u64::try_from(manifest.membership_count)
            .unwrap()
            .to_le_bytes(),
    );
    Blake3Digest32::from_bytes(*blake3::hash(&input).as_bytes())
}

fn push_str(output: &mut Vec<u8>, value: &str) {
    let len = u64::try_from(value.len()).unwrap_or(u64::MAX);
    output.extend_from_slice(&len.to_le_bytes());
    output.extend_from_slice(value.as_bytes());
}

/// Validates a staged export inventory.
///
/// # Errors
///
/// Returns [`RestoreCompositionError::ManifestInvalid`] for a partial,
/// tampered or unbounded manifest, or [`RestoreCompositionError::CapacityExceeded`]
/// for zero limits.
pub fn validate_export_manifest(
    manifest: &ExportManifest,
    limits: RestoreLimits,
) -> Result<(), RestoreCompositionError> {
    let limits = limits.validate()?;
    if manifest.source_count == 0
        || manifest.source_count > limits.max_sources
        || manifest.membership_count > limits.max_memberships
        || manifest.purge_generation == 0
    {
        return Err(RestoreCompositionError::ManifestInvalid);
    }
    if canonical_export_digest(manifest) != manifest.manifest_digest {
        return Err(RestoreCompositionError::ManifestInvalid);
    }
    Ok(())
}

/// Destination revalidation attestation supplied by the caller.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DestinationAttestation {
    /// Data root the caller restores into.
    pub root_id: DataRootId,
    /// Live data-root owner incarnation.
    pub incarnation: InstallationIncarnationId,
    /// Live data-root owner epoch.
    pub epoch: OwnerEpoch,
    /// Whether the destination is the same physical directory.
    pub same_physical_root: bool,
    /// Whether the destination ACL is restrictive.
    pub acl_restrictive: bool,
    /// Whether the restrictive policy is enforced at the destination.
    pub restrictive_policy_enforced: bool,
}

/// Live purge fence the backup must not predate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LivePurgeFence {
    /// Live purge generation.
    pub generation: u64,
    /// Live purge-fence revision.
    pub fence_revision: PurgeFenceRevision,
}

/// Claimed key unlock for the staged restore.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KeyUnlockClaim {
    /// Claimed key binding (wrong user fails closed).
    pub binding: SecretBinding,
    /// Claimed ciphertext digest (wrong key fails closed).
    pub ciphertext_digest: Blake3Digest32,
}

/// Explicit key/domain migration plan.
///
/// The staged export (original evidence) is retained unchanged until a
/// verified cutover consumes the migration. Nothing here performs I/O: the
/// caller moves key material and hands back the exact readback digest at
/// commit time.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KeyMigrationPlan {
    /// Replacement key binding.
    pub new_binding: SecretBinding,
    /// Replacement ciphertext digest.
    pub new_ciphertext_digest: Blake3Digest32,
    /// Content-free migration authorization receipt.
    pub evidence_receipt: ReceiptRef,
}

/// Registry-verified ownership cutover proof.
///
/// The only constructor consumes a [`VerifiedCutoverReceipt`] produced by the
/// registry authority (`search-source-registry`), which already proved
/// fence-before-activation. Export bytes alone can never construct a verified
/// proof.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OwnerCutoverProof {
    /// Namespace under cutover.
    pub namespace: SourceNamespaceId,
    /// Fenced old owner.
    pub old_owner: OpaqueId,
    /// Activated new owner.
    pub new_owner: OpaqueId,
    /// Owner generation before the fence.
    pub old_generation: SourceOwnerGeneration,
    /// Owner generation after activation.
    pub new_generation: SourceOwnerGeneration,
    /// Covered source count (must equal the export inventory).
    pub covered_sources: usize,
    /// Covered membership count (must equal the export inventory).
    pub covered_memberships: usize,
    /// Cutover authorization receipt reference.
    pub receipt_ref: ReceiptRef,
}

impl OwnerCutoverProof {
    /// Carries a registry-verified receipt into the restore composition.
    ///
    /// # Errors
    ///
    /// Returns [`RestoreCompositionError::OwnerCutoverRequired`] when the
    /// receipt does not advance the owner generation, reuses the same owner,
    /// or leaves any source unresolved.
    pub fn from_registry_verified(
        receipt: &VerifiedCutoverReceipt,
        old_owner: OpaqueId,
        new_owner: OpaqueId,
        unresolved_sources: usize,
        receipt_ref: ReceiptRef,
    ) -> Result<Self, RestoreCompositionError> {
        if unresolved_sources != 0 {
            return Err(RestoreCompositionError::OwnerCutoverRequired);
        }
        if old_owner == new_owner || receipt.old_generation == receipt.new_generation {
            return Err(RestoreCompositionError::OwnerCutoverRequired);
        }
        Ok(Self {
            namespace: receipt.namespace_id,
            old_owner,
            new_owner,
            old_generation: receipt.old_generation,
            new_generation: receipt.new_generation,
            covered_sources: receipt.covered_sources,
            covered_memberships: receipt.covered_memberships,
            receipt_ref,
        })
    }
}

/// Pending-validation staged restore.
///
/// The sole valid retained copy stays present until [`delete_sole_source`]
/// proves destination verification. No constructor activates serving.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Debug)]
pub struct StagedRestore {
    export: ExportManifest,
    destination: DestinationAttestation,
    coordinator: RestoreCoordinator,
    migration: Option<KeyMigrationPlan>,
    migrated: bool,
    cutover: Option<OwnerCutoverProof>,
    source_present: bool,
    source_deleted: bool,
    interrupted: bool,
}

impl PartialEq for StagedRestore {
    fn eq(&self, other: &Self) -> bool {
        self.export == other.export
            && self.destination == other.destination
            && self.migration == other.migration
            && self.migrated == other.migrated
            && self.cutover == other.cutover
            && self.source_present == other.source_present
            && self.source_deleted == other.source_deleted
            && self.interrupted == other.interrupted
            && self.coordinator.phase() == other.coordinator.phase()
    }
}

impl StagedRestore {
    /// Exact staged export (original evidence, retained until cutover).
    #[must_use]
    pub const fn export(&self) -> &ExportManifest {
        &self.export
    }

    /// Whether the sole source copy is still present.
    #[must_use]
    pub const fn source_present(&self) -> bool {
        self.source_present
    }

    /// Whether the sole source copy was released after verification.
    #[must_use]
    pub const fn source_deleted(&self) -> bool {
        self.source_deleted
    }
}

/// Current restore phase (retention-owned).
#[must_use]
pub const fn phase(staged: &StagedRestore) -> RestorePhase {
    staged.coordinator.phase()
}

/// Whether an explicit key migration committed.
#[must_use]
pub const fn is_migrated(staged: &StagedRestore) -> bool {
    staged.migrated
}

/// Whether the destination verified enough to serve or release the source.
#[must_use]
pub const fn is_destination_verified(staged: &StagedRestore) -> bool {
    matches!(
        staged.coordinator.phase(),
        RestorePhase::DirectOnly
            | RestorePhase::IndexReadbackVerified
            | RestorePhase::IndexedAdmitted
    )
}

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

/// Applies a registry-verified ownership cutover.
///
/// # Errors
///
/// Returns [`RestoreCompositionError::OutcomeUnknown`] after an interruption,
/// or [`RestoreCompositionError::OwnerCutoverRequired`] when the proof does
/// not bind this export inventory exactly.
pub fn apply_owner_cutover(
    staged: &mut StagedRestore,
    proof: OwnerCutoverProof,
) -> Result<(), RestoreCompositionError> {
    if staged.interrupted {
        return Err(RestoreCompositionError::OutcomeUnknown);
    }
    if staged.cutover.is_some() {
        return Err(RestoreCompositionError::OwnerCutoverRequired);
    }
    if proof.namespace != staged.export.namespace
        || proof.old_owner != staged.export.old_owner
        || proof.new_owner == proof.old_owner
        || proof.old_generation == proof.new_generation
        || proof.covered_sources != staged.export.source_count
        || proof.covered_memberships != staged.export.membership_count
    {
        return Err(RestoreCompositionError::OwnerCutoverRequired);
    }
    staged.cutover = Some(proof);
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
///
/// # Errors
///
/// Returns [`RestoreCompositionError::OutcomeUnknown`] after an interruption,
/// or the mapped retention revalidation failure.
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
///
/// # Errors
///
/// Returns [`RestoreCompositionError::OutcomeUnknown`] after an interruption,
/// or the mapped retention revalidation failure.
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
///
/// # Errors
///
/// Returns [`RestoreCompositionError::OutcomeUnknown`] after an interruption,
/// or [`RestoreCompositionError::RevalidationIncomplete`] out of order.
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
///
/// # Errors
///
/// Returns [`RestoreCompositionError::OutcomeUnknown`] after an interruption,
/// or the mapped retention revalidation failure.
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
///
/// # Errors
///
/// Returns [`RestoreCompositionError::OutcomeUnknown`] after an interruption,
/// or [`RestoreCompositionError::RevalidationIncomplete`] without full
/// readback and a current publication.
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

/// Authorizes serving as one explicit owner.
///
/// Export alone never changes the owner: before an accepted cutover only the
/// old owner may serve, and afterwards the old owner is fenced.
///
/// # Errors
///
/// Returns [`RestoreCompositionError::OutcomeUnknown`] after an interruption,
/// [`RestoreCompositionError::OldOwnerStillServing`] when the fenced old
/// owner attempts to serve, [`RestoreCompositionError::OwnerCutoverRequired`]
/// when no verified cutover backs a new owner, or
/// [`RestoreCompositionError::RevalidationIncomplete`] before destination
/// verification.
pub fn authorize_serve(
    staged: &StagedRestore,
    as_owner: &OpaqueId,
) -> Result<(), RestoreCompositionError> {
    if staged.interrupted {
        return Err(RestoreCompositionError::OutcomeUnknown);
    }
    match &staged.cutover {
        Some(proof) => {
            if as_owner == &staged.export.old_owner {
                return Err(RestoreCompositionError::OldOwnerStillServing);
            }
            if as_owner != &proof.new_owner {
                return Err(RestoreCompositionError::OwnerCutoverRequired);
            }
        }
        None => {
            if as_owner != &staged.export.old_owner {
                return Err(RestoreCompositionError::OwnerCutoverRequired);
            }
        }
    }
    if !is_destination_verified(staged) {
        return Err(RestoreCompositionError::RevalidationIncomplete);
    }
    Ok(())
}

/// Releases the sole valid copy only after destination verification.
///
/// The first call deletes exactly once; a retry replays the same receipt.
///
/// # Errors
///
/// Returns [`RestoreCompositionError::OutcomeUnknown`] after an interruption,
/// or [`RestoreCompositionError::SoleCopyProtection`] before destination
/// verification.
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
///
/// # Errors
///
/// Returns [`RestoreCompositionError::Quarantined`] when the expected digest
/// does not bind this staged export.
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

/// Deterministic staged receipt: no timestamps, no randomness.
#[must_use]
pub fn staged_receipt(staged: &StagedRestore) -> String {
    let (phase_name, pending) = match phase(staged) {
        RestorePhase::RestorePendingRevalidation => ("restore-pending-revalidation", true),
        RestorePhase::ControlReadbackVerified => ("control-readback-verified", true),
        RestorePhase::ObjectsReadbackVerified => ("objects-readback-verified", true),
        RestorePhase::DirectOnly => ("direct-only", false),
        RestorePhase::IndexReadbackVerified => ("index-readback-verified", false),
        RestorePhase::IndexedAdmitted => ("indexed-admitted", false),
        RestorePhase::Quarantined => ("quarantined", false),
    };
    format!(
        concat!(
            "{{\"event\":\"restore-staged\",\"schema\":\"eliot.restore-staging.v1\",",
            "\"export_id\":\"{}\",\"phase\":\"{}\",\"pending_validation\":{},\"ready\":{},",
            "\"migrated\":{},\"cutover\":{},\"source_present\":{},\"source_deleted\":{},",
            "\"interrupted\":{}}}"
        ),
        staged.export.export_id.as_str(),
        phase_name,
        pending,
        matches!(phase(staged), RestorePhase::IndexedAdmitted),
        staged.migrated,
        staged.cutover.is_some(),
        staged.source_present,
        staged.source_deleted,
        staged.interrupted,
    )
}

/// Deterministic fixture export for tests: never reads the environment.
#[must_use]
pub fn build_test_export() -> ExportManifest {
    use search_contracts::{InstallationId, InstallationIncarnationId};
    let key_binding = SecretBinding::new(
        InstallationId::from_bytes([1; 16]),
        InstallationIncarnationId::from_bytes([2; 16]),
        Blake3Digest32::from_bytes([0x02; 32]),
        OpaqueId::new("secret-purpose:restore-key").expect("purpose"),
    );
    let mut manifest = ExportManifest {
        export_id: OpaqueId::new("t39-export-1").expect("export id"),
        namespace: SourceNamespaceId::from_bytes([7; 16]),
        old_owner: OpaqueId::new("system:old").expect("old owner"),
        owner_incarnation: InstallationIncarnationId::from_bytes([2; 16]),
        owner_epoch: OwnerEpoch::new(3).expect("epoch"),
        data_root_id: DataRootId::from_bytes([0x22; 16]),
        collection_generation_id: CollectionGenerationId::from_bytes([0xA1; 16]),
        visible_epoch: Epoch::new(7).expect("visible epoch"),
        control_digest: Blake3Digest32::from_bytes([0xC1; 32]),
        index_digest: Blake3Digest32::from_bytes([0xC2; 32]),
        key_reference_id: OpaqueId::new("secret:restore-key-1").expect("key ref"),
        key_binding,
        key_ciphertext_digest: Blake3Digest32::from_bytes([0x5A; 32]),
        domain_digest: Blake3Digest32::from_bytes([0xD0; 32]),
        purge_generation: 9,
        purge_fence_revision: PurgeFenceRevision::new(6),
        backup_receipt: ReceiptRef::new("t39-backup").expect("backup receipt"),
        source_count: 1,
        membership_count: 0,
        manifest_digest: Blake3Digest32::from_bytes([0; 32]),
    };
    manifest.manifest_digest = canonical_export_digest(&manifest);
    manifest
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_digest_is_deterministic() {
        let first = build_test_export();
        let second = build_test_export();
        assert_eq!(first.manifest_digest, second.manifest_digest);
        assert_eq!(canonical_export_digest(&first), first.manifest_digest);
    }

    #[test]
    fn pending_stage_never_reports_ready() {
        let export = build_test_export();
        let destination = DestinationAttestation {
            root_id: export.data_root_id,
            incarnation: export.owner_incarnation,
            epoch: export.owner_epoch,
            same_physical_root: true,
            acl_restrictive: true,
            restrictive_policy_enforced: true,
        };
        let unlock = KeyUnlockClaim {
            binding: export.key_binding.clone(),
            ciphertext_digest: export.key_ciphertext_digest,
        };
        let live = LivePurgeFence {
            generation: export.purge_generation,
            fence_revision: export.purge_fence_revision,
        };
        let staged = stage_restore(
            &export,
            &destination,
            &unlock,
            &live,
            RestoreLimits::BASELINE,
        )
        .expect("stage stays pending");
        assert_eq!(phase(&staged), RestorePhase::RestorePendingRevalidation);
        let receipt = staged_receipt(&staged);
        assert!(receipt.contains("\"pending_validation\":true"), "{receipt}");
        assert!(receipt.contains("\"ready\":false"), "{receipt}");
    }
}

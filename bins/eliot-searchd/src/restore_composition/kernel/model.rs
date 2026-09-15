//! Staged restore state and immutable input attestations.

use search_contracts::{
    Blake3Digest32, DataRootId, InstallationIncarnationId, OwnerEpoch,
    PurgeFenceRevision, ReceiptRef,
};
use search_os_secrets::SecretBinding;
use search_retention::{RestoreCoordinator, RestorePhase};

use super::cutover::OwnerCutoverProof;
use super::manifest::ExportManifest;

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

/// Pending-validation staged restore.
///
/// The sole valid retained copy stays present until `delete_sole_source`
/// proves destination verification. No constructor activates serving.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Debug)]
pub struct StagedRestore {
    pub(super) export: ExportManifest,
    pub(super) destination: DestinationAttestation,
    pub(super) coordinator: RestoreCoordinator,
    pub(super) migration: Option<KeyMigrationPlan>,
    pub(super) migrated: bool,
    pub(super) cutover: Option<OwnerCutoverProof>,
    pub(super) source_present: bool,
    pub(super) source_deleted: bool,
    pub(super) interrupted: bool,
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

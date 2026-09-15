//! Immutable export manifest, canonical BLAKE3 identity and finite validation.

use search_contracts::{
    Blake3Digest32, CollectionGenerationId, DataRootId, Epoch,
    InstallationIncarnationId, OpaqueId, OwnerEpoch, PurgeFenceRevision,
    ReceiptRef, SourceNamespaceId,
};
use search_os_secrets::SecretBinding;

use super::spec::{RestoreCompositionError, RestoreLimits};

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

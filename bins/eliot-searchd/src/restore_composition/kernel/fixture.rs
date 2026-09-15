//! Deterministic fixture export used by restore regressions.

use search_contracts::{
    Blake3Digest32, CollectionGenerationId, DataRootId, Epoch, InstallationId,
    InstallationIncarnationId, OpaqueId, OwnerEpoch, PurgeFenceRevision,
    ReceiptRef, SourceNamespaceId,
};
use search_os_secrets::SecretBinding;

use super::manifest::{ExportManifest, canonical_export_digest};

/// Deterministic fixture export for tests: never reads the environment.
#[must_use]
pub fn build_test_export() -> ExportManifest {
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

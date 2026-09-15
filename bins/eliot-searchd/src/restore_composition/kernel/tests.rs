use super::*;

use search_retention::RestorePhase;

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

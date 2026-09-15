use std::path::{Path, PathBuf};

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

#[test]
fn rebuild_entry_and_kernel_are_thin() {
    let root = crate_root();
    let entry = read(&root, "src/rebuild_composition.rs");
    assert!(entry.contains("#[path = \"rebuild_composition/kernel.rs\"]"));
    assert!(entry.contains("pub use kernel::*;"));
    assert!(entry.len() < 2_000, "entry grew to {} bytes", entry.len());
    for forbidden in [
        "pub struct RetainedManifest",
        "pub fn propose_rebuild(",
        "pub fn authorize_reclaim(",
        "PinRegistry",
        "ReclaimPlan",
        "qdrant_client",
        "tokio::",
    ] {
        assert!(
            !entry.contains(forbidden),
            "rebuild implementation returned to facade: {forbidden}"
        );
    }

    let kernel = read(&root, "src/rebuild_composition/kernel.rs");
    for module in ["cutover", "error", "manifest", "pins", "plan", "reclaim"] {
        assert!(kernel.contains(&format!("mod {module};")));
    }
    assert!(kernel.contains("pub use manifest::{"));
    assert!(kernel.contains("pub use reclaim::{"));
    assert!(kernel.len() < 3_500, "kernel grew to {} bytes", kernel.len());
}

#[test]
fn rebuild_owners_are_bounded_and_transport_free() {
    let root = crate_root();
    let owners = [
        ("src/rebuild_composition/kernel/error.rs", "pub enum RebuildError"),
        (
            "src/rebuild_composition/kernel/manifest.rs",
            "pub struct RetainedManifest",
        ),
        (
            "src/rebuild_composition/kernel/plan.rs",
            "pub fn propose_rebuild(",
        ),
        (
            "src/rebuild_composition/kernel/cutover.rs",
            "pub fn commit_cutover(",
        ),
        (
            "src/rebuild_composition/kernel/pins.rs",
            "pub fn begin_pinned_query(",
        ),
        (
            "src/rebuild_composition/kernel/reclaim.rs",
            "pub fn authorize_reclaim(",
        ),
    ];
    for (relative, marker) in owners {
        let source = read(&root, relative);
        assert!(source.contains(marker), "{relative} lost owner {marker}");
        assert!(
            source.len() < 14_000,
            "{relative} grew to {} bytes",
            source.len()
        );
        for forbidden in [
            "qdrant_client",
            "search_qdrant_bridge",
            "RealDataPlane",
            "QdrantBridge",
            "reqwest::",
            "tokio::",
            "std::fs",
            "Command::new",
        ] {
            assert!(
                !source.contains(forbidden),
                "{relative} acquired transport/effect token {forbidden}"
            );
        }
    }

    let manifest = read(&root, "src/rebuild_composition/kernel/manifest.rs");
    assert!(manifest.contains("eliot-search/rebuild-manifest/v1\\x00"));
    assert!(manifest.contains("pair[0].id >= pair[1].id"));

    let plan = read(&root, "src/rebuild_composition/kernel/plan.rs");
    assert!(plan.contains("eliot-search/rebuild-plan/v1\\x00"));
    assert!(plan.contains("new_generation == old_route.collection_generation_id"));
    assert!(plan.contains("checked_next()"));
    assert!(plan.contains("readback.missing.is_empty()"));
    assert!(plan.contains("readback.unexpected.is_empty()"));

    let cutover = read(&root, "src/rebuild_composition/kernel/cutover.rs");
    assert!(cutover.contains("eliot-search/rebuild-staged/v1\\x00"));
    assert!(cutover.contains("eliot-search/rebuild-commit/v1\\x00"));
    assert!(cutover.contains("proof.plan_digest != staged.plan_digest"));

    let pins = read(&root, "src/rebuild_composition/kernel/pins.rs");
    assert!(pins.contains("acquire_epoch_pin"));
    assert!(pins.contains("release_owner_pins"));
    assert!(pins.contains("expire_continuation_pins"));
    assert!(!pins.contains("RetiredPointManifest"));

    let reclaim = read(&root, "src/rebuild_composition/kernel/reclaim.rs");
    assert!(reclaim.contains("validate_retired_manifest"));
    assert!(reclaim.contains("compute_reclamation_watermark"));
    assert!(reclaim.contains("OrdinaryRetiredPointReclaim"));
    assert!(!reclaim.to_ascii_lowercase().contains("purge intent"));
    assert!(!reclaim.contains("delete_points"));
}

#[test]
fn rebuild_reason_and_public_surface_stay_closed() {
    let root = crate_root();
    let error = read(&root, "src/rebuild_composition/kernel/error.rs");
    for code in [
        "REBUILD_INVALID_LIMITS",
        "REBUILD_MANIFEST_NOT_CANONICAL",
        "REBUILD_DIGEST_MISMATCH",
        "REBUILD_STALE_ROUTE",
        "REBUILD_STALE_REVISION",
        "REBUILD_GENERATION_MISMATCH",
        "REBUILD_GENERATION_REUSE",
        "REBUILD_STILL_PINNED",
        "REBUILD_READBACK_MISMATCH",
        "REBUILD_CUTOVER_MISMATCH",
        "REBUILD_BUDGET_EXCEEDED",
        "REBUILD_PUBLICATION_MISMATCH",
    ] {
        assert!(error.contains(code), "lost rebuild reason {code}");
    }

    let kernel = read(&root, "src/rebuild_composition/kernel.rs");
    for public_name in [
        "RetainedManifest",
        "RetainedPoint",
        "RebuildBudget",
        "RebuildPlan",
        "IndexReadbackView",
        "FullReadbackProof",
        "StagedCutover",
        "CommittedCutover",
        "QuerySession",
        "ReclaimTuning",
        "authorize_reclaim",
        "begin_pinned_query",
        "commit_cutover",
        "expire_continuation_pins_bounded",
        "is_ordinary_reclaim_receipt",
        "propose_rebuild",
        "release_owner_pins_of",
        "retained_manifest_digest",
        "stage_cutover",
        "validate_retained_manifest",
        "verify_full_readback",
    ] {
        assert!(kernel.contains(public_name), "lost public name {public_name}");
    }
}

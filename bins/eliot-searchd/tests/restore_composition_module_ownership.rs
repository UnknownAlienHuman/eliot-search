use std::path::{Path, PathBuf};

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

#[test]
fn restore_composition_entry_and_kernel_are_thin() {
    let root = crate_root();
    let entry = read(&root, "src/restore_composition.rs");
    assert!(entry.contains("#[path = \"restore_composition/kernel.rs\"]"));
    assert!(entry.contains("pub use kernel::*;"));
    assert!(entry.contains("#![forbid(unsafe_code)]"));
    assert!(entry.len() < 2_000, "entry grew to {} bytes", entry.len());
    for forbidden in [
        "pub struct ExportManifest",
        "pub struct StagedRestore",
        "pub fn stage_restore(",
        "pub fn canonical_export_digest(",
        "VerifiedCutoverReceipt",
        "#[cfg(test)]",
    ] {
        assert!(
            !entry.contains(forbidden),
            "restore implementation returned to facade: {forbidden}"
        );
    }

    let kernel = read(&root, "src/restore_composition/kernel.rs");
    for module in [
        "cutover", "fixture", "lifecycle", "manifest", "model", "receipt", "spec",
    ] {
        assert!(kernel.contains(&format!("mod {module};")));
        assert!(kernel.contains(&format!("pub use {module}::*;")));
    }
    assert!(kernel.contains("mod tests;"));
    assert!(kernel.len() < 3_000, "kernel grew to {} bytes", kernel.len());
}

#[test]
fn restore_responsibilities_stay_separated() {
    let root = crate_root();
    let owners = [
        ("src/restore_composition/kernel/spec.rs", "pub enum RestoreCompositionError"),
        ("src/restore_composition/kernel/manifest.rs", "pub fn canonical_export_digest("),
        ("src/restore_composition/kernel/model.rs", "pub struct StagedRestore"),
        ("src/restore_composition/kernel/cutover.rs", "pub struct OwnerCutoverProof"),
        ("src/restore_composition/kernel/lifecycle.rs", "pub fn stage_restore("),
        ("src/restore_composition/kernel/receipt.rs", "pub fn staged_receipt("),
        ("src/restore_composition/kernel/fixture.rs", "pub fn build_test_export("),
    ];
    for (relative, marker) in owners {
        let source = read(&root, relative);
        assert!(source.contains(marker), "{relative} lost owner {marker}");
        assert!(
            source.len() < 16_000,
            "{relative} grew to {} bytes",
            source.len()
        );
        for forbidden in [
            "qdrant_client",
            "search_qdrant",
            "reqwest::",
            "tokio::",
            "std::fs",
            "std::process",
        ] {
            assert!(
                !source.contains(forbidden),
                "{relative} acquired forbidden effect/vendor token {forbidden}"
            );
        }
    }

    let manifest = read(&root, "src/restore_composition/kernel/manifest.rs");
    assert!(manifest.contains("eliot-search/restore-export/v1\\x00"));
    assert!(manifest.contains("blake3::hash"));
    assert!(!manifest.contains("RestoreCoordinator"));

    let lifecycle = read(&root, "src/restore_composition/kernel/lifecycle.rs");
    assert!(lifecycle.contains("RestoreCoordinator::new"));
    assert!(lifecycle.contains("verify_control"));
    assert!(lifecycle.contains("verify_objects"));
    assert!(lifecycle.contains("admit_indexed"));
    assert!(!lifecycle.contains("VerifiedCutoverReceipt"));

    let cutover = read(&root, "src/restore_composition/kernel/cutover.rs");
    assert!(cutover.contains("VerifiedCutoverReceipt"));
    assert!(cutover.contains("OldOwnerStillServing"));
    assert!(!cutover.contains("RestoreCoordinator::new"));

    let receipt = read(&root, "src/restore_composition/kernel/receipt.rs");
    assert!(receipt.contains("eliot.restore-staging.v1"));
    assert!(!receipt.contains("SystemTime"));
    assert!(!receipt.contains("rand"));
}

#[test]
fn restore_regression_corpus_is_separate() {
    let root = crate_root();
    let tests = read(&root, "src/restore_composition/kernel/tests.rs");
    assert!(tests.len() < 8_000, "tests grew to {} bytes", tests.len());
    for case in [
        "canonical_digest_is_deterministic",
        "pending_stage_never_reports_ready",
    ] {
        assert!(tests.contains(&format!("fn {case}(")), "lost test {case}");
    }
}

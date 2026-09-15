use std::path::{Path, PathBuf};

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

#[test]
fn development_entry_and_kernel_are_thin() {
    let root = crate_root();
    let entry = read(&root, "src/development.rs");
    assert!(entry.contains("#[path = \"development/kernel.rs\"]"));
    assert!(entry.contains("pub use kernel::*;"));
    assert!(entry.len() < 1_500, "entry grew to {} bytes", entry.len());
    for forbidden in [
        "pub struct Health",
        "pub struct DataRootGuard",
        "pub fn scan_text(",
        "OpenOptions",
        "TryLockError",
        "#[cfg(test)]",
    ] {
        assert!(
            !entry.contains(forbidden),
            "development implementation returned to facade: {forbidden}"
        );
    }

    let kernel = read(&root, "src/development/kernel.rs");
    for module in ["health", "owner", "scan"] {
        assert!(kernel.contains(&format!("mod {module};")));
        assert!(kernel.contains(&format!("pub use {module}::*;")));
    }
    assert!(kernel.contains("mod tests;"));
    assert!(kernel.len() < 2_000, "kernel grew to {} bytes", kernel.len());
}

#[test]
fn development_responsibilities_stay_separated() {
    let root = crate_root();
    let owners = [
        ("src/development/kernel/health.rs", "pub struct Health"),
        ("src/development/kernel/scan.rs", "pub fn scan_text("),
        ("src/development/kernel/owner.rs", "pub struct DataRootGuard"),
    ];
    for (relative, marker) in owners {
        let source = read(&root, relative);
        assert!(source.contains(marker), "{relative} lost owner {marker}");
        assert!(
            source.len() < 18_000,
            "{relative} grew to {} bytes",
            source.len()
        );
        for forbidden in ["qdrant_client", "search_qdrant", "reqwest::", "tokio::"] {
            assert!(
                !source.contains(forbidden),
                "{relative} acquired forbidden vendor token {forbidden}"
            );
        }
    }

    let health = read(&root, "src/development/kernel/health.rs");
    assert!(health.contains("ReadinessReport"));
    assert!(health.contains("development_shell"));
    assert!(!health.contains("std::fs"));
    assert!(!health.contains("scan_chunks"));
    assert!(!health.contains("try_lock"));

    let scan = read(&root, "src/development/kernel/scan.rs");
    assert!(scan.contains("search_exact::literal"));
    assert!(scan.contains("read_full_file_via_kernel"));
    assert!(scan.contains("MAX_SCAN_INPUT_BYTES"));
    assert!(!scan.contains("OwnerLockProfile"));
    assert!(!scan.contains("SourceRootCatalog"));
    assert!(!scan.contains("OpenOptions"));

    let owner = read(&root, "src/development/kernel/owner.rs");
    assert!(owner.contains("try_lock"));
    assert!(owner.contains("owner_composition::establish"));
    assert!(owner.contains("SourceRootCatalog::load_owned"));
    assert!(owner.contains("classify_owner_mutation_boundary"));
    assert!(!owner.contains("search_exact::literal"));
    assert!(!owner.contains("read_full_file_via_kernel"));
}

#[test]
fn development_regression_corpus_is_separate() {
    let root = crate_root();
    let tests = read(&root, "src/development/kernel/tests.rs");
    assert!(tests.len() < 20_000, "tests grew to {} bytes", tests.len());
    for case in [
        "registration_reopens_under_the_same_exclusive_owner_lock",
        "single_guard_binds_epoch_and_stable_identities_across_succession",
        "release_requires_prior_drain_and_persists_one_tombstone",
        "relocated_copy_is_denied_while_original_advances",
        "corrupt_owner_state_quarantines_without_touching_catalogs",
        "shared_matcher_preserves_legacy_coordinates_and_ascii_only_folding",
        "output_ceiling_is_incomplete_only_when_an_additional_match_exists",
        "repeated_long_prefix_uses_the_shared_linear_matcher",
    ] {
        assert!(tests.contains(&format!("fn {case}(")), "lost test {case}");
    }
}

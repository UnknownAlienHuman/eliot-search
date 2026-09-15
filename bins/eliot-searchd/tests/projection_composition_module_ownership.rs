use std::path::{Path, PathBuf};

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

#[test]
fn projection_composition_entry_and_kernel_are_thin() {
    let root = crate_root();
    let entry = read(&root, "src/projection_composition.rs");
    assert!(entry.contains("#[path = \"projection_composition/kernel.rs\"]"));
    assert!(entry.contains("pub use kernel::*;"));
    assert!(entry.len() < 1_500, "entry grew to {} bytes", entry.len());
    for forbidden in [
        "struct CompositionRequest",
        "fn compose_scoped_projection",
        "fn store_projection_manifest",
        "OpenOptions",
        "#[cfg(test)]",
    ] {
        assert!(
            !entry.contains(forbidden),
            "projection implementation returned to facade: {forbidden}"
        );
    }

    let kernel = read(&root, "src/projection_composition/kernel.rs");
    for module in ["cas", "compose", "digest", "error", "model", "reference", "spec"] {
        assert!(kernel.contains(&format!("mod {module};")));
    }
    assert!(kernel.contains("mod tests;"));
    assert!(kernel.len() < 3_000, "kernel grew to {} bytes", kernel.len());
    for forbidden in ["std::fs", "OpenOptions", "ProjectionInput {", "blake3::Hasher::new"] {
        assert!(
            !kernel.contains(forbidden),
            "implementation returned to projection kernel: {forbidden}"
        );
    }
}

#[test]
fn projection_responsibilities_stay_separated_and_vendor_free() {
    let root = crate_root();
    let owners = [
        (
            "src/projection_composition/kernel/error.rs",
            "pub enum ProjectionCompositionError",
        ),
        (
            "src/projection_composition/kernel/model.rs",
            "pub struct CompositionRequest",
        ),
        (
            "src/projection_composition/kernel/digest.rs",
            "pub fn compute_payload_digest(",
        ),
        (
            "src/projection_composition/kernel/compose.rs",
            "pub fn compose_scoped_projection(",
        ),
        (
            "src/projection_composition/kernel/reference.rs",
            "impl ProjectionReference",
        ),
        (
            "src/projection_composition/kernel/cas.rs",
            "pub fn store_projection_manifest(",
        ),
    ];
    for (relative, marker) in owners {
        let source = read(&root, relative);
        assert!(source.contains(marker), "{relative} lost owner {marker}");
        assert!(
            source.len() < 18_000,
            "{relative} grew to {} bytes",
            source.len()
        );
        for forbidden in [
            "qdrant_client",
            "search_qdrant",
            "reqwest::",
            "tokio::",
            "std::process::Command",
            "Command::new",
        ] {
            assert!(
                !source.contains(forbidden),
                "{relative} acquired forbidden token {forbidden}"
            );
        }
    }

    let model = read(&root, "src/projection_composition/kernel/model.rs");
    assert!(model.contains("pub struct ProjectionReference"));
    assert!(!model.contains("std::fs"));
    assert!(!model.contains("blake3::"));

    let digest = read(&root, "src/projection_composition/kernel/digest.rs");
    assert!(!digest.contains("eliot-search/projection-payload/v1"));
    assert!(digest.contains("PAYLOAD_DIGEST_DOMAIN"));
    assert!(digest.contains("SCOPE_KEY_DOMAIN"));
    assert!(!digest.contains("OpenOptions"));

    let spec = read(&root, "src/projection_composition/kernel/spec.rs");
    assert!(spec.contains("ELSPRJ01"));
    assert!(spec.contains("eliot-search/projection-payload/v1"));
    assert!(spec.contains("eliot-search/projection-scope/v1"));
    assert!(!spec.contains("std::fs"));

    let compose = read(&root, "src/projection_composition/kernel/compose.rs");
    assert!(compose.contains("plan_scoped_projection"));
    assert!(compose.contains("PointIdentityRegistry"));
    assert!(!compose.contains("OpenOptions"));
    assert!(!compose.contains("create_dir_all"));

    let reference = read(&root, "src/projection_composition/kernel/reference.rs");
    assert!(reference.contains("REFERENCE_BYTES"));
    assert!(reference.contains("REFERENCE_MAGIC"));
    assert!(!reference.contains("OpenOptions"));

    let cas = read(&root, "src/projection_composition/kernel/cas.rs");
    assert!(cas.contains("OpenOptions::new"));
    assert!(cas.contains("create_new(true)"));
    assert!(cas.contains("sync_all"));
    assert!(cas.contains("reference.to_bytes()"));
    assert!(!cas.contains("ProjectionInput"));
}

#[test]
fn projection_regression_corpus_is_separate() {
    let root = crate_root();
    let tests = read(&root, "src/projection_composition/kernel/tests.rs");
    assert!(tests.len() < 35_000, "projection tests grew to {} bytes", tests.len());
    for case in [
        "compose_persists_reloads_and_recomposes_identical_bytes",
        "persist_is_idempotent_for_same_bytes_and_conflicts_on_divergence",
        "missing_wrong_residency_duplicate_propagate_typed_errors",
        "one_source_two_memberships_yield_distinct_manifests",
        "generation_change_replaces_manifest_and_conflicts_with_prior_reference",
        "reordered_units_yield_identical_manifest_bytes",
        "reference_carries_no_source_bodies",
        "payload_indexes_for_t24_are_exact_and_complete",
        "composition_error_codes_are_stable",
    ] {
        assert!(tests.contains(&format!("fn {case}(")), "lost test {case}");
    }
}

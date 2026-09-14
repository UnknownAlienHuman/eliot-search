use std::path::{Path, PathBuf};

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

#[test]
fn provider_composition_entry_and_kernel_are_thin() {
    let root = crate_root();
    let entry = read(&root, "src/provider_composition.rs");
    assert!(entry.contains("#[path = \"provider_composition/kernel.rs\"]"));
    assert!(entry.contains("pub use kernel::*;"));
    assert!(entry.len() < 1_500, "facade grew to {} bytes", entry.len());
    for forbidden in [
        "pub struct ProviderRouter",
        "pub fn gate_operation(",
        "pub fn read_shim_key_file(",
        "BTreeMap",
        "std::fs",
        "#[cfg(test)]",
    ] {
        assert!(
            !entry.contains(forbidden),
            "provider implementation returned to facade: {forbidden}"
        );
    }

    let kernel = read(&root, "src/provider_composition/kernel.rs");
    for module in [
        "capability",
        "child",
        "codec",
        "currentness",
        "pairing",
        "render",
        "router",
        "spec",
    ] {
        assert!(kernel.contains(&format!("mod {module};")));
        assert!(kernel.contains(&format!("pub use {module}::*;")));
    }
    assert!(kernel.contains("mod tests;"));
    assert!(kernel.len() < 2_500, "kernel grew to {} bytes", kernel.len());
}

#[test]
fn provider_responsibilities_remain_bounded_and_vendor_free() {
    let root = crate_root();
    let owners = [
        ("src/provider_composition/kernel/spec.rs", "pub enum ProviderOperation"),
        ("src/provider_composition/kernel/capability.rs", "pub fn gate_operation("),
        ("src/provider_composition/kernel/pairing.rs", "pub fn read_shim_key_file("),
        ("src/provider_composition/kernel/codec.rs", "pub fn parse_envelope_line("),
        ("src/provider_composition/kernel/child.rs", "pub enum ChildReply"),
        ("src/provider_composition/kernel/render.rs", "pub fn render_op_response("),
        ("src/provider_composition/kernel/router.rs", "pub struct ProviderRouter"),
        (
            "src/provider_composition/kernel/currentness.rs",
            "pub const fn evaluate_current_workspace_proven(",
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
        for forbidden in ["qdrant_client", "search_qdrant", "reqwest::", "tokio::"] {
            assert!(
                !source.contains(forbidden),
                "{relative} acquired forbidden vendor token {forbidden}"
            );
        }
    }

    for relative in [
        "src/provider_composition/kernel/spec.rs",
        "src/provider_composition/kernel/capability.rs",
        "src/provider_composition/kernel/codec.rs",
        "src/provider_composition/kernel/child.rs",
        "src/provider_composition/kernel/render.rs",
        "src/provider_composition/kernel/router.rs",
        "src/provider_composition/kernel/currentness.rs",
    ] {
        let source = read(&root, relative);
        assert!(!source.contains("std::fs"), "{relative} acquired filesystem I/O");
        assert!(!source.contains("File::open"), "{relative} acquired file opening");
    }

    let pairing = read(&root, "src/provider_composition/kernel/pairing.rs");
    assert!(pairing.contains("std::fs::symlink_metadata"));
    assert!(pairing.contains("File::open"));
    assert!(pairing.contains("bytes.fill(0)"));
    assert!(!pairing.contains("ProviderRouter"));

    let router = read(&root, "src/provider_composition/kernel/router.rs");
    assert!(router.contains("verify_envelope(key, envelope)?"));
    assert!(router.contains("SequenceTracker::require_accepted"));
    assert!(!router.contains("render_provider_error"));

    let currentness = read(&root, "src/provider_composition/kernel/currentness.rs");
    assert!(!currentness.contains("search_provider_protocol"));
    assert!(!currentness.contains("qdrant_client"));
}

#[test]
fn provider_regression_corpus_is_separate() {
    let root = crate_root();
    let tests = read(&root, "src/provider_composition/kernel/tests.rs");
    assert!(tests.len() < 25_000, "provider tests grew to {} bytes", tests.len());
    for case in [
        "registries_are_closed",
        "admit_checks_version_nonce_proof_sequence_replay_and_ceiling",
        "terminal_is_unique_ordered_and_releases_exactly_once",
        "capabilities_gate_recipes_but_never_the_shell",
        "response_seal_binds_receipt_without_relabeling",
        "line_parsing_is_strict_and_bounded",
        "gaps_empty_and_sync_block_proof_in_fixed_order",
    ] {
        assert!(tests.contains(&format!("fn {case}(")), "lost test {case}");
    }
}

use std::path::{Path, PathBuf};

fn package_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

#[test]
fn config_composition_public_surface_is_a_thin_facade() {
    let root = package_root();
    let facade = read(&root, "src/config_composition.rs");
    assert!(facade.len() < 1_500);
    assert!(facade.contains("mod kernel;"));
    assert!(facade.contains("pub use kernel::*;"));
    for forbidden in [
        "pub fn daemon_registry(",
        "pub fn build_effective(",
        "pub fn derive_readiness(",
        "std::fs",
        "std::env",
        "#[cfg(test)]",
    ] {
        assert!(!facade.contains(forbidden), "facade reacquired {forbidden}");
    }
}

#[test]
fn config_kernel_is_split_by_responsibility() {
    let root = package_root();
    let kernel = read(&root, "src/config_composition/kernel.rs");
    assert!(kernel.len() < 3_000, "kernel facade grew to {} bytes", kernel.len());
    for module in [
        "activation",
        "capture",
        "process",
        "readiness",
        "registry",
        "snapshot",
        "spec",
    ] {
        assert!(kernel.contains(&format!("mod {module};")));
        assert!(kernel.contains(&format!("pub use {module}::*;")));
    }
    assert!(kernel.contains("mod tests;"));
    for forbidden in [
        "pub fn daemon_registry(",
        "pub fn capture_file_document(",
        "pub fn build_effective(",
        "pub fn try_activate(",
        "pub fn derive_readiness(",
        "std::fs",
        "std::env",
    ] {
        assert!(!kernel.contains(forbidden), "kernel reacquired {forbidden}");
    }
}

#[test]
fn config_owners_remain_bounded_and_transport_free() {
    let root = package_root();
    let owners = [
        ("src/config_composition/kernel/spec.rs", "pub const DAEMON_CONFIG_SCHEMA_VERSION"),
        ("src/config_composition/kernel/registry.rs", "pub fn daemon_registry("),
        ("src/config_composition/kernel/capture.rs", "pub fn capture_file_document("),
        ("src/config_composition/kernel/snapshot.rs", "pub fn build_effective("),
        ("src/config_composition/kernel/activation.rs", "pub fn try_activate("),
        ("src/config_composition/kernel/process.rs", "pub fn effective_from_process("),
        ("src/config_composition/kernel/readiness.rs", "pub fn derive_readiness("),
    ];
    for (relative, marker) in owners {
        let source = read(&root, relative);
        assert!(source.contains(marker), "{relative} lost {marker}");
        assert!(source.len() < 20_000, "{relative} grew to {} bytes", source.len());
        for forbidden in ["qdrant_client", "search_qdrant", "reqwest::", "tokio::"] {
            assert!(!source.contains(forbidden), "{relative} acquired {forbidden}");
        }
    }

    let process = read(&root, "src/config_composition/kernel/process.rs");
    assert!(process.contains("std::fs::symlink_metadata"));
    assert!(process.contains("std::env::vars"));
    for relative in [
        "src/config_composition/kernel/spec.rs",
        "src/config_composition/kernel/registry.rs",
        "src/config_composition/kernel/capture.rs",
        "src/config_composition/kernel/snapshot.rs",
        "src/config_composition/kernel/activation.rs",
        "src/config_composition/kernel/readiness.rs",
    ] {
        let source = read(&root, relative);
        assert!(!source.contains("std::fs"), "{relative} acquired filesystem I/O");
        assert!(!source.contains("std::env"), "{relative} acquired environment I/O");
    }

    let registry = read(&root, "src/config_composition/kernel/registry.rs");
    assert_eq!(registry.matches("ConfigSectionDescriptor::new(").count(), 7);
    let capture = read(&root, "src/config_composition/kernel/capture.rs");
    assert!(capture.contains("eliot-searchd/environment/v1\\0"));
    assert!(capture.contains("eliot-searchd/cli-typed/v1\\0"));
    let snapshot = read(&root, "src/config_composition/kernel/snapshot.rs");
    assert!(snapshot.contains("assemble_effective"));
    assert!(snapshot.contains("validation_digest"));
    let activation = read(&root, "src/config_composition/kernel/activation.rs");
    assert!(activation.contains("required_receipts"));
    let readiness = read(&root, "src/config_composition/kernel/readiness.rs");
    assert!(readiness.contains("optional_gate_accepted"));
    assert!(readiness.contains("\"read_only\":true"));
}

#[test]
fn config_regression_corpus_is_separate() {
    let root = package_root();
    let tests = read(&root, "src/config_composition/kernel/tests.rs");
    assert!(tests.len() < 35_000, "config tests grew to {} bytes", tests.len());
    for case in [
        "effective_config_assembles_from_defaults_file_env_cli",
        "plaintext_secret_is_denied_on_every_layer",
        "mixed_live_restart_rebuild_failure_retains_old_snapshot",
        "w1_readiness_does_not_imply_search_available",
        "read_only_status_writes_nothing_and_leaks_nothing",
        "startup_partial_refuses_and_retains_defaults_without_leaks",
    ] {
        assert!(tests.contains(&format!("fn {case}(")), "lost test {case}");
    }
}

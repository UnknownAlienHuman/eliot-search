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
    assert!(
        facade.len() < 1_500,
        "config composition facade grew to {} bytes",
        facade.len()
    );
    assert!(facade.contains("mod kernel;"));
    assert!(facade.contains("pub use kernel::*;"));
    for forbidden in [
        "pub fn daemon_registry(",
        "pub fn capture_file_document(",
        "pub fn build_effective(",
        "pub fn try_activate(",
        "pub fn derive_readiness(",
        "std::fs",
        "std::env",
        "#[cfg(test)]",
    ] {
        assert!(
            !facade.contains(forbidden),
            "config facade reacquired {forbidden}"
        );
    }
}

#[test]
fn staged_kernel_remains_the_single_implementation_owner() {
    let root = package_root();
    let kernel = read(&root, "src/config_composition/kernel.rs");
    for marker in [
        "pub fn daemon_registry(",
        "pub fn capture_file_document(",
        "pub fn build_effective(",
        "pub fn try_activate(",
        "pub fn derive_readiness(",
        "pub fn config_status_json(",
        "#[cfg(test)]",
    ] {
        assert!(kernel.contains(marker), "staged kernel lost {marker}");
    }
    for forbidden in ["qdrant_client", "search_qdrant", "reqwest::", "tokio::"] {
        assert!(
            !kernel.contains(forbidden),
            "config kernel acquired forbidden transport token {forbidden}"
        );
    }
}

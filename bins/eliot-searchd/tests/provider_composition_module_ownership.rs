use std::path::{Path, PathBuf};

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

#[test]
fn provider_composition_entry_is_a_thin_facade() {
    let root = crate_root();
    let entry = read(&root, "src/provider_composition.rs");

    assert!(entry.contains("#[path = \"provider_composition/kernel.rs\"]"));
    assert!(entry.contains("mod kernel;"));
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
}

#[test]
fn provider_kernel_retains_the_closed_surface_without_vendor_leakage() {
    let root = crate_root();
    let kernel = read(&root, "src/provider_composition/kernel.rs");

    for marker in [
        "pub enum ProviderOperation",
        "pub fn gate_operation(",
        "pub fn read_shim_key_file(",
        "pub fn parse_envelope_line(",
        "pub fn render_op_response(",
        "pub struct ProviderRouter",
        "pub const fn evaluate_current_workspace_proven(",
        "mod tests",
        "mod roots_tests",
    ] {
        assert!(kernel.contains(marker), "provider kernel lost {marker}");
    }
    assert!(
        kernel.len() < 70_000,
        "provider kernel grew to {} bytes",
        kernel.len()
    );
    for forbidden in ["qdrant_client", "search_qdrant", "reqwest::", "tokio::"] {
        assert!(
            !kernel.contains(forbidden),
            "provider composition acquired forbidden vendor token {forbidden}"
        );
    }
}

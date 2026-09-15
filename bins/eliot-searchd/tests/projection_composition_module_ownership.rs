use std::path::{Path, PathBuf};

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

#[test]
fn projection_composition_entry_is_thin() {
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
    for marker in [
        "pub struct CompositionRequest",
        "pub fn compose_scoped_projection",
        "pub fn store_projection_manifest",
        "pub fn load_projection_manifest_bytes",
    ] {
        assert!(kernel.contains(marker), "kernel lost {marker}");
    }
}

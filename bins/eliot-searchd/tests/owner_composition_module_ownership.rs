use std::path::{Path, PathBuf};

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

#[test]
fn owner_composition_entry_is_thin() {
    let root = crate_root();
    let entry = read(&root, "src/owner_composition.rs");
    assert!(entry.contains("#[path = \"owner_composition/kernel.rs\"]"));
    assert!(entry.contains("pub use kernel::{LiveOwner, ShutdownReceipt, establish};"));
    assert!(entry.len() < 1_500, "entry grew to {} bytes", entry.len());
    for forbidden in [
        "struct DurableOwnerRecord",
        "fn load_or_create_installation",
        "fn observe_physical_root",
        "fn newest_valid",
        "fn publish_transition",
        "#[cfg(test)]",
    ] {
        assert!(
            !entry.contains(forbidden),
            "owner implementation returned to facade: {forbidden}"
        );
    }

    let kernel = read(&root, "src/owner_composition/kernel.rs");
    for marker in [
        "struct DurableOwnerRecord",
        "pub struct LiveOwner",
        "pub struct ShutdownReceipt",
        "pub fn establish(",
        "fn newest_valid(",
        "fn publish_transition(",
    ] {
        assert!(kernel.contains(marker), "kernel lost {marker}");
    }
}

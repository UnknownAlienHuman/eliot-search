use std::path::{Path, PathBuf};

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

#[test]
fn secret_composition_entry_is_thin() {
    let root = crate_root();
    let entry = read(&root, "src/secret_composition.rs");
    assert!(entry.contains("#[path = \"secret_composition/kernel.rs\"]"));
    assert!(entry.contains("pub use kernel::*;"));
    assert!(entry.len() < 1_500, "entry grew to {} bytes", entry.len());
    for forbidden in [
        "struct PairingSecretComposer",
        "trait PairingVault",
        "struct MemoryPairingVault",
        "fn derive_binding_digest",
        "fn recover_delete",
        "#[cfg(test)]",
    ] {
        assert!(
            !entry.contains(forbidden),
            "secret implementation returned to facade: {forbidden}"
        );
    }

    let kernel = read(&root, "src/secret_composition/kernel.rs");
    for marker in [
        "pub struct PairingSecretComposer",
        "pub trait PairingVault",
        "pub struct MemoryPairingVault",
        "pub fn derive_binding_digest",
        "fn recover_delete",
    ] {
        assert!(kernel.contains(marker), "kernel lost {marker}");
    }
}

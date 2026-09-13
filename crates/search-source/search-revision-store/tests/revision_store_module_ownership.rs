use std::path::PathBuf;

use search_revision_store::{
    CAS_ADDRESS_VERSION, DEFAULT_REVISION_STORE_LIMITS, RevisionStore,
};

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn public_entry_is_a_thin_stable_facade() {
    let entry = std::fs::read_to_string(crate_root().join("src/lib.rs"))
        .expect("public entry exists");

    assert!(entry.contains("mod kernel;"));
    assert!(entry.contains("pub use kernel::*;"));
    assert!(entry.lines().count() <= 32);

    for implementation_marker in [
        "pub struct RevisionStore",
        "impl RevisionStore",
        "fn validate_intent",
        "#[cfg(test)]",
    ] {
        assert!(
            !entry.contains(implementation_marker),
            "implementation returned to public facade: {implementation_marker}"
        );
    }
}

#[test]
fn kernel_owns_the_existing_revision_state_machine() {
    let kernel = std::fs::read_to_string(crate_root().join("src/kernel.rs"))
        .expect("kernel module exists");

    for required in [
        "pub struct RevisionStore",
        "impl RevisionStore",
        "pub trait RevisionObjectBackend",
        "fn validate_intent",
        "#[cfg(test)]",
    ] {
        assert!(kernel.contains(required), "kernel lost owner: {required}");
    }
}

#[test]
fn facade_preserves_the_existing_public_surface() {
    let store = RevisionStore::new(DEFAULT_REVISION_STORE_LIMITS)
        .expect("default limits remain valid");
    assert!(store.is_empty());
    assert_eq!(CAS_ADDRESS_VERSION, 1);
}

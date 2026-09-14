use std::path::{Path, PathBuf};

use search_revision_store::{
    CAS_ADDRESS_VERSION, DEFAULT_REVISION_STORE_LIMITS, RevisionStore,
};

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

#[test]
fn public_entry_is_a_thin_stable_facade() {
    let root = crate_root();
    let entry = read(&root, "src/lib.rs");

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
fn kernel_is_a_bounded_owner_facade() {
    let root = crate_root();
    let kernel = read(&root, "src/kernel.rs");
    assert!(
        kernel.len() < 4_000,
        "kernel facade grew to {} bytes",
        kernel.len()
    );
    for module in [
        "address",
        "backend",
        "binding",
        "error",
        "limits",
        "model",
        "residency",
        "store",
    ] {
        assert!(kernel.contains(&format!("mod {module};")));
        assert!(kernel.contains(&format!("pub use {module}::*;")));
    }
    assert!(kernel.contains("mod tests;"));
    for forbidden in [
        "pub struct RevisionStore",
        "impl RevisionStore",
        "pub trait RevisionObjectBackend",
        "fn validate_intent",
        "std::collections::BTreeMap",
    ] {
        assert!(
            !kernel.contains(forbidden),
            "implementation returned to kernel facade: {forbidden}"
        );
    }
}

#[test]
fn revision_store_responsibilities_stay_separated() {
    let root = crate_root();
    let owners = [
        ("src/kernel/error.rs", "pub enum RevisionStoreError"),
        ("src/kernel/limits.rs", "pub struct RevisionStoreLimits"),
        ("src/kernel/residency.rs", "pub struct ResidencyClosure"),
        ("src/kernel/address.rs", "pub struct CasObjectAddress"),
        ("src/kernel/binding.rs", "pub struct CanonicalIngestBinding"),
        ("src/kernel/model.rs", "pub struct RevisionWriteIntent"),
        ("src/kernel/store.rs", "pub struct RevisionStore"),
        ("src/kernel/backend.rs", "pub trait RevisionObjectBackend"),
    ];
    for (relative, marker) in owners {
        let source = read(&root, relative);
        assert!(source.contains(marker), "{relative} lost owner {marker}");
        assert!(
            source.len() < 45_000,
            "{relative} grew to {} bytes",
            source.len()
        );
        for forbidden in [
            "std::fs",
            "std::process",
            "qdrant_client",
            "reqwest::",
            "tokio::",
        ] {
            assert!(
                !source.contains(forbidden),
                "{relative} acquired forbidden token {forbidden}"
            );
        }
    }

    let store = read(&root, "src/kernel/store.rs");
    assert!(store.len() < 8_000, "store facade grew to {} bytes", store.len());
    for module in ["append", "lifecycle", "recovery", "support"] {
        assert!(store.contains(&format!("mod {module};")));
    }
    for forbidden in [
        "pub fn prepare_append(",
        "pub fn confirm_append(",
        "pub fn recover_unknown(",
        "pub fn install_purge_tombstone(",
        "pub fn apply_exact_object_deletion(",
        "fn validate_intent(",
        "fn record_from_readback(",
    ] {
        assert!(
            !store.contains(forbidden),
            "store facade reacquired {forbidden}"
        );
    }

    let append = read(&root, "src/kernel/store/append.rs");
    assert!(append.contains("pub fn prepare_append("));
    assert!(append.contains("pub fn confirm_append("));
    assert!(!append.contains("pub fn recover_unknown("));
    assert!(!append.contains("pub fn install_purge_tombstone("));

    let recovery = read(&root, "src/kernel/store/recovery.rs");
    assert!(recovery.contains("pub fn mark_outcome_unknown("));
    assert!(recovery.contains("pub fn recover_unknown("));
    assert!(!recovery.contains("pub fn prepare_append("));

    let lifecycle = read(&root, "src/kernel/store/lifecycle.rs");
    assert!(lifecycle.contains("pub fn install_purge_tombstone("));
    assert!(lifecycle.contains("pub fn apply_exact_object_deletion("));
    assert!(!lifecycle.contains("pub fn recover_unknown("));

    let support = read(&root, "src/kernel/store/support.rs");
    assert!(support.contains("fn validate_intent("));
    assert!(support.contains("fn record_from_readback("));
    assert!(support.contains("fn reuse_conflict("));
    assert!(!support.contains("impl RevisionStore"));

    for relative in [
        "src/kernel/store/append.rs",
        "src/kernel/store/lifecycle.rs",
        "src/kernel/store/recovery.rs",
        "src/kernel/store/support.rs",
    ] {
        let source = read(&root, relative);
        assert!(
            source.len() < 15_000,
            "store owner {relative} grew to {} bytes",
            source.len()
        );
        for forbidden in [
            "std::fs",
            "std::process",
            "qdrant_client",
            "reqwest::",
            "tokio::",
        ] {
            assert!(
                !source.contains(forbidden),
                "{relative} acquired forbidden token {forbidden}"
            );
        }
    }

    let model = read(&root, "src/kernel/model.rs");
    assert!(model.contains("<{} encrypted bytes>"));
    assert!(!model.contains("impl RevisionStore"));
}

#[test]
fn regression_corpus_stays_split_by_behavior() {
    let root = crate_root();
    let facade = read(&root, "src/kernel/tests.rs");
    for module in ["append", "fixtures", "lifecycle", "residency"] {
        assert!(facade.contains(&format!("mod {module};")));
        let source = read(&root, &format!("src/kernel/tests/{module}.rs"));
        assert!(
            source.len() < 35_000,
            "test owner {module} grew to {} bytes",
            source.len()
        );
    }
}

#[test]
fn facade_preserves_the_existing_public_surface() {
    let store = RevisionStore::new(DEFAULT_REVISION_STORE_LIMITS)
        .expect("default limits remain valid");
    assert!(store.is_empty());
    assert_eq!(CAS_ADDRESS_VERSION, 1);
}

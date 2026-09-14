use std::path::{Path, PathBuf};

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

#[test]
fn continuation_entry_is_a_thin_stable_facade() {
    let root = crate_root();
    let entry = read(&root, "src/continuation.rs");

    assert!(entry.contains("#[path = \"continuation/kernel.rs\"]"));
    assert!(entry.contains("mod kernel;"));
    assert!(entry.contains("pub use kernel::*;"));
    assert!(entry.len() < 1_500, "facade grew to {} bytes", entry.len());

    for forbidden in [
        "pub struct ContinuationCatalog",
        "pub fn qualified_entropy_32",
        "impl ContinuationCatalog",
        "DirectStore",
        "#[cfg(test)]",
        "std::fs",
    ] {
        assert!(
            !entry.contains(forbidden),
            "implementation returned to continuation facade: {forbidden}"
        );
    }
}

#[test]
fn continuation_kernel_keeps_one_bounded_owner_until_decomposition() {
    let root = crate_root();
    let kernel = read(&root, "src/continuation/kernel.rs");

    for marker in [
        "pub struct ContinuationCatalog",
        "pub fn qualified_entropy_32",
        "impl ContinuationCatalog",
        "mod live_authorization_tests",
        "DIRECT_CONTINUATION_ACCESS_REVOKED",
        "DIRECT_CONTINUATION_PURGED",
    ] {
        assert!(kernel.contains(marker), "kernel lost marker {marker}");
    }
    assert!(
        kernel.len() < 45_000,
        "continuation kernel grew to {} bytes",
        kernel.len()
    );
    for forbidden in ["qdrant_client", "search_qdrant", "reqwest::", "tokio::"] {
        assert!(
            !kernel.contains(forbidden),
            "continuation owner acquired forbidden provider token {forbidden}"
        );
    }
}

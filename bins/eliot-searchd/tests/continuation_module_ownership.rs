use std::path::{Path, PathBuf};

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

#[test]
fn continuation_entry_and_kernel_are_thin_stable_facades() {
    let root = crate_root();
    let entry = read(&root, "src/continuation.rs");
    assert!(entry.contains("#[path = \"continuation/kernel.rs\"]"));
    assert!(entry.contains("pub use kernel::*;"));
    assert!(entry.len() < 1_500, "facade grew to {} bytes", entry.len());

    let kernel = read(&root, "src/continuation/kernel.rs");
    for module in ["catalog", "entropy", "model", "spec"] {
        assert!(kernel.contains(&format!("mod {module};")));
        assert!(kernel.contains(&format!("pub use {module}::*;")));
    }
    assert!(kernel.contains("mod tests;"));
    assert!(kernel.len() < 2_000, "kernel grew to {} bytes", kernel.len());

    for source in [&entry, &kernel] {
        for forbidden in [
            "pub struct ContinuationCatalog",
            "pub fn qualified_entropy_32",
            "impl ContinuationCatalog",
            "DirectStore",
            "BTreeMap",
            "std::fs",
        ] {
            assert!(
                !source.contains(forbidden),
                "implementation returned to a continuation facade: {forbidden}"
            );
        }
    }
}

#[test]
fn continuation_responsibilities_stay_separated() {
    let root = crate_root();
    let owners = [
        ("src/continuation/kernel/spec.rs", "pub enum ContinuationError"),
        ("src/continuation/kernel/entropy.rs", "pub fn qualified_entropy_32"),
        ("src/continuation/kernel/model.rs", "pub struct SearchPage"),
        ("src/continuation/kernel/catalog.rs", "pub struct ContinuationCatalog"),
    ];
    for (relative, marker) in owners {
        let source = read(&root, relative);
        assert!(source.contains(marker), "{relative} lost owner {marker}");
        assert!(
            source.len() < 18_000,
            "{relative} grew to {} bytes",
            source.len()
        );
        for forbidden in ["qdrant_client", "search_qdrant", "reqwest::", "tokio::"] {
            assert!(
                !source.contains(forbidden),
                "{relative} acquired forbidden provider token {forbidden}"
            );
        }
    }

    let entropy = read(&root, "src/continuation/kernel/entropy.rs");
    assert!(entropy.contains("BCryptGenRandom"));
    assert!(entropy.contains("/dev/urandom"));
    assert!(!entropy.contains("ContinuationCatalog"));

    let catalog = read(&root, "src/continuation/kernel/catalog.rs");
    assert!(catalog.contains("fn allocate_token"));
    assert!(catalog.contains("fn expire"));
    assert!(!catalog.contains("unsafe extern"));
    assert!(!catalog.contains("/dev/urandom"));

    let model = read(&root, "src/continuation/kernel/model.rs");
    assert!(model.contains("pub struct LiveExpansionBarrier"));
    assert!(model.contains("pub struct PageCoverage"));
    assert!(!model.contains("BTreeMap"));
    assert!(!model.contains("qualified_entropy_32"));

    let tests = read(&root, "src/continuation/kernel/tests.rs");
    assert!(tests.contains("tokens_are_unique_opaque_session_binders"));
    assert!(tests.contains("every_live_barrier_denial_drops_the_window"));
    assert!(tests.contains("capacity_exhaustion_and_final_pages_release_every_pin"));
    assert!(tests.len() < 24_000, "test corpus grew to {} bytes", tests.len());
}

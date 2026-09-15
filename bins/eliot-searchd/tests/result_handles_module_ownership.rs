use std::path::{Path, PathBuf};

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

#[test]
fn result_handle_entry_and_kernel_are_thin() {
    let root = crate_root();
    let entry = read(&root, "src/result_handles.rs");
    assert!(entry.contains("#[path = \"result_handles/kernel.rs\"]"));
    assert!(entry.contains("pub use kernel::*;"));
    assert!(entry.len() < 1_500, "entry grew to {} bytes", entry.len());
    for forbidden in [
        "struct ResultHandleCatalog",
        "fn expand_with_live_barrier",
        "qualified_entropy_32",
        "BTreeMap",
        "#[cfg(test)]",
    ] {
        assert!(
            !entry.contains(forbidden),
            "result-handle implementation returned to facade: {forbidden}"
        );
    }

    let kernel = read(&root, "src/result_handles/kernel.rs");
    for module in ["catalog", "error", "expand", "model", "spec"] {
        assert!(kernel.contains(&format!("mod {module};")));
    }
    assert!(kernel.contains("mod tests;"));
    assert!(kernel.len() < 2_500, "kernel grew to {} bytes", kernel.len());
    for forbidden in ["BTreeMap", "read_revision_range", "qualified_entropy_32"] {
        assert!(
            !kernel.contains(forbidden),
            "implementation returned to result-handle kernel: {forbidden}"
        );
    }
}

#[test]
fn result_handle_responsibilities_stay_separated() {
    let root = crate_root();
    let owners = [
        ("src/result_handles/kernel/error.rs", "pub enum ResultHandleError"),
        ("src/result_handles/kernel/model.rs", "pub struct PublicHandledMatch"),
        ("src/result_handles/kernel/catalog.rs", "pub struct ResultHandleCatalog"),
        (
            "src/result_handles/kernel/expand.rs",
            "fn expand_with_live_barrier(",
        ),
    ];
    for (relative, marker) in owners {
        let source = read(&root, relative);
        assert!(source.contains(marker), "{relative} lost owner {marker}");
        assert!(
            source.len() < 18_000,
            "{relative} grew to {} bytes",
            source.len()
        );
        for forbidden in [
            "qdrant_client",
            "search_qdrant",
            "reqwest::",
            "tokio::",
            "std::process::Command",
            "Command::new",
        ] {
            assert!(
                !source.contains(forbidden),
                "{relative} acquired forbidden token {forbidden}"
            );
        }
    }

    let spec = read(&root, "src/result_handles/kernel/spec.rs");
    assert!(spec.contains("MAX_RESULT_HANDLES"));
    assert!(spec.contains("MAX_HANDLE_EXPANSION_BYTES"));
    assert!(spec.contains("RESULT_HANDLE_TTL"));
    assert!(!spec.contains("DirectStore"));

    let catalog = read(&root, "src/result_handles/kernel/catalog.rs");
    assert!(catalog.contains("qualified_entropy_32"));
    assert!(catalog.contains("fn mint_page("));
    assert!(catalog.contains("mint_durable_source"));
    assert!(catalog.contains("plaintext tokens never" ) == false);
    assert!(!catalog.contains("read_revision_range"));
    assert!(!catalog.contains("LiveExpansionBarrier"));

    let expand = read(&root, "src/result_handles/kernel/expand.rs");
    assert!(expand.contains("LiveExpansionBarrier"));
    assert!(expand.contains("read_revision_range"));
    assert!(expand.contains("ReadbackMismatch"));
    assert!(!expand.contains("qualified_entropy_32"));
    assert!(!expand.contains("sha256::hex"));

    let model = read(&root, "src/result_handles/kernel/model.rs");
    assert!(model.contains("ResultHandleRecord"));
    assert!(model.contains("ResultHandleExpansion"));
    assert!(!model.contains("DirectStore"));
}

#[test]
fn result_handle_regression_corpus_is_separate() {
    let root = crate_root();
    let tests = read(&root, "src/result_handles/kernel/tests.rs");
    assert!(tests.len() < 35_000, "result-handle tests grew to {} bytes", tests.len());
    for case in [
        "handle_tokens_are_unique_opaque",
        "cross_session_and_cross_root_replay_denied",
        "foreign_namespace_mint_and_expand_rejected",
        "expired_handle_reports_expired",
        "revoked_and_purged_barriers_drop_handle",
        "retired_source_denies_expansion_and_drops_handle",
        "exact_provenance_readback_roundtrip",
        "range_widening_and_oversize_denied",
        "durable_mint_is_denied_for_ephemeral_catalog",
        "restart_forgets_handles",
        "handle_capacity_bounded_and_releasable",
        "expired_handles_reaped_by_next_operation",
        "foreign_session_tag_is_not_found",
        "catalog_debug_redacts_tokens",
    ] {
        assert!(tests.contains(&format!("fn {case}(")), "lost test {case}");
    }
}

use std::path::{Path, PathBuf};

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn workspace_root() -> PathBuf {
    crate_root()
        .parent()
        .and_then(Path::parent)
        .expect("daemon is a workspace member")
        .to_owned()
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

#[test]
fn source_roots_entry_and_kernel_are_thin() {
    let root = crate_root();
    let entry = read(&root, "src/source_roots.rs");
    assert!(entry.contains("#[path = \"source_roots/kernel.rs\"]"));
    assert!(entry.contains("pub use kernel::*;"));
    assert!(entry.len() < 1_500, "entry grew to {} bytes", entry.len());
    for forbidden in [
        "struct SourceRootCatalog",
        "fn persist_entries",
        "OpenOptions",
        "symlink_metadata",
        "#[cfg(test)]",
    ] {
        assert!(
            !entry.contains(forbidden),
            "source-root implementation returned to facade: {forbidden}"
        );
    }

    let kernel = read(&root, "src/source_roots/kernel.rs");
    for module in ["catalog", "error", "model", "path", "registry", "spec"] {
        assert!(kernel.contains(&format!("mod {module};")));
    }
    assert!(kernel.contains("mod tests;"));
    assert!(kernel.contains("pub use search_source_registry::{"));
    assert!(kernel.len() < 3_500, "kernel grew to {} bytes", kernel.len());
    for forbidden in ["std::fs", "OpenOptions", "struct SourceRootCatalog"] {
        assert!(
            !kernel.contains(forbidden),
            "implementation returned to source-root kernel: {forbidden}"
        );
    }
}

#[test]
fn legacy_source_root_catalog_schema_has_one_registry_owner() {
    let workspace = workspace_root();
    let package = read(
        &workspace,
        "crates/search-source/search-source-registry/src/legacy_root_catalog.rs",
    );
    let daemon_spec = read(
        &workspace,
        "bins/eliot-searchd/src/source_roots/kernel/spec.rs",
    );
    let daemon_registry = read(
        &workspace,
        "bins/eliot-searchd/src/source_roots/kernel/registry.rs",
    );

    for marker in [
        "# ELIOT Search source roots v1",
        "pub const LEGACY_SOURCE_ROOT_CATALOG_MAX_ROOTS: usize = 32",
        "pub const LEGACY_SOURCE_ROOT_CATALOG_MAX_BYTES: usize = 64 * 1024",
        "pub fn decode_legacy_source_root_catalog(",
        "pub fn encode_legacy_source_root_catalog<I, S>(",
        "LegacySourceRootCatalogError::DuplicateRoot",
    ] {
        assert!(package.contains(marker), "registry package lost owner {marker}");
    }
    for forbidden in [
        "std::fs",
        "std::path",
        "OpenOptions",
        "canonicalize",
        "symlink_metadata",
        "source bytes",
        "qdrant_client",
    ] {
        assert!(
            !package.contains(forbidden),
            "pure legacy root codec acquired forbidden token {forbidden}"
        );
    }

    assert!(daemon_spec.contains(
        "LEGACY_SOURCE_ROOT_CATALOG_MAX_ROOTS as MAX_SOURCE_ROOTS"
    ));
    assert!(daemon_spec.contains(
        "LEGACY_SOURCE_ROOT_CATALOG_MAX_BYTES as MAX_SOURCE_ROOT_FILE_BYTES"
    ));
    assert!(!daemon_spec.contains("# ELIOT Search source roots v1"));
    assert!(daemon_registry.contains("decode_legacy_source_root_catalog"));
    assert!(daemon_registry.contains("encode_legacy_source_root_catalog"));
    assert!(!daemon_registry.contains("# ELIOT Search source roots v1"));
}

#[test]
fn root_currentness_state_has_one_registry_owner() {
    let workspace = workspace_root();
    let package = read(
        &workspace,
        "crates/search-source/search-source-registry/src/root_currentness.rs",
    );
    let daemon_catalog = read(
        &workspace,
        "bins/eliot-searchd/src/source_roots/kernel/catalog.rs",
    );
    let daemon_model = read(
        &workspace,
        "bins/eliot-searchd/src/source_roots/kernel/model.rs",
    );
    let daemon_spec = read(
        &workspace,
        "bins/eliot-searchd/src/source_roots/kernel/spec.rs",
    );

    for marker in [
        "pub struct SourceRootCurrentness",
        "pub enum SourceRootState",
        "pub enum ObservationGapReason",
        "pub struct ReconciliationCursor",
        "pub struct CurrentWorkspaceTruth",
        "pub const MAX_SOURCE_ROOT_WATCHER_HINTS: usize = 64",
        "pub const MAX_SOURCE_ROOT_OBSERVATION_GAPS: usize = 40",
        "pub fn reconcile(&mut self, observed: &[SourceRootState]) -> bool",
        "pub fn note_watcher_hint(",
        "pub fn mark_reconciled_synced(&mut self) -> bool",
    ] {
        assert!(package.contains(marker), "registry package lost owner {marker}");
    }
    for forbidden in [
        "std::fs",
        "std::path",
        "PathBuf",
        "OpenOptions",
        "probe_root",
        "canonicalize",
        "qdrant_client",
        "search_source_reconcile",
    ] {
        assert!(
            !package.contains(forbidden),
            "pure currentness owner acquired forbidden token {forbidden}"
        );
    }

    assert!(daemon_catalog.contains("currentness: SourceRootCurrentness"));
    assert!(daemon_catalog.contains("self.currentness.reconcile(&observed)"));
    assert!(daemon_catalog.contains("self.currentness.mark_update_outcome_unknown()"));
    for removed in [
        "needs_reopen: bool",
        "watcher_sequence: u64",
        "pending_hints: Vec<WatcherHint>",
        "watcher_overflowed: bool",
        "reconciliation_generation: u64",
        "last_synced_generation: Option<u64>",
    ] {
        assert!(
            !daemon_catalog.contains(removed),
            "daemon restored currentness owner {removed}"
        );
    }
    assert!(!daemon_model.contains("pub enum SourceRootState"));
    assert!(!daemon_model.contains("pub enum ObservationGapReason"));
    assert!(!daemon_model.contains("pub struct ReconciliationCursor"));
    assert!(!daemon_model.contains("pub struct CurrentWorkspaceTruth"));
    assert!(daemon_spec.contains(
        "MAX_SOURCE_ROOT_WATCHER_HINTS as MAX_WATCHER_HINTS"
    ));
    assert!(daemon_spec.contains(
        "MAX_SOURCE_ROOT_OBSERVATION_GAPS as MAX_OBSERVATION_GAPS"
    ));
}

#[test]
fn source_root_responsibilities_stay_separated() {
    let root = crate_root();
    let owners = [
        ("src/source_roots/kernel/error.rs", "pub enum SourceRootError"),
        ("src/source_roots/kernel/model.rs", "pub struct SourceRootView"),
        ("src/source_roots/kernel/path.rs", "fn canonicalize_new_root("),
        ("src/source_roots/kernel/registry.rs", "fn persist_entries("),
        ("src/source_roots/kernel/catalog.rs", "pub struct SourceRootCatalog"),
    ];
    for (relative, marker) in owners {
        let source = read(&root, relative);
        assert!(source.contains(marker), "{relative} lost owner {marker}");
        assert!(
            source.len() < 20_000,
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

    let spec = read(&root, "src/source_roots/kernel/spec.rs");
    assert!(spec.contains("MAX_SOURCE_ROOTS"));
    assert!(spec.contains("MAX_WATCHER_HINTS"));
    assert!(!spec.contains("# ELIOT Search source roots v1"));
    assert!(!spec.contains("std::fs"));

    let path = read(&root, "src/source_roots/kernel/path.rs");
    assert!(path.contains("probe_root"));
    assert!(path.contains("ensure_outside_data_root"));
    assert!(path.contains("reject_symlink"));
    assert!(!path.contains("OpenOptions"));
    assert!(!path.contains("source-roots.v1"));

    let registry = read(&root, "src/source_roots/kernel/registry.rs");
    assert!(registry.contains("OpenOptions::new"));
    assert!(registry.contains("create_new(true)"));
    assert!(registry.contains("UpdateOutcomeUnknown"));
    assert!(registry.contains("pub fn migration_input"));
    assert!(registry.contains("decode_legacy_source_root_catalog"));
    assert!(registry.contains("encode_legacy_source_root_catalog"));
    assert!(!registry.contains("mark_reconciled_synced"));

    let catalog = read(&root, "src/source_roots/kernel/catalog.rs");
    assert!(catalog.contains("note_watcher_hint"));
    assert!(catalog.contains("observation_gaps"));
    assert!(catalog.contains("current_workspace_truth"));
    assert!(catalog.contains("mark_reconciled_synced"));
    assert!(catalog.contains("SourceRootCurrentness"));
    assert!(!catalog.contains("OpenOptions"));

    let model = read(&root, "src/source_roots/kernel/model.rs");
    assert!(model.contains("SourceRootView"));
    assert!(model.contains("SourceRootEntry"));
    assert!(!model.contains("ObservationGapReason"));
    assert!(!model.contains("CurrentWorkspaceTruth"));
    assert!(!model.contains("std::fs"));
}

#[test]
fn source_root_regression_corpus_is_separate() {
    let root = crate_root();
    let tests = read(&root, "src/source_roots/kernel/tests.rs");
    assert!(tests.len() < 35_000, "source-root tests grew to {} bytes", tests.len());
    for case in [
        "persists_reloads_adds_and_removes",
        "missing_root_is_retained_but_unavailable",
        "refresh_reports_swapped_availability_with_unchanged_count",
        "owned_catalog_rejects_data_root_in_both_overlap_directions",
        "malformed_current_catalog_is_not_replaced_by_valid_backup",
        "interrupted_replacement_restores_last_current_catalog",
        "replacement_by_regular_file_does_not_prevent_unregistering",
        "nested_sources_are_rejected_and_duplicates_are_idempotent",
        "watcher_hints_are_bounded_and_never_prove_availability",
        "missing_and_replaced_roots_are_gaps_and_block_sync_proof",
        "active_set_mutation_invalidates_sync_proof",
    ] {
        assert!(tests.contains(&format!("fn {case}(")), "lost test {case}");
    }
}

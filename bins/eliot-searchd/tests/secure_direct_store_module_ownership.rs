use std::path::{Path, PathBuf};

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

#[test]
fn secure_direct_store_entry_and_kernel_are_thin() {
    let root = crate_root();
    let entry = read(&root, "src/secure_direct_store.rs");
    assert!(entry.contains("#[path = \"secure_direct_store/kernel.rs\"]"));
    assert!(entry.contains("pub use kernel::DirectStore;"));
    assert!(entry.len() < 4_000, "entry grew to {} bytes", entry.len());
    for forbidden in [
        "pub struct DirectStore",
        "pub(crate) fn search(",
        "pub(crate) fn index_file(",
        "fn migrate_referenced_plaintext",
        "CANONICAL_CORPUS_BUDGET",
        "#[cfg(test)]",
    ] {
        assert!(
            !entry.contains(forbidden),
            "store implementation returned to facade: {forbidden}"
        );
    }

    let kernel = read(&root, "src/secure_direct_store/kernel.rs");
    for module in ["catalog", "lifecycle", "read", "search"] {
        assert!(kernel.contains(&format!("mod {module};")));
    }
    assert!(kernel.contains("pub struct DirectStore"));
    assert!(kernel.contains("pub(super) root: PathBuf"));
    assert!(kernel.contains("pub(super) inner: plaintext::DirectStore"));
    assert!(kernel.contains("pub(super) protector: RevisionProtector"));
    assert!(kernel.contains("mod tests;"));
    assert!(kernel.len() < 5_000, "kernel grew to {} bytes", kernel.len());
}

#[test]
fn direct_store_responsibilities_remain_bounded() {
    let root = crate_root();
    let owners = [
        (
            "src/secure_direct_store/kernel/lifecycle.rs",
            "pub(crate) fn open(",
        ),
        (
            "src/secure_direct_store/kernel/catalog.rs",
            "pub(crate) fn index_file(",
        ),
        (
            "src/secure_direct_store/kernel/search.rs",
            "pub(crate) fn search(",
        ),
        (
            "src/secure_direct_store/kernel/read.rs",
            "pub(crate) fn read_revision_range(",
        ),
    ];
    for (relative, marker) in owners {
        let source = read(&root, relative);
        assert!(source.contains(marker), "{relative} lost owner {marker}");
        assert!(
            source.len() < 16_000,
            "{relative} grew to {} bytes",
            source.len()
        );
        for forbidden in [
            "qdrant_client",
            "search_qdrant_bridge",
            "reqwest::",
            "tokio::",
            "Command::new",
        ] {
            assert!(
                !source.contains(forbidden),
                "{relative} acquired vendor/runtime token {forbidden}"
            );
        }
    }

    let lifecycle = read(&root, "src/secure_direct_store/kernel/lifecycle.rs");
    assert!(lifecycle.contains("catalog_presence::check_before_open"));
    assert!(lifecycle.contains("migrate_referenced_plaintext"));
    assert!(lifecycle.contains("persist_verified"));
    assert!(lifecycle.contains("remove_plaintext_after_readback"));
    assert!(!lifecycle.contains("scan_prepared"));

    let catalog = read(&root, "src/secure_direct_store/kernel/catalog.rs");
    assert!(catalog.contains("index_file_with_writer"));
    assert!(catalog.contains("index_directory_with_writer"));
    assert!(catalog.contains("preparation_store::persist_source"));
    assert!(catalog.contains("preparation_store::persist("));
    assert!(!catalog.contains("scan_prepared"));

    let search = read(&root, "src/secure_direct_store/kernel/search.rs");
    assert!(search.contains("preparation_store::load"));
    assert!(search.contains("scan_prepared"));
    assert!(search.contains("validate_source_backed_match"));
    assert!(search.contains("eliot-search/direct-evidence/v1"));
    assert!(!search.contains("persist_source"));
    assert!(!search.contains("persist_immutable_object"));
    assert!(!search.contains("index_file_with_writer"));
    assert!(!search.contains("std::fs"));

    let read_owner = read(&root, "src/secure_direct_store/kernel/read.rs");
    assert!(read_owner.contains("DIRECT_REVISION_RANGE_TOO_LARGE"));
    assert!(read_owner.contains("DIRECT_REVISION_KEY_UNAVAILABLE"));
    assert!(read_owner.contains("verify_revision_identity(metadata)"));
    assert!(!read_owner.contains("preparation_store"));
    assert!(!read_owner.contains("index_file_with_writer"));
}

#[test]
fn direct_store_compatibility_and_query_regression_stay_closed() {
    let root = crate_root();
    let entry = read(&root, "src/secure_direct_store.rs");
    for compatibility_name in [
        "PreparationBatch",
        "PreparationCursor",
        "IndexedSource",
        "RevisionSlice",
        "SourceSummary",
        "StoreGap",
        "StoreSearchResult",
        "StoreVerification",
        "StoredMatch",
        "MAX_REVISION_OBJECT_BYTES",
        "REVISION_DIRECTORY",
        "verify_plaintext",
        "legacy_path",
        "protected_path",
        "read_regular_file",
    ] {
        assert!(entry.contains(compatibility_name), "lost compatibility name {compatibility_name}");
    }

    let kernel = read(&root, "src/secure_direct_store/kernel.rs");
    assert!(kernel.contains(
        "LEGACY_REVISION_DIRECTORY as REVISION_DIRECTORY"
    ));
    assert!(kernel.contains(
        "LEGACY_REVISION_MAX_OBJECT_BYTES as MAX_REVISION_OBJECT_BYTES"
    ));
    assert!(!kernel.contains(
        "const REVISION_DIRECTORY: &str = \"revisions\""
    ));
    assert!(!kernel.contains(
        "const MAX_REVISION_OBJECT_BYTES: usize = 65 * 1024 * 1024"
    ));

    let tests = read(&root, "src/secure_direct_store/kernel/tests.rs");
    assert!(tests.contains("fn ten_thousand_bounded_queries_cause_no_durable_corpus_writes("));
    assert!(tests.contains("for _ in 0..10_000"));
    assert!(tests.contains("corpus_snapshot(guard.canonical_root())"));
    assert!(tests.len() < 12_000, "regression corpus grew to {} bytes", tests.len());
}

#[test]
fn existing_sync_child_reuses_replay_and_credential_without_effect_surface() {
    let root = crate_root();
    let source = read(&root, "src/secure_direct_store/existing_read.rs");
    assert_eq!(source.matches("plaintext::DirectStore::from_existing_bytes_checked(").count(), 1);
    assert_eq!(source.matches("sha256::decode_digest(").count(), 1);
    assert_eq!(source.matches("RevisionProtector::for_existing_credential(").count(), 3);
    let constructor = read(&root, "src/revision_protection/protector/initialization.rs");
    assert_eq!(constructor.matches("fn for_existing_credential(").count(), 1);
    assert!(!constructor.contains("for_existing_read"));
    let child = source.split("pub struct ExistingSyncStore {").nth(1).unwrap()
        .split("impl core::fmt::Debug").next().unwrap();
    assert!(child.contains("store: DirectStore"));
    assert!(!child.contains("pub store:"));
    assert_eq!(child.matches("pub(crate) fn ").count(), 1);
    for required in [
        "self.store.root != operation.root()", "operation.selected_binding()",
        "self.store.protector.initialization_key_binding()?", "operation.verify()",
    ] { assert!(child.contains(required), "missing closed child check {required}"); }
    for forbidden in ["impl DirectRead for ExistingSyncStore", "impl Deref", "for_existing_read"] {
        assert!(!source.contains(forbidden), "sync child escape: {forbidden}");
    }
    let open = source.split("pub fn open_existing_sync(").nth(1).unwrap()
        .split("fn owner_error(").next().unwrap();
    assert!(open.find("replay_existing(").unwrap() < open.find("credential_call(").unwrap());
    assert_eq!(open.matches("operation.verify()?").count(), 2);
    for forbidden in [
        "DirectStore::open", "verify_control", "for_initialization", "load_or_create",
        "protect(", "unprotect(", "index_file(", "sync_directory(", "write_all",
        "read_dir", "create(true)", "mark_reconciled_synced",
    ] { assert!(!open.contains(forbidden), "effect reached sync composition: {forbidden}"); }
}

#[test]
fn migration_child_is_closed_and_reuses_pinned_replay_and_selected_credential() {
    let root = crate_root();
    let source = read(&root, "src/secure_direct_store/existing_read.rs");
    let child = source.split("pub struct ExistingMigrationStore {").nth(1).unwrap()
        .split("impl core::fmt::Debug").next().unwrap();
    assert_eq!(child.matches("pub(crate) fn ").count(), 1);
    for required in ["stage_mapping(", "operation.verify()", "operation.selected_binding()",
        "MigrationContext::new(operation)", "namespace: &self.namespace", "log: &self.log"] {
        assert!(child.contains(required), "missing migration boundary {required}");
    }
    let open = source.split("pub fn open_existing_migration(").nth(1).unwrap()
        .split("/// Private daemon read surface").next().unwrap();
    assert_eq!(open.matches("for_existing_credential(").count(), 1);
    assert!(open.find("replay_existing_checked(").unwrap() < open.find("credential_call(").unwrap());
    for forbidden in ["impl DirectRead for ExistingMigrationStore", "impl Deref", "pub store:",
        "DirectStore::open(", "load_or_create(", "for_initialization("] {
        assert!(!child.contains(forbidden) && !open.contains(forbidden), "migration escape: {forbidden}");
    }
}

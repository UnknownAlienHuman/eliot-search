use std::path::{Path, PathBuf};

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .expect("revision-store package is nested under crates/search-source")
        .to_owned()
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

#[test]
fn immutable_revision_object_filesystem_lifecycle_has_one_package_owner() {
    let root = repository_root();
    let facade = read(
        &root,
        "crates/search-source/search-revision-store/src/lib.rs",
    );
    assert!(facade.contains("mod immutable_object;"));
    assert!(facade.contains("pub use immutable_object::*;"));

    let owner = read(
        &root,
        "crates/search-source/search-revision-store/src/immutable_object.rs",
    );
    for required in [
        "pub trait LegacyRevisionObjectPlatform",
        "pub fn read_legacy_revision_object",
        "pub fn publish_legacy_revision_object",
        "OpenOptions",
        "fs::hard_link",
        "fs::remove_file",
        "read_to_end",
        "sync_all",
        "IdentityChanged",
        "PublishOutcomeUnknown",
    ] {
        assert!(
            owner.contains(required),
            "revision-store lost immutable-object behavior {required}"
        );
    }
    for forbidden in [
        "RevisionProtector",
        "DPAPI",
        "search_os_secrets",
        "verify_plaintext",
        "SourceRegistry",
        "ControlJournal",
        "LegacyPreparationArtifact",
    ] {
        assert!(
            !owner.contains(forbidden),
            "revision-store acquired foreign responsibility {forbidden}"
        );
    }
}

#[test]
fn daemon_composes_revision_owner_and_keeps_materializer_distinct() {
    let root = repository_root();
    let adapter = read(
        &root,
        "bins/eliot-searchd/src/secure_direct_store_storage_io.rs",
    );
    assert!(adapter.contains("read_legacy_revision_object"));
    assert!(adapter.contains("publish_legacy_revision_object"));
    assert!(adapter.contains("pub(super) fn persist_revision_object"));
    assert!(adapter.contains("pub(super) fn read_revision_object"));
    assert!(adapter.contains("read_legacy_preparation_artifact"));
    assert!(adapter.contains("publish_legacy_preparation_artifact"));

    let writer = read(
        &root,
        "bins/eliot-searchd/src/secure_revision_writer.rs",
    );
    assert!(writer.contains("persist_revision_object"));
    assert!(writer.contains("read_revision_object"));
    assert!(!writer.contains("persist_immutable_object"));
    assert!(!writer.contains("read_regular_file"));

    let store_facade = read(
        &root,
        "bins/eliot-searchd/src/secure_direct_store.rs",
    );
    assert!(store_facade.contains("read_revision_object as read_regular_file"));

    let preparation = read(
        &root,
        "bins/eliot-searchd/src/preparation_store/kernel/persist.rs",
    );
    assert!(preparation.contains("persist_immutable_object"));
    assert!(!preparation.contains("persist_revision_object"));
}

#[test]
fn revision_store_is_a_baseline_direct_dependency() {
    let root = repository_root();
    let manifest = read(&root, "bins/eliot-searchd/Cargo.toml");
    assert!(manifest.contains("wave2-source = []"));
    assert!(manifest.contains("search-revision-store.workspace = true"));
    assert!(!manifest.contains(
        "search-revision-store = { workspace = true, optional = true }"
    ));
    assert!(!manifest.contains("dep:search-revision-store"));
}

#[test]
fn legacy_revision_layout_and_inventory_grammar_have_one_package_owner() {
    let root = repository_root();
    let facade = read(
        &root,
        "crates/search-source/search-revision-store/src/lib.rs",
    );
    assert!(facade.contains("mod legacy_inventory;"));
    assert!(facade.contains("pub use legacy_inventory::*;"));

    let owner = read(
        &root,
        "crates/search-source/search-revision-store/src/legacy_inventory.rs",
    );
    for required in [
        "pub const LEGACY_REVISION_DIRECTORY",
        "pub const LEGACY_REVISION_MAX_OBJECT_BYTES",
        "pub const LEGACY_REVISION_MAX_INVENTORY_NAME_BYTES",
        "pub enum LegacyRevisionProtection",
        "pub enum LegacyRevisionInventoryKind",
        "pub fn classify_legacy_revision_inventory_name",
        "pub fn is_legacy_revision_inventory_shard",
        "pub fn legacy_revision_inventory_relative_locator",
        "pub fn legacy_revision_rooted_locator",
        "catalog_referenced",
        "unreferenced_revision_object",
        "uncommitted_temporary_object",
    ] {
        assert!(
            owner.contains(required),
            "revision-store lost inventory grammar {required}"
        );
    }
    for forbidden in [
        "std::fs",
        "Metadata",
        "read_dir",
        "RevisionProtector",
        "SourceRegistry",
        "ControlJournal",
        "verify_plaintext",
    ] {
        assert!(
            !owner.contains(forbidden),
            "inventory grammar acquired foreign responsibility {forbidden}"
        );
    }

    let daemon = read(
        &root,
        "bins/eliot-searchd/src/control_migration_orphans.rs",
    );
    for required in [
        "classify_legacy_revision_inventory_name",
        "is_legacy_revision_inventory_shard",
        "legacy_revision_inventory_relative_locator",
        "legacy_revision_rooted_locator",
        "LegacyRevisionInventoryKind as Kind",
    ] {
        assert!(daemon.contains(required));
    }
    for forbidden in [
        "enum Kind",
        "fn generated_name(",
        "fn lower_hex(",
        "MAX_NAME_BYTES",
        "Self::Referenced => \"catalog_referenced\"",
        "unreferenced_revision_object\"",
        "uncommitted_temporary_object\"",
        "strip_suffix(\".bin\")",
        "strip_suffix(\".dpapi\")",
    ] {
        assert!(
            !daemon.contains(forbidden),
            "daemon restored inventory grammar {forbidden}"
        );
    }

    let kernel = read(
        &root,
        "bins/eliot-searchd/src/secure_direct_store/kernel.rs",
    );
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
}

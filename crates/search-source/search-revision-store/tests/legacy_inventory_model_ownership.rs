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
fn legacy_revision_inventory_model_has_one_package_owner() {
    let root = repository_root();
    let model = read(
        &root,
        "crates/search-source/search-revision-store/src/legacy_inventory/model.rs",
    );
    let page = read(
        &root,
        "crates/search-source/search-revision-store/src/legacy_inventory/page.rs",
    );
    let report = read(
        &root,
        "crates/search-source/search-revision-store/src/legacy_inventory/report.rs",
    );
    for required in [
        "pub struct LegacyRevisionInventoryEntry",
        "pub struct LegacyRevisionInventory",
        "pub trait LegacyRevisionInventoryDigest",
        "pub fn build_legacy_revision_inventory",
        "eliot-search/revision-residue-inventory/v1",
        "eliot-search/revision-residue-shard/v1",
        "eliot-search/revision-residue-file/v1",
    ] {
        assert!(model.contains(required), "model lost owner {required}");
    }
    for required in [
        "pub struct LegacyRevisionInventoryCursor",
        "pub struct LegacyRevisionInventoryPage",
        "pub fn legacy_revision_inventory_checkpoint",
        "pub fn plan_legacy_revision_inventory_page",
        "eliot-search/control-migration-orphans/v1",
    ] {
        assert!(page.contains(required), "page owner lost {required}");
    }
    for required in [
        "pub struct LegacyRevisionObjectEvidence",
        "pub fn render_legacy_revision_inventory_report",
        "eliot-search/control-migration-orphan-page/v1",
        "legacy-revision-orphans-v1",
    ] {
        assert!(report.contains(required), "report owner lost {required}");
    }
    for forbidden in [
        "std::fs",
        "read_dir",
        "Metadata",
        "RevisionProtector",
        "read_regular_file",
        "SourceRegistry",
        "ControlJournal",
    ] {
        assert!(!model.contains(forbidden));
        assert!(!page.contains(forbidden));
        assert!(!report.contains(forbidden));
    }
}

#[test]
fn daemon_retains_only_observation_and_fingerprint_composition() {
    let root = repository_root();
    let daemon = read(
        &root,
        "bins/eliot-searchd/src/control_migration_orphans.rs",
    );
    for required in [
        "build_legacy_revision_inventory",
        "LegacyRevisionInventoryCursor::parse",
        "legacy_revision_inventory_checkpoint",
        "plan_legacy_revision_inventory_page",
        "render_legacy_revision_inventory_report",
        "fs::read_dir",
        "fs::symlink_metadata",
        "read_regular_file",
        "verify_migration_snapshot",
    ] {
        assert!(daemon.contains(required), "daemon lost composition {required}");
    }
    for forbidden in [
        "struct Entry",
        "struct Inventory",
        "struct Cursor",
        "const MAX_FILES",
        "const PAGE_FILES",
        "eliot-search/revision-residue-inventory/v1",
        "eliot-search/revision-residue-shard/v1",
        "eliot-search/revision-residue-file/v1",
        "eliot-search/control-migration-orphans/v1",
        "eliot-search/control-migration-orphan-page/v1",
        "legacy-revision-orphans-v1",
        "DIRECT_MIGRATION_ORPHAN_CURSOR_INVALID",
        "DIRECT_MIGRATION_ORPHAN_CURSOR_STALE",
        "DIRECT_MIGRATION_NO_PROGRESS",
        "DIRECT_MIGRATION_PAGE_TOO_LARGE",
    ] {
        assert!(
            !daemon.contains(forbidden),
            "daemon restored inventory model/projection {forbidden}"
        );
    }
}

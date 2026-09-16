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
    ] {
        assert!(
            !owner.contains(forbidden),
            "revision-store acquired foreign responsibility {forbidden}"
        );
    }
}

#[test]
fn daemon_composes_revision_owner_without_moving_preparation_objects() {
    let root = repository_root();
    let adapter = read(
        &root,
        "bins/eliot-searchd/src/secure_direct_store_storage_io.rs",
    );
    assert!(adapter.contains("read_legacy_revision_object"));
    assert!(adapter.contains("publish_legacy_revision_object"));
    assert!(adapter.contains("struct DaemonRevisionObjectPlatform"));
    assert!(adapter.contains("pub(super) fn persist_revision_object"));
    assert!(adapter.contains("pub(super) fn read_revision_object"));
    assert!(
        adapter.contains("pub(super) fn persist_immutable_object"),
        "preparation artifacts retain their separate temporary owner until the materializer slice"
    );

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

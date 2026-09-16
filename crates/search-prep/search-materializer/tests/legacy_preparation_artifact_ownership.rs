use std::path::{Path, PathBuf};

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .expect("materializer package is nested under crates/search-prep")
        .to_owned()
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

#[test]
fn preparation_artifact_filesystem_lifecycle_has_one_package_owner() {
    let root = repository_root();
    let facade = read(
        &root,
        "crates/search-prep/search-materializer/src/lib.rs",
    );
    assert!(facade.contains("mod legacy_artifact;"));
    assert!(facade.contains("pub use legacy_artifact::*;"));

    let owner = read(
        &root,
        "crates/search-prep/search-materializer/src/legacy_artifact.rs",
    );
    for required in [
        "pub trait LegacyPreparationArtifactPlatform",
        "pub fn read_legacy_preparation_artifact",
        "pub fn publish_legacy_preparation_artifact",
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
            "materializer lost preparation-artifact behavior {required}"
        );
    }
    for forbidden in [
        "RevisionProtector",
        "DPAPI",
        "search_os_secrets",
        "SourceRegistry",
        "ControlJournal",
        "verify_plaintext",
    ] {
        assert!(
            !owner.contains(forbidden),
            "materializer acquired foreign responsibility {forbidden}"
        );
    }
}

#[test]
fn daemon_generic_preparation_io_composes_materializer_owner() {
    let root = repository_root();
    let adapter = read(
        &root,
        "bins/eliot-searchd/src/secure_direct_store_storage_io.rs",
    );
    assert!(adapter.contains("read_legacy_preparation_artifact"));
    assert!(adapter.contains("publish_legacy_preparation_artifact"));
    assert!(adapter.contains("impl LegacyPreparationArtifactPlatform"));
    assert!(adapter.contains("pub(super) fn read_regular_file"));
    assert!(adapter.contains("pub(super) fn persist_immutable_object"));
    assert!(!adapter.contains("OpenOptions"));
    assert!(!adapter.contains("fs::hard_link"));
    assert!(!adapter.contains("write_all"));

    let preparation = read(
        &root,
        "bins/eliot-searchd/src/preparation_store/kernel/persist.rs",
    );
    assert!(preparation.contains("persist_immutable_object"));
    assert!(preparation.contains("read_regular_file"));
    assert!(!preparation.contains("persist_revision_object"));

    let revision_writer = read(
        &root,
        "bins/eliot-searchd/src/secure_revision_writer.rs",
    );
    assert!(revision_writer.contains("persist_revision_object"));
    assert!(!revision_writer.contains("persist_immutable_object"));
}

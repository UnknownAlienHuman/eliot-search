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
fn preparation_store_wire_schema_has_one_package_owner() {
    let root = repository_root();
    let facade = read(
        &root,
        "crates/search-prep/search-materializer/src/lib.rs",
    );
    assert!(facade.contains("mod legacy_store;"));
    assert!(facade.contains("pub use legacy_store::*;"));

    let owner = read(
        &root,
        "crates/search-prep/search-materializer/src/legacy_store.rs",
    );
    for required in [
        "pub const LEGACY_PREPARATION_MAGIC",
        "pub const LEGACY_PREPARATION_REFERENCE_MAGIC",
        "pub const LEGACY_PREPARATION_BINDING_BYTES",
        "pub const LEGACY_PREPARATION_REFERENCE_BYTES",
        "pub const LEGACY_PREPARATION_MAX_MANIFEST_BYTES",
        "pub enum LegacyPreparationProtection",
        "pub trait LegacyPreparationStoreDigest",
        "pub fn encode_legacy_preparation_binding",
        "pub fn decode_legacy_preparation_binding",
        "pub fn derive_legacy_preparation_lookup_key",
        "pub fn derive_legacy_preparation_object_id",
        "pub fn encode_legacy_preparation_reference",
        "pub fn decode_legacy_preparation_reference",
        "pub fn encode_legacy_preparation_manifest",
        "pub fn verify_legacy_preparation_manifest",
        "pub fn verify_legacy_preparation_payload",
        "eliot-search/direct-preparation-ref/v2",
        "eliot-search/direct-preparation-object/v2",
    ] {
        assert!(
            owner.contains(required),
            "materializer lost preparation-store schema {required}"
        );
    }
    for forbidden in [
        "std::fs",
        "RevisionProtector",
        "DPAPI",
        "search_os_secrets",
        "SourceRegistry",
        "ControlJournal",
        "verify_plaintext",
    ] {
        assert!(
            !owner.contains(forbidden),
            "preparation schema acquired foreign responsibility {forbidden}"
        );
    }

    let spec = read(
        &root,
        "bins/eliot-searchd/src/preparation_store/kernel/spec.rs",
    );
    assert!(spec.contains("LEGACY_PREPARATION_MAX_OBJECT_BYTES as MAX_OBJECT_BYTES"));
    assert!(spec.contains("LEGACY_PREPARATION_REFERENCE_BYTES as REF_BYTES"));
    for forbidden in [
        "ELSPRP02",
        "ELSPRF01",
        "BINDING_BYTES: usize = 208",
        "OLD_BINDING_BYTES: usize = 176",
        "REF_BYTES: usize = 81",
        "MAX_OBJECT_BYTES: usize = 65 * 1024 * 1024",
    ] {
        assert!(
            !spec.contains(forbidden),
            "daemon restored preparation schema literal {forbidden}"
        );
    }

    let codec = read(
        &root,
        "bins/eliot-searchd/src/preparation_store/kernel/codec.rs",
    );
    for required in [
        "encode_legacy_preparation_binding",
        "decode_legacy_preparation_reference",
        "derive_legacy_preparation_lookup_key",
        "derive_legacy_preparation_object_id",
        "verify_legacy_preparation_manifest",
        "verify_legacy_preparation_payload",
    ] {
        assert!(codec.contains(required));
    }
    for forbidden in [
        "ELSPRP02",
        "ELSPRF01",
        "eliot-search/direct-preparation-ref/v2",
        "eliot-search/direct-preparation-object/v2",
        "manifest[144..176]",
        "manifest[176..208]",
    ] {
        assert!(
            !codec.contains(forbidden),
            "daemon restored preparation wire behavior {forbidden}"
        );
    }

    let persist = read(
        &root,
        "bins/eliot-searchd/src/preparation_store/kernel/persist.rs",
    );
    assert!(persist.contains("encode_legacy_preparation_manifest"));
    assert!(!persist.contains("manifest.extend_from_slice(&representation)"));
    assert!(!persist.contains("CONTENT_DIGEST_ALGORITHM"));

    let load = read(
        &root,
        "bins/eliot-searchd/src/preparation_store/kernel/load.rs",
    );
    assert!(load.contains("verified.body().to_vec()"));
    assert!(!load.contains("manifest[HEADER_BYTES..]"));

    let inspect = read(
        &root,
        "bins/eliot-searchd/src/preparation_store/kernel/inspect.rs",
    );
    assert!(inspect.contains("verified.binding().materializer_digest"));
    assert!(inspect.contains("verified.materializer_revision()"));
    assert!(!inspect.contains("manifest[144..176]"));
    assert!(!inspect.contains("manifest[176..208]"));
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

#[test]
fn preparation_inventory_grammar_and_locators_have_one_package_owner() {
    let root = repository_root();
    let facade = read(
        &root,
        "crates/search-prep/search-materializer/src/lib.rs",
    );
    assert!(facade.contains("mod legacy_inventory;"));
    assert!(facade.contains("pub use legacy_inventory::*;"));

    let owner = read(
        &root,
        "crates/search-prep/search-materializer/src/legacy_inventory.rs",
    );
    for required in [
        "pub enum LegacyPreparationInventoryTree",
        "pub enum LegacyPreparationInventoryKind",
        "pub fn classify_legacy_preparation_inventory_name",
        "pub fn legacy_preparation_reference_relative_locator",
        "pub fn legacy_preparation_object_relative_locator",
        "pub fn legacy_preparation_rooted_locator",
        "unmapped_profile_or_revision_reference",
        "uncommitted_temporary_object",
        ".dpapi.tmp",
    ] {
        assert!(
            owner.contains(required),
            "materializer lost inventory grammar {required}"
        );
    }
    for forbidden in [
        "std::fs",
        "Metadata",
        "read_dir",
        "RevisionProtector",
        "SourceRegistry",
        "ControlJournal",
    ] {
        assert!(
            !owner.contains(forbidden),
            "inventory grammar acquired foreign responsibility {forbidden}"
        );
    }

    let daemon = read(
        &root,
        "bins/eliot-searchd/src/control_migration_preparation.rs",
    );
    for required in [
        "classify_legacy_preparation_inventory_name",
        "legacy_preparation_reference_relative_locator",
        "legacy_preparation_object_relative_locator",
        "legacy_preparation_rooted_locator",
        "LegacyPreparationInventoryKind as Kind",
        "LegacyPreparationInventoryTree",
    ] {
        assert!(daemon.contains(required));
    }
    for forbidden in [
        "enum Kind",
        "fn generated(",
        "fn hex_len(",
        "fn decimal(",
        "current_profile_reference\"",
        "uncommitted_temporary_object\"",
        "strip_suffix(\".ref\")",
        "strip_suffix(\".dpapi.tmp\")",
    ] {
        assert!(
            !daemon.contains(forbidden),
            "daemon restored inventory grammar {forbidden}"
        );
    }
}

use std::path::{Path, PathBuf};

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

#[test]
fn preparation_store_entry_and_kernel_are_thin() {
    let root = crate_root();
    let entry = read(&root, "src/preparation_store.rs");
    assert!(entry.contains("#[path = \"preparation_store/kernel.rs\"]"));
    assert!(entry.contains("pub use kernel::{PreparationBatch, PreparationCursor};"));
    assert!(entry.len() < 2_500, "entry grew to {} bytes", entry.len());
    for forbidden in [
        "fn persist_canonical(",
        "fn verify_manifest(",
        "fn prepare_root(",
        "struct PreparationBatch {",
        "std::fs",
        "Zeroizing",
    ] {
        assert!(
            !entry.contains(forbidden),
            "implementation returned to preparation facade: {forbidden}"
        );
    }

    let kernel = read(&root, "src/preparation_store/kernel.rs");
    for module in [
        "batch", "codec", "inspect", "load", "paths", "persist", "spec",
    ] {
        assert!(kernel.contains(&format!("mod {module};")));
    }
    assert!(kernel.len() < 2_500, "kernel grew to {} bytes", kernel.len());
}

#[test]
fn preparation_store_responsibilities_stay_bounded() {
    let root = crate_root();
    let owners = [
        (
            "src/preparation_store/kernel/spec.rs",
            "pub(crate) const REF_BYTES: usize = 81;",
        ),
        (
            "src/preparation_store/kernel/codec.rs",
            "pub(crate) fn decode_reference_fields(",
        ),
        (
            "src/preparation_store/kernel/paths.rs",
            "pub(crate) fn directories(",
        ),
        (
            "src/preparation_store/kernel/persist.rs",
            "pub(crate) fn persist_canonical(",
        ),
        (
            "src/preparation_store/kernel/load.rs",
            "pub(crate) fn load(",
        ),
        (
            "src/preparation_store/kernel/inspect.rs",
            "pub(crate) fn inspect(",
        ),
        (
            "src/preparation_store/kernel/batch.rs",
            "pub struct PreparationCursor",
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
        for forbidden in ["qdrant_client", "search_qdrant", "reqwest::", "tokio::"] {
            assert!(
                !source.contains(forbidden),
                "{relative} acquired forbidden vendor token {forbidden}"
            );
        }
    }

    let spec = read(&root, "src/preparation_store/kernel/spec.rs");
    for exact in [
        "b\"ELSPRP02\"",
        "b\"ELSPRF01\"",
        "BINDING_BYTES: usize = 208",
        "OLD_BINDING_BYTES: usize = 176",
        "REF_BYTES: usize = 81",
        "MAX_BATCH_REVISIONS: usize = 64",
    ] {
        assert!(spec.contains(exact), "spec lost {exact}");
    }

    let codec = read(&root, "src/preparation_store/kernel/codec.rs");
    for exact in [
        "eliot-search/direct-preparation-ref/v2",
        "eliot-search/direct-preparation-object/v2",
        "DIRECT_PREPARATION_REFERENCE_INVALID",
        "DIRECT_PREPARATION_DIGEST_ALGORITHM_MISMATCH",
        "DIRECT_PREPARATION_PROFILE_MISMATCH",
    ] {
        assert!(codec.contains(exact), "codec lost {exact}");
    }
    assert!(!codec.contains("persist_immutable_object"));

    let persist = read(&root, "src/preparation_store/kernel/persist.rs");
    assert!(persist.contains("persist_immutable_object(&object_path"));
    assert!(persist.contains("persist_immutable_object(&reference_path"));
    assert!(persist.contains("DIRECT_PREPARATION_OBJECT_CONFLICT"));
    assert!(!persist.contains("PreparationCursor"));

    let load = read(&root, "src/preparation_store/kernel/load.rs");
    assert!(!load.contains("persist_immutable_object"));
    assert!(!load.contains("create_dir_all"));

    let batch = read(&root, "src/preparation_store/kernel/batch.rs");
    assert!(batch.contains("eliot-search/direct-preparation-cursor/v2"));
    assert!(batch.contains("self.inner.verify_control()?;"));
    assert!(!batch.contains("std::fs"));
}

#[test]
fn migration_inventory_remains_read_only_and_separate() {
    let root = crate_root();
    let migration = read(&root, "src/control_migration_preparation.rs");
    assert!(migration.contains("deletion_authorized\\\":false"));
    assert!(migration.contains("read_only\\\":true"));
    assert!(migration.contains("cutover_revalidation_required\\\":true"));
    assert!(!migration.contains("persist_immutable_object"));
    assert!(!migration.contains("remove_file"));
}

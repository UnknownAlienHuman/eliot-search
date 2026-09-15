use std::path::{Path, PathBuf};

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

#[test]
fn directory_manifest_entry_and_kernel_are_thin() {
    let root = crate_root();
    let entry = read(&root, "src/directory_manifest.rs");
    assert!(entry.contains("#[path = \"directory_manifest/kernel.rs\"]"));
    assert!(entry.contains("pub use kernel::*;"));
    assert!(entry.len() < 1_500, "entry grew to {} bytes", entry.len());
    for forbidden in [
        "pub struct DirectoryManifest",
        "pub fn sync_directory(",
        "OpenOptions",
        "read_to_string",
        "std::fs",
    ] {
        assert!(
            !entry.contains(forbidden),
            "manifest implementation returned to facade: {forbidden}"
        );
    }

    let kernel = read(&root, "src/directory_manifest/kernel.rs");
    for module in [
        "codec", "load", "migration", "model", "paths", "persist",
        "spec", "sync",
    ] {
        assert!(kernel.contains(&format!("mod {module};")));
    }
    assert!(kernel.contains("pub use sync::sync_directory;"));
    assert!(kernel.contains("pub use migration::{"));
    assert!(kernel.len() < 3_000, "kernel grew to {} bytes", kernel.len());
}

#[test]
fn directory_manifest_responsibilities_remain_bounded() {
    let root = crate_root();
    let owners = [
        (
            "src/directory_manifest/kernel/model.rs",
            "pub struct DirectoryManifest",
        ),
        (
            "src/directory_manifest/kernel/codec.rs",
            "pub(super) fn build_manifest(",
        ),
        (
            "src/directory_manifest/kernel/paths.rs",
            "pub(super) fn manifest_root(",
        ),
        (
            "src/directory_manifest/kernel/persist.rs",
            "pub(super) fn persist_manifest(",
        ),
        (
            "src/directory_manifest/kernel/load.rs",
            "pub fn verify_directory_manifests(",
        ),
        (
            "src/directory_manifest/kernel/migration.rs",
            "pub fn migration_manifest(",
        ),
        (
            "src/directory_manifest/kernel/sync.rs",
            "pub fn sync_directory(",
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
            "search_qdrant",
            "reqwest::",
            "tokio::",
            "std::process::Command",
        ] {
            assert!(
                !source.contains(forbidden),
                "{relative} acquired forbidden token {forbidden}"
            );
        }
    }

    let codec = read(&root, "src/directory_manifest/kernel/codec.rs");
    assert!(codec.contains("eliot-search/direct-directory-manifest/v1"));
    assert!(codec.contains("V1\\t{}\\t{}\\t{}\\n"));
    assert!(!codec.contains("OpenOptions"));
    assert!(!codec.contains("File::open"));

    let persist = read(&root, "src/directory_manifest/kernel/persist.rs");
    assert!(persist.contains("OpenOptions::new()"));
    assert!(persist.contains("create_new(true)"));
    assert!(persist.contains("file.sync_all()"));
    assert!(persist.contains("DIRECT_MANIFEST_IMMUTABLE_CONFLICT"));
    assert!(!persist.contains("retire_source"));

    let sync = read(&root, "src/directory_manifest/kernel/sync.rs");
    assert!(sync.contains("store.index_directory"));
    assert!(sync.contains("store.retire_source"));
    assert!(sync.contains("current.path_digest != old_entry.path_digest"));
    assert!(sync.contains("persist_manifest"));
    assert!(!sync.contains("OpenOptions"));
    assert!(!sync.contains("read_to_string"));

    let migration = read(&root, "src/directory_manifest/kernel/migration.rs");
    assert!(migration.contains("encode_manifest(&manifest)? != text"));
    assert!(!migration.contains("create_dir"));
    assert!(!migration.contains("OpenOptions"));
}

#[test]
fn directory_manifest_format_and_limits_stay_closed() {
    let root = crate_root();
    let spec = read(&root, "src/directory_manifest/kernel/spec.rs");
    for token in [
        "ELIOT_SEARCH_DIRECTORY_MANIFEST_V1",
        "128 * 1024 * 1024",
        "100_000",
        "1_000_000",
        "1_024",
    ] {
        assert!(spec.contains(token), "lost manifest token {token}");
    }

    let load = read(&root, "src/directory_manifest/kernel/load.rs");
    for reason in [
        "DIRECT_MANIFEST_CHANGED_DURING_READ",
        "DIRECT_MANIFEST_GENERATION_AMBIGUOUS",
        "DIRECT_MANIFEST_DIGEST_MISMATCH",
        "DIRECT_MANIFEST_SOURCE_DUPLICATE",
        "DIRECT_MANIFEST_FILENAME_MISMATCH",
    ] {
        assert!(load.contains(reason), "lost manifest reason {reason}");
    }

    let paths = read(&root, "src/directory_manifest/kernel/paths.rs");
    assert!(paths.contains("DIRECT_MIGRATION_MANIFEST_PENDING"));
    assert!(paths.contains("DIRECT_MANIFEST_UNEXPECTED_OBJECT"));
    assert!(paths.contains("is_reparse"));
    assert!(paths.contains("MAX_MANIFEST_FILES"));
}

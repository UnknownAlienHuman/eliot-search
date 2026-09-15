use std::path::{Path, PathBuf};

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

#[test]
fn legacy_snapshot_target_is_harness_only() {
    let root = crate_root();
    let manifest = read(&root, "Cargo.toml");
    let target = concat!(
        "[[test]]\n",
        "name = \"eliot-search-snapshotd\"\n",
        "path = \"src/main.rs\"\n",
        "harness = true\n",
        "test = true",
    );
    assert!(manifest.contains(target));
    assert!(!manifest.contains("[[bin]]\nname = \"eliot-search-snapshotd\""));
    assert!(!manifest.contains("[[example]]\nname = \"eliot-search-snapshotd\""));

    let production = read(&root, "src/entry.rs");
    assert!(!production.contains("mod snapshot;"));
    assert!(!production.contains("mod snapshot_admin;"));

    let harness = read(&root, "src/main.rs");
    assert!(harness.contains("mod snapshot;"));
    assert!(!harness.contains("mod snapshot_admin;"));
}

#[test]
fn legacy_snapshot_entry_is_a_thin_isolation_facade() {
    let root = crate_root();
    let entry = read(&root, "src/snapshot.rs");
    assert!(entry.contains("#[path = \"snapshot/kernel.rs\"]"));
    assert!(entry.contains("pub(crate) use kernel::*;"));
    assert!(entry.len() < 1_500, "snapshot facade grew to {} bytes", entry.len());
    for forbidden in [
        "pub struct SnapshotIndex",
        "pub fn fingerprint(",
        "fn stable_read(",
        "OpenOptions",
        "read_dir",
    ] {
        assert!(
            !entry.contains(forbidden),
            "snapshot implementation returned to facade: {forbidden}"
        );
    }

    let kernel = read(&root, "src/snapshot/kernel.rs");
    assert!(kernel.len() < 35_000, "legacy kernel grew to {} bytes", kernel.len());
    for marker in [
        "pub struct SnapshotLimits",
        "pub struct SnapshotIndex",
        "pub struct SnapshotSearchResult",
        "pub fn fingerprint(",
        "pub fn hex32(",
        "ELIOT_SEARCH_SNAPSHOT_V1",
        "eliot-fnv4-v1",
    ] {
        assert!(kernel.contains(marker), "legacy snapshot lost {marker}");
    }
    for forbidden in [
        "qdrant_client",
        "search_qdrant",
        "reqwest::",
        "tokio::",
        "std::process::Command",
    ] {
        assert!(
            !kernel.contains(forbidden),
            "legacy snapshot acquired production/vendor dependency {forbidden}"
        );
    }
}

#[test]
fn orphan_snapshot_administration_source_is_absent() {
    let root = crate_root();
    assert!(
        !root.join("src/snapshot_admin.rs").exists(),
        "uncompiled snapshot administration monolith returned"
    );
    let lexical_manifest = read(&root, "src/lexical/manifest.rs");
    let lexical_build = read(&root, "src/lexical/build.rs");
    assert!(lexical_manifest.contains("crate::snapshot::{fingerprint, hex32}"));
    assert!(lexical_build.contains("crate::snapshot::fingerprint"));
}

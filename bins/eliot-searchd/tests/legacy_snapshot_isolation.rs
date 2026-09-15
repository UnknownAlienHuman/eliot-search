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
fn legacy_snapshot_entry_and_kernel_are_thin() {
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
    for module in [
        "capture",
        "fingerprint",
        "manifest",
        "model",
        "policy",
        "search",
        "spec",
        "storage",
    ] {
        assert!(kernel.contains(&format!("mod {module};")));
    }
    assert!(kernel.contains("pub use fingerprint::{fingerprint, hex32};"));
    assert!(kernel.contains("SnapshotIndex, SnapshotLimits"));
    assert!(kernel.len() < 1_500, "snapshot kernel grew to {} bytes", kernel.len());
}

#[test]
fn legacy_snapshot_responsibilities_remain_bounded() {
    let root = crate_root();
    let owners = [
        ("src/snapshot/kernel/spec.rs", "FINGERPRINT_ALGORITHM"),
        ("src/snapshot/kernel/model.rs", "pub struct SnapshotIndex"),
        ("src/snapshot/kernel/capture.rs", "pub(crate) fn capture("),
        (
            "src/snapshot/kernel/manifest.rs",
            "pub(super) fn publish_manifest(",
        ),
        (
            "src/snapshot/kernel/storage.rs",
            "pub(super) fn store_revision(",
        ),
        ("src/snapshot/kernel/search.rs", "pub(crate) fn search("),
        ("src/snapshot/kernel/fingerprint.rs", "pub fn fingerprint("),
        (
            "src/snapshot/kernel/policy.rs",
            "pub(super) fn policy_denies_file(",
        ),
    ];
    for (relative, marker) in owners {
        let source = read(&root, relative);
        assert!(source.contains(marker), "{relative} lost owner {marker}");
        assert!(
            source.len() < 14_000,
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
                "{relative} acquired production/vendor dependency {forbidden}"
            );
        }
    }

    let spec = read(&root, "src/snapshot/kernel/spec.rs");
    assert!(spec.contains("eliot-fnv4-v1"));
    assert!(spec.contains("MAX_MANIFEST_BYTES: usize = 64 * 1024 * 1024"));

    let manifest = read(&root, "src/snapshot/kernel/manifest.rs");
    assert!(manifest.contains("ELIOT_SEARCH_SNAPSHOT_V1"));
    assert!(manifest.contains("write_unique_verified"));

    let search = read(&root, "src/snapshot/kernel/search.rs");
    assert!(search.contains("read_verified_revision"));
    assert!(!search.contains("std::fs"));
    assert!(!search.contains("OpenOptions"));

    let storage = read(&root, "src/snapshot/kernel/storage.rs");
    assert!(storage.contains("create_new(true)"));
    assert!(storage.contains("revision fingerprint collision or durable readback mismatch"));

    let capture = read(&root, "src/snapshot/kernel/capture.rs");
    assert!(capture.contains("symlink_metadata"));
    assert!(capture.contains("policy_denies_file"));
    assert!(capture.contains("publish_manifest"));
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

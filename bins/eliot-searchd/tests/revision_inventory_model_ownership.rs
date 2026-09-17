use std::path::{Path, PathBuf};

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn repository_root() -> PathBuf {
    crate_root()
        .parent()
        .and_then(Path::parent)
        .expect("daemon package is nested under bins")
        .to_owned()
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

#[test]
fn revision_orphan_command_composes_package_model_without_duplicate_schema() {
    let root = repository_root();
    let daemon = read(
        &root,
        "bins/eliot-searchd/src/control_migration_orphans.rs",
    );
    assert!(daemon.contains("struct DirectRevisionInventoryDigest"));
    assert!(daemon.contains("impl LegacyRevisionInventoryDigest"));
    assert!(daemon.contains("sha256::digest_parts"));
    assert!(daemon.contains("LegacyRevisionObjectEvidence::new"));
    assert!(daemon.contains("fingerprint(&root, entry, deadline)"));
    for forbidden in [
        "rows.push(format!",
        "next_cursor =",
        "page_digest =",
        "inventory.digest",
        "entry.kind.tag()",
        "json_string(",
    ] {
        assert!(
            !daemon.contains(forbidden),
            "daemon restored report/model logic {forbidden}"
        );
    }
}

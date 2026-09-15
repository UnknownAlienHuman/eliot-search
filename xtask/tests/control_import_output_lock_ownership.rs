//! T02 ownership regression for source-import output serialization.

use std::fs;
use std::path::Path;

#[test]
fn source_import_output_lifecycle_has_one_package_owner() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask must be a workspace member");
    let daemon_path =
        root.join("bins/eliot-searchd/src/control_migration_redb.rs");
    let lock_path =
        root.join("crates/search-control-redb/src/migration/output_lock.rs");
    let artifact_facade_path =
        root.join("crates/search-control-redb/src/migration/output_artifact.rs");
    let artifact_owner_path = root.join(
        "crates/search-control-redb/src/migration/output_artifact/lifecycle.rs",
    );
    let module_path =
        root.join("crates/search-control-redb/src/migration/mod.rs");

    let daemon = read(&daemon_path);
    let lock_owner = read(&lock_path);
    let artifact_facade = read(&artifact_facade_path);
    let artifact_owner = read(&artifact_owner_path);
    let module = read(&module_path);

    assert!(
        lock_owner.contains("pub struct SourceImportOutputLock"),
        "search-control-redb must own the output lock state machine"
    );
    assert!(
        artifact_owner.contains("pub struct SourceImportOutputArtifact"),
        "search-control-redb must own pending/final artifact lifecycle"
    );
    for token in [
        "open_or_create_pending",
        "publish_pending",
        "cleanup_verified_alias",
        "fs::hard_link",
        "fs::remove_file",
    ] {
        assert!(
            artifact_owner.contains(token),
            "package artifact owner is missing {token}"
        );
    }
    assert!(
        artifact_facade.contains("pub use lifecycle::SourceImportOutputArtifact")
            && module.contains("pub use output_artifact::{")
            && module.contains("pub use output_lock::{"),
        "search-control-redb must expose both owned output boundaries"
    );
    assert!(
        daemon.contains(
            "SourceImportOutputArtifact<DaemonImportOutputPlatform>"
        ),
        "daemon must compose the package artifact owner"
    );
    assert!(
        daemon.contains(
            "impl SourceImportOutputArtifactPlatform for DaemonImportOutputPlatform"
        ),
        "daemon must retain only the native identity observation"
    );
    for forbidden in [
        "struct ImportOutputGuard",
        "TryLockError",
        ".try_lock()",
        "OpenOptions",
        "fs::hard_link",
        "fs::remove_file",
        "fn open_existing",
        "fn cleanup_published_alias",
    ] {
        assert!(
            !daemon.contains(forbidden),
            "daemon restored package-owned output behavior: {forbidden}"
        );
    }
}

fn read(path: &Path) -> String {
    fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

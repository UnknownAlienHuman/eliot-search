//! T02 ownership regression for source-import output serialization.

use std::fs;
use std::path::Path;

#[test]
fn source_import_output_lock_has_one_package_owner() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask must be a workspace member");
    let daemon_path = root.join("bins/eliot-searchd/src/control_migration_redb.rs");
    let owner_path = root.join("crates/search-control-redb/src/migration/output_lock.rs");
    let module_path = root.join("crates/search-control-redb/src/migration/mod.rs");

    let daemon = fs::read_to_string(&daemon_path)
        .unwrap_or_else(|error| panic!("{}: {error}", daemon_path.display()));
    let owner = fs::read_to_string(&owner_path)
        .unwrap_or_else(|error| panic!("{}: {error}", owner_path.display()));
    let module = fs::read_to_string(&module_path)
        .unwrap_or_else(|error| panic!("{}: {error}", module_path.display()));

    assert!(
        owner.contains("pub struct SourceImportOutputLock"),
        "search-control-redb must own the output lock state machine"
    );
    assert!(
        module.contains("pub use output_lock::{"),
        "search-control-redb must expose its owned output-lock boundary"
    );
    assert!(
        daemon.contains("SourceImportOutputLock<DaemonImportOutputPlatform>"),
        "daemon must compose the package owner through the platform adapter"
    );
    assert!(
        daemon.contains("impl SourceImportOutputLockPlatform for DaemonImportOutputPlatform"),
        "daemon must retain only native platform observations"
    );
    assert!(
        !daemon.contains("struct ImportOutputGuard")
            && !daemon.contains("TryLockError")
            && !daemon.contains(".try_lock()"),
        "daemon restored a second output-lock owner"
    );
}

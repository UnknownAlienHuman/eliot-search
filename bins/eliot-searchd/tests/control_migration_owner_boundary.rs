use std::path::{Path, PathBuf};

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("daemon package is nested under bins/")
        .to_owned()
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

#[test]
fn cutover_marker_semantics_belong_to_control_owner() {
    let root = repository_root();
    let owner = read(
        &root,
        "crates/search-control-redb/src/migration/cutover.rs",
    );
    assert!(owner.contains("pub struct ControlCutoverMarker"));
    assert!(owner.contains("ELIOT-SEARCH-CONTROL-CUTOVER-V1"));
    assert!(owner.contains("classify_control_cutover_replay"));
    assert!(!owner.contains("std::fs"));
    assert!(!owner.contains("catalog_quarantine"));
    assert!(!owner.contains("impl DirectStore"));

    let facade = read(
        &root,
        "bins/eliot-searchd/src/control_migration_cutover.rs",
    );
    assert!(facade.len() < 4_096, "cutover facade grew to {} bytes", facade.len());
    assert!(!facade.contains("struct CutoverMarker"));
    assert!(!facade.contains("ELIOT-SEARCH-CONTROL-CUTOVER-V1"));

    let marker_io = read(
        &root,
        "bins/eliot-searchd/src/control_migration_cutover/marker_io.rs",
    );
    assert!(marker_io.contains("search_control_redb::migration"));
    assert!(marker_io.contains("OpenOptions"));
    assert!(!marker_io.contains("impl DirectStore"));

    let operation = read(
        &root,
        "bins/eliot-searchd/src/control_migration_cutover/operation.rs",
    );
    assert!(operation.contains("impl DirectStore"));
    assert!(operation.contains("ControlCutoverMarker as CutoverMarker"));
    assert!(!operation.contains("OpenOptions"));
    assert!(!operation.contains("MARKER_MAGIC"));

    let status = read(
        &root,
        "bins/eliot-searchd/src/control_migration_cutover/status.rs",
    );
    assert!(status.contains("read_only"));
    assert!(!status.contains("OpenOptions"));
    assert!(!status.contains("impl DirectStore"));
}

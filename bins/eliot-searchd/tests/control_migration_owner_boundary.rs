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
    assert!(
        facade.len() < 4_096,
        "cutover facade grew to {} bytes",
        facade.len()
    );
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

#[test]
fn content_manifest_schema_belongs_to_control_owner() {
    let root = repository_root();
    let owner = read(
        &root,
        "crates/search-control-redb/src/migration/content.rs",
    );
    for required in [
        "pub struct SourceContentManifestEncoder",
        "pub struct SourceContentManifestHeader",
        "pub struct SourceContentObjectReadback",
        "pub struct SourceContentManifestSummary",
        "source_content_profile_digest",
        "source_content_header",
        "source_content_readback",
        "source_content_end",
    ] {
        assert!(
            owner.contains(required),
            "control owner is missing content-manifest boundary {required}"
        );
    }
    assert!(!owner.contains("RevisionProtector"));
    assert!(!owner.contains("read_import_revision"));
    assert!(!owner.contains("blake3::Hasher"));

    let adapter = read(
        &root,
        "bins/eliot-searchd/src/control_migration_content.rs",
    );
    assert!(adapter.contains("SourceContentManifestEncoder"));
    assert!(adapter.contains("SourceImportRecordArtifact"));
    assert!(adapter.contains("RevisionProtector"));
    assert!(adapter.contains("read_import_revision"));
    assert!(adapter.contains("blake3::Hasher"));
    for forbidden in [
        "source_content_header",
        "source_content_readback",
        "source_content_end",
        "content_digest_algorithm",
        "const PROFILE",
    ] {
        assert!(
            !adapter.contains(forbidden),
            "daemon restored package-owned manifest schema: {forbidden}"
        );
    }
}

#[test]
fn source_import_record_chain_and_artifact_lifecycle_have_one_owner() {
    let root = repository_root();
    let chain_owner = read(
        &root,
        "crates/search-control-redb/src/migration/record_chain.rs",
    );
    for required in [
        "pub struct SourceImportRecordChain",
        "MAX_SOURCE_IMPORT_RECORD_BYTES",
        "MAX_SOURCE_IMPORT_ROW_BYTES",
        "eliot-search/source-map-chain/v1",
        "eliot-search/source-map-row/v1",
        "eliot-search/source-map-end/v1",
    ] {
        assert!(
            chain_owner.contains(required),
            "control owner is missing record-chain boundary {required}"
        );
    }

    let artifact_owner = read(
        &root,
        "crates/search-control-redb/src/migration/record_artifact.rs",
    );
    for required in [
        "pub struct SourceImportRecordArtifact",
        "pub struct SourceImportRecordReadback",
        "inspect_source_import_record_artifact",
        "fs::hard_link",
        "fs::remove_file",
        "read_until",
    ] {
        assert!(
            artifact_owner.contains(required),
            "control owner is missing immutable artifact behavior {required}"
        );
    }

    let plan = read(
        &root,
        "bins/eliot-searchd/src/control_migration_plan.rs",
    );
    let content = read(
        &root,
        "bins/eliot-searchd/src/control_migration_content.rs",
    );
    assert!(plan.contains("SourceImportRecordArtifact"));
    assert!(content.contains("SourceImportRecordArtifact"));
    for daemon in [&plan, &content] {
        for forbidden in [
            "struct PlanDigest",
            "struct StagingFile",
            "SourceImportRecordChain::new",
            "fs::hard_link",
            "fs::remove_file",
            "OpenOptions",
            "BufReader",
            "BufWriter",
            "read_until",
            "eliot-search/source-map-chain/v1",
            "eliot-search/source-map-row/v1",
            "eliot-search/source-map-end/v1",
        ] {
            assert!(
                !daemon.contains(forbidden),
                "daemon restored package-owned record-artifact logic: {forbidden}"
            );
        }
    }
}

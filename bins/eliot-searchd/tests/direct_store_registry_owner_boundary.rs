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
fn legacy_direct_journal_semantics_belong_to_source_registry() {
    let root = repository_root();
    let owner = read(
        &root,
        "crates/search-source/search-source-registry/src/source/legacy_direct.rs",
    );
    assert!(owner.contains("pub struct LegacyDirectSourceRecord"));
    assert!(owner.contains("pub struct LegacyDirectRegistryState"));
    assert!(owner.contains("pub fn plan_append"));
    assert!(owner.contains("pub fn parse_record"));
    assert!(owner.contains("pub fn validate_record"));
    assert!(owner.contains("ELIOT_SEARCH_SOURCE_EVENTS_V1"));
    assert!(!owner.contains("std::fs"));
    assert!(!owner.contains("OpenOptions"));
    assert!(!owner.contains("DirectStore"));

    let facade = read(&root, "bins/eliot-searchd/src/direct_store.rs");
    assert!(facade.len() < 4_096, "DIRECT facade grew to {} bytes", facade.len());
    assert!(facade.contains("search-source-registry"));
    assert!(!facade.contains("enum SourceState"));
    assert!(!facade.contains("struct SourceRecord"));
    assert!(!facade.contains("ELIOT_SEARCH_SOURCE_EVENTS_V1"));

    let model = read(&root, "bins/eliot-searchd/src/direct_store/model.rs");
    assert!(model.contains("LegacyDirectRegistryState as RegistryState"));
    assert!(model.contains("LegacyDirectSourceRecord as SourceRecord"));
    assert!(model.contains("impl LegacyDirectDigest for DirectDigest"));
    assert!(!model.contains("enum SourceState"));
    assert!(!model.contains("struct SourceRecord"));

    let store = read(&root, "bins/eliot-searchd/src/direct_store/store.rs");
    assert!(store.contains("plan_append::<DirectDigest>"));
    assert!(store.contains("OpenOptions"));
    assert!(!store.contains("canonical_without_digest"));
    assert!(!store.contains("ELIOT_SEARCH_SOURCE_EVENTS_V1"));

    let catalog = read(&root, "bins/eliot-searchd/src/direct_store_catalog.rs");
    assert!(catalog.contains("parse_record::<DirectDigest>"));
    assert!(catalog.contains("validate_record::<DirectDigest>"));
    assert!(catalog.contains("commit_record(record)"));
    assert!(!catalog.contains("fn parse_event_record"));
    assert!(!catalog.contains("DIRECT_CONTROL_LOG_SOURCE_COLLISION"));
}

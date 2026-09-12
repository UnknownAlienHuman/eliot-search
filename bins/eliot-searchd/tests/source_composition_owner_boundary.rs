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
fn live_direct_composition_enters_canonical_admission_and_identity_owners() {
    let root = repository_root();
    let entry = read(&root, "bins/eliot-searchd/src/entry.rs");
    assert!(entry.contains("#[path = \"direct_store/composition.rs\"]\nmod source_composition;"));
    assert!(entry.contains("#[path = \"source_composition.rs\"]\nmod git_source_composition;"));
    assert_eq!(entry.matches("mod source_composition;").count(), 1);

    let facade = read(
        &root,
        "bins/eliot-searchd/src/direct_store/composition.rs",
    );
    assert!(facade.contains("mod admission;"));
    assert!(facade.contains("mod classifier;"));
    assert!(facade.contains("mod identity;"));
    assert!(facade.contains("mod registry;"));

    let admission = read(
        &root,
        "bins/eliot-searchd/src/direct_store/composition/admission.rs",
    );
    assert!(admission.contains("use search_source_admission as kernel;"));
    assert!(admission.contains("kernel::validate_observation"));
    assert!(admission.contains("kernel::evaluate("));
    assert!(admission.contains("kernel::issue_receipt"));
    assert!(admission.contains("kernel::verify_receipt"));
    assert!(!admission.contains("enum AdmissionOutcome"));
    assert!(!admission.contains("struct AdmissionReceipt"));

    let identity = read(
        &root,
        "bins/eliot-searchd/src/direct_store/composition/identity.rs",
    );
    assert!(identity.contains("resolve_legacy_digest_identity"));
    assert!(identity.contains("derive_legacy_digest_source_id"));
    assert!(identity.contains("derive_legacy_digest_revision_id"));
    assert!(!identity.contains("eliot-search/direct-source-id/v1"));
    assert!(!identity.contains("eliot-search/direct-revision-id/v1"));
    assert!(!identity.contains("enum IdentityResolution"));

    let ingest = read(&root, "bins/eliot-searchd/src/direct_store_ingest.rs");
    assert!(ingest.contains("use crate::source_composition as canonical;"));

    let manifest = read(&root, "bins/eliot-searchd/Cargo.toml");
    assert!(manifest.contains("search-source-admission.workspace = true"));
    assert!(manifest.contains("search-source-identity.workspace = true"));
    assert!(!manifest.contains("search-source-admission = { workspace = true, optional = true }"));
    assert!(!manifest.contains("search-source-identity = { workspace = true, optional = true }"));
    assert!(!manifest.contains("dep:search-source-admission"));
    assert!(!manifest.contains("dep:search-source-identity"));
}

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
fn git_composition_reuses_canonical_source_owners() {
    let root = repository_root();
    let git = read(&root, "bins/eliot-searchd/src/source_composition.rs");
    assert!(git.len() < 12_000, "Git adapter grew to {} bytes", git.len());
    assert!(git.contains("direct_store/composition.rs"));
    assert!(git.contains("derive_git_stable_digest"));
    assert!(git.contains("plan_snapshot("));
    assert!(git.contains("search_safe_reader::git"));
    assert!(!git.contains("enum AdmissionOutcome"));
    assert!(!git.contains("struct AdmissionPolicy"));
    assert!(!git.contains("struct AdmissionReceipt"));
    assert!(!git.contains("enum IdentityResolution"));
    assert!(!git.contains("struct RegistryView"));
    assert!(!git.contains("eliot-search/direct-source-id/v1"));
    assert!(!git.contains("eliot-search/direct-revision-id/v1"));
    assert!(!git.contains("eliot-searchd/source-composition-policy/v1"));
    assert!(!git.contains("eliot-searchd/source-composition-receipt/v1"));

    let identity_owner = read(
        &root,
        "crates/search-source/search-source-identity/src/git_digest.rs",
    );
    assert!(identity_owner.contains("pub trait GitIdentityDigest"));
    assert!(identity_owner.contains("derive_git_stable_identity_digest"));
    assert!(identity_owner.contains("GIT_STABLE_IDENTITY_DOMAIN"));
    assert!(!identity_owner.contains("std::fs"));
    assert!(!identity_owner.contains("std::process"));
    assert!(!identity_owner.contains("RegistryView"));
    assert!(!identity_owner.contains("AdmissionPolicy"));

    let canonical = read(
        &root,
        "bins/eliot-searchd/src/direct_store/composition.rs",
    );
    assert!(canonical.contains("pub(crate) use identity::derive_git_stable_digest;"));

    let adapter = read(
        &root,
        "bins/eliot-searchd/src/direct_store/composition/identity.rs",
    );
    assert!(adapter.contains("derive_git_stable_identity_digest"));
    assert!(adapter.contains("impl GitIdentityDigest for DirectIdentityDigest"));
}

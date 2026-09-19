use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("daemon is a workspace member")
        .to_owned()
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

#[test]
fn legacy_revision_key_derivation_has_one_pure_owner() {
    let root = workspace_root();
    let package = read(
        &root,
        "crates/search-runtime/search-os-secrets/src/legacy_revision_protection/model.rs",
    );
    let daemon = [
        read(
            &root,
            "bins/eliot-searchd/src/revision_protection/envelope.rs",
        ),
        read(
            &root,
            "bins/eliot-searchd/src/revision_protection/protector/model.rs",
        ),
        read(
            &root,
            "bins/eliot-searchd/src/revision_protection_windows/existing.rs",
        ),
    ]
    .join("\n");

    for marker in [
        "pub trait LegacyRevisionKeyDigest",
        "pub fn derive_legacy_revision_key_binding",
        "pub fn derive_legacy_revision_dpapi_entropy",
        "eliot-search/revision-key-binding/v1",
        "eliot-search/revision-dpapi-entropy/v1",
    ] {
        assert!(package.contains(marker), "package lost owner {marker}");
    }
    for forbidden in [
        "CredReadW",
        "CredWriteW",
        "CryptProtectData",
        "CryptUnprotectData",
        "BCryptGenRandom",
        "std::fs",
        "std::path::Path",
    ] {
        assert!(
            !package.contains(forbidden),
            "pure key derivation acquired forbidden token {forbidden}"
        );
    }

    assert!(daemon.contains("impl LegacyRevisionKeyDigest for DirectRevisionDigest"));
    assert!(daemon.contains(
        "derive_legacy_revision_key_binding::<DirectRevisionDigest>"
    ));
    assert!(daemon.contains(
        "derive_legacy_revision_dpapi_entropy::<DirectRevisionDigest>"
    ));
    for removed_domain in [
        "eliot-search/revision-key-binding/v1",
        "eliot-search/revision-dpapi-entropy/v1",
    ] {
        assert!(
            !daemon.contains(removed_domain),
            "daemon restored key-derivation owner {removed_domain}"
        );
    }
}

#[test]
fn credential_effects_and_digest_effects_have_distinct_owners() {
    let root = workspace_root();
    let credential_package = [
        read(
            &root,
            "crates/search-runtime/search-os-secrets-windows/src/credential.rs",
        ),
        read(
            &root,
            "crates/search-runtime/search-os-secrets-windows/src/credential/windows.rs",
        ),
    ]
    .join("\n");
    let daemon_credential = read(
        &root,
        "bins/eliot-searchd/src/revision_protection_windows/credential.rs",
    );
    let envelope = read(
        &root,
        "bins/eliot-searchd/src/revision_protection/envelope.rs",
    );

    assert!(credential_package.contains("load_or_create_legacy_revision_root_secret"));
    assert!(credential_package.contains(
        "pub const LEGACY_REVISION_ROOT_SECRET_BYTES: usize = 32"
    ));
    assert!(credential_package.contains("BCryptGenRandom"));
    assert!(credential_package.contains("CredReadW"));
    assert!(credential_package.contains("CredWriteW"));
    assert!(daemon_credential.contains("contains_protected_objects"));
    assert!(daemon_credential.contains("load_or_create_legacy_revision_root_secret"));
    assert!(!daemon_credential.contains("BCryptGenRandom"));
    assert!(!daemon_credential.contains("CredReadW"));
    assert!(!daemon_credential.contains("CredWriteW"));
    assert!(envelope.contains("sha256::digest_parts(domain, parts)"));
}

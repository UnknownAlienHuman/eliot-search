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
fn pure_legacy_revision_envelope_has_one_package_owner() {
    let root = workspace_root();
    let package = [
        read(
            &root,
            "crates/search-runtime/search-os-secrets/src/legacy_revision_protection.rs",
        ),
        read(
            &root,
            "crates/search-runtime/search-os-secrets/src/legacy_revision_protection/model.rs",
        ),
        read(
            &root,
            "crates/search-runtime/search-os-secrets/src/legacy_revision_protection/codec.rs",
        ),
    ]
    .join("\n");
    let daemon = read(&root, "bins/eliot-searchd/src/revision_protection.rs");
    let protector = [
        read(
            &root,
            "bins/eliot-searchd/src/revision_protection/protector.rs",
        ),
        read(
            &root,
            "bins/eliot-searchd/src/revision_protection/protector/operations.rs",
        ),
        read(
            &root,
            "bins/eliot-searchd/src/revision_protection/protector/windows.rs",
        ),
    ]
    .join("\n");

    for marker in [
        "pub struct LegacyRevisionExpected",
        "pub struct LegacyRevisionBinding",
        "pub enum LegacyRevisionEnvelopeError",
        "pub fn encode_legacy_revision_inner(",
        "pub fn decode_legacy_revision_inner",
        "pub fn encode_legacy_revision_outer(",
        "pub fn decode_legacy_revision_outer(",
    ] {
        assert!(package.contains(marker), "package lost owner {marker}");
    }
    for forbidden in [
        "std::fs",
        "std::path::Path",
        "CryptProtectData",
        "CryptUnprotectData",
        "CredReadW",
        "CredWriteW",
        "qdrant_client",
        "DirectStore",
    ] {
        assert!(
            !package.contains(forbidden),
            "pure package contract acquired forbidden token {forbidden}"
        );
    }

    assert!(daemon.contains("mod envelope;"));
    assert!(daemon.contains("mod protector;"));
    assert!(protector.contains("decode_legacy_revision_outer"));
    assert!(protector.contains("decode_legacy_revision_inner"));
    assert!(protector.contains("encode_legacy_revision_outer"));
    assert!(protector.contains("encode_legacy_revision_inner"));
    for removed_owner in [
        "const OUTER_MAGIC",
        "const INNER_MAGIC",
        "struct ExpectedRevision",
        "struct Binding",
        "fn decode_outer(",
        "fn decode_inner(",
        "fn decode_binding(",
        "fn encode_binding(",
        "fn take<const N",
    ] {
        assert!(
            !daemon.contains(removed_owner)
                && !protector.contains(removed_owner),
            "daemon restored envelope owner {removed_owner}"
        );
    }
}

#[test]
fn package_root_is_a_real_split_facade() {
    let root = workspace_root();
    let facade = read(&root, "crates/search-runtime/search-os-secrets/src/lib.rs");
    let lifecycle = read(
        &root,
        "crates/search-runtime/search-os-secrets/src/lifecycle.rs",
    );
    assert!(facade.contains("mod lifecycle;"));
    assert!(facade.contains("mod legacy_revision_protection;"));
    assert!(facade.contains("pub use lifecycle::*;"));
    assert!(facade.contains("pub use legacy_revision_protection::*;"));
    assert!(lifecycle.contains("pub struct SecretCatalog"));
    assert!(facade.len() < 2_500, "facade grew to {} bytes", facade.len());
}

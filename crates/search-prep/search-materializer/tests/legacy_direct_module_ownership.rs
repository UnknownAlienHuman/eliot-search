use std::path::{Path, PathBuf};

fn package_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_owned()
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

#[test]
fn legacy_direct_facade_has_distinct_persisted_owners() {
    let root = package_root();
    let facade = read(&root, "src/legacy_direct.rs");
    assert!(
        facade.len() < 2_000,
        "legacy DIRECT facade grew to {} bytes",
        facade.len()
    );
    for module in ["algorithm", "frame", "identity"] {
        assert!(facade.contains(&format!("mod {module};")));
        assert!(facade.contains(&format!("pub use {module}::*;")));
    }
    for forbidden in [
        "pub enum LegacyDirectPreparationGap",
        "pub struct LegacyDirectPreparationBinding",
        "pub fn derive_legacy_direct_representation_id",
        "std::fs",
        "std::process",
    ] {
        assert!(
            !facade.contains(forbidden),
            "legacy DIRECT facade owns implementation token {forbidden}"
        );
    }

    let algorithm = read(&root, "src/legacy_direct/algorithm.rs");
    for tag in [
        "DIGEST_ALGORITHM_BLAKE3_256",
        "DIGEST_ALGORITHM_SHA256",
        "CONTENT_DIGEST_ALGORITHM",
        "REPRESENTATION_DIGEST_ALGORITHM",
        "MANIFEST_DIGEST_ALGORITHM",
    ] {
        assert!(algorithm.contains(tag));
    }

    let frame = read(&root, "src/legacy_direct/frame.rs");
    assert!(frame.contains("pub enum LegacyDirectPreparationGap"));
    assert!(frame.contains("pub const fn decode_legacy_direct_preparation"));
    assert!(!frame.contains("LEGACY_DIRECT_REPRESENTATION_DOMAIN"));

    let identity = read(&root, "src/legacy_direct/identity.rs");
    assert!(identity.contains("LEGACY_DIRECT_REPRESENTATION_DOMAIN"));
    assert!(identity.contains("pub struct LegacyDirectPreparationBinding"));
    assert!(identity.contains("pub trait LegacyDirectRepresentationDigest"));
    assert!(identity.contains("pub fn derive_legacy_direct_representation_id"));
    assert!(!identity.contains("pub enum LegacyDirectPreparationGap"));

    for source in [algorithm, frame, identity] {
        for forbidden in [
            "std::fs",
            "std::process",
            "qdrant_client",
            "search_qdrant",
            "tokio::",
            "reqwest::",
        ] {
            assert!(
                !source.contains(forbidden),
                "legacy DIRECT owner acquired forbidden token {forbidden}"
            );
        }
    }
}

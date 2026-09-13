use std::path::{Path, PathBuf};

fn package_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_owned()
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

#[test]
fn manifest_facade_stays_thin_and_responsibilities_stay_split() {
    let root = package_root();
    let facade = read(&root, "src/manifest.rs");
    assert!(
        facade.len() < 2_000,
        "manifest facade grew to {} bytes",
        facade.len()
    );
    for module in ["build", "codec", "digest", "model", "profile", "spec", "verify"] {
        assert!(facade.contains(&format!("mod {module};")));
    }
    assert!(!facade.contains("mod kernel;"));
    assert!(!root.join("src/manifest/kernel.rs").exists());

    let expected = [
        ("src/manifest/profile.rs", "pub struct UnitizerProfileId"),
        ("src/manifest/model.rs", "pub struct UnitManifest"),
        ("src/manifest/build.rs", "pub fn build_unit_manifest"),
        (
            "src/manifest/codec.rs",
            "pub fn canonicalize_unit_manifest",
        ),
        ("src/manifest/codec.rs", "pub fn decode_unit_manifest"),
        ("src/manifest/verify.rs", "pub fn verify_unit_manifest"),
        ("src/manifest/verify.rs", "pub fn diff_unit_manifests"),
    ];
    for (relative, token) in expected {
        let source = read(&root, relative);
        assert!(
            source.contains(token),
            "{relative} missing responsibility token {token}"
        );
    }

    for relative in [
        "src/manifest/build.rs",
        "src/manifest/codec.rs",
        "src/manifest/digest.rs",
        "src/manifest/model.rs",
        "src/manifest/profile.rs",
        "src/manifest/spec.rs",
        "src/manifest/verify.rs",
    ] {
        let source = read(&root, relative);
        assert!(
            source.len() < 15_000,
            "{relative} grew to {} bytes",
            source.len()
        );
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
                "{relative} acquired forbidden dependency token {forbidden}"
            );
        }
    }

    let profile = read(&root, "src/manifest/profile.rs");
    assert!(profile.contains("PROFILE_DOMAIN"));
    assert!(!profile.contains("MAGIC"));

    let codec = read(&root, "src/manifest/codec.rs");
    assert!(codec.contains("MAGIC"));
    assert!(codec.contains("encode_body"));
    assert!(!codec.contains("classify_unitizer_profile_change"));

    let verify = read(&root, "src/manifest/verify.rs");
    assert!(verify.contains("assemble_manifest"));
    assert!(!verify.contains("fn encode_body"));
}

#[test]
fn crate_root_keeps_the_existing_manifest_surface() {
    let root = package_root();
    let lib = read(&root, "src/lib.rs");
    for public in [
        "CanonicalUnitManifestBytes",
        "MaterializerProvenance",
        "UnitDescriptor",
        "UnitManifest",
        "UnitManifestDiff",
        "UnitManifestVerificationReceipt",
        "UnitizerProfileChange",
        "UnitizerProfileDescriptor",
        "UnitizerProfileId",
        "ValidatedUnitizerProfile",
        "build_unit_manifest",
        "canonicalize_unit_manifest",
        "classify_unitizer_profile_change",
        "decode_unit_manifest",
        "diff_unit_manifests",
        "manifest_digest",
        "unitizer_profile_digest",
        "validate_unitizer_profile",
        "verify_unit_manifest",
    ] {
        assert!(lib.contains(public), "crate root no longer exports {public}");
    }
}

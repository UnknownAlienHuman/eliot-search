use std::path::{Path, PathBuf};

fn package_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_owned()
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

#[test]
fn manifest_facade_stays_thin_and_kernel_stays_pure() {
    let root = package_root();
    let facade = read(&root, "src/manifest.rs");
    assert!(
        facade.len() < 2_000,
        "manifest facade grew to {} bytes",
        facade.len()
    );
    assert!(facade.contains("mod kernel;"));
    assert!(facade.contains("pub use kernel::*;"));
    assert!(facade.contains("use crate::unitize_text;"));
    for forbidden in [
        "pub struct UnitManifest",
        "fn decode_unit_manifest",
        "fn verify_unit_manifest",
        "fn encode_body",
        "std::fs",
        "std::process",
    ] {
        assert!(
            !facade.contains(forbidden),
            "manifest facade owns implementation token {forbidden}"
        );
    }

    let kernel = read(&root, "src/manifest/kernel.rs");
    for required in [
        "pub struct UnitizerProfileId",
        "pub struct UnitManifest",
        "pub fn build_unit_manifest",
        "pub fn canonicalize_unit_manifest",
        "pub fn decode_unit_manifest",
        "pub fn verify_unit_manifest",
        "pub fn diff_unit_manifests",
        "super::unitize_text",
    ] {
        assert!(
            kernel.contains(required),
            "manifest kernel missing {required}"
        );
    }
    for forbidden in [
        "std::fs",
        "std::process",
        "qdrant_client",
        "search_qdrant",
        "tokio::",
        "reqwest::",
    ] {
        assert!(
            !kernel.contains(forbidden),
            "manifest kernel acquired forbidden dependency token {forbidden}"
        );
    }
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

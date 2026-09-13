use std::path::PathBuf;

fn daemon_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn workspace_root() -> PathBuf {
    daemon_root()
        .parent()
        .and_then(std::path::Path::parent)
        .expect("daemon is under workspace/bins")
        .to_path_buf()
}

#[test]
fn materializer_owns_direct_preparation_frame_and_identity_preimage() {
    let owner = std::fs::read_to_string(
        workspace_root().join("crates/search-prep/search-materializer/src/legacy_direct.rs"),
    )
    .expect("legacy DIRECT materializer owner exists");

    for marker in [
        "pub const fn decode_legacy_direct_preparation",
        "pub fn encode_legacy_direct_layout",
        "pub fn derive_legacy_direct_representation_id",
        "pub trait LegacyDirectRepresentationDigest",
        "LEGACY_DIRECT_REPRESENTATION_DOMAIN",
    ] {
        assert!(owner.contains(marker), "materializer lost owner marker: {marker}");
    }
}

#[test]
fn daemon_is_only_digest_and_pipeline_composition() {
    let binding = std::fs::read_to_string(
        daemon_root().join("src/direct_preparation/binding.rs"),
    )
    .expect("daemon binding adapter exists");
    let layout = std::fs::read_to_string(
        daemon_root().join("src/direct_preparation/layout.rs"),
    )
    .expect("daemon layout adapter exists");

    assert!(binding.contains("impl LegacyDirectRepresentationDigest"));
    assert!(binding.contains("derive_legacy_direct_representation_id"));
    assert!(!binding.contains("eliot-searchd/preparation-representation/v1"));
    assert!(!binding.contains("hasher.update(namespace)"));

    assert!(layout.contains("encode_legacy_direct_layout"));
    assert!(layout.contains("decode_legacy_direct_preparation"));
    assert!(!layout.contains("output.push(0)"));
    assert!(!layout.contains("[1] => Ok(Some"));
}

#[test]
fn digest_algorithm_tags_have_one_source_owner() {
    let profile = std::fs::read_to_string(
        daemon_root().join("src/direct_preparation/profile.rs"),
    )
    .expect("daemon profile adapter exists");
    assert!(profile.contains("pub use search_materializer::api"));
    assert!(!profile.contains("pub const DIGEST_ALGORITHM_BLAKE3_256"));
    assert!(!profile.contains("pub const DIGEST_ALGORITHM_SHA256"));
}

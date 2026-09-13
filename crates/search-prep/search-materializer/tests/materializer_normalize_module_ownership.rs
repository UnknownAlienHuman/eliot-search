use std::path::PathBuf;

use search_materializer::api::{
    CanonicalLine, CanonicalRepresentation, normalize_representation,
};

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn normalize_entry_is_a_thin_stable_facade() {
    let entry = std::fs::read_to_string(crate_root().join("src/normalize.rs"))
        .expect("normalize facade exists");

    for module in ["mod engine;", "mod model;", "mod tests;"] {
        assert!(entry.contains(module), "missing normalize owner: {module}");
    }
    assert!(entry.lines().count() <= 28);

    for implementation_marker in [
        "pub struct CanonicalLine",
        "pub struct CanonicalRepresentation",
        "pub fn normalize_representation",
        "#[test]",
    ] {
        assert!(
            !entry.contains(implementation_marker),
            "implementation returned to normalize facade: {implementation_marker}"
        );
    }
}

#[test]
fn normalize_responsibilities_have_distinct_private_owners() {
    let root = crate_root().join("src/normalize");
    let expectations = [
        ("model.rs", "pub struct CanonicalRepresentation"),
        ("engine.rs", "pub fn normalize_representation"),
        ("tests.rs", "fn preserve_exact_is_identity"),
    ];
    for (path, marker) in expectations {
        let source = std::fs::read_to_string(root.join(path)).expect("owner module exists");
        assert!(source.contains(marker), "{path} lost owner marker: {marker}");
    }
}

#[test]
fn public_normalize_surface_remains_importable() {
    let _ = core::mem::size_of::<CanonicalLine>();
    let _ = core::mem::size_of::<CanonicalRepresentation>();
    let _ = normalize_representation;
}

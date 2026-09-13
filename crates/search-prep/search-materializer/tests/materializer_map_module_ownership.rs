use std::path::PathBuf;

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn map_entry_is_a_thin_owner_facade() {
    let entry = std::fs::read_to_string(crate_root().join("src/maps.rs"))
        .expect("map entry exists");

    for declaration in ["mod build;", "mod model;", "mod validate;"] {
        assert!(entry.contains(declaration), "missing module: {declaration}");
    }
    assert!(entry.lines().count() <= 32);
    for implementation_marker in [
        "pub struct CoordinateMap",
        "pub fn build_coordinate_map",
        "pub fn validate_coordinate_map",
        "#[cfg(test)]\nmod tests {",
    ] {
        assert!(
            !entry.contains(implementation_marker),
            "implementation returned to map facade: {implementation_marker}"
        );
    }
}

#[test]
fn map_responsibilities_have_single_module_owners() {
    let model = std::fs::read_to_string(crate_root().join("src/maps/model.rs"))
        .expect("map model exists");
    let build = std::fs::read_to_string(crate_root().join("src/maps/build.rs"))
        .expect("map builders exist");
    let validate = std::fs::read_to_string(crate_root().join("src/maps/validate.rs"))
        .expect("map validators exist");
    let tests = std::fs::read_to_string(crate_root().join("src/maps/tests.rs"))
        .expect("map tests exist");

    assert!(model.contains("pub struct CoordinateMap"));
    assert!(model.contains("pub struct LossMap"));
    assert!(build.contains("pub fn build_coordinate_map"));
    assert!(build.contains("pub fn build_loss_map"));
    assert!(validate.contains("pub fn validate_coordinate_map"));
    assert!(validate.contains("pub fn validate_map_bundle"));
    assert!(tests.contains("fn gapped_coverage_is_rejected"));
}

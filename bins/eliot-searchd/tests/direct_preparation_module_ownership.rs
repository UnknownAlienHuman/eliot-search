use std::path::PathBuf;

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn direct_preparation_entry_is_a_thin_composition_facade() {
    let entry = std::fs::read_to_string(crate_root().join("src/direct_preparation.rs"))
        .expect("DIRECT preparation facade exists");

    for module in [
        "mod binding;",
        "mod layout;",
        "mod profile;",
        "mod spine;",
        "mod tests;",
    ] {
        assert!(entry.contains(module), "missing DIRECT preparation owner: {module}");
    }
    assert!(entry.lines().count() <= 48);

    for implementation_marker in [
        "pub fn representation_id",
        "pub fn encode_preparation",
        "pub fn validate_source_backed_match",
        "pub fn canonical_materializer_profile",
        "#[test]",
    ] {
        assert!(
            !entry.contains(implementation_marker),
            "implementation returned to facade: {implementation_marker}"
        );
    }
}

#[test]
fn direct_preparation_responsibilities_have_distinct_owners() {
    let root = crate_root().join("src/direct_preparation");
    let expectations = [
        ("binding.rs", "pub fn representation_id"),
        ("layout.rs", "pub fn encode_preparation"),
        ("profile.rs", "pub fn canonical_materializer_profile"),
        ("spine.rs", "pub fn validate_source_backed_match"),
        ("tests.rs", "fn representation_binds_bytes_profiles_and_rejects_tamper"),
    ];
    for (path, marker) in expectations {
        let source = std::fs::read_to_string(root.join(path)).expect("owner module exists");
        assert!(source.contains(marker), "{path} lost owner marker: {marker}");
    }
}

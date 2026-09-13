use std::path::PathBuf;

use search_materializer::api::{
    BomPolicy, CoordinateSpace, DEFAULT_PROFILE_LIMITS, InvalidSequencePolicy, LossBehavior,
    MAX_PROFILE_NAME_BYTES, MaterializationProfileLimits, MaterializerProfileChange,
    MaterializerProfileDescriptor, MaterializerProfileId, NewlinePolicy, SourceEncoding,
    SourceKind, UnicodeNormalization, ValidatedMaterializerProfile,
    baseline_profile_descriptor, classify_profile_change, profile_digest,
    validate_materializer_profile,
};

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn profile_entry_is_a_thin_stable_facade() {
    let entry = std::fs::read_to_string(crate_root().join("src/profile.rs"))
        .expect("profile facade exists");

    for module in [
        "mod change;",
        "mod digest;",
        "mod model;",
        "mod validate;",
        "mod tests;",
    ] {
        assert!(entry.contains(module), "missing profile owner: {module}");
    }
    assert!(entry.lines().count() <= 36);

    for implementation_marker in [
        "pub enum SourceEncoding",
        "pub fn digest32",
        "pub fn validate_materializer_profile",
        "pub enum MaterializerProfileChange",
        "#[test]",
    ] {
        assert!(
            !entry.contains(implementation_marker),
            "implementation returned to profile facade: {implementation_marker}"
        );
    }
}

#[test]
fn profile_responsibilities_have_distinct_private_owners() {
    let root = crate_root().join("src/profile");
    let expectations = [
        ("model.rs", "pub struct ValidatedMaterializerProfile"),
        ("digest.rs", "pub fn profile_digest"),
        ("validate.rs", "pub fn validate_materializer_profile"),
        ("change.rs", "pub fn classify_profile_change"),
        ("tests.rs", "fn change_classification_is_fail_closed"),
    ];
    for (path, marker) in expectations {
        let source = std::fs::read_to_string(root.join(path)).expect("owner module exists");
        assert!(source.contains(marker), "{path} lost owner marker: {marker}");
    }
}

#[test]
fn public_profile_surface_remains_importable_and_operational() {
    let descriptor = baseline_profile_descriptor("ownership-test", 1);
    let profile = validate_materializer_profile(&descriptor).expect("profile remains valid");
    assert_eq!(profile_digest(&profile), profile.id());
    assert_eq!(
        classify_profile_change(&profile, &profile),
        MaterializerProfileChange::Noop
    );

    let _ = core::mem::size_of::<BomPolicy>();
    let _ = core::mem::size_of::<CoordinateSpace>();
    let _ = core::mem::size_of::<InvalidSequencePolicy>();
    let _ = core::mem::size_of::<LossBehavior>();
    let _ = core::mem::size_of::<MaterializationProfileLimits>();
    let _ = core::mem::size_of::<MaterializerProfileDescriptor>();
    let _ = core::mem::size_of::<MaterializerProfileId>();
    let _ = core::mem::size_of::<NewlinePolicy>();
    let _ = core::mem::size_of::<SourceEncoding>();
    let _ = core::mem::size_of::<SourceKind>();
    let _ = core::mem::size_of::<UnicodeNormalization>();
    let _ = core::mem::size_of::<ValidatedMaterializerProfile>();
    assert_eq!(MAX_PROFILE_NAME_BYTES, 128);
    assert!(DEFAULT_PROFILE_LIMITS.max_steps > 0);
}

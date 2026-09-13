use super::*;
use crate::MaterializationError;
use search_contracts::Blake3Digest32;

fn valid_descriptor() -> MaterializerProfileDescriptor {
    baseline_profile_descriptor("baseline", 1)
}

#[test]
fn baseline_descriptor_validates() {
    let profile = validate_materializer_profile(&valid_descriptor()).expect("valid");
    assert_eq!(profile.revision(), 1);
    assert_eq!(profile.name(), "baseline");
    assert_eq!(profile_digest(&profile), profile.id());
}

#[test]
fn profile_names_are_bounded() {
    for name in ["", "has space", "uniçode", "semi;colon"] {
        let mut descriptor = valid_descriptor();
        descriptor.profile_name = name.to_string();
        assert_eq!(
            validate_materializer_profile(&descriptor),
            Err(MaterializationError::ProfileInvalid),
            "name {name:?} must be rejected"
        );
    }
    let mut descriptor = valid_descriptor();
    descriptor.profile_name = "a".repeat(MAX_PROFILE_NAME_BYTES + 1);
    assert_eq!(
        validate_materializer_profile(&descriptor),
        Err(MaterializationError::ProfileInvalid)
    );
}

#[test]
fn zero_revision_is_rejected() {
    let mut descriptor = valid_descriptor();
    descriptor.profile_revision = 0;
    assert_eq!(
        validate_materializer_profile(&descriptor),
        Err(MaterializationError::ProfileInvalid)
    );
}

#[test]
fn kind_and_encoding_sets_must_be_nonempty_and_duplicate_free() {
    let mut descriptor = valid_descriptor();
    descriptor.source_kinds.clear();
    assert_eq!(
        validate_materializer_profile(&descriptor),
        Err(MaterializationError::ProfileInvalid)
    );
    descriptor = valid_descriptor();
    descriptor.source_kinds.push(SourceKind::Text);
    assert_eq!(
        validate_materializer_profile(&descriptor),
        Err(MaterializationError::ProfileInvalid)
    );
    descriptor = valid_descriptor();
    descriptor.encodings.clear();
    assert_eq!(
        validate_materializer_profile(&descriptor),
        Err(MaterializationError::ProfileInvalid)
    );
}

#[test]
fn partial_coordinate_basis_is_rejected() {
    let mut descriptor = valid_descriptor();
    descriptor.coordinate_spaces.pop();
    assert_eq!(
        validate_materializer_profile(&descriptor),
        Err(MaterializationError::ProfileInvalid)
    );
}

#[test]
fn zero_limits_and_zero_golden_are_rejected() {
    let mut descriptor = valid_descriptor();
    descriptor.limits.max_steps = 0;
    assert_eq!(
        validate_materializer_profile(&descriptor),
        Err(MaterializationError::ProfileInvalid)
    );
    descriptor = valid_descriptor();
    descriptor.golden_fixture_digest = Blake3Digest32::from_bytes([0; 32]);
    assert_eq!(
        validate_materializer_profile(&descriptor),
        Err(MaterializationError::ProfileInvalid)
    );
}

#[test]
fn every_load_bearing_field_changes_identity() {
    let base = validate_materializer_profile(&valid_descriptor()).expect("base");
    let base_id = profile_digest(&base);
    let mut variant = valid_descriptor();
    variant.newline_policy = NewlinePolicy::NormalizeToLf;
    variant.golden_fixture_digest = base.golden_fixture_digest();
    variant.profile_revision = 2;
    let changed = validate_materializer_profile(&variant).expect("variant");
    assert_ne!(base_id, profile_digest(&changed));

    variant = valid_descriptor();
    variant.limits.max_lines = 7;
    variant.golden_fixture_digest = base.golden_fixture_digest();
    variant.profile_revision = 2;
    let changed = validate_materializer_profile(&variant).expect("variant");
    assert_ne!(base_id, profile_digest(&changed));
}

#[test]
fn digest_is_deterministic() {
    let first = validate_materializer_profile(&valid_descriptor()).expect("first");
    let second = validate_materializer_profile(&valid_descriptor()).expect("second");
    assert_eq!(profile_digest(&first), profile_digest(&second));
}

#[test]
fn change_classification_is_fail_closed() {
    let old = validate_materializer_profile(&valid_descriptor()).expect("old");
    let same = validate_materializer_profile(&valid_descriptor()).expect("same");
    assert_eq!(
        classify_profile_change(&old, &same),
        MaterializerProfileChange::Noop
    );
    let mut next = valid_descriptor();
    next.profile_revision = 2;
    let next = validate_materializer_profile(&next).expect("next");
    assert_eq!(
        classify_profile_change(&old, &next),
        MaterializerProfileChange::RePreparationAndReprojection
    );
    let mut rollback = valid_descriptor();
    rollback.profile_name = "rollback".to_string();
    rollback.profile_revision = 1;
    let rollback = validate_materializer_profile(&rollback).expect("rollback");
    assert_eq!(
        classify_profile_change(&next, &rollback),
        MaterializerProfileChange::Reject
    );
}

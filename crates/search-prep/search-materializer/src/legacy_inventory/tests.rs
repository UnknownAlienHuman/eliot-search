use super::*;

fn hex(byte: &str) -> String {
    byte.repeat(32)
}

#[test]
fn tree_names_are_closed() {
    assert_eq!(
        LegacyPreparationInventoryTree::from_directory_name("refs"),
        Some(LegacyPreparationInventoryTree::References)
    );
    assert_eq!(
        LegacyPreparationInventoryTree::from_directory_name("objects"),
        Some(LegacyPreparationInventoryTree::Objects)
    );
    assert_eq!(
        LegacyPreparationInventoryTree::References.directory_name(),
        "refs"
    );
    assert_eq!(
        LegacyPreparationInventoryTree::Objects.directory_name(),
        "objects"
    );
    assert_eq!(
        LegacyPreparationInventoryTree::from_directory_name("other"),
        None
    );
}

#[test]
fn final_reference_and_object_names_are_classified_exactly() {
    let reference = format!("{}.ref", hex("ab"));
    let observed = classify_legacy_preparation_inventory_name(
        LegacyPreparationInventoryTree::References,
        &reference,
    )
    .expect("reference");
    assert_eq!(observed.id(), hex("ab"));
    assert_eq!(
        observed.kind(),
        LegacyPreparationInventoryKind::UnmappedReference
    );

    for (suffix, byte) in [("bin", "cd"), ("dpapi", "ef")] {
        let object = format!("{}.{}", hex(byte), suffix);
        let observed = classify_legacy_preparation_inventory_name(
            LegacyPreparationInventoryTree::Objects,
            &object,
        )
        .expect("object");
        assert_eq!(observed.id(), hex(byte));
        assert_eq!(
            observed.kind(),
            LegacyPreparationInventoryKind::UnmappedObject
        );
    }

    assert!(classify_legacy_preparation_inventory_name(
        LegacyPreparationInventoryTree::Objects,
        &reference,
    )
    .is_none());
    assert!(classify_legacy_preparation_inventory_name(
        LegacyPreparationInventoryTree::References,
        &format!("{}.bin", hex("ab")),
    )
    .is_none());
}

#[test]
fn writer_temporary_grammar_is_exact_in_both_trees() {
    let temporary = format!(".{}.42.123456.dpapi.tmp", hex("11"));
    for tree in [
        LegacyPreparationInventoryTree::References,
        LegacyPreparationInventoryTree::Objects,
    ] {
        let observed =
            classify_legacy_preparation_inventory_name(tree, &temporary)
                .expect("temporary");
        assert_eq!(observed.id(), hex("11"));
        assert_eq!(
            observed.kind(),
            LegacyPreparationInventoryKind::Temporary
        );
    }

    for invalid in [
        format!(".{}.pid.123.dpapi.tmp", hex("11")),
        format!(".{}.42.time.dpapi.tmp", hex("11")),
        format!(".{}.42.123.extra.dpapi.tmp", hex("11")),
        format!(".{}.42.123.tmp", hex("11")),
        format!("{}.42.123.dpapi.tmp", hex("11")),
    ] {
        assert!(classify_legacy_preparation_inventory_name(
            LegacyPreparationInventoryTree::Objects,
            &invalid,
        )
        .is_none(), "accepted {invalid}");
    }
}

#[test]
fn upper_case_short_and_unknown_names_fail_closed() {
    for invalid in [
        format!("{}.ref", "AB".repeat(32)),
        format!("{}.ref", "ab".repeat(31)),
        format!("{}.zip", hex("ab")),
        "not-a-digest.ref".to_owned(),
        format!("{}.ref.extra", hex("ab")),
    ] {
        assert!(classify_legacy_preparation_inventory_name(
            LegacyPreparationInventoryTree::References,
            &invalid,
        )
        .is_none(), "accepted {invalid}");
    }

    let overlong = "x".repeat(LEGACY_PREPARATION_MAX_INVENTORY_NAME_BYTES + 1);
    assert!(classify_legacy_preparation_inventory_name(
        LegacyPreparationInventoryTree::Objects,
        &overlong,
    )
    .is_none());
}

#[test]
fn classification_tags_are_frozen() {
    assert_eq!(
        LegacyPreparationInventoryKind::CurrentReference.tag(),
        "current_profile_reference"
    );
    assert_eq!(
        LegacyPreparationInventoryKind::UnmappedReference.tag(),
        "unmapped_profile_or_revision_reference"
    );
    assert_eq!(
        LegacyPreparationInventoryKind::CurrentTarget.tag(),
        "current_profile_target"
    );
    assert_eq!(
        LegacyPreparationInventoryKind::UnmappedObject.tag(),
        "not_linked_by_current_profile"
    );
    assert_eq!(
        LegacyPreparationInventoryKind::Temporary.tag(),
        "uncommitted_temporary_object"
    );
}

#[test]
fn canonical_relative_and_rooted_locators_are_frozen() {
    let reference = legacy_preparation_reference_relative_locator(&[0xab; 32]);
    assert_eq!(
        reference,
        format!("refs/ab/{}.ref", "ab".repeat(32))
    );
    let plaintext = legacy_preparation_object_relative_locator(
        &[0xcd; 32],
        LegacyPreparationProtection::Plaintext,
    );
    assert_eq!(
        plaintext,
        format!("objects/cd/{}.bin", "cd".repeat(32))
    );
    let protected = legacy_preparation_object_relative_locator(
        &[0xef; 32],
        LegacyPreparationProtection::Protected,
    );
    assert_eq!(
        protected,
        format!("objects/ef/{}.dpapi", "ef".repeat(32))
    );
    assert_eq!(
        legacy_preparation_rooted_locator(&protected),
        format!("preparation/{protected}")
    );
}

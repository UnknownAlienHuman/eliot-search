use super::*;

fn hex(byte: &str) -> String {
    byte.repeat(32)
}

#[test]
fn layout_constants_and_shards_are_frozen() {
    assert_eq!(LEGACY_REVISION_DIRECTORY, "revisions");
    assert_eq!(LEGACY_REVISION_MAX_OBJECT_BYTES, 65 * 1024 * 1024);
    assert_eq!(LEGACY_REVISION_MAX_INVENTORY_NAME_BYTES, 192);
    assert!(is_legacy_revision_inventory_shard("0a"));
    assert!(is_legacy_revision_inventory_shard("ff"));
    assert!(!is_legacy_revision_inventory_shard("Ff"));
    assert!(!is_legacy_revision_inventory_shard("abc"));
}

#[test]
fn final_plaintext_and_protected_names_are_classified_exactly() {
    for (suffix, protection) in [
        ("bin", LegacyRevisionProtection::Plaintext),
        ("dpapi", LegacyRevisionProtection::Protected),
    ] {
        let expected_id = hex("ab");
        let name = format!("{expected_id}.{suffix}");
        let observed = classify_legacy_revision_inventory_name(&name)
            .expect("final object");
        assert_eq!(observed.id(), expected_id.as_str());
        assert_eq!(
            observed.physical_kind(),
            LegacyRevisionPhysicalKind::Final(protection)
        );
        assert_eq!(
            observed.inventory_kind(true),
            LegacyRevisionInventoryKind::Referenced
        );
        assert_eq!(
            observed.inventory_kind(false),
            LegacyRevisionInventoryKind::Orphan
        );
    }
}

#[test]
fn historical_and_current_temporary_names_are_exact() {
    for name in [
        format!(".{}.42.tmp", hex("11")),
        format!(".{}.42.123456.dpapi.tmp", hex("11")),
    ] {
        let expected_id = hex("11");
        let observed = classify_legacy_revision_inventory_name(&name)
            .expect("temporary");
        assert_eq!(observed.id(), expected_id.as_str());
        assert_eq!(
            observed.physical_kind(),
            LegacyRevisionPhysicalKind::Temporary
        );
        assert_eq!(
            observed.inventory_kind(true),
            LegacyRevisionInventoryKind::Temporary
        );
    }

    for invalid in [
        format!(".{}.pid.tmp", hex("11")),
        format!(".{}.42.time.dpapi.tmp", hex("11")),
        format!(".{}.42.123.extra.dpapi.tmp", hex("11")),
        format!(".{}.42.123.bin.tmp", hex("11")),
        format!("{}.42.tmp", hex("11")),
    ] {
        assert!(
            classify_legacy_revision_inventory_name(&invalid).is_none(),
            "accepted {invalid}"
        );
    }
}

#[test]
fn unknown_upper_case_short_and_overlong_names_fail_closed() {
    for invalid in [
        format!("{}.bin", "AB".repeat(32)),
        format!("{}.bin", "ab".repeat(31)),
        format!("{}.zip", hex("ab")),
        "not-a-digest.bin".to_owned(),
        format!("{}.dpapi.extra", hex("ab")),
    ] {
        assert!(
            classify_legacy_revision_inventory_name(&invalid).is_none(),
            "accepted {invalid}"
        );
    }

    let overlong =
        "x".repeat(LEGACY_REVISION_MAX_INVENTORY_NAME_BYTES + 1);
    assert!(classify_legacy_revision_inventory_name(&overlong).is_none());
}

#[test]
fn classification_tags_are_frozen() {
    assert_eq!(
        LegacyRevisionInventoryKind::Referenced.tag(),
        "catalog_referenced"
    );
    assert_eq!(
        LegacyRevisionInventoryKind::Orphan.tag(),
        "unreferenced_revision_object"
    );
    assert_eq!(
        LegacyRevisionInventoryKind::Temporary.tag(),
        "uncommitted_temporary_object"
    );
}

#[test]
fn canonical_names_and_locators_are_frozen() {
    let id = hex("ab");
    assert_eq!(
        legacy_revision_object_file_name(
            &id,
            LegacyRevisionProtection::Plaintext
        ),
        Some(format!("{id}.bin"))
    );
    let relative = legacy_revision_object_relative_locator(
        &id,
        LegacyRevisionProtection::Protected,
    )
    .expect("relative locator");
    assert_eq!(relative, format!("ab/{id}.dpapi"));
    assert_eq!(
        legacy_revision_rooted_locator(&relative),
        Some(format!("revisions/{relative}"))
    );

    let temporary = format!(".{}.7.tmp", hex("cd"));
    assert_eq!(
        legacy_revision_inventory_relative_locator("cd", &temporary),
        Some(format!("cd/{temporary}"))
    );
    assert_eq!(
        legacy_revision_inventory_relative_locator("ab", &temporary),
        None
    );
    assert_eq!(legacy_revision_rooted_locator("ab/../escape"), None);
}

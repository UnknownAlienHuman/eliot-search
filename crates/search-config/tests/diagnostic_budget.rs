//! #394 diagnostic admission fixtures, AUTHORED_NOT_RUN (#238/#109/#215).
use std::num::NonZeroU64;

use search_config::{
    ConfigError, ConfigFieldDescriptor, ConfigKeyName, ConfigKeyPath, ConfigLayers, ConfigLimits,
    ConfigOwner, ConfigRegistry, ConfigSectionDescriptor, ConfigSectionName, ConfigSource,
    ConfigSourceKind, ConfigSourceRef, ConfigValue, DisclosureLevel, EffectiveConfigSnapshot,
    RedactedValue, RedactionPolicy, ReloadClass, SecretPolicy, SecretReference, SecurityFloor,
    ValidatedSection, ValueBounds, assemble_effective, merge_layers, project_section,
    redacted_view, register_sections,
};
use search_contracts::{Blake3Digest32, ProfileId};

const HEADER_BYTES: usize = 32 + 6 + 8; // digest, "direct", omitted count

fn path(key: &str) -> ConfigKeyPath {
    ConfigKeyPath::new(
        ConfigSectionName::new("cfg", 128).expect("section"),
        ConfigKeyName::new(key, 128).expect("key"),
    )
}

fn fixture(
    fields: &[(&str, ConfigValue, RedactionPolicy)],
) -> (ConfigRegistry, EffectiveConfigSnapshot) {
    let limits = ConfigLimits::W1;
    let descriptors = fields.iter().map(|(key, value, policy)| {
        ConfigFieldDescriptor::new(
            ConfigKeyName::new(*key, 128).expect("key"),
            value.kind().expect("present fixture value"),
            value.clone(),
            ValueBounds {
                max_text_bytes: 4_096,
                max_list_items: 64,
                max_list_item_bytes: 512,
                integer_min: i64::MIN,
                integer_max: i64::MAX,
            },
            [ConfigSourceKind::File],
            false,
            SecurityFloor::None,
            *policy,
            [search_config::ReconfigurationAction::ApplyLive],
        )
        .expect("field descriptor")
    });
    let descriptor = ConfigSectionDescriptor::new(
        ConfigSectionName::new("cfg", 128).expect("section"),
        ConfigOwner::new("search-query-planner", 128).expect("owner"),
        NonZeroU64::new(1).expect("revision"),
        ReloadClass::ApplyLive,
        Blake3Digest32::from_bytes([4; 32]),
        SecretPolicy::ForbidPlaintext,
        descriptors,
        limits,
    )
    .expect("section descriptor");
    let registry = register_sections(1, [descriptor], limits).expect("registry");
    let profile = ProfileId::new("direct").expect("profile");
    let merged = merge_layers(
        ConfigLayers {
            defaults: ConfigSource {
                kind: ConfigSourceKind::Defaults,
                source_ref: ConfigSourceRef::new("diagnostic-defaults", 128).expect("source"),
                source_digest: Blake3Digest32::from_bytes([1; 32]),
            },
            requested_profile: profile.clone(),
            file: None,
            environment: None,
            cli: None,
        },
        &registry,
        limits,
    )
    .expect("merged defaults");
    let validated = registry.sections().map(|(_, descriptor)| {
        ValidatedSection::new(
            project_section(&merged, descriptor).expect("projected section"),
            profile.clone(),
            Blake3Digest32::from_bytes([9; 32]),
        )
    });
    let snapshot =
        assemble_effective(&registry, validated, profile.clone(), limits).expect("snapshot");
    (registry, snapshot)
}

fn budget(bytes: usize) -> ConfigLimits {
    ConfigLimits {
        max_diagnostic_bytes: bytes,
        ..ConfigLimits::W1
    }
}

#[test]
fn exact_header_fits_but_one_less_returns_no_view() {
    let (registry, snapshot) =
        fixture(&[("a", ConfigValue::Boolean(true), RedactionPolicy::Public)]);
    let view = redacted_view(
        &snapshot,
        &registry,
        DisclosureLevel::Ordinary,
        budget(HEADER_BYTES),
    )
    .expect("header fits");
    assert!(view.entries.is_empty());
    assert_eq!(view.omitted_entries, 1);
    assert_eq!(
        redacted_view(
            &snapshot,
            &registry,
            DisclosureLevel::Ordinary,
            budget(HEADER_BYTES - 1)
        ),
        Err(ConfigError::CapacityExceeded)
    );
}

#[test]
fn exact_entry_byte_boundary_admits_and_one_less_omits_whole() {
    let (registry, snapshot) =
        fixture(&[("a", ConfigValue::Boolean(true), RedactionPolicy::Public)]);
    // section/key=4, tag=1, boolean=1
    let view = redacted_view(
        &snapshot,
        &registry,
        DisclosureLevel::Ordinary,
        budget(HEADER_BYTES + 6),
    )
    .expect("entry fits");
    assert_eq!(
        view.entries.get(&path("a")),
        Some(&RedactedValue::Boolean(true))
    );
    assert_eq!(view.omitted_entries, 0);
    let view = redacted_view(
        &snapshot,
        &registry,
        DisclosureLevel::Ordinary,
        budget(HEADER_BYTES + 5),
    )
    .expect("header fits");
    assert!(view.entries.is_empty());
    assert_eq!(view.omitted_entries, 1);
}

#[test]
fn omitted_large_entry_does_not_prevent_later_smaller_entry() {
    let (registry, snapshot) = fixture(&[
        (
            "a",
            ConfigValue::Text("alpha".into()),
            RedactionPolicy::Public,
        ),
        ("b", ConfigValue::Boolean(true), RedactionPolicy::Public),
    ]);
    let view = redacted_view(
        &snapshot,
        &registry,
        DisclosureLevel::Ordinary,
        budget(HEADER_BYTES + 6),
    )
    .expect("bounded view");
    assert_eq!(view.entries.len(), 1);
    assert!(!view.entries.contains_key(&path("a")));
    assert_eq!(
        view.entries.get(&path("b")),
        Some(&RedactedValue::Boolean(true))
    );
    assert_eq!(view.omitted_entries, 1);
}

#[test]
fn entry_count_ceiling_still_applies_with_large_byte_budget() {
    let (registry, snapshot) = fixture(&[
        ("a", ConfigValue::Boolean(true), RedactionPolicy::Public),
        ("b", ConfigValue::Boolean(false), RedactionPolicy::Public),
    ]);
    let limits = ConfigLimits {
        max_diagnostic_entries: 1,
        ..budget(1_024)
    };
    let view = redacted_view(&snapshot, &registry, DisclosureLevel::Ordinary, limits)
        .expect("bounded view");
    assert_eq!(view.entries.len(), 1);
    assert!(view.entries.contains_key(&path("a")));
    assert_eq!(view.omitted_entries, 1);
}

#[test]
fn utf8_prefix_is_charged_by_bytes_and_aggregate_budget_applies() {
    let (registry, snapshot) = fixture(&[
        (
            "a",
            ConfigValue::Text("\u{e9}\u{e9}".into()),
            RedactionPolicy::Public,
        ),
        ("b", ConfigValue::Text("z".into()), RedactionPolicy::Public),
    ]);
    let view = redacted_view(
        &snapshot,
        &registry,
        DisclosureLevel::Ordinary,
        budget(HEADER_BYTES + 16),
    )
    .expect("bounded view");
    assert_eq!(view.entries.len(), 1); // 10 + 7 bytes would exceed 16
    assert_eq!(view.omitted_entries, 1);
    let limits = ConfigLimits {
        max_text_bytes: 3,
        ..budget(HEADER_BYTES + 15)
    };
    let view = redacted_view(&snapshot, &registry, DisclosureLevel::Ordinary, limits)
        .expect("UTF-8 prefix permits both entries");
    assert_eq!(view.entries.len(), 2);
    assert_eq!(
        view.entries.get(&path("a")),
        Some(&RedactedValue::Text {
            value: "\u{e9}".into(),
            truncated: true,
        })
    );
    assert_eq!(view.omitted_entries, 0);
}

#[test]
fn disclosure_prefix_ceiling_and_whole_entry_admission_are_distinct() {
    let (registry, snapshot) = fixture(&[(
        "a",
        ConfigValue::Text("x".repeat(400)),
        RedactionPolicy::Public,
    )]);
    let limits = budget(HEADER_BYTES + 4 + 1 + 256 + 1);
    let view = redacted_view(&snapshot, &registry, DisclosureLevel::Ordinary, limits)
        .expect("ordinary prefix fits");
    assert_eq!(
        view.entries.get(&path("a")),
        Some(&RedactedValue::Text {
            value: "x".repeat(256),
            truncated: true,
        })
    );
    let privileged = redacted_view(
        &snapshot,
        &registry,
        DisclosureLevel::PrivilegedMetadata,
        limits,
    )
    .expect("header fits");
    assert!(privileged.entries.is_empty());
    assert_eq!(privileged.omitted_entries, 1);
}

#[test]
fn path_secret_list_and_scalars_use_closed_costs_without_disclosure() {
    let (registry, snapshot) = fixture(&[
        (
            "a",
            ConfigValue::Text("private-path-marker".into()),
            RedactionPolicy::PathDigest,
        ),
        (
            "b",
            ConfigValue::SecretReference(
                SecretReference::new("secret://private-marker", 128).expect("opaque reference"),
            ),
            RedactionPolicy::Secret,
        ),
        (
            "c",
            ConfigValue::StringList(vec!["x".into(), "yz".into()]),
            RedactionPolicy::Public,
        ),
        ("d", ConfigValue::Integer(7), RedactionPolicy::Public),
        ("e", ConfigValue::Boolean(true), RedactionPolicy::Public),
    ]);
    let total = HEADER_BYTES + 38 + 5 + 21 + 13 + 6;
    let view = redacted_view(
        &snapshot,
        &registry,
        DisclosureLevel::Ordinary,
        budget(total),
    )
    .expect("exact closed payload fits");
    assert_eq!(view.entries.len(), 5);
    assert!(matches!(
        view.entries.get(&path("a")),
        Some(RedactedValue::PathDigest { .. })
    ));
    assert_eq!(
        view.entries.get(&path("b")),
        Some(&RedactedValue::SecretHidden)
    );
    assert_eq!(
        view.entries.get(&path("c")),
        Some(&RedactedValue::StringList {
            items: 2,
            total_bytes: 3
        })
    );
    let debug = format!("{view:?}");
    assert!(!debug.contains("private-path-marker"));
    assert!(!debug.contains("private-marker"));
    let view = redacted_view(
        &snapshot,
        &registry,
        DisclosureLevel::Ordinary,
        budget(total - 1),
    )
    .expect("header fits");
    assert_eq!(view.entries.len(), 4);
    assert_eq!(view.omitted_entries, 1);
    assert!(!view.entries.contains_key(&path("e")));
}

#[test]
fn metadata_text_extent_does_not_copy_text_and_zero_limits_reject() {
    let (registry, snapshot) = fixture(&[(
        "a",
        ConfigValue::Text("private-marker".repeat(10)),
        RedactionPolicy::MetadataOnly,
    )]);
    let view = redacted_view(
        &snapshot,
        &registry,
        DisclosureLevel::Ordinary,
        budget(HEADER_BYTES + 14),
    )
    .expect("metadata fits");
    assert!(matches!(
        view.entries.get(&path("a")),
        Some(RedactedValue::Metadata { extent: 140, .. })
    ));
    assert!(!format!("{view:?}").contains("private-marker"));
    assert_eq!(
        redacted_view(&snapshot, &registry, DisclosureLevel::Ordinary, budget(0)),
        Err(ConfigError::InvalidLimits)
    );
}

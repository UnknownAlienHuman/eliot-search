//! #398 causal identity fixtures. AUTHORED_NOT_RUN; integrated owner #238/#215.
use super::*;
#[cfg(test)]
use crate::canonical_identity::section_digest;
#[cfg(test)]
use crate::{ConfigFingerprint, ConfigSource, ConfigSourceKind, ConfigSourceRef, SecretReference};

#[cfg(test)]
fn input(value: ConfigValue) -> ConfigSectionInput {
    ConfigSectionInput {
        section_name: ConfigSectionName::new("cfg", 128).expect("name"),
        owner: ConfigOwner::new("owner", 128).expect("owner"),
        schema_revision: NonZeroU64::new(1).expect("revision"),
        field_registry_digest: Blake3Digest32::from_bytes([1; 32]),
        requested_profile: ProfileId::new("direct").expect("profile"),
        fields: BTreeMap::from([(
            ConfigKeyName::new("a", 128).expect("key"),
            ConfigSectionField {
                value,
                provenance: FieldProvenance {
                    source: ConfigSource {
                        kind: ConfigSourceKind::Defaults,
                        source_ref: ConfigSourceRef::new("defaults", 128).expect("source"),
                        source_digest: Blake3Digest32::from_bytes([2; 32]),
                    },
                    explicit_reset: false,
                },
            },
        )]),
    }
}

#[cfg(test)]
fn digest(input: ConfigSectionInput) -> Blake3Digest32 {
    ValidatedSection::new(
        input,
        ProfileId::new("direct").expect("profile"),
        ConfigLimits::W1,
    )
    .expect("bounded typed section")
    .validated_section_digest()
}

#[cfg(test)]
fn field(input: &mut ConfigSectionInput) -> &mut ConfigSectionField {
    input.fields.values_mut().next().expect("field")
}

#[test]
fn descriptor_and_requested_profile_are_bound() {
    let original = input(ConfigValue::Boolean(true));
    let expected = digest(original.clone());
    let mutations: [fn(&mut ConfigSectionInput); 5] = [
        |value| value.section_name = ConfigSectionName::new("other", 128).expect("name"),
        |value| value.owner = ConfigOwner::new("other", 128).expect("owner"),
        |value| value.schema_revision = NonZeroU64::new(2).expect("revision"),
        |value| value.field_registry_digest = Blake3Digest32::from_bytes([3; 32]),
        |value| value.requested_profile = ProfileId::new("lexical").expect("profile"),
    ];
    for mutate in mutations {
        let mut changed = original.clone();
        mutate(&mut changed);
        assert_ne!(digest(changed), expected);
    }
}

#[test]
fn selected_profile_is_bound_independently() {
    let value = input(ConfigValue::Boolean(true));
    let other = ValidatedSection::new(
        value.clone(),
        ProfileId::new("lexical").expect("profile"),
        ConfigLimits::W1,
    )
    .expect("section");
    assert_ne!(digest(value), other.validated_section_digest());
}

#[test]
fn each_provenance_component_and_reset_false_are_bound() {
    let original = input(ConfigValue::Boolean(true));
    let expected = digest(original.clone());
    let mutations: [fn(&mut ConfigSectionInput); 4] = [
        |value| field(value).provenance.source.kind = ConfigSourceKind::File,
        |value| {
            field(value).provenance.source.source_ref =
                ConfigSourceRef::new("another", 128).expect("source")
        },
        |value| field(value).provenance.source.source_digest = Blake3Digest32::from_bytes([3; 32]),
        |value| field(value).provenance.explicit_reset = true,
    ];
    for mutate in mutations {
        let mut changed = original.clone();
        mutate(&mut changed);
        assert_ne!(digest(changed), expected);
    }
}

#[test]
fn value_kind_payload_key_and_list_order_are_bound() {
    let values = [
        ConfigValue::Absent,
        ConfigValue::Boolean(true),
        ConfigValue::Boolean(false),
        ConfigValue::Integer(i64::MIN),
        ConfigValue::Integer(-1),
        ConfigValue::Integer(0),
        ConfigValue::Integer(1),
        ConfigValue::Integer(i64::MAX),
        ConfigValue::Text("1".into()),
        ConfigValue::SecretReference(SecretReference::new("secret://one", 128).expect("ref")),
        ConfigValue::StringList(vec!["a".into(), "b".into()]),
        ConfigValue::StringList(vec!["b".into(), "a".into()]),
    ];
    let digests: Vec<_> = values
        .into_iter()
        .map(|value| digest(input(value)))
        .collect();
    for (index, left) in digests.iter().enumerate() {
        for right in &digests[index + 1..] {
            assert_ne!(left, right);
        }
    }
    let original = input(ConfigValue::Text(String::new()));
    let mut renamed = original.clone();
    let value = renamed.fields.pop_first().expect("field").1;
    renamed
        .fields
        .insert(ConfigKeyName::new("b", 128).expect("key"), value);
    assert_ne!(digest(original), digest(renamed));
}

#[test]
fn equivalent_field_insertion_orders_have_one_identity() {
    let mut left = input(ConfigValue::Boolean(true));
    let mut right = left.clone();
    let additional = field(&mut left).clone();
    left.fields.insert(
        ConfigKeyName::new("z", 128).expect("key"),
        additional.clone(),
    );
    let original = right.fields.pop_first().expect("field");
    right
        .fields
        .insert(ConfigKeyName::new("z", 128).expect("key"), additional);
    right.fields.insert(original.0, original.1);
    assert_eq!(digest(left), digest(right));
}

#[test]
fn borrowed_identifier_and_value_limits_reject_without_identity() {
    let selected = ProfileId::new("direct").expect("profile");
    let value = input(ConfigValue::Text("ab".into()));
    // cfg3+owner5+requested6+selected6+key1+source8 =29 identifier bytes.
    let exact = ConfigLimits {
        max_decoded_identifier_bytes_total: 29,
        max_decoded_value_bytes_total: 2,
        max_text_bytes: 2,
        ..ConfigLimits::W1
    };
    assert!(section_digest(&value, &selected, exact).is_ok());
    for limits in [
        ConfigLimits {
            max_decoded_identifier_bytes_total: 28,
            ..exact
        },
        ConfigLimits {
            max_decoded_value_bytes_total: 1,
            ..exact
        },
        ConfigLimits {
            max_text_bytes: 1,
            ..exact
        },
        ConfigLimits {
            max_identifier_bytes: 7,
            ..exact
        },
    ] {
        assert_eq!(
            section_digest(&value, &selected, limits),
            Err(ConfigError::CapacityExceeded)
        );
    }
    assert_eq!(
        section_digest(
            &value,
            &selected,
            ConfigLimits {
                max_text_bytes: 0,
                ..exact
            }
        ),
        Err(ConfigError::InvalidLimits)
    );
}

#[test]
fn per_list_and_aggregate_list_limits_precede_canonical_conversion() {
    let mut value = input(ConfigValue::StringList(vec!["a".into(), "b".into()]));
    value.fields.insert(
        ConfigKeyName::new("b", 128).expect("key"),
        field(&mut value.clone()).clone(),
    );
    let selected = ProfileId::new("direct").expect("profile");
    assert_eq!(
        section_digest(
            &value,
            &selected,
            ConfigLimits {
                max_list_items: 1,
                ..ConfigLimits::W1
            }
        ),
        Err(ConfigError::CapacityExceeded)
    );
    assert_eq!(
        section_digest(
            &value,
            &selected,
            ConfigLimits {
                max_total_list_items: 3,
                ..ConfigLimits::W1
            }
        ),
        Err(ConfigError::CapacityExceeded)
    );
    assert!(
        section_digest(
            &value,
            &selected,
            ConfigLimits {
                max_list_items: 2,
                max_total_list_items: 4,
                ..ConfigLimits::W1
            }
        )
        .is_ok()
    );
}

#[test]
fn header_row_reduces_maximum_field_count_by_one() {
    let mut value = input(ConfigValue::Boolean(true));
    let template = field(&mut value).clone();
    value.fields.clear();
    for index in 0..4095 {
        value.fields.insert(
            ConfigKeyName::new(format!("k{index}"), 128).expect("key"),
            template.clone(),
        );
    }
    let selected = ProfileId::new("direct").expect("profile");
    let limits = ConfigLimits {
        max_fields_per_section: usize::MAX,
        max_entries_per_layer: usize::MAX,
        ..ConfigLimits::W1
    };
    assert!(section_digest(&value, &selected, limits).is_ok());
    value
        .fields
        .insert(ConfigKeyName::new("last", 128).expect("key"), template);
    assert_eq!(
        section_digest(&value, &selected, limits),
        Err(ConfigError::CapacityExceeded)
    );
}

#[test]
fn section_rows_match_the_exact_shared_schema_and_domain_inclusive_limit() {
    use search_contracts::{
        BoundedBytes, BoundedList, CanonicalDigestDomain, CanonicalText, CanonicalValue as V,
        DigestInputLimit, blake3_canonical_array, to_canonical_cbor,
    };
    let text = |value: &str| V::Text(CanonicalText::new(value).expect("text"));
    let bytes = |marker| V::Bytes(BoundedBytes::new(vec![marker; 32]).expect("digest"));
    let array = |values| V::Array(BoundedList::new(values).expect("row"));
    let rows = vec![
        array(vec![
            text("cfg"),
            text("owner"),
            V::U64(1),
            bytes(1),
            text("direct"),
            text("direct"),
        ]),
        array(vec![
            text("a"),
            V::U64(2),
            V::U64(7),
            V::U64(0),
            text("defaults"),
            bytes(2),
            V::Bool(false),
        ]),
    ];
    let domain = CanonicalDigestDomain::parse("eliot/cbor/config-section/v2").expect("domain");
    let exact = domain.as_str().len()
        + 1
        + to_canonical_cbor(&array(rows.clone()))
            .expect("shared CBOR")
            .as_slice()
            .len();
    let expected = blake3_canonical_array(
        &domain,
        2,
        rows.into_iter().map(Ok),
        DigestInputLimit::new(exact).expect("limit"),
    )
    .expect("shared identity");
    let value = input(ConfigValue::Integer(7));
    let selected = ProfileId::new("direct").expect("profile");
    assert_eq!(
        section_digest(
            &value,
            &selected,
            ConfigLimits {
                max_canonical_bytes: exact,
                ..ConfigLimits::W1
            }
        ),
        Ok(expected)
    );
    assert_eq!(
        section_digest(
            &value,
            &selected,
            ConfigLimits {
                max_canonical_bytes: exact - 1,
                ..ConfigLimits::W1
            }
        ),
        Err(ConfigError::CanonicalBytesExceeded)
    );
}

#[test]
fn snapshot_precharge_checks_names_before_assembly_clones() {
    let selected = ProfileId::new("direct").expect("profile");
    let value = ValidatedSection::new(
        input(ConfigValue::Boolean(true)),
        selected.clone(),
        ConfigLimits::W1,
    )
    .expect("section");
    let limits = ConfigLimits {
        max_decoded_identifier_bytes_total: 14,
        ..ConfigLimits::W1
    };
    let mut budget = crate::canonical_identity::snapshot_budget(&selected, limits).expect("header");
    assert!(
        budget
            .snapshot_section(value.section_name(), &value)
            .is_ok()
    );
    let mut smaller = crate::canonical_identity::snapshot_budget(
        &selected,
        ConfigLimits {
            max_decoded_identifier_bytes_total: 13,
            ..limits
        },
    )
    .expect("header");
    assert_eq!(
        smaller.snapshot_section(value.section_name(), &value),
        Err(ConfigError::CapacityExceeded)
    );
}

#[test]
fn equal_raw_bytes_with_different_profile_tags_compare_unequal() {
    // Fixture-only v2 construction; the public API has no current-value restore.
    let current = ConfigFingerprint::canonical_v2(digest(input(ConfigValue::Boolean(true))));
    let legacy = ConfigFingerprint::restore_legacy_v1(*current.as_bytes());
    assert_eq!(legacy.as_bytes(), current.as_bytes());
    assert_ne!(legacy, current);
    assert_eq!(current.algorithm(), "blake3");
    assert_eq!(current.canonical_profile(), "eliot/cbor/config-snapshot/v2");
    assert_eq!(current.revision(), 2);
    assert_ne!(legacy.revision(), current.revision());
}

// #398 AUTHORED_NOT_RUN.
#[test]
fn complete_snapshot_orders_sections_and_computes_only_current_profile() {
    use crate::{
        ConfigFieldDescriptor, ConfigRegistry, ConfigValueKind, RedactionPolicy, ReloadClass,
        SecretPolicy, SecurityFloor, ValueBounds, assemble_effective, register_sections,
    };
    let selected = ProfileId::new("direct").expect("profile");
    let names = ["alpha", "zeta"];
    let descriptors = names.map(|name| {
        ConfigSectionDescriptor::new(
            ConfigSectionName::new(name, 128).expect("name"),
            ConfigOwner::new("owner", 128).expect("owner"),
            NonZeroU64::new(1).expect("revision"),
            ReloadClass::ApplyLive,
            Blake3Digest32::from_bytes([1; 32]),
            SecretPolicy::ForbidPlaintext,
            [ConfigFieldDescriptor::new(
                ConfigKeyName::new("a", 128).expect("key"),
                ConfigValueKind::Boolean,
                ConfigValue::Boolean(true),
                ValueBounds {
                    max_text_bytes: 128,
                    max_list_items: 4,
                    max_list_item_bytes: 128,
                    integer_min: i64::MIN,
                    integer_max: i64::MAX,
                },
                [ConfigSourceKind::File],
                false,
                SecurityFloor::None,
                RedactionPolicy::Public,
                [crate::ReconfigurationAction::ApplyLive],
            )
            .expect("field")],
            ConfigLimits::W1,
        )
        .expect("descriptor")
    });
    let registry: ConfigRegistry =
        register_sections(1, descriptors, ConfigLimits::W1).expect("registry");
    let sections = names.map(|name| {
        let mut value = input(ConfigValue::Boolean(true));
        value.section_name = ConfigSectionName::new(name, 128).expect("name");
        ValidatedSection::new(value, selected.clone(), ConfigLimits::W1).expect("section")
    });
    let left = assemble_effective(
        &registry,
        sections.clone(),
        selected.clone(),
        ConfigLimits::W1,
    )
    .expect("complete");
    let right = assemble_effective(
        &registry,
        sections.iter().rev().cloned(),
        selected.clone(),
        ConfigLimits::W1,
    )
    .expect("complete");
    assert_eq!(left, right);
    assert_eq!(left.fingerprint().algorithm(), "blake3");
    assert_eq!(left.fingerprint().revision(), 2);
    assert_eq!(left.config_schema_version(), 1);
    assert_ne!(
        left.fingerprint(),
        ConfigFingerprint::restore_legacy_v1(*left.fingerprint().as_bytes())
    );
    assert_eq!(
        assemble_effective(&registry, [sections[0].clone()], selected, ConfigLimits::W1),
        Err(ConfigError::MissingSection)
    );
}

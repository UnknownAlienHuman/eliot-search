//! Focused profile tests, intended as a child module of `manifest`.
//! Fixture digests below are synthetic bindings, not qualification evidence.

use std::collections::BTreeSet;

use search_contracts::{
    Blake3Digest32, CanonicalKey, CanonicalValue, MAX_CANONICAL_BYTES, ProfileId, UnitKind,
    parse_canonical_cbor, to_canonical_cbor,
};

use super::digest::{bytes, hash_cbor, object, text};
use super::v3_profile::{
    V3AnchorPolicy, V3AttachmentPolicy, V3EmptyPolicy, V3OmissionPolicy, V3OverlapPolicy,
    V3RepresentationKind, V3UnitizerProfileDescriptor, decode_profile,
    validate_v3_unitizer_profile,
};
use crate::{UnitizationError, UnitizationLimits};

const BYTE_CEILING: usize = MAX_CANONICAL_BYTES - 128;
const FIELDS: [&str; 18] = [
    "profile_name",
    "profile_revision",
    "representation_kind",
    "unit_kind",
    "boundary_revision",
    "overlap_policy",
    "anchor_policy",
    "attachment_policy",
    "omission_policy",
    "empty_policy",
    "limits",
    "min_unit_bytes",
    "max_unit_scalars",
    "max_unit_lines",
    "max_anchor_depth",
    "max_manifest_bytes",
    "max_steps",
    "fixture_digest",
];
const LIMIT_FIELDS: [&str; 5] = [
    "max_input_bytes",
    "preferred_unit_bytes",
    "max_unit_bytes",
    "max_lines",
    "max_units",
];

fn descriptor() -> V3UnitizerProfileDescriptor {
    V3UnitizerProfileDescriptor {
        profile_name: ProfileId::new("exact-text-test-v3").expect("valid test name"),
        profile_revision: 1,
        representation_kind: V3RepresentationKind::Text,
        unit_kind: UnitKind::Section,
        boundary_revision: 1,
        overlap_policy: V3OverlapPolicy::None,
        anchor_policy: V3AnchorPolicy::ExactTextBytes,
        attachment_policy: V3AttachmentPolicy::Absent,
        omission_policy: V3OmissionPolicy::Forbidden,
        empty_policy: V3EmptyPolicy::Reject,
        limits: UnitizationLimits {
            max_input_bytes: 4096,
            preferred_unit_bytes: 16,
            max_unit_bytes: 64,
            max_lines: 512,
            max_units: 64,
        },
        min_unit_bytes: 1,
        max_unit_scalars: 64,
        max_unit_lines: 32,
        max_anchor_depth: 1,
        max_manifest_bytes: 8192,
        max_steps: 4096,
        fixture_digest: Blake3Digest32::from_stored_bytes([7; 32]),
    }
}

fn value() -> CanonicalValue {
    super::v3_profile::profile_value(&descriptor()).expect("valid test descriptor")
}

fn field(value: &CanonicalValue, name: &str) -> CanonicalValue {
    let CanonicalValue::Object(fields) = value else {
        panic!("expected object")
    };
    fields
        .get(&CanonicalKey::new_non_empty(name).expect("valid key"))
        .expect("existing field")
        .clone()
}

fn replace(mut value: CanonicalValue, name: &str, replacement: CanonicalValue) -> CanonicalValue {
    let CanonicalValue::Object(fields) = &mut value else {
        panic!("expected object")
    };
    let key = CanonicalKey::new_non_empty(name).expect("valid key");
    assert!(
        fields.remove(&key).is_some(),
        "missing replacement field: {name}"
    );
    fields
        .insert(key, replacement)
        .expect("bounded replacement");
    value
}

fn omit(mut value: CanonicalValue, name: &str) -> CanonicalValue {
    let CanonicalValue::Object(fields) = &mut value else {
        panic!("expected object")
    };
    assert!(
        fields
            .remove(&CanonicalKey::new_non_empty(name).expect("valid key"))
            .is_some()
    );
    value
}

fn add_unknown(mut value: CanonicalValue, name: &str) -> CanonicalValue {
    let CanonicalValue::Object(fields) = &mut value else {
        panic!("expected object")
    };
    let key = CanonicalKey::new_non_empty(name).expect("valid key");
    assert!(
        fields.get(&key).is_none(),
        "unknown-field helper must add a new key"
    );
    fields
        .insert(key, CanonicalValue::U64(1))
        .expect("bounded extra field");
    value
}

fn reject_wire(value: &CanonicalValue, case: &str) {
    // Shared encoding/parsing must succeed so the hostile field reaches the profile decoder.
    let encoded = to_canonical_cbor(value).expect("canonical hostile descriptor");
    let parsed = parse_canonical_cbor(encoded.as_slice()).expect("shared parser accepts value");
    assert_eq!(
        decode_profile(parsed),
        Err(UnitizationError::UnitizerProfileInvalid),
        "{case}"
    );
}

#[test]
fn validated_profile_getters_and_id_bind_the_shared_canonical_descriptor() {
    let expected = descriptor();
    let validated = validate_v3_unitizer_profile(&expected).expect("valid profile");
    assert_eq!(validated.descriptor(), &expected);
    assert_eq!(validated.limits(), expected.limits);
    assert_eq!(validated.revision(), expected.profile_revision);
    let canonical = super::v3_profile::profile_value(&expected).expect("profile value");
    assert_eq!(
        field(&canonical, "empty_policy"),
        text("reject-empty").expect("tag")
    );
    let digest = hash_cbor(
        "eliot/cbor/unitizer-profile/v3",
        &canonical,
        MAX_CANONICAL_BYTES,
    )
    .expect("shared profile digest");
    assert_eq!(validated.id().as_str(), format!("unitizer-v3-{digest}"));
    assert_eq!(validated.id().as_str().len(), "unitizer-v3-".len() + 64);
    assert_eq!(
        validate_v3_unitizer_profile(&expected).expect("repeat"),
        validated
    );
}

#[test]
fn zero_dimensions_and_zero_fixture_binding_are_refused() {
    for name in [
        "profile_revision",
        "min_unit_bytes",
        "max_unit_scalars",
        "max_unit_lines",
        "max_anchor_depth",
        "max_manifest_bytes",
        "max_steps",
    ] {
        reject_wire(&replace(value(), name, CanonicalValue::U64(0)), name);
    }
    for name in LIMIT_FIELDS {
        let original = value();
        let limits = replace(field(&original, "limits"), name, CanonicalValue::U64(0));
        reject_wire(&replace(original, "limits", limits), name);
    }
    let mut empty_binding = descriptor();
    empty_binding.fixture_digest = Blake3Digest32::from_stored_bytes([0; 32]);
    assert_eq!(
        validate_v3_unitizer_profile(&empty_binding),
        Err(UnitizationError::UnitizerProfileInvalid)
    );
    reject_wire(
        &replace(value(), "fixture_digest", bytes(&[0; 32]).expect("bytes")),
        "zero fixture digest",
    );
}

#[test]
fn budgets_refuse_oversize_values_and_inconsistent_maxima() {
    let ceiling = u64::try_from(BYTE_CEILING).expect("ceiling fits");
    for (name, number) in [
        ("max_unit_scalars", 65),
        ("max_unit_lines", 65),
        ("max_manifest_bytes", ceiling + 1),
        ("max_steps", 32_000_001),
    ] {
        reject_wire(&replace(value(), name, CanonicalValue::U64(number)), name);
    }
    for (name, number) in [
        ("max_input_bytes", ceiling + 1),
        ("preferred_unit_bytes", 65),
        ("max_unit_bytes", 4097),
        ("max_lines", ceiling + 1),
        ("max_units", 4097),
    ] {
        let original = value();
        let limits = replace(
            field(&original, "limits"),
            name,
            CanonicalValue::U64(number),
        );
        reject_wire(&replace(original, "limits", limits), name);
    }
    let original = value();
    let limits = replace(
        field(&original, "limits"),
        "max_lines",
        CanonicalValue::U64(31),
    );
    reject_wire(
        &replace(original, "limits", limits),
        "unit lines exceed input line budget",
    );
}

#[test]
fn frozen_ceilings_are_inclusive_and_unit_maxima_can_equal_their_bounds() {
    let mut at_ceiling = descriptor();
    at_ceiling.limits = UnitizationLimits {
        max_input_bytes: BYTE_CEILING,
        preferred_unit_bytes: BYTE_CEILING,
        max_unit_bytes: BYTE_CEILING,
        max_lines: BYTE_CEILING,
        max_units: 4096,
    };
    at_ceiling.max_unit_scalars = BYTE_CEILING;
    at_ceiling.max_unit_lines = BYTE_CEILING;
    at_ceiling.max_manifest_bytes = BYTE_CEILING;
    at_ceiling.max_steps = 32_000_000;
    let validated = validate_v3_unitizer_profile(&at_ceiling).expect("inclusive ceilings");
    let canonical = super::v3_profile::profile_value(&at_ceiling).expect("value");
    assert_eq!(
        decode_profile(canonical).expect("decode ceilings"),
        validated
    );
}

#[test]
fn unsupported_kinds_revisions_and_fixed_baseline_decisions_are_refused() {
    for kind in [
        UnitKind::Symbol,
        UnitKind::Reference,
        UnitKind::Test,
        UnitKind::Table,
        UnitKind::ImageRegion,
    ] {
        let mut unsupported = descriptor();
        unsupported.unit_kind = kind;
        assert_eq!(
            validate_v3_unitizer_profile(&unsupported),
            Err(UnitizationError::UnitizerProfileInvalid)
        );
        reject_wire(
            &replace(value(), "unit_kind", text(kind.as_str()).expect("kind")),
            "unsupported unit kind",
        );
    }
    for (name, numbers) in [
        ("boundary_revision", [0, 2]),
        ("min_unit_bytes", [0, 2]),
        ("max_anchor_depth", [0, 2]),
    ] {
        for number in numbers {
            reject_wire(&replace(value(), name, CanonicalValue::U64(number)), name);
        }
    }
    reject_wire(
        &replace(value(), "max_anchor_depth", CanonicalValue::U64(256)),
        "depth outside u8",
    );
    for (name, tag) in [
        ("representation_kind", "structural"),
        ("representation_kind", "predicate"),
        ("unit_kind", "unknown-kind"),
        ("overlap_policy", "overlap"),
        ("anchor_policy", "transformed"),
        ("attachment_policy", "structural"),
        ("attachment_policy", "predicate"),
        ("omission_policy", "allowed"),
        ("empty_policy", "allow-empty"),
        ("empty_policy", "unknown"),
    ] {
        reject_wire(
            &replace(value(), name, text(tag).expect("hostile tag")),
            tag,
        );
    }
}

#[test]
fn closed_descriptor_rejects_unknown_and_missing_fields_at_both_levels() {
    for name in [
        "future_policy",
        "structural_identity",
        "configuration_predicate",
    ] {
        reject_wire(&add_unknown(value(), name), name);
    }
    let original = value();
    let limits = add_unknown(field(&original, "limits"), "future_limit");
    reject_wire(&replace(original, "limits", limits), "unknown nested limit");
    for name in FIELDS {
        reject_wire(&omit(value(), name), name);
    }
    for name in LIMIT_FIELDS {
        let original = value();
        let limits = omit(field(&original, "limits"), name);
        reject_wire(&replace(original, "limits", limits), name);
    }
}

#[test]
fn descriptor_fields_are_strictly_typed_and_fixture_digest_has_exact_length() {
    for name in FIELDS {
        let wrong_type = if matches!(field(&value(), name), CanonicalValue::U64(_)) {
            text("1").expect("text")
        } else {
            CanonicalValue::U64(1)
        };
        reject_wire(&replace(value(), name, wrong_type), name);
    }
    for name in LIMIT_FIELDS {
        let original = value();
        let limits = replace(field(&original, "limits"), name, text("1").expect("text"));
        reject_wire(&replace(original, "limits", limits), name);
    }
    for length in [0, 31, 33] {
        reject_wire(
            &replace(
                value(),
                "fixture_digest",
                bytes(&vec![7; length]).expect("bytes"),
            ),
            "wrong digest length",
        );
    }
    reject_wire(
        &replace(value(), "profile_name", text("").expect("empty text")),
        "empty profile name",
    );
    reject_wire(
        &object(Vec::new()).expect("empty object"),
        "legacy/incomplete descriptor",
    );
}

type DescriptorEdit = fn(&mut V3UnitizerProfileDescriptor);

fn permitted_changes() -> Vec<(&'static str, DescriptorEdit)> {
    vec![
        ("profile name", |d| {
            d.profile_name = ProfileId::new("other-text-test-v3").expect("name");
        }),
        ("profile revision", |d| d.profile_revision = 2),
        ("code representation", |d| {
            d.representation_kind = V3RepresentationKind::Code;
        }),
        ("file units", |d| d.unit_kind = UnitKind::File),
        ("doc units", |d| d.unit_kind = UnitKind::Doc),
        ("input bytes", |d| d.limits.max_input_bytes = 4097),
        ("preferred unit bytes", |d| {
            d.limits.preferred_unit_bytes = 17;
        }),
        ("maximum unit bytes", |d| d.limits.max_unit_bytes = 65),
        ("input lines", |d| d.limits.max_lines = 513),
        ("unit count", |d| d.limits.max_units = 65),
        ("unit scalars", |d| d.max_unit_scalars = 63),
        ("unit lines", |d| d.max_unit_lines = 33),
        ("manifest bytes", |d| d.max_manifest_bytes = 8193),
        ("steps", |d| d.max_steps = 4097),
        ("fixture binding", |d| {
            d.fixture_digest = Blake3Digest32::from_stored_bytes([8; 32]);
        }),
    ]
}

#[test]
fn every_permitted_descriptor_choice_changes_identity_and_round_trips_canonically() {
    let baseline = descriptor();
    let baseline_profile = validate_v3_unitizer_profile(&baseline).expect("baseline");
    let mut identities = BTreeSet::from([baseline_profile.id().as_str().to_owned()]);
    let cases = std::iter::once(("baseline", baseline.clone())).chain(
        permitted_changes().into_iter().map(|(name, edit)| {
            let mut changed = baseline.clone();
            edit(&mut changed);
            (name, changed)
        }),
    );
    for (name, candidate) in cases {
        let validated = validate_v3_unitizer_profile(&candidate).expect("permitted decision");
        let canonical = super::v3_profile::profile_value(&candidate).expect("canonical value");
        let encoded = to_canonical_cbor(&canonical).expect("shared CBOR encode");
        let parsed = parse_canonical_cbor(encoded.as_slice()).expect("shared CBOR parse");
        let decoded = decode_profile(parsed).expect("closed profile decode");
        assert_eq!(decoded, validated, "{name}");
        assert_eq!(decoded.descriptor(), &candidate, "{name}");
        let reencoded = to_canonical_cbor(
            &super::v3_profile::profile_value(decoded.descriptor()).expect("decoded profile value"),
        )
        .expect("shared CBOR re-encode");
        assert_eq!(reencoded.as_slice(), encoded.as_slice(), "{name}");
        if name != "baseline" {
            assert_ne!(candidate, baseline, "test must change a decision: {name}");
            assert_ne!(decoded.id(), baseline_profile.id(), "{name}");
            assert!(
                identities.insert(decoded.id().as_str().to_owned()),
                "distinct decision identity: {name}"
            );
        }
    }
    assert_eq!(identities.len(), 1 + permitted_changes().len());
}

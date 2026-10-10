use super::*;
use crate::UnitizationError;
use search_contracts::{
    Blake3Digest32, MaterializationId, NativeAnchor, ProfileId, RepresentationId, SourceId,
    SourceNamespaceId, SourceRevisionId, UnitKind,
};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};
mod fixture;
mod ingress;

fn fixture_bytes(hex: &str) -> Vec<u8> {
    let compact: String = hex
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    (0..compact.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&compact[index..index + 2], 16).unwrap())
        .collect()
}

#[test]
fn v3_canonical_golden() {
    let result = set("one\ntwo\nthree\n", &descriptor(), binding());
    let bytes = canonicalize_unit_manifest(result.manifest()).unwrap();
    let expected = fixture_bytes(include_str!("../testdata/unit_manifest_v3.hex"));
    assert_eq!(bytes.as_slice(), expected);
    assert_eq!(
        result.manifest_digest().as_bytes(),
        fixture_bytes("10c02fbb46c22d7b1e86f710c785fd32a02a010f6f2708711fd57e6cca62bb39")
            .as_slice()
    );
    assert_eq!(
        result.manifest().profile_id().as_str(),
        "unitizer-v3-9de4ef6587718566abe525b2cc4bf232bf939de9b269a7b5cc47e09f67596627"
    );
    assert_eq!(
        result.units()[0].unit_id(),
        search_contracts::UnitId::from_bytes([
            0xd3, 0x1a, 0xb8, 0x5f, 0x1c, 0xf7, 0xaf, 0x83, 0x5e, 0x33, 0x8c, 0xc0, 0xbb, 0xa4,
            0xbe, 0x3a,
        ])
    );
    assert_eq!(
        decode_unit_manifest(bytes.as_slice(), 128 * 1024).unwrap(),
        *result.manifest()
    );
}

#[test]
fn truncated_identity_collision_and_duplicate_fail_closed() {
    let mut ids = std::collections::BTreeMap::new();
    let id = search_contracts::UnitId::from_bytes([1; 16]);
    let commitment = Blake3Digest32::from_bytes([1; 32]);
    assert_eq!(build::record_identity(&mut ids, id, commitment), Ok(()));
    assert_eq!(
        build::record_identity(&mut ids, id, commitment),
        Err(UnitizationError::UnitizationNondeterministic)
    );
    let mut different = [1; 32];
    different[31] = 2;
    assert_eq!(
        build::record_identity(&mut ids, id, Blake3Digest32::from_bytes(different)),
        Err(UnitizationError::IdentityCollision)
    );
    assert_eq!(ids.get(&id), Some(&commitment));
}

#[test]
fn live_layout_cancellation_stops_before_complete_allocation() {
    let text = "one\ntwo\nthree\n";
    let profile = validate_v3_unitizer_profile(&descriptor()).unwrap();
    with_input(text, &profile, binding(), |input, budget| {
        let cancel = AtomicBool::new(false);
        let mut narrowed = *budget;
        narrowed.cancelled = &cancel;
        let mut allocated = 0;
        let result = crate::layout::unitize_text_checked(
            text,
            &input.lines,
            profile.limits(),
            |next_count| {
                allocated = next_count;
                if next_count == 2 {
                    cancel.store(true, std::sync::atomic::Ordering::Release);
                }
                narrowed.check(input.prep_steps)
            },
        );
        assert_eq!(result, Err(UnitizationError::Cancelled));
        assert_eq!(allocated, 2);
        let mut narrowed = *budget;
        narrowed.max_steps = input.prep_steps + 32;
        assert_eq!(
            build_unit_manifest(input, &profile, &narrowed),
            Err(UnitizationError::WorkBudgetExceeded)
        );
    });
}

#[test]
fn exact_verified_diff_retains_only_full_equal_descriptors() {
    let first = set("one\ntwo\nthree\n", &descriptor(), binding());
    let same = set("one\ntwo\nthree\n", &descriptor(), binding());
    let diff = diff_unit_manifests(&first, &same).unwrap();
    assert!(diff.created.is_empty());
    assert!(diff.retired.is_empty());
    assert_eq!(
        diff.retained,
        first
            .units()
            .iter()
            .map(UnitDescriptor::unit_id)
            .collect::<Vec<_>>()
    );
    let mut changed = binding();
    changed.source_revision_id = SourceRevisionId::from_bytes([6; 16]);
    let next = set("one\ntwo\nthree\n", &descriptor(), changed);
    let diff = diff_unit_manifests(&first, &next).unwrap();
    assert!(diff.retained.is_empty());
    assert_eq!(diff.created.len(), 2);
    assert_eq!(diff.retired.len(), 2);
}

fn descriptor() -> V3UnitizerProfileDescriptor {
    V3UnitizerProfileDescriptor {
        profile_name: ProfileId::new("baseline-unitizer-v3").unwrap(),
        profile_revision: 1,
        representation_kind: V3RepresentationKind::Text,
        unit_kind: UnitKind::Section,
        boundary_revision: 1,
        overlap_policy: V3OverlapPolicy::None,
        anchor_policy: V3AnchorPolicy::ExactTextBytes,
        attachment_policy: V3AttachmentPolicy::Absent,
        omission_policy: V3OmissionPolicy::Forbidden,
        empty_policy: V3EmptyPolicy::Reject,
        limits: crate::UnitizationLimits {
            max_input_bytes: 4096,
            preferred_unit_bytes: 8,
            max_unit_bytes: 16,
            max_lines: 512,
            max_units: 512,
        },
        min_unit_bytes: 1,
        max_unit_scalars: 16,
        max_unit_lines: 16,
        max_anchor_depth: 1,
        max_manifest_bytes: 128 * 1024,
        max_steps: 100_000,
        fixture_digest: digest::hash_raw(
            "eliot/raw/unitizer-fixture/v3",
            b"baseline-utf8-v1:one\ntwo\nthree\n;unicode;crlf;reject-forgeries",
            4096,
        )
        .unwrap(),
    }
}
fn binding() -> V3SourceBinding {
    V3SourceBinding {
        source_namespace_id: SourceNamespaceId::from_bytes([1; 16]),
        source_id: SourceId::from_bytes([2; 16]),
        source_revision_id: SourceRevisionId::from_bytes([3; 16]),
        representation_id: RepresentationId::from_bytes([4; 16]),
        materialization_id: MaterializationId::from_bytes([5; 16]),
    }
}
fn budget(cancelled: &AtomicBool) -> UnitizationBudget<'_> {
    UnitizationBudget {
        max_steps: 100_000,
        max_encoded_bytes: 128 * 1024,
        deadline: Instant::now() + Duration::from_secs(60),
        cancelled,
    }
}
fn with_input<T>(
    text: &str,
    profile: &ValidatedV3UnitizerProfile,
    binding: V3SourceBinding,
    f: impl FnOnce(&UnitSetInput<'_>, &UnitizationBudget<'_>) -> T,
) -> T {
    let (product, request, materializer) =
        fixture::materialize_product(text.as_bytes(), &binding.source_id, 1).unwrap();
    let cancel = AtomicBool::new(false);
    let budget = budget(&cancel);
    let input =
        prepare_unit_set_input(binding, &product, &request, &materializer, profile, &budget)
            .unwrap();
    f(&input, &budget)
}
fn set(
    text: &str,
    descriptor: &V3UnitizerProfileDescriptor,
    binding: V3SourceBinding,
) -> VerifiedUnitSet {
    let profile = validate_v3_unitizer_profile(descriptor).unwrap();
    with_input(text, &profile, binding, |input, budget| {
        build_unit_manifest(input, &profile, budget).unwrap()
    })
}
fn reseal(manifest: &mut UnitManifest) {
    manifest.manifest_digest = digest::hash_cbor(
        "eliot/cbor/unit-manifest/v3",
        &codec::body_value(&manifest.body).unwrap(),
        manifest.body.profile.max_manifest_bytes + 128,
    )
    .unwrap();
}

#[test]
fn real_materialization_roundtrip_complete_occurrences_and_representation_bind() {
    let profile = validate_v3_unitizer_profile(&descriptor()).unwrap();
    with_input("one\ntwo\nthree\n", &profile, binding(), |input, budget| {
        let result = build_unit_manifest(input, &profile, budget).unwrap();
        let encoded = canonicalize_unit_manifest(result.manifest()).unwrap();
        let decoded = decode_unit_manifest(encoded.as_slice(), budget.max_encoded_bytes).unwrap();
        assert_eq!(&decoded, result.manifest());
        assert_eq!(
            verify_unit_manifest(&decoded, input, &profile, budget).unwrap(),
            result
        );
        assert_eq!(canonicalize_unit_manifest(&decoded).unwrap(), encoded);
        assert_eq!(result.manifest().emitted_bytes(), 14);
        assert_eq!(result.manifest().omitted_bytes(), 0);
        assert_eq!(result.units().len(), 2);
        assert_eq!(
            result.representation().unit_manifest_digest,
            result.manifest_digest()
        );
        let occurrences: Vec<_> = result
            .units()
            .iter()
            .map(|u| u.occurrence().clone())
            .collect();
        assert_eq!(occurrences[0].unit_id, result.units()[0].unit_id());
        assert_eq!(
            occurrences[1].representation_id,
            binding().representation_id
        );
    });
}

#[test]
fn deterministic_bytes_and_source_byte_anchors_include_unicode_and_crlf() {
    for source in [
        "one\ntwo\nthree\n",
        "é\r\n🙂汉\nline\rfinal",
        "abcdefgh🙂ijklmnop\n",
    ] {
        let first = set(source, &descriptor(), binding());
        let second = set(source, &descriptor(), binding());
        assert_eq!(first, second);
        assert_eq!(
            canonicalize_unit_manifest(first.manifest()).unwrap(),
            canonicalize_unit_manifest(second.manifest()).unwrap()
        );
        let mut end = 0;
        for unit in first.units() {
            assert_eq!(unit.source_start(), end);
            let NativeAnchor::TextBytes(anchor) = &unit.occurrence().native_anchor else {
                panic!("wrong anchor");
            };
            assert_eq!(anchor.byte_start_0, unit.source_start());
            assert_eq!(anchor.byte_end_exclusive_0, unit.source_end());
            assert_eq!(
                anchor.content_digest,
                Blake3Digest32::from_bytes(*blake3::hash(source.as_bytes()).as_bytes())
            );
            assert!(source.is_char_boundary(usize::try_from(unit.source_start()).unwrap()));
            assert!(source.is_char_boundary(usize::try_from(unit.source_end()).unwrap()));
            end = unit.source_end();
        }
        assert_eq!(end, u64::try_from(source.len()).unwrap());
        let debug = format!(
            "{:?}{:?}",
            first,
            canonicalize_unit_manifest(first.manifest()).unwrap()
        );
        assert!(!debug.contains(source));
    }
}

#[test]
fn provenance_profile_kind_and_representation_changes_change_derived_identity() {
    let first = set("one\ntwo\nthree\n", &descriptor(), binding());
    for field in 0..5 {
        let mut changed = binding();
        match field {
            0 => changed.source_namespace_id = SourceNamespaceId::from_bytes([9; 16]),
            1 => changed.source_id = SourceId::from_bytes([9; 16]),
            2 => changed.source_revision_id = SourceRevisionId::from_bytes([9; 16]),
            3 => changed.representation_id = RepresentationId::from_bytes([9; 16]),
            _ => changed.materialization_id = MaterializationId::from_bytes([9; 16]),
        }
        let next = set("one\ntwo\nthree\n", &descriptor(), changed);
        assert_ne!(first.units()[0].unit_id(), next.units()[0].unit_id());
        assert_ne!(first.manifest_digest(), next.manifest_digest());
    }
    for field in 0..3 {
        let mut changed = descriptor();
        match field {
            0 => changed.profile_revision = 2,
            1 => changed.unit_kind = UnitKind::Doc,
            _ => changed.fixture_digest = Blake3Digest32::from_bytes([8; 32]),
        }
        let next = set("one\ntwo\nthree\n", &changed, binding());
        assert_ne!(first.units()[0].unit_id(), next.units()[0].unit_id());
    }
}

#[test]
fn correctly_rehashed_forged_manifests_never_become_verified() {
    let profile = validate_v3_unitizer_profile(&descriptor()).unwrap();
    with_input("one\ntwo\nthree\n", &profile, binding(), |input, budget| {
        let good = build_unit_manifest(input, &profile, budget).unwrap();
        for field in 0..18 {
            let mut forged = good.manifest().clone();
            let body = &mut forged.body;
            match field {
                0 => {
                    body.provenance.binding.source_revision_id =
                        SourceRevisionId::from_bytes([8; 16]);
                }
                1 => {
                    body.provenance.binding.representation_id =
                        RepresentationId::from_bytes([8; 16]);
                }
                2 => body.provenance.coordinate_digest = Blake3Digest32::from_bytes([8; 32]),
                3 => body.units[0].occurrence.unit_id = body.units[1].occurrence.unit_id,
                4 => body.units[1].occurrence.ordinal = 0,
                5 => body.units[0].source_start = 1,
                6 => body.units[0].source_end += 1,
                7 => {
                    body.units[0].occurrence.representation_id =
                        RepresentationId::from_bytes([8; 16]);
                }
                8 => body.units[0].occurrence.unit_kind = UnitKind::Doc,
                9 => body.units[0].unit_content_digest = Blake3Digest32::from_bytes([8; 32]),
                10 => body.units[0].reference_digest = Blake3Digest32::from_bytes([8; 32]),
                11 => body.units[0].identity_digest = Blake3Digest32::from_bytes([8; 32]),
                12 => body.units.reverse(),
                13 => {
                    body.units.pop();
                }
                14 => body.represented_bytes -= 1,
                15 => body.omitted_bytes = 1,
                16 => body.line_count += 1,
                _ => {
                    if let NativeAnchor::TextBytes(anchor) =
                        &mut body.units[0].occurrence.native_anchor
                    {
                        anchor.byte_end_exclusive_0 += 1;
                    }
                }
            }
            reseal(&mut forged);
            let bytes = canonicalize_unit_manifest(&forged).unwrap();
            let proposed =
                decode_unit_manifest(bytes.as_slice(), budget.max_encoded_bytes).unwrap();
            assert_eq!(
                verify_unit_manifest(&proposed, input, &profile, budget),
                Err(UnitizationError::UnitManifestDigestMismatch),
                "mutation {field}"
            );
        }
    });
}

#[test]
fn attachment_and_predicate_cannot_be_silently_dropped() {
    let good = set("one\ntwo\nthree\n", &descriptor(), binding());
    let mut structural = good.manifest().clone();
    structural.body.units[0].occurrence.structural_identity =
        Some(search_contracts::OpaqueId::new("structure:test").unwrap());
    assert_eq!(
        canonicalize_unit_manifest(&structural),
        Err(UnitizationError::UnitManifestIncomplete)
    );
    let mut predicate = good.manifest().clone();
    predicate.body.units[0].occurrence.configuration_predicate =
        Some(search_contracts::BoundedExpression::new("cfg(test)").unwrap());
    assert_eq!(
        canonicalize_unit_manifest(&predicate),
        Err(UnitizationError::UnitManifestIncomplete)
    );
}

#[test]
fn cancellation_deadline_unit_and_output_budgets_never_return_a_complete_set() {
    let profile = validate_v3_unitizer_profile(&descriptor()).unwrap();
    with_input("one\ntwo\nthree\n", &profile, binding(), |input, budget| {
        let cancelled = AtomicBool::new(true);
        let mut narrowed = *budget;
        narrowed.cancelled = &cancelled;
        assert_eq!(
            build_unit_manifest(input, &profile, &narrowed),
            Err(UnitizationError::Cancelled)
        );
        narrowed = *budget;
        narrowed.deadline = Instant::now();
        assert_eq!(
            build_unit_manifest(input, &profile, &narrowed),
            Err(UnitizationError::DeadlineExceeded)
        );
        narrowed = *budget;
        narrowed.max_steps = 1;
        assert_eq!(
            build_unit_manifest(input, &profile, &narrowed),
            Err(UnitizationError::WorkBudgetExceeded)
        );
        narrowed = *budget;
        narrowed.max_encoded_bytes = 1;
        assert_eq!(
            build_unit_manifest(input, &profile, &narrowed),
            Err(UnitizationError::InputTooLarge)
        );
    });
    let mut desc = descriptor();
    desc.limits.max_units = 1;
    let profile = validate_v3_unitizer_profile(&desc).unwrap();
    with_input("one\ntwo\nthree\n", &profile, binding(), |input, budget| {
        assert_eq!(
            build_unit_manifest(input, &profile, budget),
            Err(UnitizationError::TooManyUnits)
        );
    });
}

#[test]
fn v2_cannot_be_relabelled_or_satisfy_v3_and_codec_is_closed() {
    for fixture in [
        include_str!("../testdata/unit_manifest_v1.hex"),
        include_str!("../testdata/unit_manifest_v2.hex"),
    ] {
        let hex: String = fixture.chars().filter(|c| !c.is_whitespace()).collect();
        let bytes: Vec<u8> = (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
            .collect();
        assert_eq!(
            decode_unit_manifest(&bytes, 128 * 1024),
            Err(UnitizationError::UnitManifestLegacyUnsupported)
        );
        let mut relabelled = bytes;
        relabelled[..8].copy_from_slice(b"ELSUMF03");
        assert!(decode_unit_manifest(&relabelled, 128 * 1024).is_err());
    }
    let good = set("one\ntwo\nthree\n", &descriptor(), binding());
    let bytes = canonicalize_unit_manifest(good.manifest()).unwrap();
    assert!(decode_unit_manifest(bytes.as_slice(), 0).is_err());
    assert!(decode_unit_manifest(bytes.as_slice(), 1).is_err());
    let mut trailing = bytes.as_slice().to_vec();
    trailing.push(0);
    assert!(decode_unit_manifest(&trailing, 128 * 1024).is_err());
    let body = codec::body_value(&good.manifest().body).unwrap();
    let malformed = digest::object(vec![
        ("format", digest::text(UNIT_MANIFEST_FORMAT).unwrap()),
        ("version", search_contracts::CanonicalValue::U64(3)),
        ("body", body),
        (
            "digest",
            digest::bytes(good.manifest_digest().as_bytes()).unwrap(),
        ),
        ("extra", search_contracts::CanonicalValue::Null),
    ])
    .unwrap();
    let bytes = search_contracts::to_canonical_cbor(&malformed).unwrap();
    assert_eq!(
        decode_unit_manifest(bytes.as_slice(), 128 * 1024),
        Err(UnitizationError::UnitManifestIncomplete)
    );
}

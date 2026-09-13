use super::*;
use crate::MaterializationError;
use crate::decode::{DecodedRepresentation, StepCounter, decode_text_or_code, detect_or_validate_encoding};
use crate::normalize::{CanonicalRepresentation, normalize_representation};
use crate::profile::{
    SourceEncoding, ValidatedMaterializerProfile, baseline_profile_descriptor,
    validate_materializer_profile,
};
use crate::request::{
    CancellationToken, DEFAULT_MATERIALIZATION_BUDGET, MaterializationBudget,
};
use search_contracts::{NonZeroRevision, OpaqueId};

fn profile() -> ValidatedMaterializerProfile {
    validate_materializer_profile(&baseline_profile_descriptor("maps-test", 1)).expect("profile")
}

fn identities(profile: &ValidatedMaterializerProfile) -> MapIdentities {
    MapIdentities {
        source_id: OpaqueId::new("source:test").expect("source"),
        revision: NonZeroRevision::new(1).expect("revision"),
        profile_id: profile.id(),
    }
}

fn decoded(
    bytes: &[u8],
    encoding: SourceEncoding,
    profile: &ValidatedMaterializerProfile,
) -> DecodedRepresentation {
    let decision = detect_or_validate_encoding(bytes, encoding, profile).expect("decision");
    decode_text_or_code(
        bytes,
        &decision,
        profile,
        &DEFAULT_MATERIALIZATION_BUDGET,
        &mut StepCounter::new(1 << 20),
        CancellationToken::never(),
    )
    .expect("decode")
}

fn bundle_for(
    bytes: &[u8],
    encoding: SourceEncoding,
    profile: &ValidatedMaterializerProfile,
) -> (CanonicalRepresentation, MapBundle) {
    let text = decoded(bytes, encoding, profile);
    let canonical = normalize_representation(
        &text,
        profile,
        &DEFAULT_MATERIALIZATION_BUDGET,
        &mut StepCounter::new(1 << 20),
        CancellationToken::never(),
    )
    .expect("normalize");
    let coordinate_map = build_coordinate_map(
        &text,
        &canonical,
        &identities(profile),
        profile,
        &DEFAULT_MATERIALIZATION_BUDGET,
        &mut StepCounter::new(1 << 20),
        CancellationToken::never(),
    )
    .expect("coordinate map");
    let loss_map = build_loss_map(
        &text,
        &canonical,
        &identities(profile),
        profile,
        &DEFAULT_MATERIALIZATION_BUDGET,
        &mut StepCounter::new(1 << 20),
        CancellationToken::never(),
    )
    .expect("loss map");
    (canonical, MapBundle::from_parts(coordinate_map, loss_map))
}

#[test]
fn exact_ascii_collapses_to_one_segment() {
    let profile = profile();
    let (canonical, bundle) = bundle_for(b"hello\nworld\n", SourceEncoding::Utf8, &profile);
    assert_eq!(bundle.coordinate_map().segments().len(), 1);
    assert_eq!(
        bundle.coordinate_map().segments()[0].relation,
        SegmentRelation::Exact
    );
    assert!(bundle.loss_map().records().is_empty());
    let receipt = validate_map_bundle(&canonical, &bundle, &profile).expect("valid");
    assert_eq!(receipt.coordinate_segments(), 1);
    assert_eq!(
        receipt.assurance(),
        crate::assurance::AssuranceCeiling::ExactBytes
    );
}

#[test]
fn bom_yields_unmapped_segment_and_loss() {
    let profile = profile();
    let mut bytes = vec![0xEF, 0xBB, 0xBF];
    bytes.extend_from_slice(b"ab\n");
    let (canonical, bundle) = bundle_for(&bytes, SourceEncoding::Utf8, &profile);
    assert_eq!(
        bundle.coordinate_map().segments()[0].relation,
        SegmentRelation::Unmapped
    );
    assert_eq!(bundle.coordinate_map().segments()[0].native_end, 3);
    assert_eq!(bundle.loss_map().records().len(), 1);
    validate_map_bundle(&canonical, &bundle, &profile).expect("valid");
}

#[test]
fn transcoded_lines_are_ranges_with_evidence() {
    let profile = profile();
    let mut bytes = Vec::new();
    for unit in "Aπ\n".encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    let (canonical, bundle) = bundle_for(&bytes, SourceEncoding::Utf16Le, &profile);
    assert!(
        bundle
            .coordinate_map()
            .segments()
            .iter()
            .any(|segment| segment.relation == SegmentRelation::Range)
    );
    assert!(
        bundle
            .loss_map()
            .records()
            .iter()
            .any(|record| record.kind == LossKind::TranscodedEncoding)
    );
    let receipt = validate_map_bundle(&canonical, &bundle, &profile).expect("valid");
    assert_eq!(
        receipt.assurance(),
        crate::assurance::AssuranceCeiling::ExactTranscoded
    );
}

#[test]
fn bom_only_input_maps_to_unmapped_only() {
    let profile = profile();
    let (canonical, bundle) = bundle_for(b"\xEF\xBB\xBF", SourceEncoding::Utf8, &profile);
    assert_eq!(canonical.text(), "");
    assert_eq!(bundle.coordinate_map().segments().len(), 1);
    assert_eq!(
        bundle.coordinate_map().segments()[0].relation,
        SegmentRelation::Unmapped
    );
    assert_eq!(bundle.loss_map().records().len(), 1);
    let receipt = validate_map_bundle(&canonical, &bundle, &profile).expect("valid");
    assert_eq!(
        receipt.assurance(),
        crate::assurance::AssuranceCeiling::NormalizedWithRecordedLoss
    );
}

#[test]
fn gapped_coverage_is_rejected() {
    let profile = profile();
    let (canonical, mut bundle) = bundle_for(b"ab\ncd\n", SourceEncoding::Utf8, &profile);
    bundle.coordinate_map.segments.clear();
    assert_eq!(
        validate_map_bundle(&canonical, &bundle, &profile),
        Err(MaterializationError::CoordinateMapInvalid)
    );
}

#[test]
fn phantom_loss_is_rejected() {
    let profile = profile();
    let (canonical, mut bundle) = bundle_for(b"ab\n", SourceEncoding::Utf8, &profile);
    bundle.loss_map.records.push(LossRecord {
        kind: LossKind::RemovedBom,
        native_start: 0,
        native_end: 0,
        decoded_start: 0,
        decoded_end: 0,
        canonical_start: 0,
        canonical_end: 0,
    });
    assert_eq!(
        validate_map_bundle(&canonical, &bundle, &profile),
        Err(MaterializationError::LossMapInvalid)
    );
}

#[test]
fn foreign_profile_binding_is_rejected() {
    let profile = profile();
    let other = validate_materializer_profile(&baseline_profile_descriptor("other-maps", 1))
        .expect("other");
    let (canonical, bundle) = bundle_for(b"ab\n", SourceEncoding::Utf8, &profile);
    assert_eq!(
        validate_map_bundle(&canonical, &bundle, &other),
        Err(MaterializationError::ProfileMismatch)
    );
}

#[test]
fn tiny_segment_budget_is_exhausted() {
    let profile = profile();
    let text = decoded(b"\xEF\xBB\xBFa\nb\n", SourceEncoding::Utf8, &profile);
    let canonical = normalize_representation(
        &text,
        &profile,
        &DEFAULT_MATERIALIZATION_BUDGET,
        &mut StepCounter::new(1 << 20),
        CancellationToken::never(),
    )
    .expect("normalize");
    let budget = MaterializationBudget {
        max_map_segments: 1,
        ..DEFAULT_MATERIALIZATION_BUDGET
    };
    assert_eq!(
        build_coordinate_map(
            &text,
            &canonical,
            &identities(&profile),
            &profile,
            &budget,
            &mut StepCounter::new(1 << 20),
            CancellationToken::never()
        ),
        Err(MaterializationError::BudgetExhausted)
    );
}

#[test]
fn baseline_never_produces_ambiguous_segments() {
    let profile = profile();
    for bytes in [
        b"a\n".as_slice(),
        "αβ\n".as_bytes(),
        b"\xEF\xBB\xBFq\n".as_slice(),
    ] {
        let (_, bundle) = bundle_for(bytes, SourceEncoding::Utf8, &profile);
        assert!(
            bundle
                .coordinate_map()
                .segments()
                .iter()
                .all(|segment| segment.relation != SegmentRelation::Ambiguous)
        );
    }
}

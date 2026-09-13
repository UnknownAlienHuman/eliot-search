use super::*;

struct ToyDigest;

impl LegacyDirectRepresentationDigest for ToyDigest {
    fn digest_parts(domain: &[u8], parts: &[&[u8]]) -> [u8; 32] {
        let mut output = [0_u8; 32];
        for (index, byte) in domain
            .iter()
            .chain(parts.iter().flat_map(|part| part.iter()))
            .enumerate()
        {
            let slot = index % output.len();
            output[slot] = output[slot]
                .wrapping_add(*byte)
                .rotate_left(u32::try_from(index % 8).expect("rotation below eight"));
        }
        output
    }
}

fn binding() -> LegacyDirectPreparationBinding {
    LegacyDirectPreparationBinding {
        namespace: [1; 32],
        source_id: [2; 32],
        revision_id: [3; 32],
        content_digest: [4; 32],
        byte_length: 5,
        materializer_digest: [6; 32],
        unitizer_digest: [7; 32],
    }
}

#[test]
fn layout_and_gap_frames_round_trip_exactly() {
    let layout = encode_legacy_direct_layout(b"layout").expect("layout");
    let frame = decode_legacy_direct_preparation(&layout).expect("decode");
    assert_eq!(frame, LegacyDirectPreparationFrame::Layout(b"layout"));
    assert_eq!(frame.identity_marker(), b"layout");
    assert_eq!(frame.gap_reason(), None);

    for gap in [
        LegacyDirectPreparationGap::RevisionNotUtf8,
        LegacyDirectPreparationGap::BinaryContent,
        LegacyDirectPreparationGap::TooManyLines,
        LegacyDirectPreparationGap::TooManyUnits,
        LegacyDirectPreparationGap::LayoutTooLarge,
        LegacyDirectPreparationGap::RevisionHasBom,
    ] {
        let encoded = encode_legacy_direct_gap(gap);
        let decoded = decode_legacy_direct_preparation(&encoded).expect("gap");
        assert_eq!(decoded, LegacyDirectPreparationFrame::Gap(gap));
        assert_eq!(decoded.identity_marker(), gap.reason().as_bytes());
        assert_eq!(decoded.gap_reason(), Some(gap.reason()));
    }
}

#[test]
fn malformed_frames_fail_closed() {
    for encoded in [Vec::new(), vec![0], vec![7], vec![1, 0], vec![6, 6]] {
        assert_eq!(
            decode_legacy_direct_preparation(&encoded),
            Err(LegacyDirectPreparationError::InvalidFrame)
        );
    }
    assert_eq!(
        encode_legacy_direct_layout(&[]),
        Err(LegacyDirectPreparationError::InvalidFrame)
    );
}

#[test]
fn representation_identity_binds_every_field_and_marker() {
    let base = binding();
    let first = derive_legacy_direct_representation_id::<ToyDigest>(&base, b"layout");
    assert_eq!(
        first,
        derive_legacy_direct_representation_id::<ToyDigest>(&base, b"layout")
    );
    let mut changed = base;
    changed.revision_id = [9; 32];
    assert_ne!(
        first,
        derive_legacy_direct_representation_id::<ToyDigest>(&changed, b"layout")
    );
    assert_ne!(
        first,
        derive_legacy_direct_representation_id::<ToyDigest>(&base, b"other")
    );
    assert!(verify_legacy_direct_representation::<ToyDigest>(&first, &base, b"layout").is_ok());
    assert_eq!(
        verify_legacy_direct_representation::<ToyDigest>(&first, &base, b"other"),
        Err(LegacyDirectPreparationError::InvalidFrame)
    );
}

#[test]
fn digest_algorithm_tags_remain_explicit_and_distinct() {
    assert_eq!(CONTENT_DIGEST_ALGORITHM, DIGEST_ALGORITHM_SHA256);
    assert_eq!(REPRESENTATION_DIGEST_ALGORITHM, DIGEST_ALGORITHM_BLAKE3_256);
    assert_eq!(MANIFEST_DIGEST_ALGORITHM, DIGEST_ALGORITHM_SHA256);
    assert_ne!(REPRESENTATION_DIGEST_ALGORITHM, CONTENT_DIGEST_ALGORITHM);
}

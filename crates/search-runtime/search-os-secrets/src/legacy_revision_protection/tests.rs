use super::*;

struct TestDigest;

impl LegacyRevisionContentDigest for TestDigest {
    fn digest(bytes: &[u8]) -> [u8; 32] {
        let mut output = [0_u8; 32];
        for (index, byte) in bytes.iter().enumerate() {
            let slot = index % output.len();
            output[slot] = output[slot]
                .wrapping_add(*byte)
                .wrapping_add(u8::try_from(index & 0xff).expect("bounded"));
        }
        output[31] ^= u8::try_from(bytes.len() & 0xff).expect("bounded");
        output
    }
}

fn binding(plaintext: &[u8]) -> LegacyRevisionBinding {
    LegacyRevisionBinding::new(
        LegacyRevisionExpected::new(
            [0x11; 32],
            [0x33; 32],
            TestDigest::digest(plaintext),
            u64::try_from(plaintext.len()).expect("test length"),
        ),
        [0x22; 32],
    )
}

#[test]
fn frozen_inner_and_outer_layout_round_trip() {
    let bound = binding(b"abc");
    let inner = encode_legacy_revision_inner(bound, b"abc").expect("inner");
    assert_eq!(inner.len(), LEGACY_REVISION_INNER_HEADER_BYTES + 3);
    assert_eq!(&inner[..8], b"ELSIN2\0\0");
    assert_eq!(&inner[8..12], &1_u32.to_be_bytes());
    assert_eq!(&inner[12..44], &[0x11; 32]);
    assert_eq!(&inner[44..76], &[0x22; 32]);
    assert_eq!(&inner[76..108], &[0x33; 32]);
    assert_eq!(&inner[108..140], &TestDigest::digest(b"abc"));
    assert_eq!(&inner[140..148], &3_u64.to_be_bytes());
    assert_eq!(
        decode_legacy_revision_inner::<TestDigest>(&inner, bound).expect("decode"),
        b"abc"
    );

    let outer = encode_legacy_revision_outer(bound, b"protected").expect("outer");
    assert_eq!(outer.len(), LEGACY_REVISION_OUTER_HEADER_BYTES + 9);
    assert_eq!(&outer[..8], b"ELSRV2\0\0");
    assert_eq!(&outer[148..156], &9_u64.to_be_bytes());
    let (observed, payload) = decode_legacy_revision_outer(
        &outer,
        bound.revision(),
        Some(bound.key_binding_digest()),
    )
    .expect("decode outer");
    assert_eq!(observed, bound);
    assert_eq!(payload, b"protected");
}

#[test]
fn every_truncated_envelope_fails_without_panicking() {
    let bound = binding(b"abc");
    let inner = encode_legacy_revision_inner(bound, b"abc").expect("inner");
    for end in 0..inner.len() {
        assert!(decode_legacy_revision_inner::<TestDigest>(&inner[..end], bound).is_err());
    }

    let outer = encode_legacy_revision_outer(bound, b"protected").expect("outer");
    for end in 0..outer.len() {
        assert!(
            decode_legacy_revision_outer(
                &outer[..end],
                bound.revision(),
                Some(bound.key_binding_digest()),
            )
            .is_err()
        );
    }
}

#[test]
fn every_binding_dimension_is_checked() {
    let bound = binding(b"abc");
    let object = encode_legacy_revision_outer(bound, b"protected").expect("outer");

    let expected = LegacyRevisionExpected::new(
        [0x99; 32],
        bound.revision().revision_id(),
        bound.revision().content_digest(),
        bound.revision().plaintext_len(),
    );
    assert_eq!(
        decode_legacy_revision_outer(&object, expected, Some(bound.key_binding_digest())),
        Err(LegacyRevisionEnvelopeError::NamespaceMismatch)
    );

    let mut wrong_key = bound.key_binding_digest();
    wrong_key[0] ^= 1;
    assert_eq!(
        decode_legacy_revision_outer(&object, bound.revision(), Some(wrong_key)),
        Err(LegacyRevisionEnvelopeError::KeyBindingMismatch)
    );

    for expected in [
        LegacyRevisionExpected::new(
            bound.revision().namespace_id(),
            [0x77; 32],
            bound.revision().content_digest(),
            bound.revision().plaintext_len(),
        ),
        LegacyRevisionExpected::new(
            bound.revision().namespace_id(),
            bound.revision().revision_id(),
            [0x88; 32],
            bound.revision().plaintext_len(),
        ),
        LegacyRevisionExpected::new(
            bound.revision().namespace_id(),
            bound.revision().revision_id(),
            bound.revision().content_digest(),
            bound.revision().plaintext_len() + 1,
        ),
    ] {
        assert_eq!(
            decode_legacy_revision_outer(
                &object,
                expected,
                Some(bound.key_binding_digest()),
            ),
            Err(LegacyRevisionEnvelopeError::EnvelopeBindingMismatch)
        );
    }
}

#[test]
fn inner_identity_length_and_content_are_checked() {
    let bound = binding(b"abc");
    let canonical = encode_legacy_revision_inner(bound, b"abc").expect("inner");
    for offset in [12, 44, 76, 108, 140] {
        let mut inner = canonical.clone();
        inner[offset] ^= 1;
        assert_eq!(
            decode_legacy_revision_inner::<TestDigest>(&inner, bound),
            Err(LegacyRevisionEnvelopeError::InnerBindingMismatch),
            "offset {offset}"
        );
    }

    assert_eq!(
        encode_legacy_revision_inner(bound, b"ab"),
        Err(LegacyRevisionEnvelopeError::LengthMismatch)
    );

    let mut inner = encode_legacy_revision_inner(bound, b"abc").expect("inner");
    *inner.last_mut().expect("plaintext") ^= 1;
    assert_eq!(
        decode_legacy_revision_inner::<TestDigest>(&inner, bound),
        Err(LegacyRevisionEnvelopeError::ContentMismatch)
    );
}

#[test]
fn malformed_payload_and_marker_fail_closed() {
    let bound = binding(b"abc");
    assert_eq!(
        encode_legacy_revision_outer(bound, &[]),
        Err(LegacyRevisionEnvelopeError::ProtectedPayloadInvalid)
    );
    assert!(!legacy_revision_is_protected_object(b"ELSRV2"));
    let object = encode_legacy_revision_outer(bound, b"protected").expect("outer");
    assert!(legacy_revision_is_protected_object(&object));

    for offset in [0, 8] {
        let mut changed = object.clone();
        changed[offset] ^= 1;
        assert_eq!(
            decode_legacy_revision_outer(
                &changed,
                bound.revision(),
                Some(bound.key_binding_digest()),
            ),
            Err(LegacyRevisionEnvelopeError::EnvelopeInvalid)
        );
    }

    let mut trailing = object;
    trailing.push(0);
    assert_eq!(
        decode_legacy_revision_outer(
            &trailing,
            bound.revision(),
            Some(bound.key_binding_digest()),
        ),
        Err(LegacyRevisionEnvelopeError::EnvelopeInvalid)
    );

    let oversized_binding = LegacyRevisionBinding::new(
        LegacyRevisionExpected::new(
            [0x11; 32],
            [0x33; 32],
            [0x44; 32],
            LEGACY_REVISION_MAX_PLAINTEXT_LENGTH + 1,
        ),
        [0x22; 32],
    );
    assert_eq!(
        encode_legacy_revision_outer(oversized_binding, b"protected"),
        Err(LegacyRevisionEnvelopeError::PlaintextTooLarge)
    );
}

//! Composition tests never claim that synthetic ciphertext is authenticated.
//! Native tests invoke the existing qualified Windows DPAPI adapter.

use search_os_secrets::{
    LegacyRevisionBinding, LegacyRevisionContentDigest,
    LegacyRevisionEnvelopeError, LegacyRevisionExpected,
    decode_legacy_revision_inner,
    encode_legacy_revision_inner, encode_legacy_revision_outer,
};
#[cfg(windows)]
use zeroize::Zeroizing;

use super::*;

struct TestDigest;

impl LegacyRevisionContentDigest for TestDigest {
    fn digest(bytes: &[u8]) -> [u8; 32] {
        sha256::digest(bytes)
    }
}

fn protector() -> RevisionProtector {
    RevisionProtector {
        namespace_id: [0x11; 32],
        #[cfg(windows)]
        key_binding_digest: [0x22; 32],
        #[cfg(windows)]
        entropy: [0x44; 32],
    }
}

fn binding(plaintext: &[u8]) -> LegacyRevisionBinding {
    LegacyRevisionBinding::new(
        LegacyRevisionExpected::new(
            [0x11; 32],
            [0x33; 32],
            sha256::digest(plaintext),
            u64::try_from(plaintext.len()).expect("test length"),
        ),
        [0x22; 32],
    )
}

#[test]
fn plaintext_with_the_exact_expected_hash_and_length_is_never_protected_readback() {
    let protector = protector();
    for bytes in [
        b"private-source-sentinel".as_slice(),
        b"",
        b"ELSRV2",
        b"\xff\0",
    ] {
        assert_eq!(
            protector.unprotect(
                bytes,
                &"33".repeat(32),
                &sha256::hex(&sha256::digest(bytes)),
                u64::try_from(bytes.len()).expect("test length"),
            ),
            Err("DIRECT_REVISION_PROTECTED_FORMAT_REQUIRED".to_owned())
        );
    }
}

#[test]
fn plaintext_digest_mismatch_is_rejected_before_encryption() {
    assert_eq!(
        protector().protect(&"33".repeat(32), &"00".repeat(32), b"abc"),
        Err("DIRECT_REVISION_CONTENT_MISMATCH".to_owned())
    );
}

#[test]
fn package_owned_inner_validation_preserves_direct_digest_semantics() {
    let bound = binding(b"abc");
    let mut inner = encode_legacy_revision_inner(bound, b"abc").expect("inner");
    assert_eq!(
        decode_legacy_revision_inner::<TestDigest>(&inner, bound).expect("decode"),
        b"abc"
    );
    *inner.last_mut().expect("plaintext") ^= 1;
    assert_eq!(
        decode_legacy_revision_inner::<TestDigest>(&inner, bound),
        Err(LegacyRevisionEnvelopeError::ContentMismatch)
    );
}

#[cfg(not(windows))]
#[test]
fn valid_envelope_on_an_unsupported_platform_never_returns_ciphertext_as_plaintext() {
    let bound = binding(b"abc");
    let object = encode_legacy_revision_outer(bound, b"format-only").expect("outer");
    assert_eq!(
        protector().unprotect(
            &object,
            &"33".repeat(32),
            &sha256::hex(&bound.revision().content_digest()),
            3,
        ),
        Err("DIRECT_REVISION_ENCRYPTION_UNAVAILABLE".to_owned())
    );
    // The explicit development writer remains separate from protected decoding.
    assert_eq!(
        protector()
            .protect(
                &"33".repeat(32),
                &sha256::hex(&bound.revision().content_digest()),
                b"abc",
            )
            .expect("development bytes"),
        b"abc"
    );
}

#[cfg(windows)]
#[test]
fn real_dpapi_round_trip_preserves_empty_unicode_and_binary_bytes() {
    let protector = protector();
    for bytes in [
        b"".as_slice(),
        "alpha\r\nβeta".as_bytes(),
        b"\0\xff\x01",
    ] {
        let digest = sha256::hex(&sha256::digest(bytes));
        let object = protector
            .protect(&"33".repeat(32), &digest, bytes)
            .expect("protect");
        assert!(RevisionProtector::is_protected_object(&object));
        assert_eq!(
            protector
                .unprotect(
                    &object,
                    &"33".repeat(32),
                    &digest,
                    u64::try_from(bytes.len()).expect("test length"),
                )
                .expect("unprotect"),
            bytes
        );
    }
}

#[cfg(windows)]
#[test]
fn real_dpapi_rejects_changed_entropy_and_ciphertext() {
    let first = protector();
    let digest = sha256::hex(&sha256::digest(b"private-sentinel"));
    let object = first
        .protect(&"33".repeat(32), &digest, b"private-sentinel")
        .expect("protect");
    let mut wrong_key = protector();
    wrong_key.entropy[0] ^= 1;
    assert!(
        wrong_key
            .unprotect(&object, &"33".repeat(32), &digest, 16)
            .is_err()
    );
    let mut altered = object;
    *altered.last_mut().expect("ciphertext") ^= 1;
    assert!(
        first
            .unprotect(&altered, &"33".repeat(32), &digest, 16)
            .is_err()
    );
}

#[cfg(windows)]
#[test]
fn a_real_authenticated_payload_with_different_inner_identity_is_rejected() {
    let protector = protector();
    let outer_binding = binding(b"abc");
    let wrong_binding = LegacyRevisionBinding::new(
        LegacyRevisionExpected::new(
            [0x11; 32],
            [0x77; 32],
            sha256::digest(b"abc"),
            3,
        ),
        [0x22; 32],
    );
    let mut wrong_inner = Zeroizing::new(
        encode_legacy_revision_inner(wrong_binding, b"abc").expect("inner"),
    );
    let ciphertext = windows::protect_data(&mut wrong_inner, &protector.entropy)
        .expect("protect inner");
    let object = encode_legacy_revision_outer(outer_binding, &ciphertext)
        .expect("outer");
    assert_eq!(
        protector.unprotect(
            &object,
            &"33".repeat(32),
            &sha256::hex(&outer_binding.revision().content_digest()),
            3,
        ),
        Err("DIRECT_REVISION_INNER_BINDING_MISMATCH".to_owned())
    );
}

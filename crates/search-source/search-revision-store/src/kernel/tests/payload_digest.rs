use search_contracts::{Blake3Digest32, NonZeroRevision, OpaqueId};

use super::super::*;

fn construct(
    plaintext_bytes: u64,
    nonce: Vec<u8>,
    ciphertext: Vec<u8>,
    limits: RevisionStoreLimits,
) -> Result<EncryptedRevisionPayload, RevisionStoreError> {
    EncryptedRevisionPayload::new(
        Blake3Digest32::from_bytes([7; 32]),
        plaintext_bytes,
        nonce,
        ciphertext,
        EncryptionBinding {
            key_reference: OpaqueId::new("secret:revision-key").expect("key"),
            key_version: NonZeroRevision::new(1).expect("version"),
            cipher_suite: CipherSuite::AuthenticatedEncryptionV1,
        },
        limits,
    )
}

#[test]
fn owned_ciphertext_matches_published_raw_vectors_and_clone_stays_consistent() {
    // Published BLAKE3 1.8.2 test_vectors/test_vectors.json, also used by #237/#582.
    // input[i] = i % 251; both inputs exceed the 16-byte authentication-tag floor.
    for (length, expected) in [
        (
            1024,
            "42214739f095a406f3fc83deb889744ac00df831c10daa55189b5d121c855af7",
        ),
        (
            102_400,
            "bc3e3d41a1146b069abffad3c0d44860cf664390afce4d9661f7902e7943e085",
        ),
    ] {
        let mut bytes: Vec<u8> = (0..length)
            .map(|index| u8::try_from(index % 251).expect("vector byte"))
            .collect();
        let payload = construct(3, vec![9; 12], bytes.clone(), DEFAULT_REVISION_STORE_LIMITS)
            .expect("published vector payload");
        let cloned = payload.clone();
        let expected = Blake3Digest32::parse_hex(expected).expect("published digest");
        assert_eq!(payload.ciphertext_digest(), expected);
        assert_eq!(payload.ciphertext(), bytes.as_slice());
        assert_eq!(cloned, payload);
        assert_eq!(cloned.ciphertext_digest(), expected);
        assert_eq!(cloned.ciphertext_len(), length);

        bytes[0] ^= 1;
        let changed = construct(3, vec![9; 12], bytes, DEFAULT_REVISION_STORE_LIMITS)
            .expect("changed payload");
        assert_ne!(changed.ciphertext_digest(), expected);
        assert_ne!(changed.ciphertext(), payload.ciphertext());
        assert_eq!(cloned.ciphertext_digest(), expected);
    }
}

#[test]
fn digest_ownership_preserves_validation_precedence_and_custom_ciphertext_budget() {
    let limits = RevisionStoreLimits {
        max_plaintext_bytes: 8,
        max_ciphertext_bytes: 16,
        max_nonce_bytes: 12,
        ..DEFAULT_REVISION_STORE_LIMITS
    };
    let invalid = RevisionStoreLimits {
        max_operations: 0,
        ..limits
    };
    for (limits, plaintext, ciphertext_len, nonce_len, expected) in [
        (invalid, 0, 0, 0, RevisionStoreError::InvalidLimits),
        (limits, 0, 15, 0, RevisionStoreError::PlaintextSizeInvalid),
        (limits, 9, 15, 0, RevisionStoreError::PlaintextSizeInvalid),
        (limits, 3, 15, 0, RevisionStoreError::CiphertextSizeInvalid),
        (limits, 3, 17, 0, RevisionStoreError::CiphertextSizeInvalid),
        (limits, 3, 16, 0, RevisionStoreError::NonceInvalid),
        (limits, 3, 16, 13, RevisionStoreError::NonceInvalid),
    ] {
        assert_eq!(
            construct(
                plaintext,
                vec![9; nonce_len],
                vec![7; ciphertext_len],
                limits
            ),
            Err(expected)
        );
    }

    // The caller's finite ciphertext budget may exceed the separate Source 16 MiB profile.
    let length = 17 * 1024 * 1024;
    let custom = RevisionStoreLimits {
        max_plaintext_bytes: 3,
        max_ciphertext_bytes: u64::try_from(length).expect("custom byte budget"),
        ..DEFAULT_REVISION_STORE_LIMITS
    };
    let payload = construct(3, vec![9; 12], vec![0x5a; length], custom)
        .expect("17 MiB ciphertext within its custom budget");
    assert_eq!(payload.ciphertext_len(), length);
    assert_eq!(payload.ciphertext().len(), length);
}

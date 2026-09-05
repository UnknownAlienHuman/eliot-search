//! Envelope tests never claim that synthetic ciphertext is authenticated.
//! Native tests below invoke the real Windows DPAPI with test-only entropy;
//! they create no Credential Manager entries and introduce no product fault switch.

use super::*;

fn protector() -> RevisionProtector {
    RevisionProtector {
        namespace_id: [0x11; 32],
        #[cfg(windows)]
        key_binding_digest: [0x22; 32],
        #[cfg(windows)]
        entropy: [0x44; 32],
    }
}

fn binding(plaintext: &[u8]) -> Binding {
    Binding {
        revision: ExpectedRevision {
            namespace_id: [0x11; 32], revision_id: [0x33; 32],
            content_digest: sha256::digest(plaintext), plaintext_len: plaintext.len() as u64,
        },
        key_binding_digest: [0x22; 32],
    }
}

// Independent legacy v1 field ordering, rather than the new encode_binding helper.
fn old_header(magic: &[u8; 8], bound: Binding) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(magic);
    bytes.extend_from_slice(&1_u32.to_be_bytes());
    bytes.extend_from_slice(&bound.revision.namespace_id);
    bytes.extend_from_slice(&bound.key_binding_digest);
    bytes.extend_from_slice(&bound.revision.revision_id);
    bytes.extend_from_slice(&bound.revision.content_digest);
    bytes.extend_from_slice(&bound.revision.plaintext_len.to_be_bytes());
    bytes
}

fn outer(bound: Binding, payload: &[u8]) -> Vec<u8> {
    let mut bytes = old_header(&OUTER_MAGIC, bound);
    bytes.extend_from_slice(&(payload.len() as u64).to_be_bytes());
    bytes.extend_from_slice(payload);
    bytes
}

fn inner(bound: Binding, plaintext: &[u8]) -> Vec<u8> {
    let mut bytes = old_header(&INNER_MAGIC, bound);
    bytes.extend_from_slice(plaintext);
    bytes
}

#[test]
fn plaintext_with_the_exact_expected_hash_and_length_is_never_protected_readback() {
    let protector = protector();
    for bytes in [b"private-source-sentinel".as_slice(), b"", b"ELSRV2", b"\xff\0"] {
        assert_eq!(protector.unprotect(bytes, &"33".repeat(32),
            &sha256::hex(&sha256::digest(bytes)), bytes.len() as u64),
            Err("DIRECT_REVISION_PROTECTED_FORMAT_REQUIRED".to_owned()));
    }
}

#[test]
fn plaintext_digest_mismatch_is_rejected_before_encryption() {
    assert_eq!(protector().protect(&"33".repeat(32), &"00".repeat(32), b"abc"),
        Err("DIRECT_REVISION_CONTENT_MISMATCH".to_owned()));
}

#[test]
fn legacy_v1_envelope_ordering_is_unchanged() {
    let bound = binding(b"abc");
    let outer = outer(bound, b"synthetic-format-only");
    let (decoded, ciphertext) = decode_outer(&outer, bound.revision, Some(bound.key_binding_digest)).unwrap();
    assert_eq!(OUTER_HEADER_BYTES, 156);
    assert_eq!(INNER_HEADER_BYTES, 148);
    assert_eq!(decoded, bound);
    assert_eq!(ciphertext, b"synthetic-format-only");
    let legacy_inner = inner(bound, b"abc");
    let borrowed = decode_inner(&legacy_inner, bound).unwrap();
    assert_eq!(borrowed, b"abc");
    assert_eq!(borrowed.as_ptr(), legacy_inner[INNER_HEADER_BYTES..].as_ptr());
    let mut new_fields = Vec::new();
    encode_binding(&mut new_fields, bound);
    assert_eq!(new_fields, legacy_inner[12..INNER_HEADER_BYTES]);
}

#[test]
fn every_truncated_outer_envelope_fails_without_a_panic() {
    let bound = binding(b"abc");
    let object = outer(bound, b"synthetic-format-only");
    for end in 0..object.len() {
        assert!(decode_outer(&object[..end], bound.revision, Some(bound.key_binding_digest)).is_err(), "{end}");
    }
}

#[test]
fn every_truncated_inner_envelope_fails_without_a_panic() {
    let bound = binding(b"abc");
    let object = inner(bound, b"abc");
    for end in 0..object.len() {
        assert!(decode_inner(&object[..end], bound).is_err(), "{end}");
    }
}

#[test]
fn each_outer_binding_and_version_field_is_checked() {
    let bound = binding(b"abc");
    let object = outer(bound, b"format-only");
    // Magic, version, namespace, key, revision, digest, length, ciphertext length.
    for offset in [0, 8, 12, 44, 76, 108, 140, 148] {
        let mut changed = object.clone();
        changed[offset] ^= 1;
        assert!(decode_outer(&changed, bound.revision, Some(bound.key_binding_digest)).is_err(), "{offset}");
    }
}

#[test]
fn each_inner_binding_and_content_field_is_checked() {
    let bound = binding(b"abc");
    let object = inner(bound, b"abc");
    for offset in [0, 8, 12, 44, 76, 108, 140, 148] {
        let mut changed = object.clone();
        changed[offset] ^= 1;
        assert!(decode_inner(&changed, bound).is_err(), "{offset}");
    }
}

#[test]
fn zero_ciphertext_trailing_bytes_and_oversized_declared_plaintext_are_rejected() {
    let bound = binding(b"abc");
    assert!(decode_outer(&outer(bound, &[]), bound.revision, Some(bound.key_binding_digest)).is_err());
    let mut object = outer(bound, b"format-only");
    object.push(0);
    assert!(decode_outer(&object, bound.revision, Some(bound.key_binding_digest)).is_err());
    let mut inner = inner(bound, b"abc");
    inner.push(0);
    assert!(decode_inner(&inner, bound).is_err());
    assert_eq!(protector().unprotect(&object, &"33".repeat(32), &sha256::hex(&bound.revision.content_digest),
        MAX_PLAINTEXT_BYTES as u64 + 1), Err("DIRECT_REVISION_PLAINTEXT_TOO_LARGE".to_owned()));
}

#[cfg(not(windows))]
#[test]
fn valid_envelope_on_an_unsupported_platform_never_returns_ciphertext_as_plaintext() {
    let bound = binding(b"abc");
    assert_eq!(protector().unprotect(&outer(bound, b"format-only"), &"33".repeat(32),
        &sha256::hex(&bound.revision.content_digest), 3),
        Err("DIRECT_REVISION_ENCRYPTION_UNAVAILABLE".to_owned()));
    // The explicit development writer remains separate from protected decoding.
    assert_eq!(protector().protect(&"33".repeat(32), &sha256::hex(&bound.revision.content_digest), b"abc").unwrap(), b"abc");
}

#[cfg(windows)]
#[test]
fn real_dpapi_round_trip_preserves_empty_unicode_and_binary_bytes() {
    let protector = protector();
    for bytes in [b"".as_slice(), "alpha\r\nβeta".as_bytes(), b"\0\xff\x01"] {
        let digest = sha256::hex(&sha256::digest(bytes));
        let object = protector.protect(&"33".repeat(32), &digest, bytes).unwrap();
        assert!(RevisionProtector::is_protected_object(&object));
        assert_eq!(protector.unprotect(&object, &"33".repeat(32), &digest, bytes.len() as u64).unwrap(), bytes);
    }
}

#[cfg(windows)]
#[test]
fn real_dpapi_rejects_changed_entropy_and_ciphertext() {
    let first = protector();
    let digest = sha256::hex(&sha256::digest(b"private-sentinel"));
    let object = first.protect(&"33".repeat(32), &digest, b"private-sentinel").unwrap();
    let mut wrong_key = protector();
    wrong_key.entropy[0] ^= 1;
    assert!(wrong_key.unprotect(&object, &"33".repeat(32), &digest, 16).is_err());
    let mut altered = object;
    *altered.last_mut().unwrap() ^= 1;
    assert!(first.unprotect(&altered, &"33".repeat(32), &digest, 16).is_err());
}

#[cfg(windows)]
#[test]
fn a_real_authenticated_payload_with_different_inner_identity_is_rejected() {
    let protector = protector();
    let outer_binding = binding(b"abc");
    let mut wrong_binding = outer_binding;
    wrong_binding.revision.revision_id[0] ^= 1;
    let mut wrong_inner = Zeroizing::new(inner(wrong_binding, b"abc"));
    let ciphertext = windows::protect_data(&mut wrong_inner, &protector.entropy).unwrap();
    let object = outer(outer_binding, &ciphertext);
    assert_eq!(protector.unprotect(&object, &"33".repeat(32),
        &sha256::hex(&outer_binding.revision.content_digest), 3),
        Err("DIRECT_REVISION_INNER_BINDING_MISMATCH".to_owned()));
}

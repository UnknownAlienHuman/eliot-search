//! Focused causal tests for the selected-reference v2 AEAD primitive.
//!
//! These tests exercise the pure cryptographic boundary only. They do not
//! claim durable nonce uniqueness, lease enforcement, or access control.

use super::*;

/// Published BLAKE3 empty-input digest (RFC 7693 test vector).
const EMPTY_BLAKE3_HEX: &str =
    "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262";

fn test_key() -> [u8; REVISION_KEY_BYTES] {
    let mut key = [0u8; REVISION_KEY_BYTES];
    for (i, byte) in key.iter_mut().enumerate() {
        *byte = (i as u8).wrapping_mul(17).wrapping_add(3);
    }
    key
}

fn other_key() -> [u8; REVISION_KEY_BYTES] {
    let mut key = test_key();
    key[0] ^= 0xFF;
    key
}

fn valid_record(seed: u8) -> Vec<u8> {
    let value = V::Array(c::BoundedList::new(vec![
        V::U64(u64::from(seed)),
        V::Text(c::CanonicalText::new(format!("record-{seed}")).unwrap()),
    ]).unwrap());
    c::to_canonical_cbor(&value).unwrap().as_slice().to_vec()
}

fn valid_operation() -> OpaqueId {
    OpaqueId::new("op-test-0001").unwrap()
}

fn empty_blake3() -> Blake3Digest32 {
    Blake3Digest32::from_bytes(hex_to_bytes(EMPTY_BLAKE3_HEX))
}

fn hex_to_bytes(hex: &str) -> [u8; 32] {
    let mut out = [0u8; 32];
    for i in 0..32 {
        out[i] = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap();
    }
    out
}

fn make_binding(
    current: &[u8],
    occurrence: &[u8],
    residency: &[u8],
    operation: &OpaqueId,
    counter: u64,
    content_digest: Blake3Digest32,
    plaintext_length: u64,
) -> SelectedRevisionBindingV2 {
    SelectedRevisionBindingV2::from_owner_records(
        current,
        occurrence,
        residency,
        operation,
        counter,
        content_digest,
        plaintext_length,
    )
    .unwrap()
}

fn seal_open_roundtrip(plaintext: &[u8]) {
    let key = test_key();
    let digest = c::blake3_bounded_bytes(
        plaintext,
        c::Blake3ByteBudget::new(MAX_REVISION_PLAINTEXT_BYTES).unwrap(),
    )
    .unwrap();
    let binding = make_binding(
        &valid_record(1),
        &valid_record(2),
        &valid_record(3),
        &valid_operation(),
        0,
        digest,
        plaintext.len() as u64,
    );
    let envelope = seal_selected_revision_v2(&key, &binding, plaintext).unwrap();
    let opened = open_selected_revision_v2(&key, &binding, &envelope).unwrap();
    assert_eq!(opened.as_bytes(), plaintext);
}

#[test]
fn empty_plaintext_seal_open_roundtrip() {
    seal_open_roundtrip(&[]);
}

#[test]
fn nonempty_plaintext_seal_open_roundtrip() {
    seal_open_roundtrip(b"hello eliot search");
}

#[test]
fn encode_decode_roundtrip() {
    let key = test_key();
    let plaintext = b"roundtrip payload";
    let digest = c::blake3_bounded_bytes(
        plaintext,
        c::Blake3ByteBudget::new(MAX_REVISION_PLAINTEXT_BYTES).unwrap(),
    )
    .unwrap();
    let binding = make_binding(
        &valid_record(10),
        &valid_record(20),
        &valid_record(30),
        &OpaqueId::new("op-roundtrip").unwrap(),
        42,
        digest,
        plaintext.len() as u64,
    );
    let envelope = seal_selected_revision_v2(&key, &binding, plaintext).unwrap();
    let encoded = envelope.encode().unwrap();
    let decoded = SelectedRevisionEnvelopeV2::decode(&encoded).unwrap();
    assert_eq!(decoded.binding(), &binding);
    assert_eq!(decoded.ciphertext_and_tag(), envelope.ciphertext_and_tag());
    let opened = open_selected_revision_v2(&key, &binding, &decoded).unwrap();
    assert_eq!(opened.as_bytes(), plaintext);
}

#[test]
fn counter_zero_has_all_zero_nonce_and_is_valid() {
    let key = test_key();
    let plaintext = b"counter zero";
    let digest = c::blake3_bounded_bytes(
        plaintext,
        c::Blake3ByteBudget::new(MAX_REVISION_PLAINTEXT_BYTES).unwrap(),
    )
    .unwrap();
    let binding = make_binding(
        &valid_record(1),
        &valid_record(2),
        &valid_record(3),
        &valid_operation(),
        0,
        digest,
        plaintext.len() as u64,
    );
    assert_eq!(binding.nonce(), [0u8; REVISION_NONCE_BYTES]);
    let envelope = seal_selected_revision_v2(&key, &binding, plaintext).unwrap();
    let opened = open_selected_revision_v2(&key, &binding, &envelope).unwrap();
    assert_eq!(opened.as_bytes(), plaintext);
}

#[test]
fn canonical_aad_golden() {
    let current = valid_record(1);
    let occurrence = valid_record(2);
    let residency = valid_record(3);
    let operation = valid_operation();
    let counter = 7u64;
    let digest = empty_blake3();
    let plaintext_length = 0u64;
    let binding = make_binding(
        &current,
        &occurrence,
        &residency,
        &operation,
        counter,
        digest,
        plaintext_length,
    );
    let nonce = nonce(counter);
    let expected_value = V::Array(c::BoundedList::new(vec![
        V::Text(c::CanonicalText::new(AAD_MARKER).unwrap()),
        V::U64(2),
        V::Text(c::CanonicalText::new(CIPHER_PROFILE).unwrap()),
        V::Text(c::CanonicalText::new(NONCE_PROFILE).unwrap()),
        V::Bytes(c::BoundedBytes::new(current.clone()).unwrap()),
        V::Bytes(c::BoundedBytes::new(occurrence.clone()).unwrap()),
        V::Bytes(c::BoundedBytes::new(residency.clone()).unwrap()),
        V::Text(c::CanonicalText::new(operation.as_str()).unwrap()),
        V::U64(counter),
        V::Bytes(c::BoundedBytes::new(nonce.to_vec()).unwrap()),
        V::Text(c::CanonicalText::new("blake3-256").unwrap()),
        V::Bytes(c::BoundedBytes::new(digest.as_bytes().to_vec()).unwrap()),
        V::U64(plaintext_length),
    ]).unwrap());
    let expected_aad = c::to_canonical_cbor(&expected_value).unwrap();
    assert_eq!(binding.aad_bytes(), expected_aad.as_slice());
}

#[test]
fn exact_ghash_charge() {
    let current = valid_record(1);
    let occurrence = valid_record(2);
    let residency = valid_record(3);
    let operation = valid_operation();
    let digest = empty_blake3();
    let plaintext_length = 32u64;
    let binding = make_binding(
        &current,
        &occurrence,
        &residency,
        &operation,
        0,
        digest,
        plaintext_length,
    );
    let aad_len = binding.aad_bytes().len() as u64;
    let expected = (plaintext_length + 15) / 16 + (aad_len + 15) / 16 + 1;
    assert_eq!(binding.authenticated_blocks().unwrap(), expected);
}

#[test]
fn changed_owner_record_refuses_open() {
    let key = test_key();
    let plaintext = b"tamper test";
    let digest = c::blake3_bounded_bytes(
        plaintext,
        c::Blake3ByteBudget::new(MAX_REVISION_PLAINTEXT_BYTES).unwrap(),
    )
    .unwrap();
    let binding = make_binding(
        &valid_record(1),
        &valid_record(2),
        &valid_record(3),
        &valid_operation(),
        0,
        digest,
        plaintext.len() as u64,
    );
    let envelope = seal_selected_revision_v2(&key, &binding, plaintext).unwrap();
    let tampered_binding = make_binding(
        &valid_record(99),
        &valid_record(2),
        &valid_record(3),
        &valid_operation(),
        0,
        digest,
        plaintext.len() as u64,
    );
    let result = open_selected_revision_v2(&key, &tampered_binding, &envelope);
    assert!(matches!(
        result,
        Err(SelectedRevisionErrorV2::Authentication)
    ));
}

#[test]
fn changed_operation_refuses_open() {
    let key = test_key();
    let plaintext = b"op tamper";
    let digest = c::blake3_bounded_bytes(
        plaintext,
        c::Blake3ByteBudget::new(MAX_REVISION_PLAINTEXT_BYTES).unwrap(),
    )
    .unwrap();
    let binding = make_binding(
        &valid_record(1),
        &valid_record(2),
        &valid_record(3),
        &valid_operation(),
        0,
        digest,
        plaintext.len() as u64,
    );
    let envelope = seal_selected_revision_v2(&key, &binding, plaintext).unwrap();
    let tampered_binding = make_binding(
        &valid_record(1),
        &valid_record(2),
        &valid_record(3),
        &OpaqueId::new("op-evil").unwrap(),
        0,
        digest,
        plaintext.len() as u64,
    );
    let result = open_selected_revision_v2(&key, &tampered_binding, &envelope);
    assert!(matches!(
        result,
        Err(SelectedRevisionErrorV2::Authentication)
    ));
}

#[test]
fn changed_counter_refuses_open() {
    let key = test_key();
    let plaintext = b"counter tamper";
    let digest = c::blake3_bounded_bytes(
        plaintext,
        c::Blake3ByteBudget::new(MAX_REVISION_PLAINTEXT_BYTES).unwrap(),
    )
    .unwrap();
    let binding = make_binding(
        &valid_record(1),
        &valid_record(2),
        &valid_record(3),
        &valid_operation(),
        0,
        digest,
        plaintext.len() as u64,
    );
    let envelope = seal_selected_revision_v2(&key, &binding, plaintext).unwrap();
    let tampered_binding = make_binding(
        &valid_record(1),
        &valid_record(2),
        &valid_record(3),
        &valid_operation(),
        1,
        digest,
        plaintext.len() as u64,
    );
    let result = open_selected_revision_v2(&key, &tampered_binding, &envelope);
    assert!(matches!(
        result,
        Err(SelectedRevisionErrorV2::Authentication)
    ));
}

#[test]
fn changed_content_digest_refuses_open() {
    let key = test_key();
    let plaintext = b"digest tamper";
    let digest = c::blake3_bounded_bytes(
        plaintext,
        c::Blake3ByteBudget::new(MAX_REVISION_PLAINTEXT_BYTES).unwrap(),
    )
    .unwrap();
    let binding = make_binding(
        &valid_record(1),
        &valid_record(2),
        &valid_record(3),
        &valid_operation(),
        0,
        digest,
        plaintext.len() as u64,
    );
    let envelope = seal_selected_revision_v2(&key, &binding, plaintext).unwrap();
    let wrong_digest = Blake3Digest32::from_bytes([0xAB; 32]);
    let tampered_binding = make_binding(
        &valid_record(1),
        &valid_record(2),
        &valid_record(3),
        &valid_operation(),
        0,
        wrong_digest,
        plaintext.len() as u64,
    );
    let result = open_selected_revision_v2(&key, &tampered_binding, &envelope);
    assert!(matches!(
        result,
        Err(SelectedRevisionErrorV2::Authentication)
    ));
}

#[test]
fn wrong_key_refuses_open() {
    let key = test_key();
    let bad_key = other_key();
    let plaintext = b"wrong key";
    let digest = c::blake3_bounded_bytes(
        plaintext,
        c::Blake3ByteBudget::new(MAX_REVISION_PLAINTEXT_BYTES).unwrap(),
    )
    .unwrap();
    let binding = make_binding(
        &valid_record(1),
        &valid_record(2),
        &valid_record(3),
        &valid_operation(),
        0,
        digest,
        plaintext.len() as u64,
    );
    let envelope = seal_selected_revision_v2(&key, &binding, plaintext).unwrap();
    let result = open_selected_revision_v2(&bad_key, &binding, &envelope);
    assert!(matches!(
        result,
        Err(SelectedRevisionErrorV2::Authentication)
    ));
}

#[test]
fn tampered_ciphertext_refuses_open() {
    let key = test_key();
    let plaintext = b"cipher tamper";
    let digest = c::blake3_bounded_bytes(
        plaintext,
        c::Blake3ByteBudget::new(MAX_REVISION_PLAINTEXT_BYTES).unwrap(),
    )
    .unwrap();
    let binding = make_binding(
        &valid_record(1),
        &valid_record(2),
        &valid_record(3),
        &valid_operation(),
        0,
        digest,
        plaintext.len() as u64,
    );
    let envelope = seal_selected_revision_v2(&key, &binding, plaintext).unwrap();
    let mut encoded = envelope.encode().unwrap();
    let ct_start = SELECTED_REVISION_HEADER_BYTES + binding.aad_bytes().len();
    encoded[ct_start] ^= 0x01;
    let decoded = SelectedRevisionEnvelopeV2::decode(&encoded).unwrap();
    let result = open_selected_revision_v2(&key, &binding, &decoded);
    assert!(matches!(
        result,
        Err(SelectedRevisionErrorV2::Authentication)
    ));
}

#[test]
fn tampered_tag_refuses_open() {
    let key = test_key();
    let plaintext = b"tag tamper";
    let digest = c::blake3_bounded_bytes(
        plaintext,
        c::Blake3ByteBudget::new(MAX_REVISION_PLAINTEXT_BYTES).unwrap(),
    )
    .unwrap();
    let binding = make_binding(
        &valid_record(1),
        &valid_record(2),
        &valid_record(3),
        &valid_operation(),
        0,
        digest,
        plaintext.len() as u64,
    );
    let envelope = seal_selected_revision_v2(&key, &binding, plaintext).unwrap();
    let mut encoded = envelope.encode().unwrap();
    let tag_last = encoded.len() - 1;
    encoded[tag_last] ^= 0x80;
    let decoded = SelectedRevisionEnvelopeV2::decode(&encoded).unwrap();
    let result = open_selected_revision_v2(&key, &binding, &decoded);
    assert!(matches!(
        result,
        Err(SelectedRevisionErrorV2::Authentication)
    ));
}

#[test]
fn v1_magic_is_rejected() {
    let key = test_key();
    let plaintext = b"v1 reject";
    let digest = c::blake3_bounded_bytes(
        plaintext,
        c::Blake3ByteBudget::new(MAX_REVISION_PLAINTEXT_BYTES).unwrap(),
    )
    .unwrap();
    let binding = make_binding(
        &valid_record(1),
        &valid_record(2),
        &valid_record(3),
        &valid_operation(),
        0,
        digest,
        plaintext.len() as u64,
    );
    let envelope = seal_selected_revision_v2(&key, &binding, plaintext).unwrap();
    let mut encoded = envelope.encode().unwrap();
    encoded[0..8].copy_from_slice(b"ELSREV01");
    let result = SelectedRevisionEnvelopeV2::decode(&encoded);
    assert!(matches!(
        result,
        Err(SelectedRevisionErrorV2::InvalidEnvelope)
    ));
}

#[test]
fn truncated_envelope_is_rejected() {
    let key = test_key();
    let plaintext = b"truncate";
    let digest = c::blake3_bounded_bytes(
        plaintext,
        c::Blake3ByteBudget::new(MAX_REVISION_PLAINTEXT_BYTES).unwrap(),
    )
    .unwrap();
    let binding = make_binding(
        &valid_record(1),
        &valid_record(2),
        &valid_record(3),
        &valid_operation(),
        0,
        digest,
        plaintext.len() as u64,
    );
    let envelope = seal_selected_revision_v2(&key, &binding, plaintext).unwrap();
    let encoded = envelope.encode().unwrap();
    let truncated = &encoded[..encoded.len() - 1];
    let result = SelectedRevisionEnvelopeV2::decode(truncated);
    assert!(matches!(
        result,
        Err(SelectedRevisionErrorV2::InvalidEnvelope)
    ));
}

#[test]
fn trailing_bytes_are_rejected() {
    let key = test_key();
    let plaintext = b"trailing";
    let digest = c::blake3_bounded_bytes(
        plaintext,
        c::Blake3ByteBudget::new(MAX_REVISION_PLAINTEXT_BYTES).unwrap(),
    )
    .unwrap();
    let binding = make_binding(
        &valid_record(1),
        &valid_record(2),
        &valid_record(3),
        &valid_operation(),
        0,
        digest,
        plaintext.len() as u64,
    );
    let envelope = seal_selected_revision_v2(&key, &binding, plaintext).unwrap();
    let mut encoded = envelope.encode().unwrap();
    encoded.push(0x00);
    let result = SelectedRevisionEnvelopeV2::decode(&encoded);
    assert!(matches!(
        result,
        Err(SelectedRevisionErrorV2::InvalidEnvelope)
    ));
}

#[test]
fn wrong_nonce_in_header_is_rejected() {
    let key = test_key();
    let plaintext = b"nonce tamper";
    let digest = c::blake3_bounded_bytes(
        plaintext,
        c::Blake3ByteBudget::new(MAX_REVISION_PLAINTEXT_BYTES).unwrap(),
    )
    .unwrap();
    let binding = make_binding(
        &valid_record(1),
        &valid_record(2),
        &valid_record(3),
        &valid_operation(),
        0,
        digest,
        plaintext.len() as u64,
    );
    let envelope = seal_selected_revision_v2(&key, &binding, plaintext).unwrap();
    let mut encoded = envelope.encode().unwrap();
    let nonce_offset = SELECTED_REVISION_HEADER_BYTES - REVISION_NONCE_BYTES;
    encoded[nonce_offset] ^= 0xFF;
    let result = SelectedRevisionEnvelopeV2::decode(&encoded);
    assert!(matches!(
        result,
        Err(SelectedRevisionErrorV2::InvalidEnvelope)
    ));
}

#[test]
fn plaintext_length_mismatch_refuses_seal() {
    let key = test_key();
    let plaintext = b"length mismatch";
    let digest = c::blake3_bounded_bytes(
        plaintext,
        c::Blake3ByteBudget::new(MAX_REVISION_PLAINTEXT_BYTES).unwrap(),
    )
    .unwrap();
    let binding = make_binding(
        &valid_record(1),
        &valid_record(2),
        &valid_record(3),
        &valid_operation(),
        0,
        digest,
        plaintext.len() as u64 + 1,
    );
    let result = seal_selected_revision_v2(&key, &binding, plaintext);
    assert!(matches!(
        result,
        Err(SelectedRevisionErrorV2::ContentMismatch)
    ));
}

#[test]
fn plaintext_digest_mismatch_refuses_seal() {
    let key = test_key();
    let plaintext = b"digest mismatch";
    let wrong_digest = Blake3Digest32::from_bytes([0xCD; 32]);
    let binding = make_binding(
        &valid_record(1),
        &valid_record(2),
        &valid_record(3),
        &valid_operation(),
        0,
        wrong_digest,
        plaintext.len() as u64,
    );
    let result = seal_selected_revision_v2(&key, &binding, plaintext);
    assert!(matches!(
        result,
        Err(SelectedRevisionErrorV2::ContentMismatch)
    ));
}

#[test]
fn all_zero_key_is_rejected() {
    let key = [0u8; REVISION_KEY_BYTES];
    let plaintext = b"zero key";
    let digest = c::blake3_bounded_bytes(
        plaintext,
        c::Blake3ByteBudget::new(MAX_REVISION_PLAINTEXT_BYTES).unwrap(),
    )
    .unwrap();
    let binding = make_binding(
        &valid_record(1),
        &valid_record(2),
        &valid_record(3),
        &valid_operation(),
        0,
        digest,
        plaintext.len() as u64,
    );
    let result = seal_selected_revision_v2(&key, &binding, plaintext);
    assert!(matches!(
        result,
        Err(SelectedRevisionErrorV2::InvalidKey)
    ));
}

#[test]
fn debug_redacts_binding() {
    let binding = make_binding(
        &valid_record(1),
        &valid_record(2),
        &valid_record(3),
        &valid_operation(),
        0,
        empty_blake3(),
        0,
    );
    let debug = format!("{binding:?}");
    assert!(!debug.contains("record-1"));
    assert!(!debug.contains("record-2"));
    assert!(!debug.contains("record-3"));
    assert!(!debug.contains("op-test-0001"));
}

#[test]
fn debug_redacts_envelope() {
    let key = test_key();
    let plaintext = b"debug redact";
    let digest = c::blake3_bounded_bytes(
        plaintext,
        c::Blake3ByteBudget::new(MAX_REVISION_PLAINTEXT_BYTES).unwrap(),
    )
    .unwrap();
    let binding = make_binding(
        &valid_record(1),
        &valid_record(2),
        &valid_record(3),
        &valid_operation(),
        0,
        digest,
        plaintext.len() as u64,
    );
    let envelope = seal_selected_revision_v2(&key, &binding, plaintext).unwrap();
    let debug = format!("{envelope:?}");
    assert!(!debug.contains("record-1"));
    assert!(!debug.contains("op-test-0001"));
}

#[test]
fn debug_redacts_plaintext() {
    let key = test_key();
    let plaintext = b"secret plaintext";
    let digest = c::blake3_bounded_bytes(
        plaintext,
        c::Blake3ByteBudget::new(MAX_REVISION_PLAINTEXT_BYTES).unwrap(),
    )
    .unwrap();
    let binding = make_binding(
        &valid_record(1),
        &valid_record(2),
        &valid_record(3),
        &valid_operation(),
        0,
        digest,
        plaintext.len() as u64,
    );
    let envelope = seal_selected_revision_v2(&key, &binding, plaintext).unwrap();
    let opened = open_selected_revision_v2(&key, &binding, &envelope).unwrap();
    let debug = format!("{opened:?}");
    assert!(!debug.contains("secret plaintext"));
}

#[test]
fn empty_blake3_matches_published_vector() {
    let digest = c::blake3_bounded_bytes(
        &[],
        c::Blake3ByteBudget::new(MAX_REVISION_PLAINTEXT_BYTES).unwrap(),
    )
    .unwrap();
    let expected = hex_to_bytes(EMPTY_BLAKE3_HEX);
    assert_eq!(digest.as_bytes(), &expected);
}

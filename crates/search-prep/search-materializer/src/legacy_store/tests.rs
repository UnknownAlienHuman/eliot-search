use crate::legacy_direct::{
    LegacyDirectPreparationBinding, LegacyDirectPreparationGap,
    LegacyDirectRepresentationDigest, decode_legacy_direct_preparation,
    derive_legacy_direct_representation_id,
    encode_legacy_direct_gap, encode_legacy_direct_layout,
};

use super::*;

struct TestDigest;

impl LegacyPreparationStoreDigest for TestDigest {
    fn digest(bytes: &[u8]) -> [u8; 32] {
        fold(b"digest", &[bytes])
    }

    fn digest_parts(domain: &[u8], parts: &[&[u8]]) -> [u8; 32] {
        fold(domain, parts)
    }
}

impl LegacyDirectRepresentationDigest for TestDigest {
    fn digest_parts(domain: &[u8], parts: &[&[u8]]) -> [u8; 32] {
        fold(domain, parts)
    }
}

fn fold(domain: &[u8], parts: &[&[u8]]) -> [u8; 32] {
    let mut output = [0_u8; 32];
    let mut index = 0_usize;
    for byte in domain
        .iter()
        .chain(parts.iter().flat_map(|part| part.iter()))
    {
        let slot = index % output.len();
        output[slot] = output[slot]
            .wrapping_add(*byte)
            .rotate_left(u32::try_from(index % 8).unwrap_or(0));
        index += 1;
    }
    output
}

fn binding() -> LegacyDirectPreparationBinding {
    LegacyDirectPreparationBinding {
        namespace: [0x11; 32],
        source_id: [0x22; 32],
        revision_id: [0x33; 32],
        content_digest: [0x44; 32],
        byte_length: 0x0102_0304_0506_0708,
        materializer_digest: [0x55; 32],
        unitizer_digest: [0x66; 32],
    }
}

fn encoded_binding() -> [u8; LEGACY_PREPARATION_BINDING_BYTES] {
    encode_legacy_preparation_binding(&binding())
}

#[test]
fn binding_layout_is_byte_exact_and_round_trips() {
    let encoded = encoded_binding();
    assert_eq!(&encoded[..8], LEGACY_PREPARATION_MAGIC);
    assert_eq!(&encoded[8..40], &[0x11; 32]);
    assert_eq!(&encoded[40..72], &[0x22; 32]);
    assert_eq!(&encoded[72..104], &[0x33; 32]);
    assert_eq!(&encoded[104..136], &[0x44; 32]);
    assert_eq!(
        &encoded[136..144],
        &0x0102_0304_0506_0708_u64.to_be_bytes()
    );
    assert_eq!(&encoded[144..176], &[0x55; 32]);
    assert_eq!(&encoded[176..208], &[0x66; 32]);
    assert_eq!(
        decode_legacy_preparation_binding(&encoded).expect("decode binding"),
        binding()
    );

    let mut wrong_magic = encoded;
    wrong_magic[0] ^= 1;
    assert_eq!(
        decode_legacy_preparation_binding(&wrong_magic),
        Err(LegacyPreparationStoreError::BindingInvalid)
    );
    assert_eq!(
        decode_legacy_preparation_binding(&encoded[..207]),
        Err(LegacyPreparationStoreError::BindingInvalid)
    );
}

#[test]
fn lookup_reference_layout_is_byte_exact_and_closed() {
    let key = [0x77; 32];
    let digest = [0x88; 32];
    let encoded = encode_legacy_preparation_reference(
        &key,
        &digest,
        4096,
        LegacyPreparationProtection::Protected,
    )
    .expect("encode reference");
    assert_eq!(&encoded[..8], LEGACY_PREPARATION_REFERENCE_MAGIC);
    assert_eq!(&encoded[8..40], &key);
    assert_eq!(&encoded[40..72], &digest);
    assert_eq!(&encoded[72..80], &4096_u64.to_be_bytes());
    assert_eq!(encoded[80], 1);

    let decoded =
        decode_legacy_preparation_reference(&encoded, &key).expect("decode");
    assert_eq!(decoded.manifest_digest(), &digest);
    assert_eq!(decoded.manifest_bytes(), 4096);
    assert_eq!(
        decoded.protection(),
        LegacyPreparationProtection::Protected
    );

    let mut wrong_key = key;
    wrong_key[0] ^= 1;
    assert_eq!(
        decode_legacy_preparation_reference(&encoded, &wrong_key),
        Err(LegacyPreparationStoreError::ReferenceInvalid)
    );
    let mut wrong_tag = encoded;
    wrong_tag[80] = 2;
    assert_eq!(
        decode_legacy_preparation_reference(&wrong_tag, &key),
        Err(LegacyPreparationStoreError::ReferenceInvalid)
    );
    assert_eq!(
        encode_legacy_preparation_reference(
            &key,
            &digest,
            LEGACY_PREPARATION_OLD_BINDING_BYTES as u64,
            LegacyPreparationProtection::Plaintext,
        ),
        Err(LegacyPreparationStoreError::ReferenceInvalid)
    );
}

#[test]
fn digest_preimages_and_locator_names_are_deterministic() {
    let binding = encoded_binding();
    let lookup_a =
        derive_legacy_preparation_lookup_key::<TestDigest>(&binding, "dpapi-v1");
    let lookup_b =
        derive_legacy_preparation_lookup_key::<TestDigest>(&binding, "plain-v1");
    assert_ne!(lookup_a, lookup_b);

    let object = derive_legacy_preparation_object_id::<TestDigest>(
        &binding,
        "dpapi-v1",
        &[0x99; 32],
    );
    assert_ne!(lookup_a, object);
    assert_eq!(legacy_preparation_shard(&[0xab; 32]), "ab");
    assert_eq!(
        legacy_preparation_reference_file_name(&[0xab; 32]),
        format!("{}.ref", "ab".repeat(32))
    );
    assert_eq!(
        legacy_preparation_object_file_name(
            &[0xcd; 32],
            LegacyPreparationProtection::Protected,
        ),
        format!("{}.dpapi", "cd".repeat(32))
    );
    assert_eq!(
        legacy_preparation_object_file_name(
            &[0xef; 32],
            LegacyPreparationProtection::Plaintext,
        ),
        format!("{}.bin", "ef".repeat(32))
    );
}

#[test]
fn manifest_layout_and_representation_are_verified_together() {
    let binding = binding();
    let encoded_binding = encode_legacy_preparation_binding(&binding);
    let body = encode_legacy_direct_layout(b"layout").expect("layout");
    let frame = decode_legacy_direct_preparation(&body).expect("frame");
    let representation =
        derive_legacy_direct_representation_id::<TestDigest>(
            &binding,
            frame.identity_marker(),
        );
    let manifest = encode_legacy_preparation_manifest(
        &encoded_binding,
        &representation,
        3,
        5,
        &body,
    )
    .expect("manifest");

    assert_eq!(
        manifest.len(),
        LEGACY_PREPARATION_HEADER_BYTES + body.len()
    );
    assert_eq!(
        &manifest[..LEGACY_PREPARATION_BINDING_BYTES],
        &encoded_binding
    );
    let verified = verify_legacy_preparation_manifest::<TestDigest>(
        &manifest,
        &encoded_binding,
        3,
        5,
    )
    .expect("verify");
    assert_eq!(verified.binding(), &binding);
    assert_eq!(verified.representation_id(), &representation);
    assert_eq!(verified.materializer_revision(), 3);
    assert_eq!(verified.unitizer_revision(), 5);
    assert_eq!(verified.body(), body);

    let mut wrong_algorithm = manifest.clone();
    wrong_algorithm[LEGACY_PREPARATION_BINDING_BYTES + 32] ^= 1;
    assert_eq!(
        verify_legacy_preparation_manifest::<TestDigest>(
            &wrong_algorithm,
            &encoded_binding,
            3,
            5,
        ),
        Err(LegacyPreparationStoreError::DigestAlgorithmMismatch)
    );

    let mut wrong_revision = manifest.clone();
    wrong_revision[LEGACY_PREPARATION_BINDING_BYTES + 34] ^= 1;
    assert_eq!(
        verify_legacy_preparation_manifest::<TestDigest>(
            &wrong_revision,
            &encoded_binding,
            3,
            5,
        ),
        Err(LegacyPreparationStoreError::ProfileMismatch)
    );

    let mut wrong_representation = manifest;
    wrong_representation[LEGACY_PREPARATION_BINDING_BYTES] ^= 1;
    assert_eq!(
        verify_legacy_preparation_manifest::<TestDigest>(
            &wrong_representation,
            &encoded_binding,
            3,
            5,
        ),
        Err(LegacyPreparationStoreError::BindingMismatch)
    );
}

#[test]
fn every_closed_gap_is_a_valid_manifest_body() {
    let binding = binding();
    let encoded_binding = encode_legacy_preparation_binding(&binding);
    for gap in [
        LegacyDirectPreparationGap::RevisionNotUtf8,
        LegacyDirectPreparationGap::BinaryContent,
        LegacyDirectPreparationGap::TooManyLines,
        LegacyDirectPreparationGap::TooManyUnits,
        LegacyDirectPreparationGap::LayoutTooLarge,
        LegacyDirectPreparationGap::RevisionHasBom,
    ] {
        let body = encode_legacy_direct_gap(gap);
        let frame = decode_legacy_direct_preparation(&body).expect("gap frame");
        let representation =
            derive_legacy_direct_representation_id::<TestDigest>(
                &binding,
                frame.identity_marker(),
            );
        let manifest = encode_legacy_preparation_manifest(
            &encoded_binding,
            &representation,
            1,
            1,
            &body,
        )
        .expect("gap manifest");
        assert_eq!(
            verify_legacy_preparation_manifest::<TestDigest>(
                &manifest,
                &encoded_binding,
                1,
                1,
            )
            .expect("verify gap")
            .body(),
            body
        );
    }
}

#[test]
fn malformed_manifest_fails_closed_without_partial_admission() {
    let binding = encoded_binding();
    assert_eq!(
        encode_legacy_preparation_manifest(
            &binding,
            &[0; 32],
            0,
            1,
            &[1],
        ),
        Err(LegacyPreparationStoreError::ProfileMismatch)
    );
    assert_eq!(
        encode_legacy_preparation_manifest(
            &binding,
            &[0; 32],
            1,
            1,
            &[0],
        ),
        Err(LegacyPreparationStoreError::ObjectInvalid)
    );
    assert_eq!(
        LEGACY_PREPARATION_MAX_MANIFEST_BYTES,
        LEGACY_PREPARATION_HEADER_BYTES
            + LEGACY_DIRECT_MAX_LAYOUT_BYTES
            + 1
    );
}

#[test]
fn decoded_payload_is_bound_to_length_and_digest() {
    let bytes = b"decoded manifest";
    let digest = TestDigest::digest(bytes);
    assert_eq!(
        verify_legacy_preparation_payload::<TestDigest>(
            bytes,
            &digest,
            bytes.len() as u64,
        ),
        Ok(())
    );
    assert_eq!(
        verify_legacy_preparation_payload::<TestDigest>(
            bytes,
            &digest,
            bytes.len() as u64 + 1,
        ),
        Err(LegacyPreparationStoreError::ContentMismatch)
    );
    assert_eq!(
        verify_legacy_preparation_payload::<TestDigest>(
            b"changed",
            &digest,
            7,
        ),
        Err(LegacyPreparationStoreError::ContentMismatch)
    );
}

use super::*;

struct TranscriptDigest;

impl LegacyRevisionKeyDigest for TranscriptDigest {
    fn digest_parts(domain: &[u8], parts: &[&[u8]]) -> [u8; 32] {
        let mut output = [0_u8; 32];
        output[0] = u8::try_from(domain.len()).expect("bounded domain");
        output[1] = u8::try_from(parts.len()).expect("bounded parts");
        output[2] = u8::try_from(parts[0].len()).expect("bounded namespace");
        output[3] = u8::try_from(parts[1].len()).expect("bounded secret");
        output[4] = *domain.first().expect("non-empty domain");
        output[5] = *domain.last().expect("non-empty domain");
        output[6] = parts[0][0];
        output[7] = parts[1][0];
        for (index, byte) in domain
            .iter()
            .chain(parts.iter().flat_map(|part| part.iter()))
            .enumerate()
        {
            let slot = 8 + (index % 24);
            output[slot] = output[slot]
                .wrapping_mul(31)
                .wrapping_add(*byte)
                .wrapping_add(u8::try_from(index & 0xff).expect("bounded index"));
        }
        output
    }
}

#[test]
fn legacy_key_derivation_domains_and_part_order_are_frozen() {
    assert_eq!(
        LEGACY_REVISION_KEY_BINDING_DOMAIN,
        b"eliot-search/revision-key-binding/v1"
    );
    assert_eq!(
        LEGACY_REVISION_DPAPI_ENTROPY_DOMAIN,
        b"eliot-search/revision-dpapi-entropy/v1"
    );
    assert_eq!(LEGACY_REVISION_ROOT_SECRET_BYTES, 32);

    let namespace = [0x11; 32];
    let root_secret = [0x22; LEGACY_REVISION_ROOT_SECRET_BYTES];
    let key_binding = derive_legacy_revision_key_binding::<TranscriptDigest>(
        &namespace,
        &root_secret,
    );
    let entropy = derive_legacy_revision_dpapi_entropy::<TranscriptDigest>(
        &namespace,
        &root_secret,
    );

    assert_eq!(key_binding[1], 2);
    assert_eq!(key_binding[2], 32);
    assert_eq!(
        key_binding[3],
        u8::try_from(LEGACY_REVISION_ROOT_SECRET_BYTES).expect("bounded secret")
    );
    assert_eq!(key_binding[6], 0x11);
    assert_eq!(key_binding[7], 0x22);
    assert_ne!(key_binding, entropy);
}

#[test]
fn legacy_key_derivation_is_bound_to_namespace_and_root_secret() {
    let namespace = [0x11; 32];
    let root_secret = [0x22; LEGACY_REVISION_ROOT_SECRET_BYTES];
    let baseline = derive_legacy_revision_key_binding::<TranscriptDigest>(
        &namespace,
        &root_secret,
    );

    let mut foreign_namespace = namespace;
    foreign_namespace[0] ^= 1;
    assert_ne!(
        baseline,
        derive_legacy_revision_key_binding::<TranscriptDigest>(
            &foreign_namespace,
            &root_secret,
        )
    );

    let mut foreign_secret = root_secret;
    foreign_secret[0] ^= 1;
    assert_ne!(
        baseline,
        derive_legacy_revision_key_binding::<TranscriptDigest>(
            &namespace,
            &foreign_secret,
        )
    );
}

//! #237 byte goldens and the public compute/restore boundary.
use search_contracts::*;

const DOMAIN: &str = "eliot/cbor/eliot-canonical-foundation/v1";

fn domain(name: &str) -> CanonicalDigestDomain {
    CanonicalDigestDomain::parse(name).expect("fixture domain")
}

fn limit(bytes: usize) -> DigestInputLimit {
    DigestInputLimit::new(bytes).expect("finite ceiling")
}

#[test]
fn canonical_null_and_map_have_frozen_full_width_digests() {
    let map = CanonicalValue::Object(
        BoundedMap::from_entries([
            (
                CanonicalKey::new_non_empty("b").expect("key"),
                CanonicalValue::U64(2),
            ),
            (
                CanonicalKey::new_non_empty("a").expect("key"),
                CanonicalValue::U64(1),
            ),
        ])
        .expect("map"),
    );
    for (value, sha, blake) in [
        (
            CanonicalValue::Null,
            "1cbb50e2588cb09ebb7f5517596c7dedde00206ff17b9f76ed21c8e5588ada3a",
            "85726dd0e21055700979aff48295e0ae281bda07d401ed40840775ee721cb449",
        ),
        (
            map,
            "571761399d6d053dcb33f43e086a93b6daab5c83a7c031330ea1c74777325c4a",
            "c0f48612d03956d75fd034c844c3f056c28768d25f9f50e8d9e1e81731d1d054",
        ),
    ] {
        assert_eq!(
            sha256_canonical(&domain(DOMAIN), &value, limit(4096))
                .expect("SHA")
                .to_string(),
            sha
        );
        assert_eq!(
            blake3_canonical(&domain(DOMAIN), &value, limit(4096))
                .expect("BLAKE3")
                .to_string(),
            blake
        );
    }
}

#[test]
fn complete_preimage_ceiling_counts_prefix_and_one_cbor_byte() {
    let value = CanonicalValue::Null;
    let exact = DOMAIN.len() + 2;
    assert!(sha256_canonical(&domain(DOMAIN), &value, limit(exact)).is_ok());
    assert_eq!(
        sha256_canonical(&domain(DOMAIN), &value, limit(exact - 1))
            .expect_err("payload cannot fit")
            .kind(),
        ContractErrorKind::OversizePayload
    );
    assert!(blake3_canonical(&domain(DOMAIN), &value, limit(DOMAIN.len())).is_err());
}

#[test]
fn representation_and_schema_revision_are_load_bearing() {
    let raw = domain("eliot/raw/eliot-canonical-foundation/v1");
    let cbor = domain(DOMAIN);
    assert!(blake3_raw(&cbor, &[0xf6], limit(4096)).is_err());
    assert!(sha256_canonical(&raw, &CanonicalValue::Null, limit(4096)).is_err());
    let original = blake3_canonical(&cbor, &CanonicalValue::Null, limit(4096)).expect("CBOR");
    assert_ne!(
        original,
        blake3_raw(&raw, &[0xf6], limit(4096)).expect("raw")
    );
    assert_ne!(
        original,
        blake3_canonical(
            &domain("eliot/cbor/eliot-canonical-foundation/v2"),
            &CanonicalValue::Null,
            limit(4096)
        )
        .expect("new schema")
    );
    assert!(sha256_raw(&raw, &[0xf6], limit(raw.as_str().len() + 2)).is_ok());
    assert!(sha256_raw(&raw, &[0xf6], limit(raw.as_str().len() + 1)).is_err());
}

#[test]
fn invalid_domains_and_nonfinite_policy_are_refused() {
    for invalid in [
        "cbor/schema/v1",
        "eliot/json/schema/v1",
        "eliot/cbor/v1",
        "eliot/cbor//v1",
        "eliot/cbor/Schema/v1",
        "eliot/cbor/schema/",
        "eliot/cbor/schema/v0",
        "eliot/cbor/schema/v01",
        "eliot/cbor/schema/vx",
        "eliot/cbor/sche ma/v1",
        "eliot/cbor/sche_ma/v1",
        "eliot/cbor/sche--ma/v1",
        "eliot/cbor/sche-/v1",
        "eliot/cbor/-schema/v1",
        "eliot/cbor/schema/v4294967296",
    ] {
        assert!(CanonicalDigestDomain::parse(invalid).is_err(), "{invalid}");
    }
    assert!(CanonicalDigestDomain::parse(&format!("eliot/cbor/{}/v1", "a".repeat(129))).is_err());
    assert!(DigestInputLimit::new(0).is_err());
    assert!(DigestInputLimit::new(MAX_CANONICAL_BYTES + 1).is_err());
}

#[test]
fn restore_preserves_algorithm_and_full_width_without_compute_claim() {
    let bytes = [0x51; 32];
    let sha = VersionedContentDigest::from_stored_bytes(DigestAlgorithm::Sha256, bytes);
    let blake = VersionedContentDigest::from_stored_bytes(DigestAlgorithm::Blake3_256, bytes);
    assert_eq!(sha.algorithm(), DigestAlgorithm::Sha256);
    assert_eq!(sha.as_bytes(), &bytes);
    assert_ne!(sha, blake);
    let computed =
        sha256_canonical(&domain(DOMAIN), &CanonicalValue::Null, limit(4096)).expect("SHA");
    let tagged = VersionedContentDigest::from_sha256(computed);
    assert_eq!(tagged.algorithm(), DigestAlgorithm::Sha256);
    assert_eq!(tagged.as_bytes(), computed.as_bytes());
}

//! #386 exact-byte compatibility vectors. `AUTHORED_NOT_RUN`.
use search_contracts::{
    CanonicalDigestDomain, ContractErrorKind, DigestInputLimit, MAX_CANONICAL_BYTES,
    sha256_exact_bytes, sha256_raw,
};

#[test]
fn exact_bytes_match_official_standard_vectors() {
    // Standard empty, abc and NIST 56-byte padding/multiblock vectors.
    for (bytes, expected) in [
        (
            b"".as_slice(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        ),
        (
            b"abc".as_slice(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        ),
        (
            b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq".as_slice(),
            "248d6a61d20638b8e5c026930c3e6039a33ee45964ff2167f6ecedd419db06c1",
        ),
    ] {
        let limit = DigestInputLimit::new(bytes.len().max(1)).expect("finite limit");
        assert_eq!(
            sha256_exact_bytes(bytes, limit)
                .expect("digest")
                .to_string(),
            expected
        );
    }
}

#[test]
fn exact_ceiling_is_inclusive_and_oversize_yields_no_digest() {
    let limit = DigestInputLimit::new(3).expect("finite limit");
    assert!(sha256_exact_bytes(b"abc", limit).is_ok());
    assert_eq!(
        sha256_exact_bytes(b"abcd", limit)
            .expect_err("too long")
            .kind(),
        ContractErrorKind::OversizePayload,
    );
    assert!(DigestInputLimit::new(0).is_err());
    assert!(DigestInputLimit::new(MAX_CANONICAL_BYTES + 1).is_err());
}

#[test]
fn exact_bytes_do_not_prefix_the_registered_raw_domain() {
    let domain = CanonicalDigestDomain::parse("eliot/raw/config-compatibility/v1")
        .expect("valid raw domain");
    let limit = DigestInputLimit::new(256).expect("finite limit");
    assert_ne!(
        sha256_exact_bytes(b"abc", limit).expect("exact"),
        sha256_raw(&domain, b"abc", limit).expect("domain-bound"),
    );
}

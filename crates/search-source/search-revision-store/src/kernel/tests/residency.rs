use search_contracts::{
    Blake3Digest32, DigestAlgorithm, SearchObjectResidencyKey,
    VersionedContentDigest,
};

use super::fixtures::*;
use super::super::*;

#[test]
fn residency_closure_round_trips_all_six_typed_domains() {
    let canonical = SearchObjectResidencyKey {
        scope_domain_id: baseline_residency().scope,
        access_domain_id: baseline_residency().access,
        confidentiality_domain_id: baseline_residency().confidentiality,
        encryption_key_domain_id: baseline_residency().encryption_key,
        retention_domain_id: baseline_residency().retention,
        erasure_domain_id: baseline_residency().erasure,
        versioned_content_digest: VersionedContentDigest {
            algorithm: DigestAlgorithm::Blake3_256,
            bytes: [0x07; 32],
        },
    };
    assert_eq!(
        ResidencyClosure::from_search_object_key(&canonical),
        baseline_residency()
    );
    let first = baseline_residency().scope_id().expect("scope id");
    assert!(first.as_str().starts_with("rs1."));
    assert_eq!(first, baseline_residency().scope_id().expect("scope id"));
    for index in 0..6 {
        assert_ne!(
            residency_variant(index).scope_id().expect("scope id"),
            first,
            "domain {index} must change the residency scope identity"
        );
    }
}

#[test]
fn same_plaintext_across_all_six_domains_gets_separate_storage_identity() {
    let mut store = RevisionStore::new(DEFAULT_REVISION_STORE_LIMITS).expect("store");
    let plaintext_digest = Blake3Digest32::from_bytes([0x07; 32]);
    let mut addresses = Vec::new();
    for index in 0..6_u8 {
        let residency = residency_variant(usize::from(index));
        // Identical plaintext, separately encrypted and keyed per residency.
        let current = intent_full(
            source(&format!("cross-{index}")),
            residency,
            1,
            &format!("cross-{index}"),
            0x07,
            0x70 + index,
            0x80 + index,
            &format!("secret:revision-key-{index}"),
        );
        let receipt = confirm(&mut store, &current);
        assert!(!receipt.replayed);
        assert_eq!(receipt.residency, residency);
        let address = derive_object_address(
            &residency,
            RevisionObjectKind::RevisionEnvelopeV1,
            plaintext_digest,
            CAS_ADDRESS_VERSION,
        )
        .expect("address");
        addresses.push(address.to_path_string());
    }
    for (left, address) in addresses.iter().enumerate() {
        for (right, other) in addresses.iter().enumerate() {
            if left != right {
                assert_ne!(address, other, "domains {left} and {right} collide");
            }
        }
    }
}

#[test]
fn cross_domain_storage_object_reuse_is_typed_conflict() {
    let mut store = RevisionStore::new(DEFAULT_REVISION_STORE_LIMITS).expect("store");
    let first = intent(1, "one", 1);
    let target = first.storage_object_id.clone();
    confirm(&mut store, &first);
    for index in 0..6_u8 {
        let mut other = intent_full(
            source(&format!("other-{index}")),
            residency_variant(usize::from(index)),
            1,
            &format!("other-{index}"),
            9,
            0x40 + index,
            0x50 + index,
            "secret:revision-key",
        );
        other.storage_object_id.clone_from(&target);
        assert_eq!(
            store.prepare_append(other),
            Err(RevisionStoreError::ResidencyMismatch),
            "domain {index} must not reuse the physical object"
        );
    }
}

#[test]
fn cross_domain_ciphertext_and_envelope_reuse_is_typed_conflict() {
    let mut store = RevisionStore::new(DEFAULT_REVISION_STORE_LIMITS).expect("store");
    let first = intent(1, "one", 1);
    confirm(&mut store, &first);
    // Identical ciphertext bytes under another residency, fresh object id.
    let second = intent_full(
        source("copy"),
        residency_variant(2),
        1,
        "copy",
        1,
        1,
        0x22,
        "secret:copy-key",
    );
    assert_eq!(
        store.prepare_append(second),
        Err(RevisionStoreError::ResidencyMismatch)
    );
    // Identical envelope binding digests under another residency, fresh bytes.
    let third = intent_full(
        source("envelope-copy"),
        residency_variant(4),
        1,
        "envelope-copy",
        9,
        0x41,
        1,
        "secret:envelope-copy-key",
    );
    assert_eq!(
        store.prepare_append(third),
        Err(RevisionStoreError::ResidencyMismatch)
    );
}

#[test]
fn cross_domain_key_reference_reuse_is_typed_conflict() {
    let mut store = RevisionStore::new(DEFAULT_REVISION_STORE_LIMITS).expect("store");
    let first = intent(1, "one", 1);
    confirm(&mut store, &first);
    // Only the encryption-key domain differs; fresh object id and fresh
    // ciphertext, but the same secret reference is reused.
    let second = intent_full(
        source("rekey"),
        residency_variant(3),
        1,
        "rekey",
        9,
        0x42,
        0x52,
        "secret:revision-key",
    );
    assert_eq!(
        store.prepare_append(second),
        Err(RevisionStoreError::ResidencyMismatch)
    );
}

#[test]
fn derive_object_address_is_domain_separated() {
    let digest = Blake3Digest32::from_bytes([0x07; 32]);
    let base = derive_object_address(
        &baseline_residency(),
        RevisionObjectKind::RevisionEnvelopeV1,
        digest,
        CAS_ADDRESS_VERSION,
    )
    .expect("address");
    let path = base.to_path_string();
    assert!(path.starts_with("cas/v1/revision-envelope-v1/"));
    assert!(!path.contains("source:test"));
    assert_eq!(
        path,
        derive_object_address(
            &baseline_residency(),
            RevisionObjectKind::RevisionEnvelopeV1,
            digest,
            CAS_ADDRESS_VERSION,
        )
        .expect("address")
        .to_path_string()
    );
    for index in 0..6 {
        let other = derive_object_address(
            &residency_variant(index),
            RevisionObjectKind::RevisionEnvelopeV1,
            digest,
            CAS_ADDRESS_VERSION,
        )
        .expect("address");
        assert_ne!(path, other.to_path_string(), "domain {index} collides");
    }
    let other_digest = derive_object_address(
        &baseline_residency(),
        RevisionObjectKind::RevisionEnvelopeV1,
        Blake3Digest32::from_bytes([0x08; 32]),
        CAS_ADDRESS_VERSION,
    )
    .expect("address");
    assert_ne!(path, other_digest.to_path_string());
    assert_eq!(
        derive_object_address(
            &baseline_residency(),
            RevisionObjectKind::RevisionEnvelopeV1,
            Blake3Digest32::from_bytes([0; 32]),
            CAS_ADDRESS_VERSION,
        ),
        Err(RevisionStoreError::AddressInvalid)
    );
    assert_eq!(
        derive_object_address(
            &baseline_residency(),
            RevisionObjectKind::RevisionEnvelopeV1,
            digest,
            99,
        ),
        Err(RevisionStoreError::AddressInvalid)
    );
}

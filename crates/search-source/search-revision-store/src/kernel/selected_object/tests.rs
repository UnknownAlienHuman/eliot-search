//! Focused causal tests for domain bindings, pre-seal descriptors, and late binding.

use search_contracts::{
    Blake3Digest32, NonZeroRevision, OpaqueId, SourceId, SourceNamespaceId, SourceRevisionId,
};

use super::super::error::RevisionStoreError;
use super::domain::{DomainBinding, DomainKind, residency_canonical_bytes};
use super::source::{ObjectMetadataBinding, PreSealDescriptor, TypedSourceIdentity};
use super::super::model::EncryptionBinding;
use super::super::CipherSuite;

fn uuid(byte: u8) -> [u8; 16] {
    [byte; 16]
}

fn binding(kind: DomainKind, uuid_byte: u8, body: &[u8]) -> DomainBinding {
    DomainBinding::new(kind, uuid(uuid_byte), body.to_vec()).expect("valid binding")
}

fn six_bindings() -> [DomainBinding; 6] {
    [
        binding(DomainKind::Scope, 0x11, b"scope-body"),
        binding(DomainKind::Access, 0x22, b"access-body"),
        binding(DomainKind::Confidentiality, 0x33, b"confidentiality-body"),
        binding(DomainKind::EncryptionKey, 0x44, b"encryption-key-body"),
        binding(DomainKind::Retention, 0x55, b"retention-body"),
        binding(DomainKind::Erasure, 0x66, b"erasure-body"),
    ]
}

fn typed_identity() -> TypedSourceIdentity {
    TypedSourceIdentity::new(
        SourceNamespaceId::from_bytes([0xAA; 16]),
        SourceId::from_bytes([0xBB; 16]),
        SourceRevisionId::from_bytes([0xCC; 16]),
        NonZeroRevision::new(1).expect("sequence"),
    )
}

#[test]
fn domain_binding_computes_full_tagged_digest() {
    let binding = binding(DomainKind::Scope, 0x11, b"test-body");
    assert_eq!(binding.kind, DomainKind::Scope);
    assert_eq!(binding.uuid, uuid(0x11));
    assert_eq!(binding.canonical_body, b"test-body");
    assert_ne!(binding.tagged_digest, Blake3Digest32::from_bytes([0; 32]));
}

#[test]
fn domain_binding_refuses_empty_body() {
    assert_eq!(
        DomainBinding::new(DomainKind::Scope, uuid(0x11), vec![]),
        Err(RevisionStoreError::AddressInvalid)
    );
}

#[test]
fn domain_binding_refuses_oversize_body() {
    let oversize = vec![0u8; DomainBinding::MAX_BODY_BYTES + 1];
    assert_eq!(
        DomainBinding::new(DomainKind::Scope, uuid(0x11), oversize),
        Err(RevisionStoreError::AddressInvalid)
    );
}

#[test]
fn same_uuid_different_body_is_refused() {
    let first = binding(DomainKind::Scope, 0x11, b"body-a");
    let second = binding(DomainKind::Scope, 0x11, b"body-b");
    assert_eq!(
        first.refuse_uuid_collision(&second),
        Err(RevisionStoreError::ResidencyMismatch)
    );
}

#[test]
fn same_digest_different_body_is_refused() {
    // Two different bodies that happen to produce the same tagged digest
    // (collision) must be refused. In practice BLAKE3 collisions are
    // infeasible, but the API must enforce the invariant.
    let first = binding(DomainKind::Scope, 0x11, b"body-a");
    // Simulate a collision by constructing a binding with the same digest
    // but different body.
    let colliding = DomainBinding {
        kind: DomainKind::Scope,
        uuid: uuid(0x11),
        canonical_body: b"body-b".to_vec(),
        tagged_digest: first.tagged_digest,
    };
    assert_eq!(
        first.refuse_digest_collision(&colliding),
        Err(RevisionStoreError::ResidencyMismatch)
    );
}

#[test]
fn verify_rejects_mismatched_kind() {
    let first = binding(DomainKind::Scope, 0x11, b"body");
    let second = binding(DomainKind::Access, 0x11, b"body");
    assert_eq!(
        first.verify(&second),
        Err(RevisionStoreError::ResidencyMismatch)
    );
}

#[test]
fn verify_accepts_exact_match() {
    let first = binding(DomainKind::Scope, 0x11, b"body");
    let second = binding(DomainKind::Scope, 0x11, b"body");
    assert_eq!(first.verify(&second), Ok(()));
}

#[test]
fn residency_canonical_bytes_produces_deterministic_output() {
    let bindings = six_bindings();
    let first = residency_canonical_bytes(&bindings).expect("canonical");
    let second = residency_canonical_bytes(&bindings).expect("canonical");
    assert_eq!(first, second);
    assert!(!first.is_empty());
    assert!(first.len() <= 8192);
}

#[test]
fn residency_canonical_bytes_rejects_wrong_domain_order() {
    let mut bindings = six_bindings();
    bindings.swap(0, 1);
    assert_eq!(
        residency_canonical_bytes(&bindings),
        Err(RevisionStoreError::ResidencyMismatch)
    );
}

#[test]
fn pre_seal_descriptor_binds_complete_metadata() {
    let descriptor = PreSealDescriptor::new(
        typed_identity(),
        six_bindings(),
        Blake3Digest32::from_bytes([0x07; 32]),
        1024,
        OpaqueId::new("op:test").expect("operation"),
    )
    .expect("descriptor");
    assert_eq!(descriptor.content_bytes, 1024);
    assert!(!descriptor.residency_canonical().is_empty());
    assert_eq!(descriptor.source().sequence.get(), 1);
}

#[test]
fn pre_seal_descriptor_rejects_zero_content_bytes() {
    assert_eq!(
        PreSealDescriptor::new(
            typed_identity(),
            six_bindings(),
            Blake3Digest32::from_bytes([0x07; 32]),
            0,
            OpaqueId::new("op:test").expect("operation"),
        ),
        Err(RevisionStoreError::PlaintextSizeInvalid)
    );
}

#[test]
fn pre_seal_descriptor_rejects_oversize_residency_canonical() {
    // Create bindings with maximum-size bodies to exceed 8192 bytes.
    let large_body = vec![0xABu8; DomainBinding::MAX_BODY_BYTES];
    let bindings = [
        binding(DomainKind::Scope, 0x11, &large_body),
        binding(DomainKind::Access, 0x22, &large_body),
        binding(DomainKind::Confidentiality, 0x33, &large_body),
        binding(DomainKind::EncryptionKey, 0x44, &large_body),
        binding(DomainKind::Retention, 0x55, &large_body),
        binding(DomainKind::Erasure, 0x66, &large_body),
    ];
    assert_eq!(
        PreSealDescriptor::new(
            typed_identity(),
            bindings,
            Blake3Digest32::from_bytes([0x07; 32]),
            1024,
            OpaqueId::new("op:test").expect("operation"),
        ),
        Err(RevisionStoreError::ContractExhausted)
    );
}

#[test]
fn object_metadata_binding_extends_preseal() {
    let descriptor = PreSealDescriptor::new(
        typed_identity(),
        six_bindings(),
        Blake3Digest32::from_bytes([0x07; 32]),
        1024,
        OpaqueId::new("op:test").expect("operation"),
    )
    .expect("descriptor");
    let binding = ObjectMetadataBinding::new(
        descriptor.clone(),
        OpaqueId::new("object:test").expect("object"),
        Blake3Digest32::from_bytes([0x08; 32]),
        2048,
        EncryptionBinding {
            key_reference: OpaqueId::new("secret:key").expect("key"),
            key_version: NonZeroRevision::new(1).expect("version"),
            cipher_suite: CipherSuite::AuthenticatedEncryptionV1,
        },
    )
    .expect("binding");
    assert_eq!(binding.ciphertext_bytes(), 2048);
    assert_eq!(binding.residency_canonical(), descriptor.residency_canonical());
    assert_eq!(binding.operation_id().as_str(), "op:test");
}

#[test]
fn object_metadata_binding_rejects_zero_ciphertext() {
    let descriptor = PreSealDescriptor::new(
        typed_identity(),
        six_bindings(),
        Blake3Digest32::from_bytes([0x07; 32]),
        1024,
        OpaqueId::new("op:test").expect("operation"),
    )
    .expect("descriptor");
    assert_eq!(
        ObjectMetadataBinding::new(
            descriptor,
            OpaqueId::new("object:test").expect("object"),
            Blake3Digest32::from_bytes([0x08; 32]),
            0,
            EncryptionBinding {
                key_reference: OpaqueId::new("secret:key").expect("key"),
                key_version: NonZeroRevision::new(1).expect("version"),
                cipher_suite: CipherSuite::AuthenticatedEncryptionV1,
            },
        ),
        Err(RevisionStoreError::CiphertextSizeInvalid)
    );
}

#[test]
fn object_metadata_binding_rejects_zero_digest() {
    let descriptor = PreSealDescriptor::new(
        typed_identity(),
        six_bindings(),
        Blake3Digest32::from_bytes([0x07; 32]),
        1024,
        OpaqueId::new("op:test").expect("operation"),
    )
    .expect("descriptor");
    assert_eq!(
        ObjectMetadataBinding::new(
            descriptor,
            OpaqueId::new("object:test").expect("object"),
            Blake3Digest32::from_bytes([0; 32]),
            2048,
            EncryptionBinding {
                key_reference: OpaqueId::new("secret:key").expect("key"),
                key_version: NonZeroRevision::new(1).expect("version"),
                cipher_suite: CipherSuite::AuthenticatedEncryptionV1,
            },
        ),
        Err(RevisionStoreError::CiphertextSizeInvalid)
    );
}

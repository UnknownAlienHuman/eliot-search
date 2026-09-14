use search_contracts::{
    AccessDomainId, Blake3Digest32, ConfidentialityDomainId,
    EncryptionKeyDomainId, ErasureDomainId, NonZeroRevision, OpaqueId,
    ReceiptRef, RetentionDomainId, ScopeDomainId,
};

use super::super::*;

pub(super) fn baseline_residency() -> ResidencyClosure {
    ResidencyClosure::new(
        ScopeDomainId::from_bytes([0x11; 16]),
        AccessDomainId::from_bytes([0x22; 16]),
        ConfidentialityDomainId::from_bytes([0x33; 16]),
        EncryptionKeyDomainId::from_bytes([0x44; 16]),
        RetentionDomainId::from_bytes([0x55; 16]),
        ErasureDomainId::from_bytes([0x66; 16]),
    )
}

pub(super) fn residency_variant(which: usize) -> ResidencyClosure {
    let mut residency = baseline_residency();
    match which {
        0 => residency.scope = ScopeDomainId::from_bytes([0x99; 16]),
        1 => residency.access = AccessDomainId::from_bytes([0x99; 16]),
        2 => residency.confidentiality = ConfidentialityDomainId::from_bytes([0x99; 16]),
        3 => residency.encryption_key = EncryptionKeyDomainId::from_bytes([0x99; 16]),
        4 => residency.retention = RetentionDomainId::from_bytes([0x99; 16]),
        5 => residency.erasure = ErasureDomainId::from_bytes([0x99; 16]),
        _ => panic!("unknown residency domain index"),
    }
    residency
}

pub(super) fn source(name: &str) -> OpaqueId {
    OpaqueId::new(format!("source:{name}")).expect("source")
}

pub(super) fn hex_opaque(byte: u8) -> OpaqueId {
    OpaqueId::new(format!("{byte:02x}").repeat(32)).expect("hex digest")
}

pub(super) fn operation(name: &str, digest: u8) -> RevisionOperation {
    RevisionOperation::new(
        OpaqueId::new(format!("revision-operation:{name}")).expect("operation"),
        Blake3Digest32::from_bytes([digest; 32]),
    )
}

pub(super) fn payload(
    plaintext_seed: u8,
    ciphertext_seed: u8,
) -> EncryptedRevisionPayload {
    payload_with_key(plaintext_seed, ciphertext_seed, "secret:revision-key")
}

pub(super) fn payload_with_key(
    plaintext_seed: u8,
    ciphertext_seed: u8,
    key_name: &str,
) -> EncryptedRevisionPayload {
    EncryptedRevisionPayload::new(
        Blake3Digest32::from_bytes([plaintext_seed; 32]),
        3,
        Blake3Digest32::from_bytes([ciphertext_seed.wrapping_add(1); 32]),
        vec![ciphertext_seed; 12],
        vec![ciphertext_seed; 32],
        EncryptionBinding {
            key_reference: OpaqueId::new(key_name).expect("key"),
            key_version: NonZeroRevision::new(1).expect("version"),
            cipher_suite: CipherSuite::AuthenticatedEncryptionV1,
        },
        DEFAULT_REVISION_STORE_LIMITS,
    )
    .expect("payload")
}

pub(super) fn envelope(binding_seed: u8) -> EnvelopeBinding {
    EnvelopeBinding {
        version: ENVELOPE_BINDING_VERSION,
        key_generation: NonZeroRevision::new(1).expect("generation"),
        source_revision_binding_digest: [binding_seed; 32],
        residency_binding_digest: [binding_seed.wrapping_add(0x10); 32],
        encryption_profile_digest: [0xD0; 32],
        content_digest_sha256: [0xE0; 32],
        plaintext_length: 3,
    }
}

pub(super) fn ingest(name: &str, seed: u8) -> CanonicalIngestBinding {
    CanonicalIngestBinding::new(
        ReceiptRef::new(format!("receipt:t13-admission:{name}")).expect("receipt"),
        hex_opaque(0xA0),
        NonZeroRevision::new(1).expect("policy"),
        hex_opaque(0xB0),
        hex_opaque(seed),
        hex_opaque(seed.wrapping_add(1)),
    )
    .expect("ingest")
}

pub(super) fn key(revision: u64) -> RevisionKey {
    RevisionKey {
        source_id: source("test"),
        revision: NonZeroRevision::new(revision).expect("revision"),
        residency: baseline_residency(),
    }
}

pub(super) fn intent(
    revision: u64,
    name: &str,
    content: u8,
) -> RevisionWriteIntent {
    intent_full(
        source("test"),
        baseline_residency(),
        revision,
        name,
        content,
        content,
        content,
        "secret:revision-key",
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn intent_full(
    source_id: OpaqueId,
    residency: ResidencyClosure,
    revision: u64,
    name: &str,
    plaintext_seed: u8,
    ciphertext_seed: u8,
    binding_seed: u8,
    key_name: &str,
) -> RevisionWriteIntent {
    let key = RevisionKey {
        source_id,
        revision: NonZeroRevision::new(revision).expect("revision"),
        residency,
    };
    let residency_key = residency.scope_id().expect("scope id");
    RevisionWriteIntent {
        key,
        source_binding_revision: NonZeroRevision::new(1).expect("revision"),
        payload: payload_with_key(plaintext_seed, ciphertext_seed, key_name),
        envelope: envelope(binding_seed),
        ingest: ingest(name, plaintext_seed),
        storage_object_id: OpaqueId::new(format!("object:{name}")).expect("object"),
        residency_key,
        legacy_migration: None,
        authorization_receipt: Some(
            ReceiptRef::new(format!("receipt:authorization:{name}"))
                .expect("receipt"),
        ),
        operation: operation(name, plaintext_seed),
    }
}

pub(super) fn readback(
    intent: &RevisionWriteIntent,
) -> RevisionObjectReadback {
    RevisionObjectReadback {
        key: intent.key.clone(),
        storage_object_id: intent.storage_object_id.clone(),
        ciphertext_digest: intent.payload.ciphertext_digest,
        ciphertext_bytes: u64::try_from(intent.payload.ciphertext_len())
            .expect("length"),
        plaintext_digest: intent.payload.plaintext_digest,
        plaintext_bytes: intent.payload.plaintext_bytes,
        encryption: intent.payload.encryption.clone(),
        envelope: intent.envelope,
        readback_verified: true,
        object_receipt: Some(
            ReceiptRef::new("receipt:object").expect("receipt"),
        ),
    }
}

pub(super) fn confirm(
    store: &mut RevisionStore,
    intent: &RevisionWriteIntent,
) -> RevisionStoreReceipt {
    store
        .prepare_append(intent.clone())
        .expect("prepare must succeed");
    store
        .confirm_append(&intent.key, &intent.operation, readback(intent))
        .expect("confirm must succeed")
}

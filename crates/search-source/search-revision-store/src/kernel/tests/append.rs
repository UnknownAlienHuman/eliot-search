use search_contracts::{Blake3Digest32, NonZeroRevision, OpaqueId, ReceiptRef};

use super::fixtures::*;
use super::super::*;

#[test]
fn encrypted_payload_debug_never_dumps_ciphertext() {
    let mut payload = payload(7, 7);
    // Deliberately distinct from both public digest byte patterns (7 and 8).
    payload.nonce = vec![113, 59, 211, 41];
    payload.ciphertext = vec![229, 17, 193, 83, 251, 47];
    let nonce_sentinel = format!("{:?}", payload.nonce());
    let ciphertext_sentinel = format!("{:?}", payload.ciphertext());
    for debug in [format!("{payload:?}"), format!("{payload:#?}")] {
        assert!(!debug.contains(&nonce_sentinel));
        assert!(!debug.contains(&ciphertext_sentinel));
        assert!(!debug.contains("229,"));
        assert!(!debug.contains("113,"));
        assert!(debug.contains("<4 bytes>"));
        assert!(debug.contains("<6 encrypted bytes>"));
    }
    // Positive controls: the sentinels would detect an accidental raw dump.
    assert!(format!("{:?}", payload.ciphertext()).contains(&ciphertext_sentinel));
    assert!(format!("{:?}", payload.nonce()).contains(&nonce_sentinel));
}

#[test]
fn first_revision_must_be_one_and_revisions_are_sequential() {
    let mut store = RevisionStore::new(DEFAULT_REVISION_STORE_LIMITS)
        .expect("store");
    assert_eq!(
        store.prepare_append(intent(2, "two", 2)),
        Err(RevisionStoreError::RevisionSequenceInvalid)
    );
    let first = intent(1, "one", 1);
    store.prepare_append(first.clone()).expect("prepare");
    store
        .confirm_append(&first.key, &first.operation, readback(&first))
        .expect("confirm");
    assert!(matches!(
        store.prepare_append(intent(2, "two", 2)),
        Ok(PrepareAppendResult::Prepared(_))
    ));
}

#[test]
fn exact_active_revision_replays_without_rewrite() {
    let mut store = RevisionStore::new(DEFAULT_REVISION_STORE_LIMITS)
        .expect("store");
    let intent = intent(1, "one", 1);
    store.prepare_append(intent.clone()).expect("prepare");
    store
        .confirm_append(&intent.key, &intent.operation, readback(&intent))
        .expect("confirm");
    let PrepareAppendResult::AlreadyStored(receipt) = store
        .prepare_append(intent)
        .expect("replay")
    else {
        panic!("exact immutable revision must replay")
    };
    assert!(receipt.replayed);
}

#[test]
fn same_revision_with_other_content_is_conflict() {
    let mut store = RevisionStore::new(DEFAULT_REVISION_STORE_LIMITS)
        .expect("store");
    let first = intent(1, "one", 1);
    store.prepare_append(first.clone()).expect("prepare");
    store
        .confirm_append(&first.key, &first.operation, readback(&first))
        .expect("confirm");
    assert_eq!(
        store.prepare_append(intent(1, "other", 9)),
        Err(RevisionStoreError::RevisionConflict)
    );
}

#[test]
fn unknown_write_is_not_active_success() {
    let mut store = RevisionStore::new(DEFAULT_REVISION_STORE_LIMITS)
        .expect("store");
    let intent = intent(1, "one", 1);
    store.prepare_append(intent.clone()).expect("prepare");
    store
        .mark_outcome_unknown(&intent.key, &intent.operation)
        .expect("unknown");
    assert_eq!(
        store.active_record(&intent.key),
        Err(RevisionStoreError::OutcomeUnknown)
    );
}

#[test]
fn exact_readback_recovers_unknown_write() {
    let mut store = RevisionStore::new(DEFAULT_REVISION_STORE_LIMITS)
        .expect("store");
    let intent = intent(1, "one", 1);
    store.prepare_append(intent.clone()).expect("prepare");
    store
        .mark_outcome_unknown(&intent.key, &intent.operation)
        .expect("unknown");
    assert!(matches!(
        store
            .recover_unknown(
                &intent.key,
                &intent.operation,
                Some(readback(&intent)),
            )
            .expect("recover"),
        RecoveryResult::Applied(_)
    ));
    assert!(store.active_record(&intent.key).is_ok());
}

#[test]
fn absent_readback_removes_unknown_intent_for_retry() {
    let mut store = RevisionStore::new(DEFAULT_REVISION_STORE_LIMITS)
        .expect("store");
    let intent = intent(1, "one", 1);
    store.prepare_append(intent.clone()).expect("prepare");
    store
        .mark_outcome_unknown(&intent.key, &intent.operation)
        .expect("unknown");
    assert_eq!(
        store
            .recover_unknown(&intent.key, &intent.operation, None)
            .expect("recover"),
        RecoveryResult::NotApplied
    );
    assert_eq!(
        store.state(&intent.key),
        Err(RevisionStoreError::RevisionNotFound)
    );
}

#[test]
fn contradictory_readback_quarantines_revision() {
    let mut store = RevisionStore::new(DEFAULT_REVISION_STORE_LIMITS)
        .expect("store");
    let intent = intent(1, "one", 1);
    store.prepare_append(intent.clone()).expect("prepare");
    store
        .mark_outcome_unknown(&intent.key, &intent.operation)
        .expect("unknown");
    let mut wrong = readback(&intent);
    wrong.ciphertext_digest = Blake3Digest32::from_bytes([99; 32]);
    assert_eq!(
        store
            .recover_unknown(&intent.key, &intent.operation, Some(wrong))
            .expect("recover"),
        RecoveryResult::Quarantined
    );
    assert_eq!(
        store.active_record(&intent.key),
        Err(RevisionStoreError::Quarantined)
    );
}

#[test]
fn operation_id_reuse_with_other_request_digest_is_rejected() {
    let mut store = RevisionStore::new(DEFAULT_REVISION_STORE_LIMITS)
        .expect("store");
    let first = intent(1, "same", 1);
    store.prepare_append(first.clone()).expect("prepare");
    store
        .confirm_append(&first.key, &first.operation, readback(&first))
        .expect("confirm");
    let mut second = intent(2, "second", 2);
    second.operation = RevisionOperation::new(
        first.operation.operation_id().clone(),
        Blake3Digest32::from_bytes([88; 32]),
    );
    assert_eq!(
        store.prepare_append(second),
        Err(RevisionStoreError::OperationConflict)
    );
}

#[test]
fn equivalent_residency_reuse_verifies_exact_bytes() {
    let mut store = RevisionStore::new(DEFAULT_REVISION_STORE_LIMITS).expect("store");
    let first = intent(1, "one", 1);
    confirm(&mut store, &first);
    let PrepareAppendResult::AlreadyStored(receipt) =
        store.prepare_append(intent(1, "one", 1)).expect("replay")
    else {
        panic!("exact immutable revision must replay")
    };
    assert!(receipt.replayed);
    assert_eq!(receipt.residency, baseline_residency());
    assert_eq!(
        store.prepare_append(intent(1, "other", 9)),
        Err(RevisionStoreError::RevisionConflict)
    );
    let mut clash = intent(2, "two", 2);
    clash.storage_object_id = first.storage_object_id.clone();
    assert_eq!(
        store.prepare_append(clash),
        Err(RevisionStoreError::RevisionConflict)
    );
}

#[test]
fn same_occurrence_with_different_residency_is_typed_conflict() {
    let mut store = RevisionStore::new(DEFAULT_REVISION_STORE_LIMITS).expect("store");
    let pending = intent(1, "one", 1);
    store.prepare_append(pending.clone()).expect("prepare");
    let variant = intent_full(
        source("test"),
        residency_variant(0),
        1,
        "variant",
        9,
        0x43,
        0x53,
        "secret:variant-key",
    );
    assert_eq!(
        store.prepare_append(variant.clone()),
        Err(RevisionStoreError::ResidencyMismatch)
    );
    store
        .confirm_append(&pending.key, &pending.operation, readback(&pending))
        .expect("confirm");
    assert_eq!(
        store.prepare_append(variant),
        Err(RevisionStoreError::ResidencyMismatch)
    );
}

#[test]
fn wrong_envelope_binding_fails_prepare_and_confirm() {
    let mut store = RevisionStore::new(DEFAULT_REVISION_STORE_LIMITS).expect("store");
    let mut versioned = intent(1, "bad-version", 1);
    versioned.envelope.version = 2;
    assert_eq!(
        store.prepare_append(versioned),
        Err(RevisionStoreError::EnvelopeInvalid)
    );
    let mut lengthed = intent(1, "bad-length", 1);
    lengthed.envelope.plaintext_length = 99;
    assert_eq!(
        store.prepare_append(lengthed),
        Err(RevisionStoreError::EnvelopeInvalid)
    );
    let mut generated = intent(1, "bad-generation", 1);
    generated.envelope.key_generation = NonZeroRevision::new(2).expect("generation");
    assert_eq!(
        store.prepare_append(generated),
        Err(RevisionStoreError::EnvelopeInvalid)
    );
    let good = intent(1, "good", 1);
    store.prepare_append(good.clone()).expect("prepare");
    let mut rebound = readback(&good);
    rebound.envelope.residency_binding_digest = [0xFF; 32];
    assert_eq!(
        store.confirm_append(&good.key, &good.operation, rebound),
        Err(RevisionStoreError::ReadbackMismatch)
    );
    let mut redomained = readback(&good);
    redomained.key.residency = residency_variant(5);
    assert_eq!(
        store.confirm_append(&good.key, &good.operation, redomained),
        Err(RevisionStoreError::ReadbackMismatch)
    );
}

#[test]
fn truncated_ciphertext_is_rejected() {
    let short = EncryptedRevisionPayload::new(
        Blake3Digest32::from_bytes([9; 32]),
        3,
        Blake3Digest32::from_bytes([10; 32]),
        vec![9; 12],
        vec![9; 5],
        EncryptionBinding {
            key_reference: OpaqueId::new("secret:revision-key").expect("key"),
            key_version: NonZeroRevision::new(1).expect("version"),
            cipher_suite: CipherSuite::AuthenticatedEncryptionV1,
        },
        DEFAULT_REVISION_STORE_LIMITS,
    );
    assert_eq!(short, Err(RevisionStoreError::CiphertextSizeInvalid));
    let tagged = EncryptedRevisionPayload::new(
        Blake3Digest32::from_bytes([9; 32]),
        3,
        Blake3Digest32::from_bytes([10; 32]),
        vec![9; 12],
        vec![9; 16],
        EncryptionBinding {
            key_reference: OpaqueId::new("secret:revision-key").expect("key"),
            key_version: NonZeroRevision::new(1).expect("version"),
            cipher_suite: CipherSuite::AuthenticatedEncryptionV1,
        },
        DEFAULT_REVISION_STORE_LIMITS,
    );
    assert!(tagged.is_ok());
}

#[test]
fn legacy_migration_requires_explicit_receipt() {
    let mut store = RevisionStore::new(DEFAULT_REVISION_STORE_LIMITS).expect("store");
    let mut legacy = intent(1, "legacy", 1);
    legacy.residency_key = OpaqueId::new("residency:legacy-opaque").expect("legacy");
    assert_eq!(
        store.prepare_append(legacy.clone()),
        Err(RevisionStoreError::ResidencyMismatch)
    );
    legacy.legacy_migration = Some(migrate_legacy_residency_key(
        legacy.residency_key.clone(),
        ReceiptRef::new("receipt:migration:1").expect("receipt"),
    ));
    store
        .prepare_append(legacy.clone())
        .expect("migrated prepare");
    let receipt = store
        .confirm_append(&legacy.key, &legacy.operation, readback(&legacy))
        .expect("migrated confirm");
    assert_eq!(receipt.residency, baseline_residency());
    let mut smuggled = intent_full(
        source("smuggled"),
        baseline_residency(),
        1,
        "smuggled",
        4,
        4,
        4,
        "secret:revision-key",
    );
    smuggled.residency_key = OpaqueId::new("residency:legacy-a").expect("legacy");
    smuggled.legacy_migration = Some(migrate_legacy_residency_key(
        OpaqueId::new("residency:legacy-b").expect("legacy"),
        ReceiptRef::new("receipt:migration:2").expect("receipt"),
    ));
    assert_eq!(
        store.prepare_append(smuggled),
        Err(RevisionStoreError::ResidencyMismatch)
    );
}

#[test]
fn t13_ingest_hex_fields_are_validated() {
    let receipt = ReceiptRef::new("receipt:t13-admission:probe").expect("receipt");
    let policy = NonZeroRevision::new(1).expect("policy");
    for field in ["short", "UPPERCASE-HEX-DIGEST-PLACEHOLDER-INVALID!!", "zz"] {
        let candidate = OpaqueId::new(field).expect("candidate");
        assert_eq!(
            CanonicalIngestBinding::new(
                receipt.clone(),
                hex_opaque(0xA0),
                policy,
                candidate,
                hex_opaque(0x07),
                hex_opaque(0x08),
            ),
            Err(RevisionStoreError::IngestBindingInvalid)
        );
    }
    let uppercase = OpaqueId::new("A".repeat(64)).expect("candidate");
    assert_eq!(
        CanonicalIngestBinding::new(
            receipt.clone(),
            uppercase,
            policy,
            hex_opaque(0xB0),
            hex_opaque(0x07),
            hex_opaque(0x08),
        ),
        Err(RevisionStoreError::IngestBindingInvalid)
    );
    assert!(
        CanonicalIngestBinding::new(
            receipt,
            hex_opaque(0xA0),
            policy,
            hex_opaque(0xB0),
            hex_opaque(0x07),
            hex_opaque(0x08),
        )
        .is_ok()
    );
}

#[test]
fn restart_has_no_history_and_debug_hides_plaintext_bytes() {
    let mut store = RevisionStore::new(DEFAULT_REVISION_STORE_LIMITS).expect("store");
    let mut sentinel = intent(1, "sentinel", 1);
    sentinel.payload = payload_with_key(0x01, 0xE5, "secret:revision-key");
    sentinel.envelope = envelope(0x01);
    let nonce_sentinel = format!("{:?}", sentinel.payload.nonce());
    let ciphertext_sentinel = format!("{:?}", sentinel.payload.ciphertext());
    assert!(nonce_sentinel.contains("113") || ciphertext_sentinel.contains("229"));
    let stored = confirm(&mut store, &sentinel);
    let record = store.active_record(&sentinel.key).expect("record");
    for debug in [
        format!("{:?}", sentinel.payload),
        format!("{sentinel:?}"),
        format!("{record:?}"),
        format!("{:?}", readback(&sentinel)),
        format!("{stored:?}"),
    ] {
        assert!(!debug.contains("229"), "ciphertext residue: {debug}");
        assert!(!debug.contains("113"), "nonce residue: {debug}");
    }
    drop(store);
    let fresh = RevisionStore::new(DEFAULT_REVISION_STORE_LIMITS).expect("store");
    assert!(fresh.is_empty());
    assert_eq!(
        fresh.state(&sentinel.key),
        Err(RevisionStoreError::RevisionNotFound)
    );
}

#[test]
fn operation_reuse_with_different_key_is_rejected() {
    let mut store = RevisionStore::new(DEFAULT_REVISION_STORE_LIMITS).expect("store");
    let first = intent(1, "same", 1);
    confirm(&mut store, &first);
    let mut second = intent(2, "second", 2);
    second.operation = RevisionOperation::new(
        first.operation.operation_id().clone(),
        Blake3Digest32::from_bytes([88; 32]),
    );
    assert_eq!(
        store.prepare_append(second),
        Err(RevisionStoreError::OperationConflict)
    );
    let mut third = intent(2, "third", 1);
    third.operation = first.operation.clone();
    assert_eq!(
        store.prepare_append(third),
        Err(RevisionStoreError::OperationConflict)
    );
}

//! Shared append, recovery, and lifecycle validation helpers.

use std::collections::BTreeMap;

use search_contracts::{Blake3Digest32, NonZeroRevision, OpaqueId};

use super::super::error::RevisionStoreError;
use super::super::limits::{
    ENVELOPE_BINDING_VERSION, MIN_CIPHERTEXT_BYTES, RevisionStoreLimits,
};
use super::super::model::{
    PurgeTombstone, PurgeTombstoneReceipt, RevisionKey,
    RevisionObjectReadback, RevisionOperation, RevisionRecord, RevisionState,
    RevisionStoreReceipt, RevisionWriteIntent,
};
use super::super::residency::ResidencyClosure;

pub(super) fn validate_intent(
    intent: &RevisionWriteIntent,
    limits: RevisionStoreLimits,
) -> Result<(), RevisionStoreError> {
    let limits = limits.validate()?;
    if intent.payload.plaintext_bytes == 0
        || intent.payload.plaintext_bytes > limits.max_plaintext_bytes
    {
        return Err(RevisionStoreError::PlaintextSizeInvalid);
    }
    let ciphertext_bytes = u64::try_from(intent.payload.ciphertext_len())
        .map_err(|_| RevisionStoreError::CiphertextSizeInvalid)?;
    if ciphertext_bytes == 0
        || ciphertext_bytes > limits.max_ciphertext_bytes
        || intent.payload.ciphertext_len() < MIN_CIPHERTEXT_BYTES
    {
        return Err(RevisionStoreError::CiphertextSizeInvalid);
    }
    if intent.payload.nonce().is_empty()
        || intent.payload.nonce().len() > limits.max_nonce_bytes
    {
        return Err(RevisionStoreError::NonceInvalid);
    }
    if intent.authorization_receipt.is_none() {
        return Err(RevisionStoreError::EvidenceMissing);
    }
    if intent.envelope.version != ENVELOPE_BINDING_VERSION {
        return Err(RevisionStoreError::EnvelopeInvalid);
    }
    if intent.envelope.plaintext_length != intent.payload.plaintext_bytes {
        return Err(RevisionStoreError::EnvelopeInvalid);
    }
    if intent.envelope.key_generation != intent.payload.encryption.key_version {
        return Err(RevisionStoreError::EnvelopeInvalid);
    }
    match &intent.legacy_migration {
        Some(migration) => {
            if intent.residency_key != migration.legacy_residency_key {
                return Err(RevisionStoreError::ResidencyMismatch);
            }
        }
        None => {
            if intent.residency_key != intent.key.residency.scope_id()? {
                return Err(RevisionStoreError::ResidencyMismatch);
            }
        }
    }
    Ok(())
}

/// Returns the residency bound to one occurrence, if any state names it.
///
/// Occurrence identity is `(source_id, revision)`; the map key additionally
/// carries the closure so this scan is the single binding check.
pub(super) fn occurrence_residency(
    states: &BTreeMap<RevisionKey, RevisionState>,
    source_id: &OpaqueId,
    revision: NonZeroRevision,
) -> Option<ResidencyClosure> {
    states
        .keys()
        .find(|key| key.source_id == *source_id && key.revision == revision)
        .map(|key| key.residency)
}

/// Detects physical reuse across inequivalent residency closures.
///
/// Object identities, ciphertext digests, envelope binding digests, and
/// secret references must never cross residency boundaries. Inside one
/// equivalent closure, sharing one object identity for different bytes is an
/// immutable object conflict, never silent overwriting.
pub(super) fn reuse_conflict(
    states: &BTreeMap<RevisionKey, RevisionState>,
    intent: &RevisionWriteIntent,
) -> Option<RevisionStoreError> {
    for state in states.values() {
        let (
            residency,
            storage_object_id,
            ciphertext_digest,
            key_reference,
            source_binding_digest,
            residency_binding_digest,
        ) = match state {
            RevisionState::Pending(existing) | RevisionState::OutcomeUnknown(existing) => (
                existing.key.residency,
                &existing.storage_object_id,
                existing.payload.ciphertext_digest,
                &existing.payload.encryption.key_reference,
                existing.envelope.source_revision_binding_digest,
                existing.envelope.residency_binding_digest,
            ),
            RevisionState::Active(record) => (
                record.key.residency,
                &record.storage_object_id,
                record.ciphertext_digest,
                &record.encryption.key_reference,
                record.envelope.source_revision_binding_digest,
                record.envelope.residency_binding_digest,
            ),
            RevisionState::Quarantined { .. } => continue,
        };
        if residency == intent.key.residency {
            if storage_object_id == &intent.storage_object_id
                && ciphertext_digest != intent.payload.ciphertext_digest
            {
                return Some(RevisionStoreError::RevisionConflict);
            }
            continue;
        }
        if storage_object_id == &intent.storage_object_id
            || ciphertext_digest == intent.payload.ciphertext_digest
            || key_reference == &intent.payload.encryption.key_reference
            || source_binding_digest == intent.envelope.source_revision_binding_digest
            || residency_binding_digest == intent.envelope.residency_binding_digest
        {
            return Some(RevisionStoreError::ResidencyMismatch);
        }
    }
    None
}

/// Records one append operation identity unless it is already indexed.
///
/// Re-admission of a deleted occurrence reuses its bound operation identity;
/// the index keeps the first entry so history never duplicates.
pub(super) fn push_operation(
    operations: &mut Vec<(OpaqueId, Blake3Digest32, RevisionStoreReceipt)>,
    operation: &RevisionOperation,
    receipt: RevisionStoreReceipt,
) {
    if !operations
        .iter()
        .any(|(operation_id, _, _)| operation_id == operation.operation_id())
    {
        operations.push((
            operation.operation_id().clone(),
            operation.request_digest(),
            receipt,
        ));
    }
}

/// Builds a content-free tombstone install receipt.
pub(super) fn tombstone_receipt(
    tombstone: &PurgeTombstone,
    replayed: bool,
) -> PurgeTombstoneReceipt {
    PurgeTombstoneReceipt {
        scope: tombstone.scope,
        generation: tombstone.generation,
        tombstone_receipt: tombstone.tombstone_receipt.clone(),
        operation: tombstone.operation.clone(),
        replayed,
    }
}

pub(super) fn validate_next_source_revision(
    states: &BTreeMap<RevisionKey, RevisionState>,
    key: &RevisionKey,
) -> Result<(), RevisionStoreError> {
    let latest = states
        .keys()
        .filter(|existing| existing.source_id == key.source_id)
        .map(|existing| existing.revision)
        .max();
    match latest {
        None if key.revision.get() == 1 => Ok(()),
        Some(current)
            if current
                .checked_next()
                .map_err(|_| RevisionStoreError::ContractExhausted)?
                == key.revision =>
        {
            Ok(())
        }
        None | Some(_) => Err(RevisionStoreError::RevisionSequenceInvalid),
    }
}

pub(super) fn exact_record_matches_intent(
    record: &RevisionRecord,
    intent: &RevisionWriteIntent,
) -> bool {
    record.key == intent.key
        && record.source_binding_revision == intent.source_binding_revision
        && record.content_digest == intent.payload.plaintext_digest
        && record.plaintext_bytes == intent.payload.plaintext_bytes
        && record.ciphertext_digest == intent.payload.ciphertext_digest
        && record.storage_object_id == intent.storage_object_id
        && record.residency_key == intent.residency_key
        && record.encryption == intent.payload.encryption
        && record.envelope == intent.envelope
        && record.ingest == intent.ingest
        && record.operation == intent.operation
}

pub(super) fn record_from_readback(
    intent: &RevisionWriteIntent,
    readback: RevisionObjectReadback,
) -> Result<RevisionRecord, RevisionStoreError> {
    if !readback.readback_verified {
        return Err(RevisionStoreError::EvidenceMissing);
    }
    let object_receipt = readback
        .object_receipt
        .ok_or(RevisionStoreError::EvidenceMissing)?;
    let authorization_receipt = intent
        .authorization_receipt
        .clone()
        .ok_or(RevisionStoreError::EvidenceMissing)?;
    let expected_ciphertext_bytes = u64::try_from(intent.payload.ciphertext_len())
        .map_err(|_| RevisionStoreError::BackendContractViolation)?;
    if readback.key != intent.key
        || readback.storage_object_id != intent.storage_object_id
        || readback.ciphertext_digest != intent.payload.ciphertext_digest
        || readback.ciphertext_bytes != expected_ciphertext_bytes
        || readback.plaintext_digest != intent.payload.plaintext_digest
        || readback.plaintext_bytes != intent.payload.plaintext_bytes
        || readback.encryption != intent.payload.encryption
        || readback.envelope != intent.envelope
    {
        return Err(RevisionStoreError::ReadbackMismatch);
    }
    Ok(RevisionRecord {
        key: intent.key.clone(),
        source_binding_revision: intent.source_binding_revision,
        content_digest: intent.payload.plaintext_digest,
        plaintext_bytes: intent.payload.plaintext_bytes,
        ciphertext_digest: intent.payload.ciphertext_digest,
        ciphertext_bytes: expected_ciphertext_bytes,
        storage_object_id: intent.storage_object_id.clone(),
        residency_key: intent.residency_key.clone(),
        encryption: intent.payload.encryption.clone(),
        envelope: intent.envelope,
        ingest: intent.ingest.clone(),
        authorization_receipt,
        object_receipt,
        operation: intent.operation.clone(),
    })
}

pub(super) fn receipt_from_record(
    record: &RevisionRecord,
    replayed: bool,
) -> RevisionStoreReceipt {
    RevisionStoreReceipt {
        key: record.key.clone(),
        residency: record.key.residency,
        operation: record.operation.clone(),
        content_digest: record.content_digest,
        ciphertext_digest: record.ciphertext_digest,
        object_receipt: record.object_receipt.clone(),
        replayed,
    }
}

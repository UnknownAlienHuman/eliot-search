//! Append preparation and exact durable confirmation.

use super::super::error::RevisionStoreError;
use super::super::model::{
    PrepareAppendResult, RevisionKey, RevisionObjectReadback,
    RevisionOperation, RevisionState, RevisionStoreReceipt,
    RevisionWriteIntent,
};
use super::support::{
    exact_record_matches_intent, occurrence_residency, push_operation,
    receipt_from_record, record_from_readback, reuse_conflict, validate_intent,
    validate_next_source_revision,
};
use super::RevisionStore;

impl RevisionStore {
    /// Prepares one append-only revision intent or replays an exact active receipt.
    ///
    /// Reuse across inequivalent residency closures is a typed
    /// [`RevisionStoreError::ResidencyMismatch`]; reuse inside one equivalent
    /// closure replays only after verifying exact bytes. A bound operation
    /// whose state was deleted falls through to honest re-admission with
    /// fresh readback instead of replaying a stale receipt.
    pub fn prepare_append(
        &mut self,
        intent: RevisionWriteIntent,
    ) -> Result<PrepareAppendResult, RevisionStoreError> {
        validate_intent(&intent, self.limits)?;
        if let Some((_, digest, receipt)) = self
            .operations
            .iter()
            .find(|(operation_id, _, _)| operation_id == intent.operation.operation_id())
        {
            let bound = *digest == intent.operation.request_digest()
                && receipt.key == intent.key
                && receipt.content_digest == intent.payload.plaintext_digest
                && receipt.ciphertext_digest == intent.payload.ciphertext_digest;
            if !bound {
                return Err(RevisionStoreError::OperationConflict);
            }
            if let Some(RevisionState::Active(record)) = self.states.get(&intent.key)
                && record.operation == intent.operation
                && exact_record_matches_intent(record, &intent)
            {
                let mut replay = receipt.clone();
                replay.replayed = true;
                return Ok(PrepareAppendResult::AlreadyStored(replay));
            }
        }
        if self
            .deletions
            .iter()
            .any(|(operation_id, _, _)| operation_id == intent.operation.operation_id())
            || self.tombstones.iter().any(|tombstone| {
                tombstone.operation.operation_id() == intent.operation.operation_id()
            })
        {
            return Err(RevisionStoreError::OperationConflict);
        }
        if self.is_fenced(&intent.key) {
            return Err(RevisionStoreError::Tombstoned);
        }
        if let Some(bound) =
            occurrence_residency(&self.states, &intent.key.source_id, intent.key.revision)
            && bound != intent.key.residency
        {
            return Err(RevisionStoreError::ResidencyMismatch);
        }
        if self.operation_count() >= self.limits.max_operations
            || self.states.len() >= self.limits.max_revisions
        {
            return Err(RevisionStoreError::CapacityExceeded);
        }
        if let Some(existing) = self.states.get(&intent.key) {
            return match existing {
                RevisionState::Active(record) if exact_record_matches_intent(record, &intent) => {
                    Ok(PrepareAppendResult::AlreadyStored(receipt_from_record(
                        record, true,
                    )))
                }
                RevisionState::Pending(existing) | RevisionState::OutcomeUnknown(existing) => {
                    if existing == &intent {
                        Ok(PrepareAppendResult::Prepared(existing.clone()))
                    } else if existing.operation == intent.operation {
                        Err(RevisionStoreError::OperationConflict)
                    } else {
                        Err(RevisionStoreError::RevisionConflict)
                    }
                }
                RevisionState::Active(_) | RevisionState::Quarantined { .. } => {
                    Err(RevisionStoreError::RevisionConflict)
                }
            };
        }
        if let Some(conflict) = reuse_conflict(&self.states, &intent) {
            return Err(conflict);
        }
        validate_next_source_revision(&self.states, &intent.key)?;
        self.states
            .insert(intent.key.clone(), RevisionState::Pending(intent.clone()));
        Ok(PrepareAppendResult::Prepared(intent))
    }

    /// Confirms an exact prepared write after authoritative durable readback.
    pub fn confirm_append(
        &mut self,
        key: &RevisionKey,
        operation: &RevisionOperation,
        readback: RevisionObjectReadback,
    ) -> Result<RevisionStoreReceipt, RevisionStoreError> {
        let intent = match self.state(key)? {
            RevisionState::Pending(intent) | RevisionState::OutcomeUnknown(intent)
                if &intent.operation == operation => intent.clone(),
            RevisionState::Pending(_) | RevisionState::OutcomeUnknown(_) => {
                return Err(RevisionStoreError::OperationConflict);
            }
            RevisionState::Active(record) if &record.operation == operation => {
                return Ok(receipt_from_record(record, true));
            }
            RevisionState::Active(_) => return Err(RevisionStoreError::RevisionConflict),
            RevisionState::Quarantined { .. } => return Err(RevisionStoreError::Quarantined),
        };
        if self.is_fenced(key) {
            return Err(RevisionStoreError::Tombstoned);
        }
        let record = record_from_readback(&intent, readback)?;
        let receipt = receipt_from_record(&record, false);
        self.states
            .insert(key.clone(), RevisionState::Active(record));
        push_operation(&mut self.operations, operation, receipt.clone());
        Ok(receipt)
    }
}

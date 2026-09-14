//! Unknown-outcome marking and exact authoritative recovery.

use super::super::error::RevisionStoreError;
use super::super::model::{
    RecoveryResult, RevisionKey, RevisionObjectReadback, RevisionOperation,
    RevisionState,
};
use super::support::{push_operation, receipt_from_record, record_from_readback};
use super::RevisionStore;

impl RevisionStore {
    /// Marks a possible external object write as unresolved.
    pub fn mark_outcome_unknown(
        &mut self,
        key: &RevisionKey,
        operation: &RevisionOperation,
    ) -> Result<(), RevisionStoreError> {
        let state = self
            .states
            .get_mut(key)
            .ok_or(RevisionStoreError::RevisionNotFound)?;
        match state {
            RevisionState::Pending(intent) if &intent.operation == operation => {
                *state = RevisionState::OutcomeUnknown(intent.clone());
                Ok(())
            }
            RevisionState::Pending(_) | RevisionState::OutcomeUnknown(_) => {
                Err(RevisionStoreError::OperationConflict)
            }
            RevisionState::Active(_) => Err(RevisionStoreError::RevisionConflict),
            RevisionState::Quarantined { .. } => Err(RevisionStoreError::Quarantined),
        }
    }

    /// Recovers a possible write by exact authoritative readback.
    pub fn recover_unknown(
        &mut self,
        key: &RevisionKey,
        operation: &RevisionOperation,
        readback: Option<RevisionObjectReadback>,
    ) -> Result<RecoveryResult, RevisionStoreError> {
        let intent = match self.state(key)? {
            RevisionState::OutcomeUnknown(intent) if &intent.operation == operation => {
                intent.clone()
            }
            RevisionState::OutcomeUnknown(_) | RevisionState::Pending(_) => {
                return Err(RevisionStoreError::OperationConflict);
            }
            RevisionState::Active(record) if &record.operation == operation => {
                return Ok(RecoveryResult::Applied(receipt_from_record(record, true)));
            }
            RevisionState::Active(_) => return Err(RevisionStoreError::RevisionConflict),
            RevisionState::Quarantined { .. } => return Err(RevisionStoreError::Quarantined),
        };
        if self.is_fenced(key) {
            return Err(RevisionStoreError::Tombstoned);
        }
        let Some(readback) = readback else {
            self.states.remove(key);
            return Ok(RecoveryResult::NotApplied);
        };
        match record_from_readback(&intent, readback) {
            Ok(record) => {
                let receipt = receipt_from_record(&record, false);
                self.states
                    .insert(key.clone(), RevisionState::Active(record));
                push_operation(&mut self.operations, operation, receipt.clone());
                Ok(RecoveryResult::Applied(receipt))
            }
            Err(
                RevisionStoreError::ReadbackMismatch
                | RevisionStoreError::EvidenceMissing
                | RevisionStoreError::BackendContractViolation,
            ) => {
                self.states.insert(
                    key.clone(),
                    RevisionState::Quarantined {
                        key: key.clone(),
                        operation: operation.clone(),
                    },
                );
                Ok(RecoveryResult::Quarantined)
            }
            Err(error) => Err(error),
        }
    }
}

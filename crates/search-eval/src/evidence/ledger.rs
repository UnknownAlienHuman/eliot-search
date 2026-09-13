//! Idempotent finite process-local attempt ledger.

use std::collections::BTreeMap;

use search_contracts::Blake3Digest32;

use crate::EvalError;

use super::attempt::ValidatedCaseEvidence;

/// Idempotent process-local attempt ledger that preserves original failures.
#[derive(Clone, Debug)]
pub struct EvidenceLedger {
    max_attempts: usize,
    attempts: BTreeMap<Blake3Digest32, ValidatedCaseEvidence>,
}

impl EvidenceLedger {
    /// Creates a finite attempt ledger.
    pub const fn new(max_attempts: usize) -> Result<Self, EvalError> {
        if max_attempts == 0 {
            return Err(EvalError::InvalidLimits);
        }
        Ok(Self {
            max_attempts,
            attempts: BTreeMap::new(),
        })
    }

    /// Records one exact validated attempt or returns an idempotent replay.
    pub fn record(
        &mut self,
        evidence: ValidatedCaseEvidence,
    ) -> Result<&ValidatedCaseEvidence, EvalError> {
        let key = evidence.evidence().attempt_digest;
        if self.attempts.contains_key(&key) {
            if self.attempts.get(&key) != Some(&evidence) {
                return Err(EvalError::AttemptConflict);
            }
            return self.attempts.get(&key).ok_or(EvalError::AttemptConflict);
        }
        if self.attempts.len() >= self.max_attempts {
            return Err(EvalError::BudgetExceeded);
        }
        self.attempts.insert(key, evidence);
        self.attempts.get(&key).ok_or(EvalError::AttemptConflict)
    }

    /// Deterministically ordered accepted evidence.
    #[must_use]
    pub fn evidence(&self) -> impl ExactSizeIterator<Item = &ValidatedCaseEvidence> {
        self.attempts.values()
    }
}

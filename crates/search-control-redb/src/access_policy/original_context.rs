//! Additive original-context variants of the bounded policy-record path.
//!
//! These methods reuse the identical record key, the identical pre-built
//! conditional command and the same checked read/commit/recovery engine. They
//! add no journal, record map, context or reset: the caller's single original
//! absolute deadline and cumulative work callback are borrowed for the whole
//! call and are never renewed.
//!
//! Key construction, record decode, command preparation and the retained
//! descriptor clone are reserved from actual input sizes before that work runs.
//! A checkpoint alone never substitutes for concrete accounting, and the first
//! callback refusal is preserved unchanged in the returned failure value.

use std::time::Instant;

use search_contracts::SourceNamespaceId;

use super::{AccessPolicyCommit, AccessPolicyMutation, AccessPolicyReadback, KEY_PREFIX};
use crate::persistent::original_seam::OriginalSeam;
use crate::policy_codec::{ACCESS_POLICY_RECORD_LEN, decode_access_policy};
use crate::{
    CommitRecoveryDecision, ConditionalControlMutation, ControlError, ControlKey,
    ControlRecordClass, MigrationBootstrapFailure, MigrationBootstrapWork as Work,
    PersistentControlJournal,
};

/// Record prefix bytes plus the fixed namespace width.
const NAMESPACE_KEY_BYTES: usize = 16;

impl PersistentControlJournal {
    /// Reads bounded policy metadata under the caller's original allowance.
    ///
    /// The additive twin of the context-budgeted read: one coherent journal
    /// snapshot, the same key, the same class check and the same fail-closed
    /// decode. Absence stays explicit; no default, initialization or write
    /// occurs, and the returned value grants no authority.
    ///
    /// # Errors
    /// Returns the original callback refusal, elapsed absolute deadline, or the
    /// existing unavailable/corrupt classification for this record.
    pub fn read_access_policy_with_original_context<F, E>(
        &self,
        namespace_id: SourceNamespaceId,
        deadline: Instant,
        work: &F,
    ) -> Result<AccessPolicyReadback, MigrationBootstrapFailure<E>>
    where
        F: Fn(Work) -> Result<(), E>,
    {
        let seam = OriginalSeam::new(deadline, work);
        let result = (|| {
            seam.start()?;
            let key = metered_key(namespace_id, &seam)?;
            let snapshot = seam.read_snapshot(self)?;
            let record = snapshot
                .get(&key)
                .map(|value| {
                    if value.class() != ControlRecordClass::State {
                        return Err(ControlError::StoreCorrupt);
                    }
                    metered_decode(value.len(), &seam)?;
                    let record = decode_access_policy(value.as_bytes())
                        .map_err(|_| ControlError::StoreCorrupt)?;
                    if record.namespace_id != namespace_id {
                        return Err(ControlError::StoreCorrupt);
                    }
                    Ok(record)
                })
                .transpose()?;
            seam.complete()?;
            Ok(AccessPolicyReadback {
                identity: snapshot.identity,
                generation: snapshot.generation,
                namespace_id,
                record,
            })
        })();
        result.map_err(|error| seam.failure(error))
    }

    /// Writes one policy row through the existing conditional engine under the
    /// caller's original allowance.
    ///
    /// The additive twin of the context-budgeted commit: the same exact
    /// pre-state, the same generation guard and the same command identity.
    /// Same-command replay returns the original historical receipt and is not a
    /// claim of current policy. A possible write stays outcome-unknown until the
    /// identical request is recovered; no publication occurs here.
    ///
    /// # Errors
    /// Returns the original callback refusal, elapsed absolute deadline, or the
    /// existing identity/transaction/unknown-outcome classification.
    pub fn commit_access_policy_with_original_context<F, E>(
        &mut self,
        mutation: &AccessPolicyMutation,
        deadline: Instant,
        work: &F,
    ) -> Result<AccessPolicyCommit, MigrationBootstrapFailure<E>>
    where
        F: Fn(Work) -> Result<(), E>,
    {
        let seam = OriginalSeam::new(deadline, work);
        let result = (|| {
            seam.start()?;
            if self.identity() != mutation.identity() {
                return Err(ControlError::IdentityMismatch);
            }
            metered_command(&mutation.command, &seam)?;
            let receipt = seam.transact(self, mutation.command.clone())?;
            metered_clone(&mutation.command, &seam)?;
            Ok(AccessPolicyCommit { mutation: mutation.clone(), receipt })
        })();
        result.map_err(|error| seam.failure(error))
    }

    /// Resolves the SAME complete request under the caller's original allowance
    /// without writing or rebuilding its guards.
    ///
    /// The additive twin of the context-budgeted recovery: `Some` proves a
    /// commit, possibly historical; `None` proves resolved absence. Conflict,
    /// quarantine and an interrupted inspection remain errors and never clear
    /// the pending fence.
    ///
    /// # Errors
    /// Returns the original callback refusal, elapsed absolute deadline, or the
    /// existing identity/conflict/quarantine/unknown-outcome classification.
    pub fn recover_access_policy_with_original_context<F, E>(
        &mut self,
        mutation: &AccessPolicyMutation,
        deadline: Instant,
        work: &F,
    ) -> Result<Option<AccessPolicyCommit>, MigrationBootstrapFailure<E>>
    where
        F: Fn(Work) -> Result<(), E>,
    {
        let seam = OriginalSeam::new(deadline, work);
        let result = (|| {
            seam.start()?;
            if self.identity() != mutation.identity() {
                return Err(ControlError::IdentityMismatch);
            }
            metered_command(&mutation.command, &seam)?;
            metered_clone(&mutation.command, &seam)?;
            match seam.recover(self, &mutation.command)? {
                CommitRecoveryDecision::Committed(receipt) => Ok(Some(AccessPolicyCommit {
                    mutation: mutation.clone(),
                    receipt,
                })),
                CommitRecoveryDecision::NotCommittedRetrySameOperation => Ok(None),
                CommitRecoveryDecision::ConflictingInput => Err(ControlError::OperationConflict),
                CommitRecoveryDecision::PartialOrCorruptQuarantine => {
                    Err(ControlError::StoreQuarantined)
                }
            }
        })();
        result.map_err(|error| {
            let error = if seam.interrupted() {
                ControlError::CommitOutcomeUnknown
            } else {
                error
            };
            seam.failure(error)
        })
    }
}

/// Builds the namespace policy key after reserving its exact bytes.
fn metered_key<F, E>(
    namespace_id: SourceNamespaceId,
    seam: &OriginalSeam<'_, F, E>,
) -> Result<ControlKey, ControlError>
where
    F: Fn(Work) -> Result<(), E>,
{
    let bytes = KEY_PREFIX.len().checked_add(NAMESPACE_KEY_BYTES).ok_or(ControlError::BudgetExceeded)?;
    seam.reserve(Work::Allocate(bytes))?;
    seam.reserve(Work::Process(bytes))?;
    let built = super::policy_key(namespace_id)?;
    debug_assert_eq!(built.as_bytes().len(), bytes);
    Ok(built)
}

/// Reserves the bounded policy-record decode before it runs.
fn metered_decode<F, E>(
    observed_bytes: usize,
    seam: &OriginalSeam<'_, F, E>,
) -> Result<(), ControlError>
where
    F: Fn(Work) -> Result<(), E>,
{
    seam.reserve(Work::Read(observed_bytes))?;
    seam.reserve(Work::Allocate(ACCESS_POLICY_RECORD_LEN))?;
    seam.reserve(Work::Process(ACCESS_POLICY_RECORD_LEN))?;
    Ok(())
}

/// Reserves the complete command preparation and the executing engine's own
/// validation work before either runs.
fn metered_command<F, E>(
    command: &ConditionalControlMutation,
    seam: &OriginalSeam<'_, F, E>,
) -> Result<(), ControlError>
where
    F: Fn(Work) -> Result<(), E>,
{
    let bytes = command_capacity(command)?;
    seam.reserve(Work::Allocate(bytes))?;
    seam.reserve(Work::Process(bytes))?;
    Ok(())
}

/// Reserves the retained clone of one prepared descriptor.
fn metered_clone<F, E>(
    command: &ConditionalControlMutation,
    seam: &OriginalSeam<'_, F, E>,
) -> Result<(), ControlError>
where
    F: Fn(Work) -> Result<(), E>,
{
    let bytes = command_capacity(command)?;
    seam.reserve(Work::Allocate(bytes))?;
    seam.reserve(Work::Process(bytes))?;
    Ok(())
}

/// Exact input-derived capacity of one prepared conditional command.
fn command_capacity(command: &ConditionalControlMutation) -> Result<usize, ControlError> {
    let mutation = command.mutation();
    let mut parts = vec![32 + 32 + 8];
    for write in mutation.writes() {
        parts.push(write.key.as_bytes().len() + write.value.len());
    }
    for key in mutation.deletes() {
        parts.push(key.as_bytes().len());
    }
    for condition in command.conditions() {
        parts.push(condition.key().as_bytes().len());
        parts.push(condition.expected().map_or(0, crate::ControlValue::len));
    }
    parts
        .into_iter()
        .try_fold(0_usize, |total, part| {
            total.checked_add(part).ok_or(ControlError::BudgetExceeded)
        })
}

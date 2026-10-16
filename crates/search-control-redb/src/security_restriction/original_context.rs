//! Additive original-context variants of the bounded restriction path.
//!
//! These methods reuse the identical record codec, key construction and checked
//! read/commit/recovery engine. They add no journal, record map, context or
//! reset: the caller's single original absolute deadline and cumulative work
//! callback are borrowed for the whole call and never renewed.
//!
//! Key construction, record decode, clone and full command preparation are
//! reserved from the input sizes before that work runs, so a checkpoint alone
//! never substitutes for concrete accounting. The first callback refusal is
//! preserved unchanged in the returned failure value.

use std::time::Instant;

use search_contracts::SourceNamespaceId;

use super::{
    POLICY_PREFIX, RESTRICTION_PREFIX, SecurityPolicyState, SecurityRestrictionCommit,
    SecurityRestrictionMutation, SecurityRestrictionReadback, codec, key, same_journal_history,
};
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
    /// Reads both records coherently under the caller's original allowance.
    ///
    /// This is the additive twin of the context-budgeted read: the same keys,
    /// the same classes, the same coherent snapshot and the same fail-closed
    /// decode. Absence stays explicit and no default, repair or write occurs.
    ///
    /// # Errors
    /// Returns the original callback refusal, elapsed absolute deadline, or the
    /// existing identity/corruption classification for this record pair.
    pub fn read_security_restriction_with_original_context<F, E>(
        &self,
        namespace: SourceNamespaceId,
        deadline: Instant,
        work: &F,
    ) -> Result<SecurityRestrictionReadback, MigrationBootstrapFailure<E>>
    where
        F: Fn(Work) -> Result<(), E>,
    {
        let seam = OriginalSeam::new(deadline, work);
        self.read_security_restriction_at_owner(namespace, &seam, false)
            .map_err(|error| seam.failure(error))
    }

    fn read_security_restriction_at_owner<F, E>(&self, namespace: SourceNamespaceId,
        seam: &OriginalSeam<'_, F, E>, retained_owner: bool,
    ) -> Result<SecurityRestrictionReadback, ControlError>
    where F: Fn(Work) -> Result<(), E>,
    {
            seam.start()?;
            let policy_key = metered_key(POLICY_PREFIX, namespace, seam)?;
            let command_key = metered_key(RESTRICTION_PREFIX, namespace, seam)?;
            let snapshot = seam.read_snapshot(self)?;
            let policy = snapshot
                .get(&policy_key)
                .map(|value| {
                    if value.class() != ControlRecordClass::State {
                        return Err(ControlError::StoreCorrupt);
                    }
                    metered_policy_decode(value.len(), seam)?;
                    let record = decode_access_policy(value.as_bytes())
                        .map_err(|_| ControlError::StoreCorrupt)?;
                    if record.namespace_id != namespace { return Err(ControlError::StoreCorrupt); }
                    Ok(record)
                })
                .transpose()?;
            let mutation = snapshot
                .get(&command_key)
                .map(|value| {
                    if value.class() != ControlRecordClass::Operation {
                        return Err(ControlError::StoreCorrupt);
                    }
                    metered_command_decode(value.len(), seam)?;
                    let command = codec::decode(value.as_bytes())?;
                    if command.identity != snapshot.identity
                        && !(retained_owner && same_journal_history(command.identity, snapshot.identity)) {
                        return Err(ControlError::IdentityMismatch);
                    }
                    if command.replacement.policy.namespace_id != namespace
                        || policy != Some(command.replacement.policy)
                        || command.expected_generation >= snapshot.generation
                    {
                        return Err(ControlError::StoreCorrupt);
                    }
                    Ok(command)
                })
                .transpose()?;
            seam.complete()?;
            Ok(SecurityRestrictionReadback {
                identity: snapshot.identity,
                generation: snapshot.generation,
                namespace,
                policy,
                mutation,
            })
    }

    /// Observes the latest complete restriction and its exact committed native
    /// request after a verified owner succession on the same durable journal.
    /// No record, original identity or receipt is rewritten, and no write is
    /// replayed. Strict commit and ordinary recovery entry points remain fenced
    /// to the caller's current owner identity.
    ///
    /// # Errors
    /// Refuses future/foreign identities, missing or conflicting native receipts,
    /// partial state and any policy movement before current confirmation.
    pub fn observe_security_restriction_with_original_context<F, E>(&mut self,
        namespace: SourceNamespaceId, deadline: Instant, work: &F,
    ) -> Result<(SecurityRestrictionReadback, Option<SecurityRestrictionCommit>), MigrationBootstrapFailure<E>>
    where F: Fn(Work) -> Result<(), E>,
    {
        let seam = OriginalSeam::new(deadline, work);
        let result = (|| {
            let read = self.read_security_restriction_at_owner(namespace, &seam, true)?;
            let Some(mutation) = read.mutation() else { return Ok((read, None)); };
            let command = metered_command(mutation, &seam)?;
            metered_clone(mutation, &seam)?;
            let receipt = match seam.recover(self, &command)? {
                CommitRecoveryDecision::Committed(receipt) => receipt,
                CommitRecoveryDecision::ConflictingInput => return Err(ControlError::OperationConflict),
                CommitRecoveryDecision::NotCommittedRetrySameOperation
                    | CommitRecoveryDecision::PartialOrCorruptQuarantine => return Err(ControlError::StoreCorrupt),
            };
            let committed = SecurityRestrictionCommit { mutation: mutation.clone(), receipt,
                observed_identity: self.identity() };
            let current = self.read_security_restriction_at_owner(namespace, &seam, true)?;
            current.confirm_current(&committed)?;
            seam.complete()?;
            Ok((current, Some(committed)))
        })();
        result.map_err(|error| {
            let error = if seam.interrupted() { ControlError::CommitOutcomeUnknown } else { error };
            seam.failure(error)
        })
    }

    /// Commits metadata and the complete original restart request atomically
    /// under the caller's original allowance.
    ///
    /// The additive twin of the context-budgeted commit: same command identity,
    /// same conditions and the same checked engine. Same-identity replay stays
    /// bound to the same root/domain and exact native request. A possible write
    /// remains outcome-unknown and is resolved only by the identical request.
    ///
    /// # Errors
    /// Returns the original callback refusal, elapsed absolute deadline, or the
    /// existing identity/precondition/unknown-outcome classification.
    pub fn commit_security_restriction_with_original_context<F, E>(
        &mut self,
        mutation: &SecurityRestrictionMutation,
        deadline: Instant,
        work: &F,
    ) -> Result<SecurityRestrictionCommit, MigrationBootstrapFailure<E>>
    where
        F: Fn(Work) -> Result<(), E>,
    {
        let seam = OriginalSeam::new(deadline, work);
        let result = (|| {
            seam.start()?;
            if self.identity() != mutation.identity() {
                return Err(ControlError::IdentityMismatch);
            }
            let command = metered_command(mutation, &seam)?;
            metered_clone(mutation, &seam)?;
            let receipt = seam.transact(self, command)?;
            Ok(SecurityRestrictionCommit { mutation: mutation.clone(), receipt, observed_identity: self.identity() })
        })();
        result.map_err(|error| seam.failure(error))
    }

    /// Resolves the exact stored request under the caller's original allowance
    /// without writing, rebuilding or rebinding anything.
    ///
    /// The additive twin of the context-budgeted recovery: same request, same
    /// ledger readback and the same decision mapping. An interrupted inspection
    /// stays outcome-unknown and never proves rollback or absence.
    ///
    /// # Errors
    /// Returns the original callback refusal, elapsed absolute deadline, or the
    /// existing conflict/quarantine/unknown-outcome classification.
    pub fn recover_security_restriction_with_original_context<F, E>(
        &mut self,
        mutation: &SecurityRestrictionMutation,
        deadline: Instant,
        work: &F,
    ) -> Result<Option<SecurityRestrictionCommit>, MigrationBootstrapFailure<E>>
    where
        F: Fn(Work) -> Result<(), E>,
    {
        let seam = OriginalSeam::new(deadline, work);
        let result = (|| {
            seam.start()?;
            if self.identity() != mutation.identity() {
                return Err(ControlError::IdentityMismatch);
            }
            let command = metered_command(mutation, &seam)?;
            metered_clone(mutation, &seam)?;
            match seam.recover(self, &command)? {
                CommitRecoveryDecision::Committed(receipt) => Ok(Some(SecurityRestrictionCommit {
                    observed_identity: self.identity(),
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

/// Builds one namespace key after reserving its exact bytes.
fn metered_key<F, E>(
    prefix: &[u8],
    namespace: SourceNamespaceId,
    seam: &OriginalSeam<'_, F, E>,
) -> Result<ControlKey, ControlError>
where
    F: Fn(Work) -> Result<(), E>,
{
    let bytes = prefix.len().checked_add(NAMESPACE_KEY_BYTES).ok_or(ControlError::BudgetExceeded)?;
    seam.reserve(Work::Allocate(bytes))?;
    seam.reserve(Work::Process(bytes))?;
    let built = key(prefix, namespace)?;
    debug_assert_eq!(built.as_bytes().len(), bytes);
    Ok(built)
}

/// Reserves the bounded policy-record decode before it runs.
fn metered_policy_decode<F, E>(
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

/// Reserves the full command decode, including its canonical re-encode check.
fn metered_command_decode<F, E>(
    observed_bytes: usize,
    seam: &OriginalSeam<'_, F, E>,
) -> Result<(), ControlError>
where
    F: Fn(Work) -> Result<(), E>,
{
    // The decoder also reconstructs bounded strings/sets and re-encodes the
    // complete command. Include vector growth and all transient copies.
    let doubled = observed_bytes.checked_mul(8).ok_or(ControlError::BudgetExceeded)?;
    seam.reserve(Work::Read(observed_bytes))?;
    seam.reserve(Work::Allocate(doubled))?;
    seam.reserve(Work::Process(doubled))?;
    Ok(())
}

/// Reserves the retained clone of one original descriptor.
fn metered_clone<F, E>(
    mutation: &SecurityRestrictionMutation,
    seam: &OriginalSeam<'_, F, E>,
) -> Result<(), ControlError>
where
    F: Fn(Work) -> Result<(), E>,
{
    let bytes = mutation_capacity(mutation)?;
    seam.reserve(Work::Allocate(bytes))?;
    seam.reserve(Work::Process(bytes))?;
    Ok(())
}

/// Reserves the complete command preparation, then builds it with the existing
/// private constructor so the exact same bytes and identity are produced.
fn metered_command<F, E>(
    mutation: &SecurityRestrictionMutation,
    seam: &OriginalSeam<'_, F, E>,
) -> Result<ConditionalControlMutation, ControlError>
where
    F: Fn(Work) -> Result<(), E>,
{
    mutation.validate()?;
    let bytes = command_capacity(mutation)?;
    seam.reserve(Work::Allocate(bytes))?;
    seam.reserve(Work::Process(bytes))?;
    mutation.native_command()
}

/// Conservative input-derived preparation capacity, computed without allocation.
fn command_capacity(mutation: &SecurityRestrictionMutation) -> Result<usize, ControlError> {
    let mut total = bounded_sum([
        8 + 4,                                     // magic and version
        16 + 16 + 8 + 32 + 32 + 4,                  // full journal identity
        8 + 32,                                     // expected generation and digest
        4 + mutation.operation_id().as_str().len(), // operation-id text
        1,                                          // expected-policy flag
    ])?;
    // The expected-policy flag and the expected-state flag are independent
    // fields in the canonical codec; neither implies the other.
    if mutation.expected_policy.is_some() {
        total = bounded_sum([total, ACCESS_POLICY_RECORD_LEN])?;
    }
    total = bounded_sum([total, 1])?; // expected-state flag
    for state in [mutation.expected_state(), Some(mutation.replacement())].into_iter().flatten() {
        total = bounded_sum([total, state_capacity(state)?])?;
    }
    total = bounded_sum([total, 4])?; // dependent count
    for owner in mutation.required_dependents() {
        total = bounded_sum([total, 4, owner.as_str().len()])?;
    }
    // Encoding can grow a buffer, while the native command owns key/condition/
    // write vectors and policy copies. Charge this before their first allocation.
    total.checked_mul(8).and_then(|bytes| bytes.checked_add(1024))
        .ok_or(ControlError::BudgetExceeded)
}

/// Finite input-derived size of one retained policy-state descriptor.
fn state_capacity(state: &SecurityPolicyState) -> Result<usize, ControlError> {
    let mut total = bounded_sum([
        ACCESS_POLICY_RECORD_LEN,
        4 + state.security_domain_ref.as_str().len(),
        32 + 1,
    ])?;
    for members in [&state.denied_memberships, &state.purged_memberships] {
        total = bounded_sum([total, 4, members.len().checked_mul(16).ok_or(ControlError::BudgetExceeded)?])?;
    }
    Ok(total)
}

/// Capacity of one original descriptor without its encoded framing.
fn mutation_capacity(mutation: &SecurityRestrictionMutation) -> Result<usize, ControlError> {
    let mut total = bounded_sum([4, mutation.operation_id().as_str().len()])?;
    for state in [mutation.expected_state(), Some(mutation.replacement())]
        .into_iter()
        .flatten()
    {
        total = bounded_sum([total, state_capacity(state)?])?;
    }
    for owner in mutation.required_dependents() {
        total = bounded_sum([total, std::mem::size_of::<search_contracts::OpaqueId>(), owner.as_str().len()])?;
    }
    Ok(total)
}

/// Saturating-free exact sum of input-derived capacities.
fn bounded_sum(parts: impl IntoIterator<Item = usize>) -> Result<usize, ControlError> {
    parts
        .into_iter()
        .try_fold(0_usize, |total, part| {
            total.checked_add(part).ok_or(ControlError::BudgetExceeded)
        })
}

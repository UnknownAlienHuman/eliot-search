//! Typed policy-record persistence on the existing conditional redb journal.
//! This boundary stores metadata; it neither compiles grants nor publishes a
//! live restriction. A historical transaction receipt is not a current head.

use core::fmt;

use search_contracts::{Blake3Digest32, SourceNamespaceId};
use search_ports::{CancellationProbe, OperationContext};

use crate::policy_codec::{AccessPolicyRecord, decode_access_policy, encode_access_policy};
use crate::{
    CommitRecoveryDecision, ConditionalControlMutation, ControlCallError,
    ControlCommitReceipt, ControlError, ControlKey, ControlMutation,
    ControlRecordClass, ControlRecordCondition, ControlValue, ControlWrite,
    JournalIdentity, JournalLimits, JournalReadSnapshot, MutationId, PersistentControlJournal,
};

const KEY_PREFIX: &[u8] = b"eliot.control.access-policy.v1\0";

/// Exact immutable transaction descriptor, retained unchanged for recovery.
///
/// A `None` expected record means explicit initialization, not upsert. Policy
/// meaning and monotonicity remain the access owner's responsibility. The
/// supplied command digest is metadata; the journal independently binds the
/// complete actual request, including identity, generation, classes and bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessPolicyMutation {
    identity: JournalIdentity,
    key: ControlKey,
    replacement: AccessPolicyRecord,
    command: ConditionalControlMutation,
}

impl AccessPolicyMutation {
    /// Binds one policy replacement to the exact journal and previous record.
    ///
    /// The namespace cannot change through a replacement. All other expected
    /// fields are compared byte-for-byte in the write transaction. Actual
    /// journal limits are rechecked by the existing transaction engine.
    ///
    /// # Errors
    /// Returns an identity/schema error or a key/value bound failure.
    pub fn new(
        identity: JournalIdentity,
        operation_id: MutationId,
        command_digest: Blake3Digest32,
        expected_generation: u64,
        expected: Option<AccessPolicyRecord>,
        replacement: AccessPolicyRecord,
    ) -> Result<Self, ControlError> {
        identity.validate()?;
        if expected.is_some_and(|record| record.namespace_id != replacement.namespace_id) {
            return Err(ControlError::IdentityMismatch);
        }
        let key = policy_key(replacement.namespace_id)?;
        let condition = match expected {
            Some(record) => ControlRecordCondition::exact(key.clone(), policy_value(&record)?),
            None => ControlRecordCondition::absent(key.clone()),
        };
        let mutation = ControlMutation::new(
            operation_id,
            command_digest,
            expected_generation,
            vec![ControlWrite { key: key.clone(), value: policy_value(&replacement)? }],
            Vec::new(),
        );
        Ok(Self {
            identity,
            key,
            replacement,
            command: ConditionalControlMutation::new(mutation, vec![condition]),
        })
    }

    /// Exact journal binding; callers must retain its owner guard separately.
    #[must_use]
    pub const fn identity(&self) -> JournalIdentity { self.identity }

    /// Stable namespace-scoped technical key, not an authorization locator.
    #[must_use]
    pub const fn key(&self) -> &ControlKey { &self.key }
}

/// Failure preserving the original journal cancellation and outcome metadata.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccessPolicyJournalError {
    /// Local identity, codec or readback inconsistency; never a success fallback.
    Record(ControlError),
    /// Original context-controlled disk error, including a possible commit.
    Call(ControlCallError),
}

impl fmt::Display for AccessPolicyJournalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Record(error) => fmt::Display::fmt(error, formatter),
            Self::Call(error) => fmt::Display::fmt(error, formatter),
        }
    }
}

impl std::error::Error for AccessPolicyJournalError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Record(error) => Some(error),
            Self::Call(error) => Some(error),
        }
    }
}

impl From<ControlError> for AccessPolicyJournalError {
    fn from(error: ControlError) -> Self { Self::Record(error) }
}
impl From<ControlCallError> for AccessPolicyJournalError {
    fn from(error: ControlCallError) -> Self { Self::Call(error) }
}

/// Exact native transaction receipt bound to its original policy descriptor.
///
/// Only journal commit or ledger recovery constructs this value. Callers can
/// inspect the receipt but cannot turn an arbitrary receipt into verified policy
/// evidence. Historical commits remain historical until a fresh readback agrees.
///
/// ```compile_fail
/// use search_control_redb::{access_policy::AccessPolicyCommit, ControlCommitReceipt};
/// fn forge(receipt: ControlCommitReceipt) -> AccessPolicyCommit { receipt.into() }
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessPolicyCommit {
    mutation: AccessPolicyMutation,
    receipt: ControlCommitReceipt,
}

impl AccessPolicyCommit {
    /// Native receipt for the existing guarded snapshot-publication boundary.
    /// This reference is immutable and alone is not a current-policy permit.
    #[must_use]
    pub const fn receipt(&self) -> &ControlCommitReceipt { &self.receipt }
}

/// A policy row observed in one coherent native read-only transaction.
///
/// Absence stays explicit. The observation is not a grant or live-publication
/// receipt and becomes stale after any journal mutation. Fields are private so
/// callers cannot construct a disk readback from arbitrary policy inputs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessPolicyReadback {
    identity: JournalIdentity,
    generation: u64,
    namespace_id: SourceNamespaceId,
    record: Option<AccessPolicyRecord>,
}

impl AccessPolicyReadback {
    /// Verified installation/root/schema/owner identity at read time.
    #[must_use]
    pub const fn identity(&self) -> JournalIdentity { self.identity }
    /// Global journal generation observed with the policy row.
    #[must_use]
    pub const fn generation(&self) -> u64 { self.generation }
    /// Exact policy metadata, or absence; never synthesized defaults.
    #[must_use]
    pub const fn record(&self) -> Option<&AccessPolicyRecord> { self.record.as_ref() }

    /// Confirms that a receipt and the exact replacement describe this readback.
    ///
    /// Historical or foreign receipts fail even when the policy bytes happen
    /// to match again. This is a point-in-time check, not a lease: subsequent
    /// publication must still use the journal's guarded publisher boundary.
    ///
    /// # Errors
    /// Rejects foreign identity, mismatched receipts, stale generation or bytes.
    pub fn confirm_commit(
        &self,
        committed: &AccessPolicyCommit,
    ) -> Result<(), ControlError> {
        let mutation = &committed.mutation;
        let receipt = &committed.receipt;
        let request = mutation.command.mutation();
        if self.identity != mutation.identity || self.namespace_id != mutation.replacement.namespace_id {
            return Err(ControlError::IdentityMismatch);
        }
        if receipt.operation_id != request.id()
            || receipt.command_digest != request.command_digest()
            || receipt.before_generation != request.expected_generation()
            || receipt.before_generation.checked_add(1) != Some(receipt.after_generation)
            || receipt.changed_keys.as_slice() != std::slice::from_ref(&mutation.key)
        {
            return Err(ControlError::OperationConflict);
        }
        if self.generation != receipt.after_generation {
            return Err(ControlError::TransactionConflict);
        }
        if self.record.as_ref() != Some(&mutation.replacement) {
            return Err(ControlError::StoreCorrupt);
        }
        Ok(())
    }
}

impl PersistentControlJournal {
    /// Writes a policy row through the existing conditional transaction engine.
    ///
    /// Exact pre-state and generation guard the write; its receipt and row commit
    /// together. Same-command replay returns the original historical receipt,
    /// NOT a claim of current policy. Preserve `mutation` for exact recovery on
    /// any possible-write failure. This method never publishes live authority.
    ///
    /// # Errors
    /// Returns identity mismatch or the original bounded transaction failure.
    /// A possible write remains outcome-unknown and requires exact recovery.
    pub fn commit_access_policy<C: CancellationProbe>(
        &mut self,
        mutation: &AccessPolicyMutation,
        context: &OperationContext<C>,
    ) -> Result<AccessPolicyCommit, AccessPolicyJournalError> {
        if self.identity() != mutation.identity {
            return Err(ControlError::IdentityMismatch.into());
        }
        let receipt = self.transact_conditionally(mutation.command.clone(), context)?;
        Ok(AccessPolicyCommit { mutation: mutation.clone(), receipt })
    }

    /// Resolves the SAME complete request without writing or rebuilding its guards.
    ///
    /// `Some` proves a commit, possibly historical; `None` proves resolved
    /// absence. Conflict, quarantine and uncertain inspection remain errors.
    /// Read and confirm current policy separately before guarded publication;
    /// never retry with a fresh operation identity
    /// or substitute the latest generation for the original expected generation.
    ///
    /// # Errors
    /// Returns identity/conflict/quarantine errors or the original recovery
    /// failure with its interruption and outcome metadata preserved.
    pub fn recover_access_policy<C: CancellationProbe>(
        &mut self,
        mutation: &AccessPolicyMutation,
        context: &OperationContext<C>,
    ) -> Result<Option<AccessPolicyCommit>, AccessPolicyJournalError> {
        if self.identity() != mutation.identity {
            return Err(ControlError::IdentityMismatch.into());
        }
        match self.recover_conditional_transaction(&mutation.command, context)? {
            CommitRecoveryDecision::Committed(receipt) => {
                Ok(Some(AccessPolicyCommit { mutation: mutation.clone(), receipt }))
            }
            CommitRecoveryDecision::NotCommittedRetrySameOperation => Ok(None),
            CommitRecoveryDecision::ConflictingInput => Err(ControlError::OperationConflict.into()),
            CommitRecoveryDecision::PartialOrCorruptQuarantine => Err(ControlError::StoreQuarantined.into()),
        }
    }

    /// Reads bounded policy metadata with existing cancellation/deadline semantics.
    ///
    /// This administrative/recovery path uses one bounded coherent journal
    /// snapshot, not a per-query scan. Hot admission uses a published snapshot.
    /// Malformed bytes, wrong class or an embedded foreign namespace fail closed.
    /// No database write, default policy, implicit initialization or live
    /// publication occurs on this path.
    ///
    /// # Errors
    /// Returns the original interrupted/unavailable read or semantic corruption.
    pub fn read_access_policy<C: CancellationProbe>(
        &self,
        namespace_id: SourceNamespaceId,
        context: &OperationContext<C>,
    ) -> Result<AccessPolicyReadback, AccessPolicyJournalError> {
        let snapshot = self.read_snapshot_with_context(context)?;
        decode_readback(&snapshot, namespace_id).map_err(Into::into)
    }
}

fn decode_readback(
    snapshot: &JournalReadSnapshot,
    namespace_id: SourceNamespaceId,
) -> Result<AccessPolicyReadback, ControlError> {
    let record = snapshot.get(&policy_key(namespace_id)?).map(|value| {
        if value.class() != ControlRecordClass::State { return Err(ControlError::StoreCorrupt); }
        let record = decode_access_policy(value.as_bytes()).map_err(|_| ControlError::StoreCorrupt)?;
        if record.namespace_id != namespace_id { return Err(ControlError::StoreCorrupt); }
        Ok(record)
    }).transpose()?;
    Ok(AccessPolicyReadback {
        identity: snapshot.identity,
        generation: snapshot.generation,
        namespace_id,
        record,
    })
}

fn policy_key(namespace_id: SourceNamespaceId) -> Result<ControlKey, ControlError> {
    let mut key = Vec::with_capacity(KEY_PREFIX.len() + 16);
    key.extend_from_slice(KEY_PREFIX);
    key.extend_from_slice(namespace_id.as_bytes());
    ControlKey::new(key, JournalLimits::BASELINE)
}

fn policy_value(record: &AccessPolicyRecord) -> Result<ControlValue, ControlError> {
    ControlValue::new(ControlRecordClass::State, encode_access_policy(record), JournalLimits::BASELINE)
}

#[cfg(test)]
mod tests;

//! Complete bounded security-state recovery on the existing native journal.
//!
//! Policy metadata and the exact restart command commit together. This module
//! stores technical identities and restrictions, not grants or source content.
//! Semantic restriction validation and live publication remain access-owned.

use core::fmt;

use search_contracts::{
    Blake3Digest32, BoundedSet, MAX_SET_ITEMS, OpaqueId, OpaqueRef, SourceMembershipId,
    SourceNamespaceId,
};
use search_ports::{CancellationProbe, OperationContext};
use sha2::{Digest, Sha256};

use crate::access_policy::AccessPolicyJournalError;
use crate::policy_codec::{AccessPolicyRecord, decode_access_policy, encode_access_policy};
use crate::{
    CommitRecoveryDecision, ConditionalControlMutation, ControlCommitReceipt, ControlError,
    ControlKey, ControlMutation, ControlRecordClass, ControlRecordCondition, ControlValue,
    ControlWrite, JournalIdentity, JournalLimits, MutationId, PersistentControlJournal,
};

mod codec;

/// Maximum configured dependent-owner identifiers in one restart command.
pub const MAX_RESTRICTION_DEPENDENTS: usize = 64;
/// Inline command ceiling. Larger sets require a separately qualified manifest
/// path, never truncation or an automatic increase of journal limits.
pub const MAX_RESTRICTION_RECORD_BYTES: usize = 64 * 1024;

const POLICY_PREFIX: &[u8] = b"eliot.control.access-policy.v1\0";
const RESTRICTION_PREFIX: &[u8] = b"eliot.control.access-restriction.v1\0";

/// Full captured state. Digests are preserved, never used to reconstruct sets.
/// This is persistence data, not a validated access decision.
#[derive(Clone, Eq, PartialEq)]
pub struct SecurityPolicyState {
    /// Existing namespace/owner/policy/live/shadow/purge metadata.
    pub policy: AccessPolicyRecord,
    /// Exact server-owned security domain; never inferred from the namespace.
    pub security_domain_ref: OpaqueRef,
    /// Identity of the full immutable live snapshot, distinct from policy digest.
    pub snapshot_digest: Blake3Digest32,
    /// Complete denied membership set in canonical identity order.
    pub denied_memberships: BoundedSet<SourceMembershipId, MAX_SET_ITEMS>,
    /// Complete purged membership set in canonical identity order.
    pub purged_memberships: BoundedSet<SourceMembershipId, MAX_SET_ITEMS>,
    /// Captured fail-closed flag; decoding does not clear it.
    pub fail_closed: bool,
}

impl fmt::Debug for SecurityPolicyState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SecurityPolicyState")
            .field("live_generation", &self.policy.live_deny_generation)
            .field("fail_closed", &self.fail_closed)
            .finish_non_exhaustive()
    }
}

/// Immutable original native request reconstructed from its versioned record.
/// Only a coherent disk readback can prepare a new request; record decoding is
/// private. Holding this descriptor proves neither commit nor authorization.
#[derive(Clone, Eq, PartialEq)]
pub struct SecurityRestrictionMutation {
    identity: JournalIdentity,
    expected_generation: u64,
    command_digest: Blake3Digest32,
    operation_id: OpaqueId,
    expected_policy: Option<AccessPolicyRecord>,
    expected_state: Option<SecurityPolicyState>,
    replacement: SecurityPolicyState,
    dependents: BoundedSet<OpaqueId, MAX_RESTRICTION_DEPENDENTS>,
}

impl fmt::Debug for SecurityRestrictionMutation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SecurityRestrictionMutation")
            .field("expected_generation", &self.expected_generation)
            .field("initialization", &self.expected_state.is_none())
            .finish_non_exhaustive()
    }
}

impl SecurityRestrictionMutation {
    /// Exact original journal identity, including the original owner epoch.
    #[must_use]
    pub const fn identity(&self) -> JournalIdentity { self.identity }
    /// Access-owner operation identity. Native replay identity is derived from
    /// root/incarnation, namespace, domain and this ID, never from mutable input.
    #[must_use]
    pub const fn operation_id(&self) -> &OpaqueId { &self.operation_id }
    /// Original complete state, absent only for explicit initialization.
    #[must_use]
    pub const fn expected_state(&self) -> Option<&SecurityPolicyState> {
        self.expected_state.as_ref()
    }
    /// Complete committed candidate for later publication/recovery.
    #[must_use]
    pub const fn replacement(&self) -> &SecurityPolicyState { &self.replacement }
    /// Exact required owners captured before mutation, not acknowledged owners.
    #[must_use]
    pub const fn required_dependents(&self) -> &BoundedSet<OpaqueId, MAX_RESTRICTION_DEPENDENTS> {
        &self.dependents
    }

    fn validate(&self) -> Result<(), ControlError> {
        self.identity.validate()?;
        if self.dependents.is_empty() { return Err(ControlError::InvalidValue); }
        if self.expected_policy.is_some_and(|old| {
            old.namespace_id != self.replacement.policy.namespace_id
                || old.owner_generation != self.replacement.policy.owner_generation
        }) {
            return Err(ControlError::IdentityMismatch);
        }
        match &self.expected_state {
            Some(old) if Some(old.policy) != self.expected_policy
                || old.security_domain_ref != self.replacement.security_domain_ref => {
                Err(ControlError::IdentityMismatch)
            }
            // Explicitly completing an existing metadata-only row must not
            // silently rewrite that policy while installing missing state.
            None if self.expected_policy.is_some_and(|old| old != self.replacement.policy) => {
                Err(ControlError::GenerationMismatch)
            }
            _ => Ok(()),
        }
    }

    fn native_id(&self) -> MutationId {
        let mut hash = Sha256::new();
        hash.update(b"eliot-search/access-restriction-operation/sha256/v1\0");
        hash.update(self.identity.installation_incarnation_id.as_bytes());
        hash.update(self.identity.data_root_id.as_bytes());
        hash.update(self.replacement.policy.namespace_id.as_bytes());
        for text in [self.replacement.security_domain_ref.as_str(), self.operation_id.as_str()] {
            // Contract text limits fit u32; use its fixed framing width.
            hash.update(u32::try_from(text.len()).expect("bounded contract text").to_be_bytes());
            hash.update(text.as_bytes());
        }
        MutationId(hash.finalize().into())
    }

    fn native_command(&self) -> Result<ConditionalControlMutation, ControlError> {
        self.validate()?;
        let namespace = self.replacement.policy.namespace_id;
        let policy_key = key(POLICY_PREFIX, namespace)?;
        let command_key = key(RESTRICTION_PREFIX, namespace)?;
        let mut conditions = vec![match self.expected_policy {
            Some(old) => ControlRecordCondition::exact(policy_key.clone(), policy_value(&old)?),
            None => ControlRecordCondition::absent(policy_key.clone()),
        }];
        if self.expected_state.is_none() {
            conditions.push(ControlRecordCondition::absent(command_key.clone()));
        }
        // For replacement, the disk-derived global generation fences the entire
        // prior command row. Do not recursively embed its previous history just
        // to reconstruct a byte condition. The command stores both state values.
        let writes = vec![
            ControlWrite { key: policy_key, value: policy_value(&self.replacement.policy)? },
            ControlWrite {
                key: command_key,
                value: ControlValue::new(
                    ControlRecordClass::Operation, codec::encode(self)?, JournalLimits::BASELINE,
                )?,
            },
        ];
        Ok(ConditionalControlMutation::new(
            ControlMutation::new(self.native_id(), self.command_digest, self.expected_generation,
                writes, Vec::new()),
            conditions,
        ))
    }
}

/// Coherent policy and last-command disk observation. Absence does not create an
/// empty deny set. The command remains unverified until native ledger recovery.
#[derive(Clone, Eq, PartialEq)]
pub struct SecurityRestrictionReadback {
    identity: JournalIdentity,
    generation: u64,
    namespace: SourceNamespaceId,
    policy: Option<AccessPolicyRecord>,
    mutation: Option<SecurityRestrictionMutation>,
}

impl fmt::Debug for SecurityRestrictionReadback {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SecurityRestrictionReadback")
            .field("generation", &self.generation)
            .field("has_command", &self.mutation.is_some())
            .finish_non_exhaustive()
    }
}

impl SecurityRestrictionReadback {
    /// Journal generation observed with both records.
    #[must_use]
    pub const fn generation(&self) -> u64 { self.generation }
    /// Stored original request for exact ledger recovery; not a completion receipt.
    #[must_use]
    pub const fn mutation(&self) -> Option<&SecurityRestrictionMutation> { self.mutation.as_ref() }

    /// Explicitly installs complete initial state, including for a metadata-only
    /// policy row. Caller must establish the sets from authoritative sources;
    /// neither absent state nor a digest is evidence for an empty restriction.
    ///
    /// # Errors
    /// Refuses an existing full record, changed metadata, identity or finite bounds.
    pub fn initialize(
        &self, operation_id: OpaqueId, command_digest: Blake3Digest32,
        state: SecurityPolicyState, dependents: BoundedSet<OpaqueId, MAX_RESTRICTION_DEPENDENTS>,
    ) -> Result<SecurityRestrictionMutation, ControlError> {
        if self.mutation.is_some() { return Err(ControlError::OperationConflict); }
        self.prepare(operation_id, command_digest, state, dependents, None)
    }

    /// Captures a replacement of the complete disk-observed prior state. The
    /// access owner must validate restrictive semantics before committing it.
    /// Exact predecessor evidence is mandatory; an unverified command row may
    /// not be overwritten to hide a missing or inconsistent native receipt.
    ///
    /// # Errors
    /// Refuses missing full state, changed identity, or oversized commands.
    pub fn prepare_restriction(
        &self, predecessor: &SecurityRestrictionCommit,
        operation_id: OpaqueId, command_digest: Blake3Digest32,
        state: SecurityPolicyState, dependents: BoundedSet<OpaqueId, MAX_RESTRICTION_DEPENDENTS>,
    ) -> Result<SecurityRestrictionMutation, ControlError> {
        self.confirm_current(predecessor)?;
        let previous = self.mutation.as_ref().ok_or(ControlError::GenerationMismatch)?;
        self.prepare(operation_id, command_digest, state, dependents,
            Some(previous.replacement.clone()))
    }

    fn prepare(
        &self, operation_id: OpaqueId, command_digest: Blake3Digest32,
        replacement: SecurityPolicyState,
        dependents: BoundedSet<OpaqueId, MAX_RESTRICTION_DEPENDENTS>,
        expected_state: Option<SecurityPolicyState>,
    ) -> Result<SecurityRestrictionMutation, ControlError> {
        if replacement.policy.namespace_id != self.namespace { return Err(ControlError::IdentityMismatch); }
        let mutation = SecurityRestrictionMutation {
            identity: self.identity, expected_generation: self.generation, command_digest,
            operation_id, expected_policy: self.policy, expected_state, replacement, dependents,
        };
        let _ = mutation.native_command()?;
        Ok(mutation)
    }

    /// Confirms an executed native receipt against the exact latest domain record.
    /// Unrelated journal writes are allowed; a later domain operation, even an
    /// ABA return to equal policy bytes, replaces the command and is rejected.
    /// This is a point-in-time observation, not a publication or serving permit.
    ///
    /// # Errors
    /// Returns identity, native-receipt or current-domain correspondence failures.
    pub fn confirm_current(&self, commit: &SecurityRestrictionCommit) -> Result<(), ControlError> {
        let command = &commit.mutation;
        let receipt = &commit.receipt;
        if self.identity != command.identity || self.namespace != command.replacement.policy.namespace_id {
            return Err(ControlError::IdentityMismatch);
        }
        let mut changed = vec![key(POLICY_PREFIX, self.namespace)?, key(RESTRICTION_PREFIX, self.namespace)?];
        changed.sort();
        if receipt.operation_id != command.native_id()
            || receipt.command_digest != command.command_digest
            || receipt.before_generation != command.expected_generation
            || receipt.before_generation.checked_add(1) != Some(receipt.after_generation)
            || receipt.changed_keys != changed
        { return Err(ControlError::OperationConflict); }
        if self.generation < receipt.after_generation
            || self.mutation.as_ref() != Some(command)
            || self.policy != Some(command.replacement.policy)
        { return Err(ControlError::TransactionConflict); }
        Ok(())
    }
}

/// Opaque native commit/recovery evidence. Publication and dependent completion
/// have NOT been performed by this value.
#[derive(Clone, Eq, PartialEq)]
pub struct SecurityRestrictionCommit {
    mutation: SecurityRestrictionMutation,
    receipt: ControlCommitReceipt,
}

impl fmt::Debug for SecurityRestrictionCommit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SecurityRestrictionCommit")
            .field("generation", &self.receipt.after_generation)
            .field("replayed", &self.receipt.replayed)
            .finish_non_exhaustive()
    }
}

impl SecurityRestrictionCommit {
    /// Original lossless request, including the prior state and all required owners.
    #[must_use]
    pub const fn mutation(&self) -> &SecurityRestrictionMutation { &self.mutation }
    /// Actual native receipt; may be historical until `confirm_current` succeeds.
    #[must_use]
    pub const fn receipt(&self) -> &ControlCommitReceipt { &self.receipt }
}

impl PersistentControlJournal {
    /// Reads both records coherently without inventing defaults or doing writes.
    /// This bounded administrative/recovery scan is not a query hot path.
    ///
    /// # Errors
    /// Preserves read cancellation and denies corrupt, foreign or orphan records.
    pub fn read_security_restriction<C: CancellationProbe>(
        &self, namespace: SourceNamespaceId, context: &OperationContext<C>,
    ) -> Result<SecurityRestrictionReadback, AccessPolicyJournalError> {
        let snapshot = self.read_snapshot_with_context(context)?;
        let policy = snapshot.get(&key(POLICY_PREFIX, namespace)?).map(|value| {
            if value.class() != ControlRecordClass::State { return Err(ControlError::StoreCorrupt); }
            let record = decode_access_policy(value.as_bytes()).map_err(|_| ControlError::StoreCorrupt)?;
            if record.namespace_id != namespace { return Err(ControlError::StoreCorrupt); }
            Ok(record)
        }).transpose()?;
        let mutation = snapshot.get(&key(RESTRICTION_PREFIX, namespace)?).map(|value| {
            if value.class() != ControlRecordClass::Operation { return Err(ControlError::StoreCorrupt); }
            let command = codec::decode(value.as_bytes())?;
            if command.identity != snapshot.identity { return Err(ControlError::IdentityMismatch); }
            if command.replacement.policy.namespace_id != namespace
                || policy != Some(command.replacement.policy)
                || command.expected_generation >= snapshot.generation
            { return Err(ControlError::StoreCorrupt); }
            Ok(command)
        }).transpose()?;
        Ok(SecurityRestrictionReadback {
            identity: snapshot.identity, generation: snapshot.generation, namespace, policy, mutation,
        })
    }

    /// Commits metadata and the complete original restart request atomically.
    /// Same-ID replay stays bound to the same root/domain and exact native request.
    ///
    /// # Errors
    /// Returns original conditional-engine failures; possible writes require recovery.
    pub fn commit_security_restriction<C: CancellationProbe>(
        &mut self, mutation: &SecurityRestrictionMutation, context: &OperationContext<C>,
    ) -> Result<SecurityRestrictionCommit, AccessPolicyJournalError> {
        if self.identity() != mutation.identity { return Err(ControlError::IdentityMismatch.into()); }
        let receipt = self.transact_conditionally(mutation.native_command()?, context)?;
        Ok(SecurityRestrictionCommit { mutation: mutation.clone(), receipt })
    }

    /// Resolves the exact stored request against the native ledger without writing.
    /// To restart: read the full record, recover its `mutation()`, then confirm it
    /// against a fresh readback. Keep publication/invalidation blocked until their
    /// actual owners finish. `None` is resolved absence, never an empty policy.
    /// Owner succession is explicit; this method never rebinds an old descriptor.
    ///
    /// # Errors
    /// Preserves cancellation, conflict, quarantine and unknown-outcome distinctions.
    pub fn recover_security_restriction<C: CancellationProbe>(
        &mut self, mutation: &SecurityRestrictionMutation, context: &OperationContext<C>,
    ) -> Result<Option<SecurityRestrictionCommit>, AccessPolicyJournalError> {
        if self.identity() != mutation.identity { return Err(ControlError::IdentityMismatch.into()); }
        match self.recover_conditional_transaction(&mutation.native_command()?, context)? {
            CommitRecoveryDecision::Committed(receipt) => Ok(Some(SecurityRestrictionCommit {
                mutation: mutation.clone(), receipt,
            })),
            CommitRecoveryDecision::NotCommittedRetrySameOperation => Ok(None),
            CommitRecoveryDecision::ConflictingInput => Err(ControlError::OperationConflict.into()),
            CommitRecoveryDecision::PartialOrCorruptQuarantine => Err(ControlError::StoreQuarantined.into()),
        }
    }
}

fn key(prefix: &[u8], namespace: SourceNamespaceId) -> Result<ControlKey, ControlError> {
    let mut bytes = prefix.to_vec();
    bytes.extend_from_slice(namespace.as_bytes());
    ControlKey::new(bytes, JournalLimits::BASELINE)
}

fn policy_value(record: &AccessPolicyRecord) -> Result<ControlValue, ControlError> {
    ControlValue::new(ControlRecordClass::State, encode_access_policy(record), JournalLimits::BASELINE)
}

#[cfg(test)]
mod tests;

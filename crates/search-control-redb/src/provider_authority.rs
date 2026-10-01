//! Typed provider-binding and standalone grant-policy persistence.
//!
//! This module owns the durable key layout, atomic pair mutation, current
//! published readback and exact recovery boundary. Strict byte codecs are private.
//! It stores bounded authorization metadata only; it does not authenticate a
//! transport, mint a grant, interpret source scope or publish a live barrier.

mod codec;

use core::fmt;

use search_contracts::{
    BindingId, Blake3Digest32, InstallationIncarnationId, ProviderBindingRecord,
    ProviderBindingStatus, StandalonePolicyRecord, StandalonePolicyState,
    protocol::PeerRole,
};
use search_ports::{CancellationProbe, OperationContext};

use crate::{
    CommitRecoveryDecision, ConditionalControlMutation, ControlCallError, ControlCommitReceipt,
    ControlError, ControlKey, ControlMutation, ControlRecordClass, ControlRecordCondition,
    ControlSnapshotPublisher, ControlValue, ControlWrite, JournalIdentity, JournalLimits,
    MutationId, PersistentControlJournal,
};
use codec::{decode_binding, decode_policy, encode_binding, encode_policy};

const BINDING_KEY_PREFIX: &[u8] = b"eliot.control.provider-binding.v1\0";
const POLICY_KEY_PREFIX: &[u8] = b"eliot.control.standalone-policy.v1\0";

/// One validated binding and policy that must be committed and read together.
#[derive(Clone, Eq, PartialEq)]
pub struct ProviderAuthorityRecord {
    binding: ProviderBindingRecord,
    policy: StandalonePolicyRecord,
}

impl ProviderAuthorityRecord {
    /// Validate and retain one coherent standalone binding/policy pair.
    ///
    /// # Errors
    /// Rejects malformed records, non-standalone peers, identity/generation skew
    /// and lifecycle mismatch.
    pub fn new(
        binding: ProviderBindingRecord,
        policy: StandalonePolicyRecord,
    ) -> Result<Self, ControlError> {
        validate_pair(&binding, &policy)?;
        Ok(Self { binding, policy })
    }

    /// Exact durable binding metadata.
    #[must_use]
    pub const fn binding(&self) -> &ProviderBindingRecord { &self.binding }

    /// Exact durable policy metadata.
    #[must_use]
    pub const fn policy(&self) -> &StandalonePolicyRecord { &self.policy }
}

impl fmt::Debug for ProviderAuthorityRecord {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProviderAuthorityRecord")
            .field("binding_status", &self.binding.status)
            .field("policy_state", &self.policy.state)
            .field("binding_generation", &self.binding.pairing_generation)
            .field("policy_generation", &self.policy.policy.policy_generation)
            .finish_non_exhaustive()
    }
}

/// Immutable atomic authority replacement retained unchanged through recovery.
#[derive(Clone)]
pub struct ProviderAuthorityMutation {
    identity: JournalIdentity,
    binding_key: ControlKey,
    policy_key: ControlKey,
    replacement: ProviderAuthorityRecord,
    command: ConditionalControlMutation,
}

impl ProviderAuthorityMutation {
    /// Build an explicit absent-pair initialization or exact-pair replacement.
    ///
    /// The command never upserts or repairs one missing half. Replacement keeps
    /// immutable peer identity fields stable, advances pairing and binding
    /// revocation revisions exactly once, advances policy generation exactly once,
    /// forbids lifetime widening and never reactivates a terminal record.
    /// `command_digest` is caller-owned operation metadata; the journal still
    /// fingerprints the complete keys, classes, bytes, conditions and generation.
    ///
    /// # Errors
    /// Rejects identity mismatch, malformed pairs, nonmonotone transitions and
    /// bounded codec/key failures.
    pub fn new(
        identity: JournalIdentity,
        operation_id: MutationId,
        command_digest: Blake3Digest32,
        expected_generation: u64,
        expected: Option<&ProviderAuthorityRecord>,
        replacement: ProviderAuthorityRecord,
    ) -> Result<Self, ControlError> {
        identity.validate()?;
        validate_pair(replacement.binding(), replacement.policy())?;
        if identity.installation_incarnation_id
            != replacement.binding.installation_incarnation_id
        {
            return Err(ControlError::IdentityMismatch);
        }

        let binding_key = binding_key(
            replacement.binding.installation_incarnation_id,
            replacement.binding.binding_id,
        )?;
        let policy_key = policy_key(
            replacement.binding.installation_incarnation_id,
            replacement.binding.binding_id,
        )?;
        if binding_key == policy_key {
            return Err(ControlError::DuplicateMutationKey);
        }
        let binding_value = encode_binding(&replacement.binding)?;
        let policy_value = encode_policy(&replacement.policy)?;

        let mut conditions = Vec::with_capacity(2);
        if let Some(before) = expected {
            validate_pair(before.binding(), before.policy())?;
            validate_binding_transition(before.binding(), replacement.binding())?;
            validate_policy_transition(before.policy(), replacement.policy())?;
            conditions.push(ControlRecordCondition::exact(
                binding_key.clone(),
                encode_binding(before.binding())?,
            ));
            conditions.push(ControlRecordCondition::exact(
                policy_key.clone(),
                encode_policy(before.policy())?,
            ));
        } else {
            if replacement.binding.status != ProviderBindingStatus::Active
                || replacement.policy.state != StandalonePolicyState::Active
            {
                return Err(ControlError::InvalidValue);
            }
            conditions.push(ControlRecordCondition::absent(binding_key.clone()));
            conditions.push(ControlRecordCondition::absent(policy_key.clone()));
        }
        conditions.sort_unstable_by(|left, right| left.key().cmp(right.key()));

        let mut writes = vec![
            ControlWrite { key: binding_key.clone(), value: binding_value },
            ControlWrite { key: policy_key.clone(), value: policy_value },
        ];
        writes.sort_unstable_by(|left, right| left.key.cmp(&right.key));
        let mutation = ControlMutation::new(
            operation_id,
            command_digest,
            expected_generation,
            writes,
            Vec::new(),
        );
        Ok(Self {
            identity,
            binding_key,
            policy_key,
            replacement,
            command: ConditionalControlMutation::new(mutation, conditions),
        })
    }

    /// Bind one exact operation-state transition to the same authority commit.
    ///
    /// This is used for crash-safe provisioning: the marker must be a distinct
    /// non-authority `Operation` record, observed at `prepared` and replaced by
    /// `committed`. The caller supplies a digest over the complete augmented
    /// command; the journal still fingerprints and condition-checks every byte.
    ///
    /// # Errors
    /// Rejects authority namespaces, duplicate keys, wrong record classes,
    /// identical marker values and conflicting existing command entries.
    pub fn with_completion_marker(
        mut self,
        command_digest: Blake3Digest32,
        key: ControlKey,
        prepared: ControlValue,
        committed: ControlValue,
    ) -> Result<Self, ControlError> {
        if prepared == committed
            || prepared.class() != ControlRecordClass::Operation
            || committed.class() != ControlRecordClass::Operation
            || port_reserved_key(key.as_bytes())
            || self
                .command
                .mutation()
                .writes()
                .iter()
                .any(|write| write.key == key)
            || self
                .command
                .conditions()
                .iter()
                .any(|condition| condition.key() == &key)
        {
            return Err(ControlError::InvalidValue);
        }
        let base = self.command.mutation();
        let mut writes = base.writes().to_vec();
        writes.push(ControlWrite {
            key: key.clone(),
            value: committed,
        });
        writes.sort_unstable_by(|left, right| left.key.cmp(&right.key));
        let mut conditions = self.command.conditions().to_vec();
        conditions.push(ControlRecordCondition::exact(key, prepared));
        conditions.sort_unstable_by(|left, right| left.key().cmp(right.key()));
        let mutation = ControlMutation::new(
            base.id(),
            command_digest,
            base.expected_generation(),
            writes,
            Vec::new(),
        );
        self.command = ConditionalControlMutation::new(mutation, conditions);
        Ok(self)
    }

    /// Exact immutable conditional command for provisioning evidence/recovery.
    #[must_use]
    pub const fn command(&self) -> &ConditionalControlMutation { &self.command }

    /// Exact journal identity bound by this command.
    #[must_use]
    pub const fn identity(&self) -> JournalIdentity { self.identity }

    /// Exact validated replacement pair.
    #[must_use]
    pub const fn replacement(&self) -> &ProviderAuthorityRecord { &self.replacement }
}

impl fmt::Debug for ProviderAuthorityMutation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProviderAuthorityMutation")
            .field("identity", &self.identity)
            .field("replacement", &self.replacement)
            .finish_non_exhaustive()
    }
}

/// Error preserving semantic corruption separately from context-controlled calls.
#[derive(Debug)]
pub enum ProviderAuthorityJournalError {
    /// Identity, shape, codec, transition or readback inconsistency.
    Record(ControlError),
    /// Original bounded disk call failure, including possible commit metadata.
    Call(ControlCallError),
}

impl fmt::Display for ProviderAuthorityJournalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Record(error) => fmt::Display::fmt(error, formatter),
            Self::Call(error) => fmt::Display::fmt(error, formatter),
        }
    }
}

impl std::error::Error for ProviderAuthorityJournalError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Record(error) => Some(error),
            Self::Call(error) => Some(error),
        }
    }
}

impl From<ControlError> for ProviderAuthorityJournalError {
    fn from(error: ControlError) -> Self { Self::Record(error) }
}
impl From<ControlCallError> for ProviderAuthorityJournalError {
    fn from(error: ControlCallError) -> Self { Self::Call(error) }
}

/// Commit evidence bound to the exact typed authority mutation.
#[derive(Clone)]
pub struct ProviderAuthorityCommit {
    mutation: ProviderAuthorityMutation,
    receipt: ControlCommitReceipt,
}

impl ProviderAuthorityCommit {
    /// Native journal receipt for guarded snapshot publication.
    #[must_use]
    pub const fn receipt(&self) -> &ControlCommitReceipt { &self.receipt }

    /// Exact replacement described by the committed command.
    #[must_use]
    pub const fn replacement(&self) -> &ProviderAuthorityRecord {
        &self.mutation.replacement
    }
}

impl fmt::Debug for ProviderAuthorityCommit {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProviderAuthorityCommit")
            .field("replacement", &self.mutation.replacement)
            .field("after_generation", &self.receipt.after_generation)
            .finish_non_exhaustive()
    }
}

/// One coherent published binding/policy observation and its journal generation.
///
/// Absence is explicit. Half-present, malformed, foreign or mismatched pairs fail
/// before construction. This readback is not a session, grant or source permit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderAuthorityReadback {
    identity: JournalIdentity,
    generation: u64,
    binding_id: BindingId,
    record: Option<ProviderAuthorityRecord>,
}

impl ProviderAuthorityReadback {
    /// Verified journal identity at read time.
    #[must_use]
    pub const fn identity(&self) -> JournalIdentity { self.identity }

    /// One generation shared by both records and the published snapshot.
    #[must_use]
    pub const fn generation(&self) -> u64 { self.generation }

    /// Coherent current pair, or verified absence of both rows.
    #[must_use]
    pub const fn record(&self) -> Option<&ProviderAuthorityRecord> {
        self.record.as_ref()
    }

    /// Confirm this exact point-in-time readback against one typed commit.
    ///
    /// # Errors
    /// Rejects foreign identity/keys, stale generation, unrelated receipt or
    /// replacement mismatch.
    pub fn confirm_commit(
        &self,
        committed: &ProviderAuthorityCommit,
    ) -> Result<(), ControlError> {
        let mutation = &committed.mutation;
        let request = mutation.command.mutation();
        let receipt = &committed.receipt;
        if self.identity != mutation.identity
            || self.binding_id != mutation.replacement.binding.binding_id
        {
            return Err(ControlError::IdentityMismatch);
        }
        if receipt.operation_id != request.id()
            || receipt.command_digest != request.command_digest()
            || receipt.before_generation != request.expected_generation()
            || receipt.before_generation.checked_add(1) != Some(receipt.after_generation)
        {
            return Err(ControlError::OperationConflict);
        }
        let expected_keys = request
            .writes()
            .iter()
            .map(|write| write.key.clone())
            .collect::<Vec<_>>();
        if receipt.changed_keys.as_slice() != expected_keys.as_slice() {
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
    /// Atomically commit one typed provider binding and matching policy.
    ///
    /// The returned receipt is historical evidence only. Serving requires a
    /// current published readback and the separate live barrier/domain checks.
    ///
    /// # Errors
    /// Preserves identity, generation, condition, interruption and possible-write
    /// failures without retrying under another operation identity.
    pub fn commit_provider_authority<C: CancellationProbe>(
        &mut self,
        mutation: &ProviderAuthorityMutation,
        context: &OperationContext<C>,
    ) -> Result<ProviderAuthorityCommit, ProviderAuthorityJournalError> {
        if self.identity() != mutation.identity {
            return Err(ControlError::IdentityMismatch.into());
        }
        let receipt = self.transact_conditionally(mutation.command.clone(), context)?;
        Ok(ProviderAuthorityCommit { mutation: mutation.clone(), receipt })
    }

    /// Resolve the exact typed authority mutation without dispatching a write.
    ///
    /// `Some` proves the original commit, not that it is still current. `None`
    /// proves resolved absence and allows retry of the same immutable command.
    ///
    /// # Errors
    /// Preserves identity, conflict, corruption, quarantine and interruption.
    pub fn recover_provider_authority<C: CancellationProbe>(
        &mut self,
        mutation: &ProviderAuthorityMutation,
        context: &OperationContext<C>,
    ) -> Result<Option<ProviderAuthorityCommit>, ProviderAuthorityJournalError> {
        if self.identity() != mutation.identity {
            return Err(ControlError::IdentityMismatch.into());
        }
        match self.recover_conditional_transaction(&mutation.command, context)? {
            CommitRecoveryDecision::Committed(receipt) => Ok(Some(ProviderAuthorityCommit {
                mutation: mutation.clone(),
                receipt,
            })),
            CommitRecoveryDecision::NotCommittedRetrySameOperation => Ok(None),
            CommitRecoveryDecision::ConflictingInput => {
                Err(ControlError::OperationConflict.into())
            }
            CommitRecoveryDecision::PartialOrCorruptQuarantine => {
                Err(ControlError::StoreQuarantined.into())
            }
        }
    }

    /// Read the exact binding/policy pair from one current published generation.
    ///
    /// Both rows are observed under one native header and one immutable published
    /// snapshot. Missing rows return `None` only when both are absent. Any half
    /// pair, foreign identity, malformed bytes or publication drift fails closed.
    /// The read performs no durable write or snapshot publication.
    ///
    /// # Errors
    /// Preserves bounded current-head call failures and semantic corruption.
    pub fn read_published_provider_authority<C: CancellationProbe>(
        &self,
        publisher: &ControlSnapshotPublisher,
        binding_id: BindingId,
        context: &OperationContext<C>,
    ) -> Result<ProviderAuthorityReadback, ProviderAuthorityJournalError> {
        let identity = self.identity();
        let binding_key = binding_key(identity.installation_incarnation_id, binding_id)?;
        let policy_key = policy_key(identity.installation_incarnation_id, binding_id)?;
        let (generation, [binding, policy]) = self.read_published_record_pair(
            publisher,
            [&binding_key, &policy_key],
            context,
        )?;
        let record = match (binding, policy) {
            (None, None) => None,
            (Some(binding), Some(policy)) => {
                let binding = decode_binding(&binding)?;
                let policy = decode_policy(&policy)?;
                if binding.binding_id != binding_id
                    || binding.installation_incarnation_id
                        != identity.installation_incarnation_id
                {
                    return Err(ControlError::StoreCorrupt.into());
                }
                Some(
                    ProviderAuthorityRecord::new(binding, policy)
                        .map_err(|_| ControlError::StoreCorrupt)?,
                )
            }
            (None, Some(_)) | (Some(_), None) => {
                return Err(ControlError::StoreCorrupt.into());
            }
        };
        Ok(ProviderAuthorityReadback {
            identity,
            generation,
            binding_id,
            record,
        })
    }
}

pub(crate) fn port_reserved_key(key: &[u8]) -> bool {
    key.starts_with(BINDING_KEY_PREFIX) || key.starts_with(POLICY_KEY_PREFIX)
}

fn validate_pair(
    binding: &ProviderBindingRecord,
    record: &StandalonePolicyRecord,
) -> Result<(), ControlError> {
    binding.validate_shape().map_err(|_| ControlError::InvalidValue)?;
    record.validate_shape().map_err(|_| ControlError::InvalidValue)?;
    let policy = &record.policy;
    let lifecycle_matches = matches!(
        (binding.status, record.state),
        (ProviderBindingStatus::Active, StandalonePolicyState::Active)
            | (ProviderBindingStatus::Revoked, StandalonePolicyState::Revoked)
            | (ProviderBindingStatus::Expired, StandalonePolicyState::Expired)
    );
    if binding.peer_role != PeerRole::StandaloneCli
        || binding.binding_id != policy.binding_id
        || binding.installation_id != policy.installation_id
        || binding.installation_incarnation_id != policy.installation_incarnation_id
        || binding.pairing_generation.get() != policy.binding_generation
        || !lifecycle_matches
    {
        return Err(ControlError::InvalidValue);
    }
    Ok(())
}

fn validate_binding_transition(
    before: &ProviderBindingRecord,
    after: &ProviderBindingRecord,
) -> Result<(), ControlError> {
    if before.status != ProviderBindingStatus::Active
        || before.binding_id != after.binding_id
        || before.installation_id != after.installation_id
        || before.installation_incarnation_id != after.installation_incarnation_id
        || before.peer_role != after.peer_role
        || before.peer_identity_digest != after.peer_identity_digest
        || before.issued_at != after.issued_at
        || before.pairing_generation.checked_next().ok() != Some(after.pairing_generation)
        || before.revocation_generation.checked_next().ok()
            != Some(after.revocation_generation)
        || before.expires_at.as_ref().is_some_and(|old| {
            after.expires_at.as_ref().is_none_or(|new| new > old)
        })
    {
        return Err(ControlError::InvalidValue);
    }
    Ok(())
}

fn validate_policy_transition(
    before: &StandalonePolicyRecord,
    after: &StandalonePolicyRecord,
) -> Result<(), ControlError> {
    let old = &before.policy;
    let new = &after.policy;
    if before.state != StandalonePolicyState::Active
        || old.binding_id != new.binding_id
        || old.installation_id != new.installation_id
        || old.installation_incarnation_id != new.installation_incarnation_id
        || old.principal_opaque_id != new.principal_opaque_id
        || old.client_scope_ref != new.client_scope_ref
        || old.scope_domain_id != new.scope_domain_id
        || before.issued_at != after.issued_at
        || old.policy_generation.checked_add(1) != Some(new.policy_generation)
        || new.binding_generation < old.binding_generation
        || new.revocation_generation < old.revocation_generation
        || (after.state != StandalonePolicyState::Active
            && new.revocation_generation == old.revocation_generation)
        || before.expires_at.as_ref().is_some_and(|old_end| {
            after.expires_at.as_ref().is_none_or(|end| end > old_end)
        })
    {
        return Err(ControlError::InvalidValue);
    }
    Ok(())
}

fn binding_key(
    incarnation: InstallationIncarnationId,
    binding: BindingId,
) -> Result<ControlKey, ControlError> {
    let mut bytes = Vec::with_capacity(BINDING_KEY_PREFIX.len() + 32);
    bytes.extend_from_slice(BINDING_KEY_PREFIX);
    bytes.extend_from_slice(incarnation.as_bytes());
    bytes.extend_from_slice(binding.as_bytes());
    ControlKey::new(bytes, JournalLimits::BASELINE)
}

fn policy_key(
    incarnation: InstallationIncarnationId,
    binding: BindingId,
) -> Result<ControlKey, ControlError> {
    let mut bytes = Vec::with_capacity(POLICY_KEY_PREFIX.len() + 32);
    bytes.extend_from_slice(POLICY_KEY_PREFIX);
    bytes.extend_from_slice(incarnation.as_bytes());
    bytes.extend_from_slice(binding.as_bytes());
    ControlKey::new(bytes, JournalLimits::BASELINE)
}

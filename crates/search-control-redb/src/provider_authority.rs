//! Typed provider-binding and standalone grant-policy persistence.
//!
//! This module owns the durable key layout, strict binary codecs, atomic pair
//! mutation, current published readback and exact recovery boundary. It stores
//! bounded authorization metadata only; it does not authenticate a transport,
//! mint a grant, interpret source scope or publish a live security barrier.

use core::fmt;

use search_contracts::{
    AccessPartitionId, AuthoritativeGrantPolicy, BindingId, Blake3Digest32, BoundedSet,
    CorpusId, CorpusOrPortfolioId, DisclosureCeiling, InstallationId,
    InstallationIncarnationId, Modality, NonZeroRevision, OpaqueId, OpaqueRef,
    PortfolioRevision, ProfileId, ProviderBindingRecord, ProviderBindingStatus, RecipeIdV1,
    ReferencePortfolioId, ScopeDomainId, SensitivityClass, SourceMembershipId,
    StandalonePolicyRecord, StandalonePolicyState, UtcTimestamp, MAX_OPAQUE_ID_BYTES,
    MAX_OPAQUE_REF_BYTES, MAX_PROFILE_ID_BYTES, MAX_SET_ITEMS, protocol::PeerRole,
};
use search_ports::{CancellationProbe, OperationContext};

use crate::{
    CommitRecoveryDecision, ConditionalControlMutation, ControlCallError, ControlCommitReceipt,
    ControlError, ControlKey, ControlMutation, ControlRecordClass, ControlRecordCondition,
    ControlSnapshotPublisher, ControlValue, ControlWrite, JournalIdentity, JournalLimits,
    MutationId, PersistentControlJournal,
};

const BINDING_KEY_PREFIX: &[u8] = b"eliot.control.provider-binding.v1\0";
const POLICY_KEY_PREFIX: &[u8] = b"eliot.control.standalone-policy.v1\0";
const BINDING_MAGIC: &[u8; 8] = b"ELBIND01";
const POLICY_MAGIC: &[u8; 8] = b"ELGRPOL1";
const VALUE_LIMIT: usize = JournalLimits::BASELINE.max_value_bytes;

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
    /// immutable peer identity fields stable, advances the pairing and binding
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
        let expected_keys = [mutation.binding_key.clone(), mutation.policy_key.clone()];
        if receipt.changed_keys.as_slice() != expected_keys {
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
                Some(ProviderAuthorityRecord::new(binding, policy)?)
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

fn encode_binding(record: &ProviderBindingRecord) -> Result<ControlValue, ControlError> {
    record.validate_shape().map_err(|_| ControlError::InvalidValue)?;
    let mut out = Vec::new();
    append(&mut out, BINDING_MAGIC, ControlError::InvalidValue)?;
    append(&mut out, record.binding_id.as_bytes(), ControlError::InvalidValue)?;
    append(&mut out, record.installation_id.as_bytes(), ControlError::InvalidValue)?;
    append(
        &mut out,
        record.installation_incarnation_id.as_bytes(),
        ControlError::InvalidValue,
    )?;
    append(
        &mut out,
        &[match record.peer_role {
            PeerRole::StandaloneCli => 0,
            PeerRole::ClientAdapter => 1,
            _ => return Err(ControlError::InvalidValue),
        }],
        ControlError::InvalidValue,
    )?;
    append(
        &mut out,
        record.peer_identity_digest.as_bytes(),
        ControlError::InvalidValue,
    )?;
    append(
        &mut out,
        &record.pairing_generation.get().to_be_bytes(),
        ControlError::InvalidValue,
    )?;
    append(
        &mut out,
        &u32::try_from(record.permitted_profile_ids.len())
            .map_err(|_| ControlError::InvalidValue)?
            .to_be_bytes(),
        ControlError::InvalidValue,
    )?;
    for profile in record.permitted_profile_ids.iter() {
        write_text(&mut out, profile.as_str(), ControlError::InvalidValue)?;
    }
    write_text(
        &mut out,
        record.disclosure_ceiling_ref.as_str(),
        ControlError::InvalidValue,
    )?;
    append(
        &mut out,
        record.issued_at.as_str().as_bytes(),
        ControlError::InvalidValue,
    )?;
    append(
        &mut out,
        &[u8::from(record.expires_at.is_some())],
        ControlError::InvalidValue,
    )?;
    if let Some(expires) = &record.expires_at {
        append(
            &mut out,
            expires.as_str().as_bytes(),
            ControlError::InvalidValue,
        )?;
    }
    append(
        &mut out,
        &record.revocation_generation.get().to_be_bytes(),
        ControlError::InvalidValue,
    )?;
    append(
        &mut out,
        &[match record.status {
            ProviderBindingStatus::Active => 0,
            ProviderBindingStatus::Revoked => 1,
            ProviderBindingStatus::Expired => 2,
        }],
        ControlError::InvalidValue,
    )?;
    ControlValue::new(ControlRecordClass::Identity, out, JournalLimits::BASELINE)
}

fn decode_binding(value: &ControlValue) -> Result<ProviderBindingRecord, ControlError> {
    if value.class() != ControlRecordClass::Identity
        || value.is_empty()
        || value.len() > VALUE_LIMIT
    {
        return Err(ControlError::StoreCorrupt);
    }
    let mut input = Reader::new(value.as_bytes());
    if input.take(8)? != BINDING_MAGIC {
        return Err(ControlError::StoreCorrupt);
    }
    let binding_id = BindingId::from_bytes(input.array()?);
    let installation_id = InstallationId::from_bytes(input.array()?);
    let installation_incarnation_id = InstallationIncarnationId::from_bytes(input.array()?);
    let peer_role = match input.byte()? {
        0 => PeerRole::StandaloneCli,
        1 => PeerRole::ClientAdapter,
        _ => return Err(ControlError::StoreCorrupt),
    };
    let peer_identity_digest = Blake3Digest32::from_bytes(input.array()?);
    let pairing_generation = NonZeroRevision::new(input.u64()?)
        .map_err(|_| ControlError::StoreCorrupt)?;
    let count = input.length(MAX_SET_ITEMS)?;
    if count > input.remaining().len() / 5 {
        return Err(ControlError::StoreCorrupt);
    }
    let mut profiles = Vec::new();
    profiles
        .try_reserve_exact(count)
        .map_err(|_| ControlError::StoreCorrupt)?;
    for _ in 0..count {
        let profile = ProfileId::new(input.text(MAX_PROFILE_ID_BYTES)?)
            .map_err(|_| ControlError::StoreCorrupt)?;
        if profiles.last().is_some_and(|previous| previous >= &profile) {
            return Err(ControlError::StoreCorrupt);
        }
        profiles.push(profile);
    }
    let permitted_profile_ids = BoundedSet::from_items(profiles)
        .map_err(|_| ControlError::StoreCorrupt)?;
    let disclosure_ceiling_ref = OpaqueRef::new(input.text(MAX_OPAQUE_REF_BYTES)?)
        .map_err(|_| ControlError::StoreCorrupt)?;
    let issued_at = input.timestamp()?;
    let expires_at = match input.byte()? {
        0 => None,
        1 => Some(input.timestamp()?),
        _ => return Err(ControlError::StoreCorrupt),
    };
    let revocation_generation = NonZeroRevision::new(input.u64()?)
        .map_err(|_| ControlError::StoreCorrupt)?;
    let status = match input.byte()? {
        0 => ProviderBindingStatus::Active,
        1 => ProviderBindingStatus::Revoked,
        2 => ProviderBindingStatus::Expired,
        _ => return Err(ControlError::StoreCorrupt),
    };
    input.finish()?;
    let record = ProviderBindingRecord {
        binding_id,
        installation_id,
        installation_incarnation_id,
        peer_role,
        peer_identity_digest,
        pairing_generation,
        permitted_profile_ids,
        disclosure_ceiling_ref,
        issued_at,
        expires_at,
        revocation_generation,
        status,
    };
    record
        .validate_shape()
        .map_err(|_| ControlError::StoreCorrupt)?;
    Ok(record)
}

fn encode_policy(record: &StandalonePolicyRecord) -> Result<ControlValue, ControlError> {
    record.validate_shape().map_err(|_| ControlError::InvalidValue)?;
    let mut out = Writer::new();
    out.raw(POLICY_MAGIC)?;
    out.byte(match record.state {
        StandalonePolicyState::Active => 0,
        StandalonePolicyState::Revoked => 1,
        StandalonePolicyState::Expired => 2,
    })?;
    out.text(record.issued_at.as_str())?;
    out.byte(u8::from(record.expires_at.is_some()))?;
    if let Some(value) = &record.expires_at {
        out.text(value.as_str())?;
    }
    let policy = &record.policy;
    out.raw(policy.binding_id.as_bytes())?;
    out.u64(policy.binding_generation)?;
    out.u64(policy.policy_generation)?;
    out.raw(policy.installation_id.as_bytes())?;
    out.raw(policy.installation_incarnation_id.as_bytes())?;
    out.text(policy.principal_opaque_id.as_str())?;
    out.text(policy.client_scope_ref.as_str())?;
    out.raw(policy.scope_domain_id.as_bytes())?;
    out.set(&policy.allowed_membership_ids, |out, id| {
        out.raw(id.as_bytes())
    })?;
    out.set(&policy.allowed_corpus_or_portfolio_ids, |out, id| match id {
        CorpusOrPortfolioId::Corpus(id) => {
            out.byte(0)?;
            out.raw(id.as_bytes())
        }
        CorpusOrPortfolioId::Portfolio(id) => {
            out.byte(1)?;
            out.raw(id.as_bytes())
        }
    })?;
    out.byte(u8::from(policy.reference_portfolio_revision.is_some()))?;
    if let Some(value) = policy.reference_portfolio_revision {
        out.u64(value.get())?;
    }
    out.set(&policy.allowed_access_partitions, |out, id| {
        out.raw(id.as_bytes())
    })?;
    out.set(&policy.allowed_modalities, |out, value| {
        out.text(value.as_str())
    })?;
    out.set(&policy.permitted_recipe_families, |out, value| {
        out.text(value.as_str())
    })?;
    out.set(&policy.allowed_budget_classes, |out, value| {
        out.text(value.as_str())
    })?;
    out.text(policy.sensitivity_ceiling.as_str())?;
    out.text(policy.disclosure_ceiling.as_str())?;
    out.byte(u8::from(policy.source_read_permission))?;
    out.byte(u8::from(policy.exact_scan_permission))?;
    out.text(policy.issued_boot_id.as_str())?;
    out.u64(policy.revocation_generation)?;
    out.u64(policy.maximum_ttl_ms)?;
    ControlValue::new(ControlRecordClass::State, out.finish(), JournalLimits::BASELINE)
}

fn decode_policy(value: &ControlValue) -> Result<StandalonePolicyRecord, ControlError> {
    if value.class() != ControlRecordClass::State
        || value.is_empty()
        || value.len() > VALUE_LIMIT
    {
        return Err(ControlError::StoreCorrupt);
    }
    let mut input = Reader::new(value.as_bytes());
    if input.take(POLICY_MAGIC.len())? != POLICY_MAGIC {
        return Err(ControlError::StoreCorrupt);
    }
    let state = match input.byte()? {
        0 => StandalonePolicyState::Active,
        1 => StandalonePolicyState::Revoked,
        2 => StandalonePolicyState::Expired,
        _ => return Err(ControlError::StoreCorrupt),
    };
    let issued_at = UtcTimestamp::parse(input.text(27)?)
        .map_err(|_| ControlError::StoreCorrupt)?;
    let expires_at = if input.boolean()? {
        Some(
            UtcTimestamp::parse(input.text(27)?)
                .map_err(|_| ControlError::StoreCorrupt)?,
        )
    } else {
        None
    };
    let policy = AuthoritativeGrantPolicy {
        binding_id: BindingId::from_bytes(input.array()?),
        binding_generation: input.u64()?,
        policy_generation: input.u64()?,
        installation_id: InstallationId::from_bytes(input.array()?),
        installation_incarnation_id: InstallationIncarnationId::from_bytes(input.array()?),
        principal_opaque_id: OpaqueId::new(input.text(MAX_OPAQUE_ID_BYTES)?)
            .map_err(|_| ControlError::StoreCorrupt)?,
        client_scope_ref: OpaqueRef::new(input.text(MAX_OPAQUE_REF_BYTES)?)
            .map_err(|_| ControlError::StoreCorrupt)?,
        scope_domain_id: ScopeDomainId::from_bytes(input.array()?),
        allowed_membership_ids: input.set(|input| {
            Ok(SourceMembershipId::from_bytes(input.array()?))
        })?,
        allowed_corpus_or_portfolio_ids: input.set(|input| match input.byte()? {
            0 => Ok(CorpusOrPortfolioId::Corpus(CorpusId::from_bytes(
                input.array()?,
            ))),
            1 => Ok(CorpusOrPortfolioId::Portfolio(
                ReferencePortfolioId::from_bytes(input.array()?),
            )),
            _ => Err(ControlError::StoreCorrupt),
        })?,
        reference_portfolio_revision: if input.boolean()? {
            Some(PortfolioRevision::new(input.u64()?))
        } else {
            None
        },
        allowed_access_partitions: input.set(|input| {
            Ok(AccessPartitionId::from_bytes(input.array()?))
        })?,
        allowed_modalities: input.set(|input| {
            Modality::parse(input.text(64)?).map_err(|_| ControlError::StoreCorrupt)
        })?,
        permitted_recipe_families: input.set(|input| {
            RecipeIdV1::parse(input.text(64)?).map_err(|_| ControlError::StoreCorrupt)
        })?,
        allowed_budget_classes: input.set(|input| {
            ProfileId::new(input.text(MAX_PROFILE_ID_BYTES)?)
                .map_err(|_| ControlError::StoreCorrupt)
        })?,
        sensitivity_ceiling: SensitivityClass::parse(input.text(64)?)
            .map_err(|_| ControlError::StoreCorrupt)?,
        disclosure_ceiling: DisclosureCeiling::parse(input.text(64)?)
            .map_err(|_| ControlError::StoreCorrupt)?,
        source_read_permission: input.boolean()?,
        exact_scan_permission: input.boolean()?,
        issued_boot_id: OpaqueId::new(input.text(MAX_OPAQUE_ID_BYTES)?)
            .map_err(|_| ControlError::StoreCorrupt)?,
        revocation_generation: input.u64()?,
        maximum_ttl_ms: input.u64()?,
    };
    input.finish()?;
    let record = StandalonePolicyRecord {
        policy,
        state,
        issued_at,
        expires_at,
    };
    record
        .validate_shape()
        .map_err(|_| ControlError::StoreCorrupt)?;
    Ok(record)
}

fn append(
    output: &mut Vec<u8>,
    bytes: &[u8],
    error: ControlError,
) -> Result<(), ControlError> {
    if output
        .len()
        .checked_add(bytes.len())
        .is_none_or(|end| end > VALUE_LIMIT)
    {
        return Err(error);
    }
    output.try_reserve(bytes.len()).map_err(|_| error)?;
    output.extend_from_slice(bytes);
    Ok(())
}

fn write_text(
    output: &mut Vec<u8>,
    value: &str,
    error: ControlError,
) -> Result<(), ControlError> {
    append(
        output,
        &u32::try_from(value.len())
            .map_err(|_| error)?
            .to_be_bytes(),
        error,
    )?;
    append(output, value.as_bytes(), error)
}

struct Writer(Vec<u8>);

impl Writer {
    fn new() -> Self { Self(Vec::new()) }

    fn finish(self) -> Vec<u8> { self.0 }

    fn raw(&mut self, bytes: &[u8]) -> Result<(), ControlError> {
        append(&mut self.0, bytes, ControlError::InvalidValue)
    }

    fn byte(&mut self, value: u8) -> Result<(), ControlError> { self.raw(&[value]) }

    fn u64(&mut self, value: u64) -> Result<(), ControlError> {
        self.raw(&value.to_be_bytes())
    }

    fn text(&mut self, value: &str) -> Result<(), ControlError> {
        write_text(&mut self.0, value, ControlError::InvalidValue)
    }

    fn set<T: Ord>(
        &mut self,
        values: &BoundedSet<T, MAX_SET_ITEMS>,
        mut put: impl FnMut(&mut Self, &T) -> Result<(), ControlError>,
    ) -> Result<(), ControlError> {
        self.raw(
            &u32::try_from(values.len())
                .map_err(|_| ControlError::InvalidValue)?
                .to_be_bytes(),
        )?;
        for value in values.iter() {
            put(self, value)?;
        }
        Ok(())
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Reader<'a> {
    const fn new(bytes: &'a [u8]) -> Self { Self { bytes, position: 0 } }

    fn remaining(&self) -> &'a [u8] { &self.bytes[self.position..] }

    fn take(&mut self, length: usize) -> Result<&'a [u8], ControlError> {
        let end = self
            .position
            .checked_add(length)
            .ok_or(ControlError::StoreCorrupt)?;
        let bytes = self
            .bytes
            .get(self.position..end)
            .ok_or(ControlError::StoreCorrupt)?;
        self.position = end;
        Ok(bytes)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], ControlError> {
        self.take(N)?
            .try_into()
            .map_err(|_| ControlError::StoreCorrupt)
    }

    fn byte(&mut self) -> Result<u8, ControlError> { Ok(self.array::<1>()?[0]) }

    fn u64(&mut self) -> Result<u64, ControlError> {
        Ok(u64::from_be_bytes(self.array()?))
    }

    fn length(&mut self, maximum: usize) -> Result<usize, ControlError> {
        let length = usize::try_from(u32::from_be_bytes(self.array()?))
            .map_err(|_| ControlError::StoreCorrupt)?;
        if length > maximum {
            return Err(ControlError::StoreCorrupt);
        }
        Ok(length)
    }

    fn boolean(&mut self) -> Result<bool, ControlError> {
        match self.byte()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(ControlError::StoreCorrupt),
        }
    }

    fn text(&mut self, maximum: usize) -> Result<&'a str, ControlError> {
        let length = self.length(maximum)?;
        core::str::from_utf8(self.take(length)?).map_err(|_| ControlError::StoreCorrupt)
    }

    fn timestamp(&mut self) -> Result<UtcTimestamp, ControlError> {
        UtcTimestamp::parse(
            core::str::from_utf8(self.take(27)?)
                .map_err(|_| ControlError::StoreCorrupt)?,
        )
        .map_err(|_| ControlError::StoreCorrupt)
    }

    fn set<T: Ord>(
        &mut self,
        mut get: impl FnMut(&mut Self) -> Result<T, ControlError>,
    ) -> Result<BoundedSet<T, MAX_SET_ITEMS>, ControlError> {
        let count = self.length(MAX_SET_ITEMS)?;
        if count > self.remaining().len() {
            return Err(ControlError::StoreCorrupt);
        }
        let mut values = Vec::new();
        values
            .try_reserve_exact(count)
            .map_err(|_| ControlError::StoreCorrupt)?;
        for _ in 0..count {
            let next = get(self)?;
            if values.last().is_some_and(|previous| previous >= &next) {
                return Err(ControlError::StoreCorrupt);
            }
            values.push(next);
        }
        BoundedSet::from_items(values).map_err(|_| ControlError::StoreCorrupt)
    }

    fn finish(self) -> Result<(), ControlError> {
        if self.position == self.bytes.len() {
            Ok(())
        } else {
            Err(ControlError::StoreCorrupt)
        }
    }
}

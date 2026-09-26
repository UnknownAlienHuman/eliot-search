//! Durable exact-input provisioning intent in the existing control journal.
//!
//! Credential Manager cannot atomically commit with redb and cannot hold the
//! complete bounded binding/policy command. Five immutable operation-specific
//! records therefore retain the prepared header and exact prior/next values.
//! The final registration transaction changes only the header to COMMITTED while
//! replacing binding and policy. No second database, table or mutable owner exists.

use search_contracts::{BindingId, Blake3Digest32, ProfileId, MAX_PROFILE_ID_BYTES};
use search_control_redb::{
    CommitRecoveryDecision, ConditionalControlMutation, ControlCommitReceipt, ControlError,
    ControlKey, ControlMutation, ControlRecordClass, ControlRecordCondition,
    ControlSnapshotPublisher, ControlValue, ControlWrite, JournalIdentity, JournalLimits,
    MutationId, PersistentControlJournal,
};
use search_ports::{CancellationProbe, OperationContext};

use crate::access_composition::{
    NativeGrantPolicyError, StandalonePolicyRecord,
};
use crate::access_composition::native_grant_policy::policy_key;
use super::{
    StandaloneRegistrationMutation, ProviderBindingRecord, validate_pair,
};
use super::super::{binding_key, codec};

const MAGIC: &[u8; 8] = b"ELSPIN01";
const ABSENT: &[u8; 8] = b"ELSPABS1";
const PREFIX: &[u8] = b"eliot.control.standalone-provisioning.v1\0";
const HEADER_SLOT: u8 = 0;
const PRIOR_BINDING_SLOT: u8 = 1;
const PRIOR_POLICY_SLOT: u8 = 2;
const NEXT_BINDING_SLOT: u8 = 3;
const NEXT_POLICY_SLOT: u8 = 4;
const SLOT_COUNT: usize = 5;

/// Published state of one exact provisioning intent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StandaloneProvisioningIntentState {
    /// Full command input is durable; credential/final registration may remain.
    Prepared,
    /// The final atomic binding/policy transaction was published.
    Committed,
}

/// Exact durable command material for one registration operation.
///
/// The payload rows retain original `ControlValue` classes and bytes. The header
/// binds their digests, final operation identity/generation and selected profile.
/// This value is not source authority, a credential, a receipt or a live permit.
#[must_use = "retain intent until credential and registration outcomes are resolved"]
pub struct StandaloneProvisioningIntent {
    identity: JournalIdentity,
    binding_id: BindingId,
    operation_id: MutationId,
    profile_id: ProfileId,
    state: StandaloneProvisioningIntentState,
    persistence: ConditionalControlMutation,
    registration: StandaloneRegistrationMutation,
    prepared_rows: [ControlValue; SLOT_COUNT],
    committed_header: ControlValue,
    prior: Option<(ProviderBindingRecord, StandalonePolicyRecord)>,
    next: (ProviderBindingRecord, StandalonePolicyRecord),
}

impl StandaloneProvisioningIntent {
    /// Prepare one immutable intent transaction and the final registration command.
    ///
    /// The intent commit consumes the observed generation. Consequently the final
    /// command is bound to exactly `observed_generation + 1`; no later transaction
    /// can be silently rebased between intent publication and credential effects.
    /// Intent keys are operation-specific and must all be absent.
    ///
    /// # Errors
    /// Rejects invalid pair transitions, generation exhaustion, duplicate keys,
    /// oversized values or a profile not present in the replacement binding.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare(
        identity: JournalIdentity,
        observed_generation: u64,
        operation_id: MutationId,
        prior: Option<(&ProviderBindingRecord, &StandalonePolicyRecord)>,
        binding: &ProviderBindingRecord,
        policy: &StandalonePolicyRecord,
        profile_id: ProfileId,
    ) -> Result<Self, NativeGrantPolicyError> {
        let _ = identity.validate()?;
        validate_pair(binding, policy)?;
        if let Some((before_binding, before_policy)) = prior {
            validate_pair(before_binding, before_policy)?;
        }
        if !binding.permitted_profile_ids.contains(&profile_id) {
            return Err(NativeGrantPolicyError::InvalidRecord);
        }
        let final_generation = observed_generation.checked_add(1)
            .ok_or(ControlError::GenerationExhausted)?;

        let base = StandaloneRegistrationMutation::new(
            identity, operation_id, final_generation, prior, binding, policy,
        )?;
        let actual_binding_key = binding_key(
            binding.installation_incarnation_id, binding.binding_id,
        )?;
        let actual_policy_key = policy_key(
            binding.installation_incarnation_id, binding.binding_id,
        )?;
        let prior_binding = condition_value(base.command(), &actual_binding_key)?;
        let prior_policy = condition_value(base.command(), &actual_policy_key)?;
        let next_binding = write_value(base.command(), &actual_binding_key)?;
        let next_policy = write_value(base.command(), &actual_policy_key)?;
        if prior_binding.is_some() != prior_policy.is_some() {
            return Err(NativeGrantPolicyError::InvalidRecord);
        }

        let slots = [
            prior_binding.clone().unwrap_or(absent_value()?),
            prior_policy.clone().unwrap_or(absent_value()?),
            next_binding.clone(),
            next_policy.clone(),
        ];
        let base_digest = base.command().mutation().command_digest();
        let prepared_header = encode_header(
            StandaloneProvisioningIntentState::Prepared,
            operation_id,
            base_digest,
            final_generation,
            &profile_id,
            &slots,
        )?;
        let committed_header = encode_header(
            StandaloneProvisioningIntentState::Committed,
            operation_id,
            base_digest,
            final_generation,
            &profile_id,
            &slots,
        )?;
        let keys = intent_keys(identity, binding.binding_id, operation_id)?;
        let prepared_rows = [
            prepared_header.clone(),
            slots[0].clone(),
            slots[1].clone(),
            slots[2].clone(),
            slots[3].clone(),
        ];
        let registration = base.with_intent_header(
            keys[HEADER_SLOT as usize].clone(),
            prepared_header,
            committed_header.clone(),
        )?;
        let persistence = persistence_command(
            observed_generation, operation_id, &keys, &prepared_rows,
        )?;
        Ok(Self {
            identity,
            binding_id: binding.binding_id,
            operation_id,
            profile_id,
            state: StandaloneProvisioningIntentState::Prepared,
            persistence,
            registration,
            prepared_rows,
            committed_header,
            prior: prior.map(|(binding, policy)| (binding.clone(), policy.clone())),
            next: (binding.clone(), policy.clone()),
        })
    }

    /// Read and validate one exact operation-specific intent and the actual
    /// binding/policy rows from the same disk-published generation.
    ///
    /// All five intent records must be present or absent together. PREPARED must
    /// still represent the exact prior pair at the final command's expected
    /// generation. COMMITTED must represent the exact replacement pair at a later
    /// generation. Payload digests, codecs and command digest are reconstructed.
    ///
    /// # Errors
    /// Half-present, malformed, moved, stale or contradictory state fails closed.
    pub fn read_published<C: CancellationProbe>(
        journal: &PersistentControlJournal,
        publisher: &ControlSnapshotPublisher,
        binding_id: BindingId,
        operation_id: MutationId,
        context: &OperationContext<C>,
    ) -> Result<Option<Self>, NativeGrantPolicyError> {
        let identity = journal.identity();
        let actual_binding_key = binding_key(identity.installation_incarnation_id, binding_id)?;
        let actual_policy_key = policy_key(identity.installation_incarnation_id, binding_id)?;
        let keys = intent_keys(identity, binding_id, operation_id)?;
        let (generation, values) = journal.read_published_records(
            publisher,
            [
                &actual_binding_key,
                &actual_policy_key,
                &keys[0],
                &keys[1],
                &keys[2],
                &keys[3],
                &keys[4],
            ],
            context,
        )?;
        Self::from_observed(identity, binding_id, operation_id, generation, keys, values)
    }

    /// Read the exact durable head for restart recovery without publishing it.
    ///
    /// This is the only path that can reconstruct a final command after its
    /// redb commit advanced beyond the last admission snapshot. Returned metadata
    /// never authorizes serving. PREPARED may subsequently be republished because
    /// it changes no binding/policy authority; COMMITTED still requires live
    /// barriers before guarded publication.
    pub fn read_current_for_recovery<C: CancellationProbe>(
        journal: &PersistentControlJournal,
        binding_id: BindingId,
        operation_id: MutationId,
        context: &OperationContext<C>,
    ) -> Result<Option<Self>, NativeGrantPolicyError> {
        let identity = journal.identity();
        let actual_binding_key = binding_key(identity.installation_incarnation_id, binding_id)?;
        let actual_policy_key = policy_key(identity.installation_incarnation_id, binding_id)?;
        let keys = intent_keys(identity, binding_id, operation_id)?;
        let (generation, values) = journal.read_current_records_for_recovery(
            [
                &actual_binding_key,
                &actual_policy_key,
                &keys[0],
                &keys[1],
                &keys[2],
                &keys[3],
                &keys[4],
            ],
            context,
        )?;
        Self::from_observed(identity, binding_id, operation_id, generation, keys, values)
    }

    fn from_observed(
        identity: JournalIdentity,
        binding_id: BindingId,
        operation_id: MutationId,
        generation: u64,
        keys: [ControlKey; SLOT_COUNT],
        values: [Option<ControlValue>; 7],
    ) -> Result<Option<Self>, NativeGrantPolicyError> {
        let [actual_binding, actual_policy, header, prior_binding, prior_policy,
            next_binding, next_policy] = values;
        let intent_values = [
            header, prior_binding, prior_policy, next_binding, next_policy,
        ];
        if intent_values.iter().all(Option::is_none) {
            return Ok(None);
        }
        if intent_values.iter().any(Option::is_none) {
            return Err(NativeGrantPolicyError::InvalidRecord);
        }
        let [Some(header_value), Some(prior_binding_value), Some(prior_policy_value),
            Some(next_binding_value), Some(next_policy_value)] = intent_values else {
            return Err(NativeGrantPolicyError::InvalidRecord);
        };
        let header = decode_header(&header_value)?;
        if header.operation_id != operation_id {
            return Err(NativeGrantPolicyError::InvalidRecord);
        }
        let slots = [
            prior_binding_value.clone(),
            prior_policy_value.clone(),
            next_binding_value.clone(),
            next_policy_value.clone(),
        ];
        for (expected, value) in header.payload_digests.iter().zip(&slots) {
            if expected != &value_digest(value) {
                return Err(NativeGrantPolicyError::InvalidRecord);
            }
        }

        let prior_binding_raw = optional_value(&prior_binding_value)?;
        let prior_policy_raw = optional_value(&prior_policy_value)?;
        if prior_binding_raw.is_some() != prior_policy_raw.is_some() {
            return Err(NativeGrantPolicyError::InvalidRecord);
        }
        let prior = match (prior_binding_raw.as_ref(), prior_policy_raw.as_ref()) {
            (Some(binding), Some(policy)) => {
                let binding = codec::decode(binding)?;
                let policy = StandalonePolicyRecord::decode_native(policy)?;
                validate_pair(&binding, &policy)?;
                Some((binding, policy))
            }
            (None, None) => None,
            _ => return Err(NativeGrantPolicyError::InvalidRecord),
        };
        let next_binding_record = codec::decode(&next_binding_value)?;
        let next_policy_record = StandalonePolicyRecord::decode_native(&next_policy_value)?;
        validate_pair(&next_binding_record, &next_policy_record)?;
        if next_binding_record.binding_id != binding_id
            || next_binding_record.installation_incarnation_id
                != identity.installation_incarnation_id
            || !next_binding_record.permitted_profile_ids.contains(&header.profile_id)
        {
            return Err(NativeGrantPolicyError::InvalidRecord);
        }

        match header.state {
            StandaloneProvisioningIntentState::Prepared => {
                if generation != header.final_expected_generation
                    || actual_binding.as_ref() != prior_binding_raw.as_ref()
                    || actual_policy.as_ref() != prior_policy_raw.as_ref()
                {
                    return Err(ControlError::TransactionConflict.into());
                }
            }
            StandaloneProvisioningIntentState::Committed => {
                let minimum = header.final_expected_generation.checked_add(1)
                    .ok_or(ControlError::GenerationExhausted)?;
                if generation < minimum
                    || actual_binding.as_ref() != Some(&next_binding_value)
                    || actual_policy.as_ref() != Some(&next_policy_value)
                {
                    return Err(ControlError::TransactionConflict.into());
                }
            }
        }

        let prior_refs = prior.as_ref().map(|(binding, policy)| (binding, policy));
        let base = StandaloneRegistrationMutation::new(
            identity,
            operation_id,
            header.final_expected_generation,
            prior_refs,
            &next_binding_record,
            &next_policy_record,
        )?;
        if base.command().mutation().command_digest() != header.base_command_digest {
            return Err(NativeGrantPolicyError::InvalidRecord);
        }
        let prepared_header = encode_header(
            StandaloneProvisioningIntentState::Prepared,
            operation_id,
            header.base_command_digest,
            header.final_expected_generation,
            &header.profile_id,
            &slots,
        )?;
        let committed_header = encode_header(
            StandaloneProvisioningIntentState::Committed,
            operation_id,
            header.base_command_digest,
            header.final_expected_generation,
            &header.profile_id,
            &slots,
        )?;
        let registration = base.with_intent_header(
            keys[0].clone(), prepared_header.clone(), committed_header.clone(),
        )?;
        let observed_generation = header.final_expected_generation.checked_sub(1)
            .ok_or(NativeGrantPolicyError::InvalidRecord)?;
        let prepared_rows = [
            prepared_header,
            slots[0].clone(),
            slots[1].clone(),
            slots[2].clone(),
            slots[3].clone(),
        ];
        let persistence = persistence_command(
            observed_generation, operation_id, &keys, &prepared_rows,
        )?;
        Ok(Some(Self {
            identity,
            binding_id,
            operation_id,
            profile_id: header.profile_id,
            state: header.state,
            persistence,
            registration,
            prepared_rows,
            committed_header,
            prior,
            next: (next_binding_record, next_policy_record),
        }))
    }

    /// Persist the five exact intent rows through the existing journal engine.
    pub fn persist<C: CancellationProbe>(
        &self,
        journal: &mut PersistentControlJournal,
        context: &OperationContext<C>,
    ) -> Result<ControlCommitReceipt, NativeGrantPolicyError> {
        self.check_identity(journal)?;
        journal.transact_conditionally(self.persistence.clone(), context)
            .map_err(Into::into)
    }

    /// Resolve an uncertain intent transaction without dispatching another write.
    pub fn recover<C: CancellationProbe>(
        &self,
        journal: &mut PersistentControlJournal,
        context: &OperationContext<C>,
    ) -> Result<Option<ControlCommitReceipt>, NativeGrantPolicyError> {
        self.check_identity(journal)?;
        match journal.recover_conditional_transaction(&self.persistence, context)? {
            CommitRecoveryDecision::Committed(receipt) => Ok(Some(receipt)),
            CommitRecoveryDecision::NotCommittedRetrySameOperation => Ok(None),
            CommitRecoveryDecision::ConflictingInput => {
                Err(ControlError::OperationConflict.into())
            }
            CommitRecoveryDecision::PartialOrCorruptQuarantine => {
                Err(ControlError::StoreQuarantined.into())
            }
        }
    }

    /// Verify the exact durable bundle and coherent live pair at one published head.
    pub fn confirm_published<C: CancellationProbe>(
        &self,
        journal: &PersistentControlJournal,
        publisher: &ControlSnapshotPublisher,
        state: StandaloneProvisioningIntentState,
        context: &OperationContext<C>,
    ) -> Result<(), NativeGrantPolicyError> {
        self.check_identity(journal)?;
        let current = Self::read_published(
            journal, publisher, self.binding_id, self.operation_id, context,
        )?.ok_or(NativeGrantPolicyError::InvalidRecord)?;
        if current.state != state
            || current.profile_id != self.profile_id
            || current.registration.credential_intent()
                != self.registration.credential_intent()
            || !same_rows(&current.rows_for(state), &self.rows_for(state))
        {
            return Err(ControlError::TransactionConflict.into());
        }
        Ok(())
    }

    /// Final registration command bound to this intent header.
    #[must_use]
    pub(super) const fn registration(&self) -> &StandaloneRegistrationMutation {
        &self.registration
    }

    /// Exact selected profile retained in the intent header.
    #[must_use]
    pub const fn profile_id(&self) -> &ProfileId { &self.profile_id }

    /// Current published intent state.
    #[must_use]
    pub const fn state(&self) -> StandaloneProvisioningIntentState { self.state }

    pub(super) fn mark_committed(&mut self) {
        self.state = StandaloneProvisioningIntentState::Committed;
    }

    /// Exact final operation identity.
    #[must_use]
    pub const fn operation_id(&self) -> MutationId { self.operation_id }

    /// Replacement binding and policy retained by the durable payload.
    #[must_use]
    pub fn replacement(&self) -> (&ProviderBindingRecord, &StandalonePolicyRecord) {
        (&self.next.0, &self.next.1)
    }

    /// Original pair, or verified absence of both.
    #[must_use]
    pub fn prior(&self) -> Option<(&ProviderBindingRecord, &StandalonePolicyRecord)> {
        self.prior.as_ref().map(|(binding, policy)| (binding, policy))
    }

    fn rows_for(&self, state: StandaloneProvisioningIntentState) -> [ControlValue; SLOT_COUNT] {
        let mut rows = self.prepared_rows.clone();
        if state == StandaloneProvisioningIntentState::Committed {
            rows[0] = self.committed_header.clone();
        }
        rows
    }

    fn check_identity(
        &self,
        journal: &PersistentControlJournal,
    ) -> Result<(), NativeGrantPolicyError> {
        if journal.identity() != self.identity {
            return Err(ControlError::IdentityMismatch.into());
        }
        Ok(())
    }
}

impl core::fmt::Debug for StandaloneProvisioningIntent {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("StandaloneProvisioningIntent")
            .field("state", &self.state)
            .field("operation_id", &self.operation_id)
            .field("profile_id", &self.profile_id)
            .finish_non_exhaustive()
    }
}

struct Header {
    state: StandaloneProvisioningIntentState,
    operation_id: MutationId,
    base_command_digest: Blake3Digest32,
    final_expected_generation: u64,
    profile_id: ProfileId,
    payload_digests: [[u8; 32]; 4],
}

fn encode_header(
    state: StandaloneProvisioningIntentState,
    operation_id: MutationId,
    base_command_digest: Blake3Digest32,
    final_expected_generation: u64,
    profile_id: &ProfileId,
    slots: &[ControlValue; 4],
) -> Result<ControlValue, NativeGrantPolicyError> {
    if profile_id.as_str().is_empty() || profile_id.as_str().len() > MAX_PROFILE_ID_BYTES {
        return Err(NativeGrantPolicyError::InvalidRecord);
    }
    let mut bytes = Vec::new();
    bytes.extend_from_slice(MAGIC);
    bytes.push(match state {
        StandaloneProvisioningIntentState::Prepared => 0,
        StandaloneProvisioningIntentState::Committed => 1,
    });
    bytes.extend_from_slice(&operation_id.0);
    bytes.extend_from_slice(base_command_digest.as_bytes());
    bytes.extend_from_slice(&final_expected_generation.to_be_bytes());
    bytes.extend_from_slice(
        &u32::try_from(profile_id.as_str().len())
            .map_err(|_| NativeGrantPolicyError::InvalidRecord)?
            .to_be_bytes(),
    );
    bytes.extend_from_slice(profile_id.as_str().as_bytes());
    for value in slots {
        bytes.extend_from_slice(&value_digest(value));
    }
    ControlValue::new(
        ControlRecordClass::Operation, bytes, JournalLimits::BASELINE,
    ).map_err(Into::into)
}

fn decode_header(value: &ControlValue) -> Result<Header, NativeGrantPolicyError> {
    if value.class() != ControlRecordClass::Operation {
        return Err(NativeGrantPolicyError::InvalidRecord);
    }
    let mut input = Input(value.as_bytes());
    if input.take(8)? != MAGIC {
        return Err(NativeGrantPolicyError::InvalidRecord);
    }
    let state = match input.byte()? {
        0 => StandaloneProvisioningIntentState::Prepared,
        1 => StandaloneProvisioningIntentState::Committed,
        _ => return Err(NativeGrantPolicyError::InvalidRecord),
    };
    let operation_id = MutationId(input.array()?);
    let base_command_digest = Blake3Digest32::from_bytes(input.array()?);
    let final_expected_generation = input.u64()?;
    let profile_length = input.length(MAX_PROFILE_ID_BYTES)?;
    let profile_id = ProfileId::new(input.text(profile_length)?)
        .map_err(|_| NativeGrantPolicyError::InvalidRecord)?;
    let mut payload_digests = [[0_u8; 32]; 4];
    for digest in &mut payload_digests {
        *digest = input.array()?;
    }
    if !input.is_empty() || final_expected_generation == 0 {
        return Err(NativeGrantPolicyError::InvalidRecord);
    }
    Ok(Header {
        state,
        operation_id,
        base_command_digest,
        final_expected_generation,
        profile_id,
        payload_digests,
    })
}

fn persistence_command(
    observed_generation: u64,
    operation_id: MutationId,
    keys: &[ControlKey; SLOT_COUNT],
    rows: &[ControlValue; SLOT_COUNT],
) -> Result<ConditionalControlMutation, NativeGrantPolicyError> {
    let mut writes = Vec::with_capacity(SLOT_COUNT);
    let mut conditions = Vec::with_capacity(SLOT_COUNT);
    let mut digest = blake3::Hasher::new();
    digest.update(b"ELIOT-STANDALONE-PROVISIONING-INTENT-v1\0");
    digest.update(&observed_generation.to_be_bytes());
    for (key, value) in keys.iter().zip(rows) {
        hash_key_value(&mut digest, key, value)?;
        writes.push(ControlWrite { key: key.clone(), value: value.clone() });
        conditions.push(ControlRecordCondition::absent(key.clone()));
    }
    writes.sort_unstable_by(|left, right| left.key.cmp(&right.key));
    conditions.sort_unstable_by(|left, right| left.key().cmp(right.key()));
    let mut id = blake3::Hasher::new();
    id.update(b"ELIOT-STANDALONE-PROVISIONING-INTENT-OP-v1\0");
    id.update(&operation_id.0);
    let mutation = ControlMutation::new(
        MutationId(*id.finalize().as_bytes()),
        Blake3Digest32::from_bytes(*digest.finalize().as_bytes()),
        observed_generation,
        writes,
        Vec::new(),
    );
    Ok(ConditionalControlMutation::new(mutation, conditions))
}

fn intent_keys(
    identity: JournalIdentity,
    binding_id: BindingId,
    operation_id: MutationId,
) -> Result<[ControlKey; SLOT_COUNT], ControlError> {
    let key = |slot: u8| {
        let mut bytes = Vec::with_capacity(
            PREFIX.len()
                + identity.installation_incarnation_id.as_bytes().len()
                + binding_id.as_bytes().len()
                + operation_id.0.len()
                + 1,
        );
        bytes.extend_from_slice(PREFIX);
        bytes.extend_from_slice(identity.installation_incarnation_id.as_bytes());
        bytes.extend_from_slice(binding_id.as_bytes());
        bytes.extend_from_slice(&operation_id.0);
        bytes.push(slot);
        ControlKey::new(bytes, JournalLimits::BASELINE)
    };
    Ok([
        key(HEADER_SLOT)?,
        key(PRIOR_BINDING_SLOT)?,
        key(PRIOR_POLICY_SLOT)?,
        key(NEXT_BINDING_SLOT)?,
        key(NEXT_POLICY_SLOT)?,
    ])
}

fn condition_value(
    command: &ConditionalControlMutation,
    key: &ControlKey,
) -> Result<Option<ControlValue>, NativeGrantPolicyError> {
    command.conditions().iter()
        .find(|condition| condition.key() == key)
        .map(|condition| condition.expected().cloned())
        .ok_or(NativeGrantPolicyError::InvalidRecord)
}

fn write_value(
    command: &ConditionalControlMutation,
    key: &ControlKey,
) -> Result<ControlValue, NativeGrantPolicyError> {
    command.mutation().writes().iter()
        .find(|write| &write.key == key)
        .map(|write| write.value.clone())
        .ok_or(NativeGrantPolicyError::InvalidRecord)
}

fn absent_value() -> Result<ControlValue, NativeGrantPolicyError> {
    ControlValue::new(
        ControlRecordClass::Operation, ABSENT.to_vec(), JournalLimits::BASELINE,
    ).map_err(Into::into)
}

fn optional_value(
    value: &ControlValue,
) -> Result<Option<ControlValue>, NativeGrantPolicyError> {
    if value.class() == ControlRecordClass::Operation && value.as_bytes() == ABSENT {
        Ok(None)
    } else {
        Ok(Some(value.clone()))
    }
}

fn value_digest(value: &ControlValue) -> [u8; 32] {
    let mut digest = blake3::Hasher::new();
    digest.update(b"ELIOT-STANDALONE-PROVISIONING-VALUE-v1\0");
    digest.update(&[class_tag(value.class())]);
    digest.update(&u64::try_from(value.len()).unwrap_or(u64::MAX).to_be_bytes());
    digest.update(value.as_bytes());
    *digest.finalize().as_bytes()
}

fn hash_key_value(
    digest: &mut blake3::Hasher,
    key: &ControlKey,
    value: &ControlValue,
) -> Result<(), NativeGrantPolicyError> {
    digest.update(
        &u64::try_from(key.as_bytes().len())
            .map_err(|_| NativeGrantPolicyError::InvalidRecord)?
            .to_be_bytes(),
    );
    digest.update(key.as_bytes());
    digest.update(&[class_tag(value.class())]);
    digest.update(
        &u64::try_from(value.len())
            .map_err(|_| NativeGrantPolicyError::InvalidRecord)?
            .to_be_bytes(),
    );
    digest.update(value.as_bytes());
    Ok(())
}

const fn class_tag(class: ControlRecordClass) -> u8 {
    match class {
        ControlRecordClass::Identity => 1,
        ControlRecordClass::Revision => 2,
        ControlRecordClass::State => 3,
        ControlRecordClass::Receipt => 4,
        ControlRecordClass::Operation => 5,
        ControlRecordClass::Snapshot => 6,
        ControlRecordClass::Migration => 7,
    }
}

fn same_rows(
    left: &[ControlValue; SLOT_COUNT],
    right: &[ControlValue; SLOT_COUNT],
) -> bool {
    left == right
}

struct Input<'a>(&'a [u8]);

impl<'a> Input<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8], NativeGrantPolicyError> {
        let value = self.0.get(..count).ok_or(NativeGrantPolicyError::InvalidRecord)?;
        self.0 = &self.0[count..];
        Ok(value)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], NativeGrantPolicyError> {
        self.take(N)?.try_into().map_err(|_| NativeGrantPolicyError::InvalidRecord)
    }

    fn byte(&mut self) -> Result<u8, NativeGrantPolicyError> {
        Ok(self.array::<1>()?[0])
    }

    fn u64(&mut self) -> Result<u64, NativeGrantPolicyError> {
        Ok(u64::from_be_bytes(self.array()?))
    }

    fn length(&mut self, maximum: usize) -> Result<usize, NativeGrantPolicyError> {
        let length = usize::try_from(u32::from_be_bytes(self.array()?))
            .map_err(|_| NativeGrantPolicyError::InvalidRecord)?;
        if length == 0 || length > maximum {
            return Err(NativeGrantPolicyError::InvalidRecord);
        }
        Ok(length)
    }

    fn text(&mut self, length: usize) -> Result<&'a str, NativeGrantPolicyError> {
        std::str::from_utf8(self.take(length)?)
            .map_err(|_| NativeGrantPolicyError::InvalidRecord)
    }

    const fn is_empty(&self) -> bool { self.0.is_empty() }
}

//! Atomically persist one standalone binding and its matching grant policy.

mod finalization;
mod intent;
mod readback;
mod provisioning;

pub use finalization::{
    StandalonePublicationError, StandalonePublicationReceipt,
    publish_committed_standalone_registration,
};
pub use intent::{StandaloneProvisioningIntent, StandaloneProvisioningIntentState};
pub use readback::StandaloneRegistrationReadback;
pub use provisioning::{
    StandaloneProvisioningError, StandaloneProvisioningPhase, StandaloneRegistrationProvisioning,
};

use search_contracts::{Blake3Digest32, protocol::PeerRole};
use search_control_redb::{
    ConditionalControlMutation, ControlCommitReceipt, ControlError, ControlKey,
    ControlRecordCondition, ControlSnapshotPublisher, ControlValue, JournalIdentity,
    MutationId, PersistentControlJournal,
    provider_authority::{ProviderAuthorityMutation, ProviderAuthorityRecord},
};
use search_ports::{CancellationProbe, OperationContext};

use super::{ProviderBindingRecord, ProviderBindingStatus, begin, check};
use super::opening::NativePairingCredentialIntent;
use super::super::{
    NativeGrantPolicyError, StandalonePolicyRecord, StandalonePolicyState,
};

/// One immutable binding/policy command, optionally bound to a durable intent head.
///
/// A crash cannot commit only the binding or policy. Their exact preconditions,
/// replacements and one operation receipt share the typed control transaction.
/// Provisioning additionally attaches one PREPARED→COMMITTED intent-header update
/// to the same command; payload rows remain immutable evidence. This is metadata
/// persistence, not credential provisioning, a live barrier or peer acknowledgement.
#[must_use = "retain the exact registration command until its outcome is resolved"]
pub struct StandaloneRegistrationMutation {
    authority: ProviderAuthorityMutation,
}

impl StandaloneRegistrationMutation {
    /// Prepare both absent rows or both exact existing rows; never upsert or
    /// repair a half-present registration by guessing its missing counterpart.
    ///
    /// Native administration supplies the records. Pair shape, transitions,
    /// durable keys, codecs and conditional writes are owned by
    /// `search-control-redb::ProviderAuthorityMutation`. The final digest is
    /// derived from that exact typed command using the previous domain separation,
    /// preserving existing provisioning intent/recovery identities without
    /// retaining a second binding/policy mutation implementation.
    ///
    /// # Errors
    /// Rejects inconsistent pairs, invalid transitions or bounded codec failures.
    pub fn new(
        identity: JournalIdentity,
        operation_id: MutationId,
        expected_generation: u64,
        expected: Option<(&ProviderBindingRecord, &StandalonePolicyRecord)>,
        binding: &ProviderBindingRecord,
        policy: &StandalonePolicyRecord,
    ) -> Result<Self, NativeGrantPolicyError> {
        validate_pair(binding, policy)?;
        if let Some((binding, policy)) = expected {
            validate_pair(binding, policy)?;
        }

        let expected_record = expected
            .map(|(binding, policy)| {
                ProviderAuthorityRecord::new(binding.clone(), policy.clone())
            })
            .transpose()
            .map_err(|_| NativeGrantPolicyError::InvalidRecord)?;
        let replacement = ProviderAuthorityRecord::new(binding.clone(), policy.clone())
            .map_err(|_| NativeGrantPolicyError::InvalidRecord)?;
        let provisional = ProviderAuthorityMutation::new(
            identity,
            operation_id,
            Blake3Digest32::from_bytes([0_u8; 32]),
            expected_generation,
            expected_record.as_ref(),
            replacement.clone(),
        )?;
        let command_digest = standalone_command_digest(provisional.command())?;
        let authority = ProviderAuthorityMutation::new(
            identity,
            operation_id,
            command_digest,
            expected_generation,
            expected_record.as_ref(),
            replacement,
        )?;
        Ok(Self { authority })
    }

    /// Exact non-secret intent persisted with the provider key.
    pub(super) fn credential_intent(&self) -> NativePairingCredentialIntent {
        let mutation = self.authority.command().mutation();
        NativePairingCredentialIntent::new(
            mutation.id().0,
            *mutation.command_digest().as_bytes(),
            mutation.expected_generation(),
        )
    }

    pub(super) const fn command(&self) -> &ConditionalControlMutation {
        self.authority.command()
    }

    /// Bind a durable PREPARED intent head to COMMITTED in the same final command.
    pub(super) fn with_intent_header(
        mut self,
        key: ControlKey,
        prepared: ControlValue,
        committed: ControlValue,
    ) -> Result<Self, NativeGrantPolicyError> {
        let base = self.authority.command().mutation();
        let mut hash = blake3::Hasher::new();
        hash.update(b"ELIOT-STANDALONE-REGISTRATION-WITH-INTENT-v1\0");
        hash.update(base.command_digest().as_bytes());
        hash_key_value(&mut hash, &key, &prepared)?;
        hash_key_value(&mut hash, &key, &committed)?;
        let command_digest = Blake3Digest32::from_bytes(*hash.finalize().as_bytes());
        self.authority = self.authority.with_completion_marker(
            command_digest,
            key,
            prepared,
            committed,
        )?;
        Ok(self)
    }

    /// Commit the exact command once through the typed authority owner.
    pub fn commit<C: CancellationProbe>(
        &self,
        journal: &mut PersistentControlJournal,
        context: &OperationContext<C>,
    ) -> Result<StandaloneRegistrationCommit<'_>, NativeGrantPolicyError> {
        let committed = journal.commit_provider_authority(&self.authority, context)?;
        Ok(StandaloneRegistrationCommit {
            registration: self,
            receipt: committed.receipt().clone(),
        })
    }

    /// Resolve this exact command without executing another write.
    pub fn recover<C: CancellationProbe>(
        &self,
        journal: &mut PersistentControlJournal,
        context: &OperationContext<C>,
    ) -> Result<Option<StandaloneRegistrationCommit<'_>>, NativeGrantPolicyError> {
        let committed = journal.recover_provider_authority(&self.authority, context)?;
        Ok(committed.map(|committed| StandaloneRegistrationCommit {
            registration: self,
            receipt: committed.receipt().clone(),
        }))
    }

    fn check_identity(
        &self,
        journal: &PersistentControlJournal,
    ) -> Result<(), NativeGrantPolicyError> {
        if journal.identity() != self.authority.identity() {
            return Err(ControlError::IdentityMismatch.into());
        }
        Ok(())
    }
}

/// Native transaction evidence bound to its exact registration command.
#[must_use = "a registration commit still requires barriers and current publication"]
pub struct StandaloneRegistrationCommit<'a> {
    registration: &'a StandaloneRegistrationMutation,
    receipt: ControlCommitReceipt,
}

impl StandaloneRegistrationCommit<'_> {
    /// Actual journal receipt for guarded snapshot publication.
    #[must_use]
    pub const fn receipt(&self) -> &ControlCommitReceipt { &self.receipt }

    /// Exact replacement binding carried by the typed authority command.
    pub fn replacement_binding(&self) -> Result<ProviderBindingRecord, NativeGrantPolicyError> {
        Ok(self.registration.authority.replacement().binding().clone())
    }

    /// Check every exact replacement against one current disk-published head.
    ///
    /// Ordinary registration has two writes. Provisioning has a third committed
    /// intent-header write. No historical receipt, partial subset or later missing
    /// evidence satisfies this check.
    pub fn confirm_published<C: CancellationProbe + Clone>(
        &self,
        journal: &PersistentControlJournal,
        publisher: &ControlSnapshotPublisher,
        context: &OperationContext<C>,
    ) -> Result<(), NativeGrantPolicyError> {
        let (started, deadline) = begin(context)?;
        self.registration.check_identity(journal)?;
        let read_context = readback::remaining_context(context, started, deadline)?;
        let writes = self.registration.authority.command().mutation().writes();
        let generation = match writes {
            [first, second] => {
                let (generation, values) = journal.read_published_records(
                    publisher,
                    [&first.key, &second.key],
                    &read_context,
                )?;
                if values[0].as_ref() != Some(&first.value)
                    || values[1].as_ref() != Some(&second.value)
                {
                    return Err(ControlError::TransactionConflict.into());
                }
                generation
            }
            [first, second, third] => {
                let (generation, values) = journal.read_published_records(
                    publisher,
                    [&first.key, &second.key, &third.key],
                    &read_context,
                )?;
                if values[0].as_ref() != Some(&first.value)
                    || values[1].as_ref() != Some(&second.value)
                    || values[2].as_ref() != Some(&third.value)
                {
                    return Err(ControlError::TransactionConflict.into());
                }
                generation
            }
            _ => return Err(NativeGrantPolicyError::InvalidRecord),
        };
        if generation < self.receipt.after_generation {
            return Err(ControlError::TransactionConflict.into());
        }
        check(context, started, deadline)?;
        Ok(())
    }
}

pub(super) fn validate_pair(
    binding: &ProviderBindingRecord,
    record: &StandalonePolicyRecord,
) -> Result<(), NativeGrantPolicyError> {
    binding
        .validate_shape()
        .map_err(|_| NativeGrantPolicyError::InvalidRecord)?;
    record
        .validate_shape()
        .map_err(|_| NativeGrantPolicyError::InvalidRecord)?;
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
        || binding.revocation_generation.get() != policy.revocation_generation
        || !lifecycle_matches
    {
        return Err(NativeGrantPolicyError::InvalidRecord);
    }
    Ok(())
}

fn standalone_command_digest(
    command: &ConditionalControlMutation,
) -> Result<Blake3Digest32, NativeGrantPolicyError> {
    let mutation = command.mutation();
    if mutation.writes().len() != 2 || command.conditions().len() != 2 {
        return Err(NativeGrantPolicyError::InvalidRecord);
    }
    let mut binding_digest = None;
    let mut policy_digest = None;
    for write in mutation.writes() {
        let condition = command
            .conditions()
            .iter()
            .find(|condition| condition.key() == &write.key)
            .ok_or(NativeGrantPolicyError::InvalidRecord)?;
        let (domain, slot) = match write.value.class() {
            search_control_redb::ControlRecordClass::Identity => (
                b"ELIOT-PROVIDER-BINDING-MUTATION-v1\0".as_slice(),
                &mut binding_digest,
            ),
            search_control_redb::ControlRecordClass::State => (
                b"ELIOT-STANDALONE-POLICY-MUTATION-v1\0".as_slice(),
                &mut policy_digest,
            ),
            _ => return Err(NativeGrantPolicyError::InvalidRecord),
        };
        if slot.is_some() {
            return Err(NativeGrantPolicyError::InvalidRecord);
        }
        *slot = Some(leaf_command_digest(
            domain,
            mutation.expected_generation(),
            &write.key,
            condition,
            &write.value,
        )?);
    }
    let binding_digest = binding_digest.ok_or(NativeGrantPolicyError::InvalidRecord)?;
    let policy_digest = policy_digest.ok_or(NativeGrantPolicyError::InvalidRecord)?;
    let mut hash = blake3::Hasher::new();
    hash.update(b"ELIOT-STANDALONE-REGISTRATION-v1\0");
    hash.update(&mutation.expected_generation().to_be_bytes());
    hash.update(binding_digest.as_bytes());
    hash.update(policy_digest.as_bytes());
    Ok(Blake3Digest32::from_bytes(*hash.finalize().as_bytes()))
}

fn leaf_command_digest(
    domain: &[u8],
    expected_generation: u64,
    key: &ControlKey,
    condition: &ControlRecordCondition,
    replacement: &ControlValue,
) -> Result<Blake3Digest32, NativeGrantPolicyError> {
    let mut hash = blake3::Hasher::new();
    hash.update(domain);
    hash.update(&expected_generation.to_be_bytes());
    hash.update(key.as_bytes());
    match condition.expected() {
        Some(prior) => {
            hash.update(&[1]);
            hash.update(
                &u64::try_from(prior.len())
                    .map_err(|_| NativeGrantPolicyError::InvalidRecord)?
                    .to_be_bytes(),
            );
            hash.update(prior.as_bytes());
        }
        None => {
            hash.update(&[0]);
        }
    }
    hash.update(replacement.as_bytes());
    Ok(Blake3Digest32::from_bytes(*hash.finalize().as_bytes()))
}

fn hash_key_value(
    hash: &mut blake3::Hasher,
    key: &ControlKey,
    value: &ControlValue,
) -> Result<(), NativeGrantPolicyError> {
    hash.update(
        &u64::try_from(key.as_bytes().len())
            .map_err(|_| NativeGrantPolicyError::InvalidRecord)?
            .to_be_bytes(),
    );
    hash.update(key.as_bytes());
    hash.update(&[record_class_tag(value.class())]);
    hash.update(
        &u64::try_from(value.len())
            .map_err(|_| NativeGrantPolicyError::InvalidRecord)?
            .to_be_bytes(),
    );
    hash.update(value.as_bytes());
    Ok(())
}

const fn record_class_tag(class: search_control_redb::ControlRecordClass) -> u8 {
    match class {
        search_control_redb::ControlRecordClass::Identity => 1,
        search_control_redb::ControlRecordClass::Revision => 2,
        search_control_redb::ControlRecordClass::State => 3,
        search_control_redb::ControlRecordClass::Receipt => 4,
        search_control_redb::ControlRecordClass::Operation => 5,
        search_control_redb::ControlRecordClass::Snapshot => 6,
        search_control_redb::ControlRecordClass::Migration => 7,
    }
}

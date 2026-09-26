//! Atomically persist one standalone binding and its matching grant policy.

mod intent;
mod readback;
mod provisioning;

pub use intent::{StandaloneProvisioningIntent, StandaloneProvisioningIntentState};
pub use readback::StandaloneRegistrationReadback;
pub use provisioning::{
    StandaloneProvisioningError, StandaloneProvisioningPhase, StandaloneRegistrationProvisioning,
};

use search_contracts::{Blake3Digest32, protocol::PeerRole};
use search_control_redb::{
    CommitRecoveryDecision, ConditionalControlMutation, ControlCommitReceipt, ControlError,
    ControlKey, ControlMutation, ControlRecordCondition, ControlSnapshotPublisher, ControlValue,
    ControlWrite, JournalIdentity, MutationId, PersistentControlJournal,
};
use search_ports::{CancellationProbe, OperationContext};

use super::{
    ProviderBindingMutation, ProviderBindingRecord, ProviderBindingStatus, begin, check,
};
use super::opening::NativePairingCredentialIntent;
use super::super::{
    NativeGrantPolicyError, StandalonePolicyMutation, StandalonePolicyRecord, StandalonePolicyState,
};

/// One immutable binding/policy command, optionally bound to a durable intent head.
///
/// A crash cannot commit only the binding or policy. Their exact preconditions,
/// replacements and one operation receipt share the existing journal transaction.
/// Provisioning additionally attaches one PREPARED→COMMITTED intent-header update
/// to the same command; payload rows remain immutable evidence. This is metadata
/// persistence, not credential provisioning, a live barrier or peer acknowledgement.
#[must_use = "retain the exact registration command until its outcome is resolved"]
pub struct StandaloneRegistrationMutation {
    identity: JournalIdentity,
    command: ConditionalControlMutation,
}

impl StandaloneRegistrationMutation {
    /// Prepare both absent rows or both exact existing rows; never upsert or
    /// repair a half-present registration by guessing its missing counterpart.
    ///
    /// Native administration supplies the records. Existing binding/policy
    /// constructors retain schema, expiry and transition checks. Pair consistency
    /// requires standalone role, equal installation/incarnation/binding/pairing
    /// generation and matching lifecycle states.
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
        let binding = ProviderBindingMutation::new(
            identity, operation_id, expected_generation, expected.map(|pair| pair.0), binding,
        )?;
        let policy = StandalonePolicyMutation::new(
            identity, operation_id, expected_generation, expected.map(|pair| pair.1), policy,
        )?;
        let parts = [binding.command(), policy.command()];
        let mut writes = Vec::with_capacity(2);
        let mut conditions = Vec::with_capacity(2);
        let mut hash = blake3::Hasher::new();
        hash.update(b"ELIOT-STANDALONE-REGISTRATION-v1\0");
        hash.update(&expected_generation.to_be_bytes());
        for part in parts {
            let [write] = part.mutation().writes() else {
                return Err(NativeGrantPolicyError::InvalidRecord);
            };
            let [condition] = part.conditions() else {
                return Err(NativeGrantPolicyError::InvalidRecord);
            };
            hash.update(part.mutation().command_digest().as_bytes());
            writes.push(write.clone());
            conditions.push(condition.clone());
        }
        writes.sort_unstable_by(|left, right| left.key.cmp(&right.key));
        conditions.sort_unstable_by(|left, right| left.key().cmp(right.key()));
        if writes[0].key == writes[1].key {
            return Err(ControlError::DuplicateMutationKey.into());
        }
        let mutation = ControlMutation::new(
            operation_id,
            Blake3Digest32::from_bytes(*hash.finalize().as_bytes()),
            expected_generation,
            writes,
            Vec::new(),
        );
        Ok(Self {
            identity,
            command: ConditionalControlMutation::new(mutation, conditions),
        })
    }

    /// Exact non-secret intent persisted with the provider key.
    pub(super) fn credential_intent(&self) -> NativePairingCredentialIntent {
        let mutation = self.command.mutation();
        NativePairingCredentialIntent::new(
            mutation.id().0,
            *mutation.command_digest().as_bytes(),
            mutation.expected_generation(),
        )
    }

    pub(super) const fn command(&self) -> &ConditionalControlMutation {
        &self.command
    }

    /// Bind a durable PREPARED intent head to COMMITTED in the same final command.
    pub(super) fn with_intent_header(
        mut self,
        key: ControlKey,
        prepared: ControlValue,
        committed: ControlValue,
    ) -> Result<Self, NativeGrantPolicyError> {
        if prepared == committed
            || self.command.mutation().writes().iter().any(|write| write.key == key)
            || self.command.conditions().iter().any(|condition| condition.key() == &key)
        {
            return Err(NativeGrantPolicyError::InvalidRecord);
        }
        let base = self.command.mutation();
        let mut hash = blake3::Hasher::new();
        hash.update(b"ELIOT-STANDALONE-REGISTRATION-WITH-INTENT-v1\0");
        hash.update(base.command_digest().as_bytes());
        hash_key_value(&mut hash, &key, &prepared)?;
        hash_key_value(&mut hash, &key, &committed)?;

        let mut writes = base.writes().to_vec();
        writes.push(ControlWrite { key: key.clone(), value: committed });
        writes.sort_unstable_by(|left, right| left.key.cmp(&right.key));
        let mut conditions = self.command.conditions().to_vec();
        conditions.push(ControlRecordCondition::exact(key, prepared));
        conditions.sort_unstable_by(|left, right| left.key().cmp(right.key()));
        let mutation = ControlMutation::new(
            base.id(),
            Blake3Digest32::from_bytes(*hash.finalize().as_bytes()),
            base.expected_generation(),
            writes,
            Vec::new(),
        );
        self.command = ConditionalControlMutation::new(mutation, conditions);
        Ok(self)
    }

    /// Commit the exact command once through the existing conditional engine.
    pub fn commit<C: CancellationProbe>(
        &self,
        journal: &mut PersistentControlJournal,
        context: &OperationContext<C>,
    ) -> Result<StandaloneRegistrationCommit<'_>, NativeGrantPolicyError> {
        self.check_identity(journal)?;
        let receipt = journal.transact_conditionally(self.command.clone(), context)?;
        Ok(StandaloneRegistrationCommit { registration: self, receipt })
    }

    /// Resolve this exact command without executing another write.
    pub fn recover<C: CancellationProbe>(
        &self,
        journal: &mut PersistentControlJournal,
        context: &OperationContext<C>,
    ) -> Result<Option<StandaloneRegistrationCommit<'_>>, NativeGrantPolicyError> {
        self.check_identity(journal)?;
        match journal.recover_conditional_transaction(&self.command, context)? {
            CommitRecoveryDecision::Committed(receipt) => {
                Ok(Some(StandaloneRegistrationCommit { registration: self, receipt }))
            }
            CommitRecoveryDecision::NotCommittedRetrySameOperation => Ok(None),
            CommitRecoveryDecision::ConflictingInput => {
                Err(ControlError::OperationConflict.into())
            }
            CommitRecoveryDecision::PartialOrCorruptQuarantine => {
                Err(ControlError::StoreQuarantined.into())
            }
        }
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
        let writes = self.registration.command.mutation().writes();
        let generation = match writes {
            [first, second] => {
                let (generation, values) = journal.read_published_records(
                    publisher, [&first.key, &second.key], &read_context,
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
                    publisher, [&first.key, &second.key, &third.key], &read_context,
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
        return Err(NativeGrantPolicyError::InvalidRecord);
    }
    Ok(())
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

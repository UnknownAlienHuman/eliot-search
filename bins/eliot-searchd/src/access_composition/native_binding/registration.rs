//! Atomically persist one standalone binding and its matching grant policy.

mod readback;
pub use readback::StandaloneRegistrationReadback;

use search_contracts::{Blake3Digest32, protocol::PeerRole};
use search_control_redb::{
    CommitRecoveryDecision, ConditionalControlMutation, ControlCommitReceipt, ControlError,
    ControlMutation, ControlSnapshotPublisher, JournalIdentity, MutationId, PersistentControlJournal,
};
use search_ports::{CancellationProbe, OperationContext};

use super::{
    ProviderBindingMutation, ProviderBindingRecord, ProviderBindingStatus, begin, check,
};
use super::super::{
    NativeGrantPolicyError, StandalonePolicyMutation, StandalonePolicyRecord, StandalonePolicyState,
};

/// One immutable two-row command for registration, rotation or joint revocation.
///
/// A crash cannot commit only the binding or only its policy. Both exact
/// preconditions, both replacements and one operation receipt share the existing
/// journal transaction. This is metadata persistence, not credential provisioning,
/// a live restriction barrier, publication or a successful pairing reply.
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
    /// constructors retain ownership of schema, expiry and transition checks.
    /// Pair consistency additionally requires standalone role, equal installation,
    /// incarnation, binding and pairing generation, and matching lifecycle states.
    /// Binding and grant revocation revisions remain distinct namespaces.
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
            // These are the two already validated single-row commands. Never
            // call their commit methods or reuse either partial receipt.
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
            operation_id, Blake3Digest32::from_bytes(*hash.finalize().as_bytes()),
            expected_generation, writes, Vec::new(),
        );
        Ok(Self {
            identity,
            command: ConditionalControlMutation::new(mutation, conditions),
        })
    }

    /// Commit the pair once through the existing conditional transaction engine.
    /// The borrowed receipt cannot be attached to another registration descriptor.
    ///
    /// The root owner must coordinate restrictive changes and dependent cleanup
    /// under its real mutation lock. No snapshot is published here. A successful
    /// disk commit alone does not allow acknowledgement or source access.
    ///
    /// # Errors
    /// Preserves native identity, conflict, interruption and possible-commit errors.
    /// Keep this command unchanged for recovery; never retry its halves separately.
    pub fn commit<C: CancellationProbe>(
        &self,
        journal: &mut PersistentControlJournal,
        context: &OperationContext<C>,
    ) -> Result<StandaloneRegistrationCommit<'_>, NativeGrantPolicyError> {
        self.check_identity(journal)?;
        let receipt = journal.transact_conditionally(self.command.clone(), context)?;
        Ok(StandaloneRegistrationCommit { registration: self, receipt })
    }

    /// Resolve this exact two-row operation without executing another write.
    /// None means resolved absence; it is not partial success or automatic retry.
    /// A committed result remains historical until current publication is checked.
    ///
    /// # Errors
    /// Retains native recovery failures; conflict/corruption never becomes absence.
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
            CommitRecoveryDecision::ConflictingInput => Err(ControlError::OperationConflict.into()),
            CommitRecoveryDecision::PartialOrCorruptQuarantine => Err(ControlError::StoreQuarantined.into()),
        }
    }

    fn check_identity(&self, journal: &PersistentControlJournal) -> Result<(), NativeGrantPolicyError> {
        if journal.identity() != self.identity {
            return Err(ControlError::IdentityMismatch.into());
        }
        Ok(())
    }
}

/// Native transaction evidence bound by borrow to its exact registration command.
/// Only successful native commit/recovery constructs it. It grants no authority
/// and owns no second copy of the records, journal, publisher or operation ledger.
#[must_use = "a registration commit still requires barriers and current publication"]
pub struct StandaloneRegistrationCommit<'a> {
    registration: &'a StandaloneRegistrationMutation,
    receipt: ControlCommitReceipt,
}

impl StandaloneRegistrationCommit<'_> {
    /// Actual journal receipt for the existing guarded publication API.
    /// The native owner must finish required barriers/dependents first. A later
    /// journal commit may require publication recovery rather than this old receipt.
    #[must_use]
    pub const fn receipt(&self) -> &ControlCommitReceipt { &self.receipt }

    /// Check both exact record classes/bytes against the current disk-published
    /// head before acknowledging registration. A changed, missing or unpublished
    /// counterpart fails; retaining a historical commit never bypasses this check.
    /// Unrelated later commits are allowed only while both records remain exact.
    ///
    /// Hold the actual root/policy lock across publication, this check and any
    /// acknowledgement. Shared borrows exclude journal/publisher mutation during
    /// both lookups, which share one native transaction and one decreasing budget.
    /// This proves metadata correspondence, not barrier completion, live lifetime,
    /// credential validity or source authorization; open_published rechecks those
    /// registration inputs and the serving owners must still validate live access.
    ///
    /// # Errors
    /// Preserves read failures and rejects stale publication, record mismatch,
    /// cancellation, clock regression and exhaustion of the original call budget.
    pub fn confirm_published<C: CancellationProbe + Clone>(
        &self,
        journal: &PersistentControlJournal,
        publisher: &ControlSnapshotPublisher,
        context: &OperationContext<C>,
    ) -> Result<(), NativeGrantPolicyError> {
        let (started, deadline) = begin(context)?;
        self.registration.check_identity(journal)?;
        let [first, second] = self.registration.command.mutation().writes() else {
            return Err(NativeGrantPolicyError::InvalidRecord);
        };
        let read_context = readback::remaining_context(context, started, deadline)?;
        let (generation, values) = journal.read_published_record_pair(
            publisher, [&first.key, &second.key], &read_context,
        )?;
        if generation < self.receipt.after_generation
            || values[0].as_ref() != Some(&first.value)
            || values[1].as_ref() != Some(&second.value)
        {
            return Err(ControlError::TransactionConflict.into());
        }
        check(context, started, deadline)?;
        Ok(())
    }
}

fn validate_pair(
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

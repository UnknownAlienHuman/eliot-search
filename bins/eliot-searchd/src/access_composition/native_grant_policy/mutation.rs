//! Explicit conditional policy persistence on the existing operation ledger.

use search_contracts::Blake3Digest32;
use search_control_redb::{CommitRecoveryDecision, ConditionalControlMutation, ControlCommitReceipt,
    ControlError, ControlMutation, ControlRecordCondition, ControlWrite, JournalIdentity,
    MutationId, PersistentControlJournal};
use search_ports::{CancellationProbe, OperationContext};

use super::{NativeGrantPolicyError, StandalonePolicyRecord, StandalonePolicyState, codec, policy_key};

/// Immutable administrative mutation retained unchanged through recovery. It
/// contains only typed policy metadata, never secrets or a searchable corpus.
/// The existing journal owns atomicity, idempotency and unknown-outcome recovery.
pub struct StandalonePolicyMutation {
    identity: JournalIdentity,
    command: ConditionalControlMutation,
}

impl StandalonePolicyMutation {
    /// Prepare explicit initialization or exact-prestate replacement. There is
    /// no upsert and no reactivation of a terminal row. The administrative owner
    /// supplies policy, time and mutation identity, never a client recipe.
    ///
    /// Policy updates must be coordinated with the native restriction/dependent
    /// barrier under the same root lock. This descriptor does not publish policy,
    /// mint grants, authenticate pairing or manufacture a barrier receipt.
    ///
    /// # Errors
    /// Rejects invalid metadata, identity changes or nonmonotone replacements.
    pub fn new(
        identity: JournalIdentity,
        operation_id: MutationId,
        expected_generation: u64,
        expected: Option<&StandalonePolicyRecord>,
        replacement: &StandalonePolicyRecord,
    ) -> Result<Self, NativeGrantPolicyError> {
        identity.validate()?;
        replacement.validate()?;
        if identity.installation_incarnation_id != replacement.policy.installation_incarnation_id {
            return Err(ControlError::IdentityMismatch.into());
        }
        let key = policy_key(replacement.policy.installation_incarnation_id, replacement.policy.binding_id)?;
        let value = codec::encode(replacement)?;
        let mut hash = blake3::Hasher::new();
        hash.update(b"ELIOT-STANDALONE-POLICY-MUTATION-v1\0");
        hash.update(&expected_generation.to_be_bytes());
        hash.update(key.as_bytes());
        let condition = if let Some(before) = expected {
            before.validate()?;
            validate_transition(before, replacement)?;
            let prior = codec::encode(before)?;
            hash.update(&[1]);
            hash.update(&u64::try_from(prior.len()).map_err(|_| NativeGrantPolicyError::InvalidRecord)?.to_be_bytes());
            hash.update(prior.as_bytes());
            ControlRecordCondition::exact(key.clone(), prior)
        } else {
            if replacement.state != StandalonePolicyState::Active {
                return Err(NativeGrantPolicyError::InvalidRecord);
            }
            hash.update(&[0]);
            ControlRecordCondition::absent(key.clone())
        };
        hash.update(value.as_bytes());
        let digest = Blake3Digest32::from_bytes(*hash.finalize().as_bytes());
        let mutation = ControlMutation::new(operation_id, digest, expected_generation,
            vec![ControlWrite { key, value }], Vec::new());
        Ok(Self { identity, command: ConditionalControlMutation::new(mutation, vec![condition]) })
    }

    /// Commit without publishing. The returned receipt is historical evidence,
    /// not current policy. Publish only through the journal's guarded native
    /// publisher after every required access-barrier/dependent operation succeeds.
    /// Errors preserve possible commits; do not retry with a new operation ID.
    ///
    /// # Errors
    /// Preserves the journal identity, conflict, capacity and unknown-outcome error.
    pub fn commit<C: CancellationProbe>(
        &self,
        journal: &mut PersistentControlJournal,
        context: &OperationContext<C>,
    ) -> Result<ControlCommitReceipt, NativeGrantPolicyError> {
        self.check_identity(journal)?;
        journal.transact_conditionally(self.command.clone(), context).map_err(Into::into)
    }

    /// Recover this exact conditional transaction without dispatching another
    /// write. Committed does not prove that the row is still the current head.
    ///
    /// # Errors
    /// Preserves identity, interruption, corruption and recovery failures.
    pub fn recover<C: CancellationProbe>(
        &self,
        journal: &mut PersistentControlJournal,
        context: &OperationContext<C>,
    ) -> Result<CommitRecoveryDecision, NativeGrantPolicyError> {
        self.check_identity(journal)?;
        journal.recover_conditional_transaction(&self.command, context).map_err(Into::into)
    }

    fn check_identity(&self, journal: &PersistentControlJournal) -> Result<(), NativeGrantPolicyError> {
        if journal.identity() != self.identity { return Err(ControlError::IdentityMismatch.into()); }
        Ok(())
    }
}

fn validate_transition(before: &StandalonePolicyRecord, after: &StandalonePolicyRecord) -> Result<(), NativeGrantPolicyError> {
    let old = &before.policy;
    let new = &after.policy;
    if before.state != StandalonePolicyState::Active
        || old.binding_id != new.binding_id || old.installation_id != new.installation_id
        || old.installation_incarnation_id != new.installation_incarnation_id
        || old.principal_opaque_id != new.principal_opaque_id || old.client_scope_ref != new.client_scope_ref
        || old.scope_domain_id != new.scope_domain_id || before.issued_at != after.issued_at
        || old.policy_generation.checked_add(1) != Some(new.policy_generation)
        || new.binding_generation < old.binding_generation
        || new.revocation_generation < old.revocation_generation
        || (after.state != StandalonePolicyState::Active && new.revocation_generation == old.revocation_generation)
        || before.expires_at.as_ref().is_some_and(|old_end| after.expires_at.as_ref().is_none_or(|end| end > old_end))
    {
        return Err(NativeGrantPolicyError::InvalidRecord);
    }
    Ok(())
}

//! Conditional binding registration, replacement and exact recovery.

use search_contracts::Blake3Digest32;
use search_control_redb::{CommitRecoveryDecision, ConditionalControlMutation, ControlCommitReceipt,
    ControlError, ControlMutation, ControlRecordCondition, ControlWrite, JournalIdentity,
    MutationId, PersistentControlJournal};
use search_ports::{CancellationProbe, OperationContext};

use super::{NativeBindingError, ProviderBindingRecord, ProviderBindingStatus, binding_key, codec};

/// One immutable native administrative command, retained for possible-write recovery.
/// This descriptor never publishes a binding or issues a successful pairing reply.
/// The existing journal owns persistence/idempotency; no second table or ledger.
pub struct ProviderBindingMutation {
    identity: JournalIdentity,
    command: ConditionalControlMutation,
}

impl ProviderBindingMutation {
    /// Build an explicit absent-row registration or exact-prestate replacement.
    /// Native administration supplies verified peer metadata, not a recipe body.
    /// Every replacement advances BOTH pairing and binding-revocation revisions:
    /// existing sessions must reconnect rather than inherit changed permissions.
    /// Terminal rows cannot be reactivated, reused or silently deleted.
    ///
    /// The native owner must fence live grants/work and complete all required
    /// dependent invalidation before guarded publication or acknowledgement.
    /// This metadata transaction alone cannot prove those effects happened.
    pub fn new(
        identity: JournalIdentity,
        operation_id: MutationId,
        expected_generation: u64,
        expected: Option<&ProviderBindingRecord>,
        replacement: &ProviderBindingRecord,
    ) -> Result<Self, NativeBindingError> {
        identity.validate()?;
        replacement.validate()?;
        if identity.installation_incarnation_id != replacement.installation_incarnation_id {
            return Err(ControlError::IdentityMismatch.into());
        }
        let key = binding_key(replacement.installation_incarnation_id, replacement.binding_id)?;
        let value = codec::encode(replacement)?;
        let mut digest = blake3::Hasher::new();
        digest.update(b"ELIOT-PROVIDER-BINDING-MUTATION-v1\0");
        digest.update(&expected_generation.to_be_bytes());
        digest.update(key.as_bytes());
        let condition = if let Some(before) = expected {
            before.validate()?;
            validate_transition(before, replacement)?;
            let prior = codec::encode(before)?;
            digest.update(&[1]);
            digest.update(&u64::try_from(prior.len()).map_err(|_| NativeBindingError::InvalidRecord)?.to_be_bytes());
            digest.update(prior.as_bytes());
            ControlRecordCondition::exact(key.clone(), prior)
        } else {
            if replacement.status != ProviderBindingStatus::Active { return Err(NativeBindingError::InvalidRecord); }
            digest.update(&[0]);
            ControlRecordCondition::absent(key.clone())
        };
        digest.update(value.as_bytes());
        let command = ControlMutation::new(operation_id,
            Blake3Digest32::from_bytes(*digest.finalize().as_bytes()), expected_generation,
            vec![ControlWrite { key, value }], Vec::new());
        Ok(Self { identity, command: ConditionalControlMutation::new(command, vec![condition]) })
    }

    /// Commit the exact conditional command. A receipt is historical evidence,
    /// NOT current registration: use native barrier/dependent work and guarded
    /// snapshot publication before acknowledgement. Preserve uncertain outcomes.
    pub fn commit<C: CancellationProbe>(
        &self,
        journal: &mut PersistentControlJournal,
        context: &OperationContext<C>,
    ) -> Result<ControlCommitReceipt, NativeBindingError> {
        self.check_identity(journal)?;
        journal.transact_conditionally(self.command.clone(), context).map_err(Into::into)
    }

    /// Resolve the same immutable command without retrying a write or changing
    /// its operation identity, conditions or expected generation. Committed does
    /// not assert that this record is still current after later replacements.
    pub fn recover<C: CancellationProbe>(
        &self,
        journal: &mut PersistentControlJournal,
        context: &OperationContext<C>,
    ) -> Result<CommitRecoveryDecision, NativeBindingError> {
        self.check_identity(journal)?;
        journal.recover_conditional_transaction(&self.command, context).map_err(Into::into)
    }

    fn check_identity(&self, journal: &PersistentControlJournal) -> Result<(), NativeBindingError> {
        if self.identity != journal.identity() { return Err(ControlError::IdentityMismatch.into()); }
        Ok(())
    }
}

fn validate_transition(before: &ProviderBindingRecord, after: &ProviderBindingRecord) -> Result<(), NativeBindingError> {
    if before.status != ProviderBindingStatus::Active
        || before.binding_id != after.binding_id || before.installation_id != after.installation_id
        || before.installation_incarnation_id != after.installation_incarnation_id
        || before.peer_role != after.peer_role || before.peer_identity_digest != after.peer_identity_digest
        || before.issued_at != after.issued_at
        || before.pairing_generation.checked_next().ok() != Some(after.pairing_generation)
        || before.revocation_generation.checked_next().ok() != Some(after.revocation_generation)
        || before.expires_at.as_ref().is_some_and(|old| after.expires_at.as_ref().is_none_or(|new| new > old))
    {
        return Err(NativeBindingError::InvalidRecord);
    }
    Ok(())
}

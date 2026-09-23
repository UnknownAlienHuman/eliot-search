//! Real native effect implementation used by NativeSecurityDomain.

use search_access::{
    AccessError, DurableSecurityRestriction, MAX_SECURITY_DEPENDENTS,
    SecurityDependentReceipt, SecurityMutationEffects, SecurityRestriction,
};
use search_contracts::{BoundedList, LiveDenySnapshotRef};
use search_control_redb::{
    ControlError, ControlSnapshotPublisher, PersistentControlJournal,
    security_restriction::SecurityRestrictionCommit,
};
use search_ports::CancellationProbe;

use super::{
    NativeSecurityBinding, NativeSecurityError, PendingNative, SecurityInvalidationSink,
    budget::MutationBudget, mapping,
};

pub(super) struct JournalEffects<'a, 'context, C: CancellationProbe, D> {
    binding: &'a NativeSecurityBinding,
    head: &'a SecurityRestrictionCommit,
    pending: &'a mut PendingNative,
    journal: &'a mut PersistentControlJournal,
    publisher: &'a mut ControlSnapshotPublisher,
    dependents: &'a mut D,
    budget: &'a MutationBudget<'context, C>,
    published: Option<LiveDenySnapshotRef>,
    pub(super) failure: Option<NativeSecurityError>,
}

impl<'a, 'context, C, D> JournalEffects<'a, 'context, C, D>
where
    C: CancellationProbe + Clone,
    D: SecurityInvalidationSink<C>,
{
    pub(super) fn new(
        binding: &'a NativeSecurityBinding,
        head: &'a SecurityRestrictionCommit,
        pending: &'a mut PendingNative,
        journal: &'a mut PersistentControlJournal,
        publisher: &'a mut ControlSnapshotPublisher,
        dependents: &'a mut D,
        budget: &'a MutationBudget<'context, C>,
    ) -> Self {
        Self { binding, head, pending, journal, publisher, dependents, budget, published: None, failure: None }
    }

    fn prepare(&mut self, command: &SecurityRestriction) -> Result<(), NativeSecurityError> {
        mapping::validate_target(self.binding, command, &self.pending.policy)?;
        if let Some(native) = &self.pending.descriptor {
            // Do not refresh the descriptor's original native generation after
            // failure. Extra policy metadata is part of exact retry identity.
            if native.replacement().policy != self.pending.policy || mapping::command(native)? != *command {
                return Err(AccessError::SecurityOperationConflict.into());
            }
            return Ok(());
        }
        let read = self.journal.read_security_restriction(self.binding.namespace, &self.budget.context()?)?;
        read.confirm_current(self.head)?;
        let previous = self.head.mutation().replacement();
        if mapping::live(previous) != command.expected_live
            || previous.policy.policy_revision != command.expected_policy_revision
        {
            return Err(AccessError::SecurityFenceStale.into());
        }
        mapping::validate_metadata(&previous.policy, &self.pending.policy)?;
        let replacement = mapping::replacement(command, self.pending.policy)?;
        let native = read.prepare_restriction(
            self.head, command.operation_id.clone(),
            mapping::command_digest(command, &previous.policy, &self.pending.policy),
            replacement, command.required_dependents.clone(),
        )?;
        // Both values are installed before the journal can perform any write.
        self.pending.expected_generation = Some(read.generation());
        self.pending.descriptor = Some(native);
        Ok(())
    }

    fn accept_commit(
        &mut self,
        command: &SecurityRestriction,
        committed: SecurityRestrictionCommit,
    ) -> Result<DurableSecurityRestriction, NativeSecurityError> {
        if self.pending.descriptor.as_ref() != Some(committed.mutation()) {
            return Err(AccessError::SecurityOperationConflict.into());
        }
        if let Some(known) = &self.pending.committed {
            if mapping::receipt_ref(known)? != mapping::receipt_ref(&committed)? {
                return Err(AccessError::SecurityOperationConflict.into());
            }
        }
        // Retain a known native commit before any further read/check can fail.
        self.pending.committed = Some(committed);
        let committed = self.pending.committed.as_ref().expect("retained native commit");
        let read = self.journal.read_security_restriction(self.binding.namespace, &self.budget.context()?)?;
        read.confirm_current(committed)?;
        self.budget.check()?;
        Ok(DurableSecurityRestriction {
            command: command.clone(), receipt_ref: mapping::receipt_ref(committed)?,
        })
    }

    fn commit(&mut self, command: &SecurityRestriction) -> Result<DurableSecurityRestriction, NativeSecurityError> {
        self.prepare(command)?;
        let native = self.pending.descriptor.as_ref().expect("prepared native request");
        let committed = self.journal.commit_security_restriction(native, &self.budget.context()?)?;
        self.accept_commit(command, committed)
    }

    fn readback(&mut self, command: &SecurityRestriction) -> Result<Option<DurableSecurityRestriction>, NativeSecurityError> {
        // If a prior attempt failed before native preparation, it could not
        // dispatch a write. Capture once, then still consult the real ledger.
        self.prepare(command)?;
        let native = self.pending.descriptor.as_ref().expect("prepared native request");
        match self.journal.recover_security_restriction(native, &self.budget.context()?)? {
            Some(committed) => self.accept_commit(command, committed).map(Some),
            None => {
                if self.pending.committed.is_some() { return Err(AccessError::SecurityFailClosed.into()); }
                let read = self.journal.read_security_restriction(self.binding.namespace, &self.budget.context()?)?;
                read.confirm_current(self.head)?;
                if self.pending.expected_generation != Some(read.generation()) {
                    return Err(AccessError::SecurityFenceStale.into());
                }
                self.budget.check()?;
                // Proven absence AND the exact unchanged expected head. The
                // canonical coordinator may now retry only this descriptor.
                Ok(None)
            }
        }
    }

    fn require_commit(&self, requested: &DurableSecurityRestriction) -> Result<&SecurityRestrictionCommit, NativeSecurityError> {
        let native = self.pending.committed.as_ref().ok_or(AccessError::SecurityFailClosed)?;
        if mapping::command(native.mutation())? != requested.command
            || mapping::receipt_ref(native)? != requested.receipt_ref
        {
            return Err(AccessError::SecurityOperationConflict.into());
        }
        Ok(native)
    }

    fn publish(&mut self, requested: &DurableSecurityRestriction) -> Result<LiveDenySnapshotRef, NativeSecurityError> {
        self.require_commit(requested)?;
        let native = self.pending.committed.as_ref().expect("verified pending commit");
        let published = publish_current(self.journal, self.publisher, native, self.budget)?;
        self.published = Some(published.clone());
        Ok(published)
    }

    fn invalidate(&mut self, requested: &DurableSecurityRestriction) -> Result<BoundedList<SecurityDependentReceipt, MAX_SECURITY_DEPENDENTS>, NativeSecurityError> {
        self.require_commit(requested)?;
        let native = self.pending.committed.as_ref().expect("verified pending commit");
        let published = self.published.as_ref().ok_or(AccessError::SecurityFailClosed)?;
        let received = self.dependents.invalidate(
            native, &mapping::receipt_ref(native)?, published, &self.budget.context()?,
        )?;
        mapping::validate_receipts(native, published, &received)?;
        self.budget.check()?;
        Ok(received)
    }

    fn retain_error<T>(&mut self, result: Result<T, NativeSecurityError>) -> Result<T, NativeSecurityError> {
        if let Err(error) = &result { self.failure = Some(*error); }
        result
    }
}

impl<C, D> SecurityMutationEffects for JournalEffects<'_, '_, C, D>
where
    C: CancellationProbe + Clone,
    D: SecurityInvalidationSink<C>,
{
    type Error = NativeSecurityError;

    fn commit_restriction(&mut self, command: &SecurityRestriction) -> Result<DurableSecurityRestriction, Self::Error> {
        let result = self.commit(command);
        self.retain_error(result)
    }

    fn readback_restriction(&mut self, command: &SecurityRestriction) -> Result<Option<DurableSecurityRestriction>, Self::Error> {
        let result = self.readback(command);
        self.retain_error(result)
    }

    fn publish_live_restriction(&mut self, committed: &DurableSecurityRestriction) -> Result<LiveDenySnapshotRef, Self::Error> {
        let result = self.publish(committed);
        self.retain_error(result)
    }

    fn invalidate_dependents(&mut self, committed: &DurableSecurityRestriction) -> Result<BoundedList<SecurityDependentReceipt, MAX_SECURITY_DEPENDENTS>, Self::Error> {
        let result = self.invalidate(committed);
        self.retain_error(result)
    }
}

pub(super) fn publish_current<C: CancellationProbe + Clone>(
    journal: &PersistentControlJournal,
    publisher: &mut ControlSnapshotPublisher,
    committed: &SecurityRestrictionCommit,
    budget: &MutationBudget<'_, C>,
) -> Result<LiveDenySnapshotRef, NativeSecurityError> {
    let namespace = committed.mutation().replacement().policy.namespace_id;
    let current = journal.read_security_restriction(namespace, &budget.context()?)?;
    current.confirm_current(committed)?;
    // The restriction can remain current after unrelated journal writes. Do
    // not present its historical receipt as the global head or replay it just
    // to acquire a new generation. The journal verifies and publishes its actual
    // latest native snapshot, retaining its own rollback and corruption fences.
    journal.recover_snapshot_publication_with_context(publisher, &budget.context()?)?
        .ok_or(ControlError::SnapshotPublicationFailed)?;
    let published = publisher.current().ok_or(ControlError::SnapshotPublicationFailed)?;
    if published.identity != committed.mutation().identity() || published.generation != current.generation() {
        return Err(ControlError::SnapshotPublicationFailed.into());
    }
    budget.check()?;
    Ok(mapping::snapshot_ref(committed))
}

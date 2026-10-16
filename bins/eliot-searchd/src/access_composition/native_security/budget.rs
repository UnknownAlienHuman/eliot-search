//! One cooperative deadline across native reads, commit, publication and owners.

use std::time::{Duration, Instant};
use std::cell::Cell;
use search_control_redb::{ControlError, ControlSnapshotPublisher, MigrationBootstrapFailure,
    MigrationBootstrapWork as Work, PersistentControlJournal,
    security_restriction::{SecurityPolicyState, SecurityRestrictionCommit, SecurityRestrictionMutation, SecurityRestrictionReadback}};
use search_contracts::{BoundedList, LiveDenySnapshotRef, ReceiptRef, SourceNamespaceId};
use search_access::{SecurityDependentReceipt, MAX_SECURITY_DEPENDENTS, SecurityRestriction};
use search_runtime_owner::OwnerError;
use search_ports::{CancellationProbe, OperationContext};
use super::{NativeSecurityError, SecurityInvalidationSink};

type OriginalWork<'a> = &'a dyn Fn(Work) -> Result<(), OwnerError>;

pub(super) struct MutationBudget<'a, C: CancellationProbe> {
    original: &'a OperationContext<C>,
    deadline: Instant,
    work: Option<&'a dyn Fn(Work) -> Result<(), OwnerError>>,
    first: Cell<Option<NativeSecurityError>>,
}

impl<'a, C: CancellationProbe + Clone> MutationBudget<'a, C> {
    pub(super) fn new(original: &'a OperationContext<C>) -> Result<Self, NativeSecurityError> {
        let deadline = Instant::now()
            .checked_add(Duration::from_millis(original.relative_deadline_ms().get()))
            .ok_or(NativeSecurityError::DeadlineElapsed)?;
        let budget = Self { original, deadline, work: None, first: Cell::new(None) };
        budget.check()?;
        Ok(budget)
    }

    pub(super) fn direct(original: &'a OperationContext<C>, deadline: Instant,
        work: &'a dyn Fn(Work) -> Result<(), OwnerError>,
    ) -> Result<Self, NativeSecurityError> {
        let budget = Self { original, deadline, work: Some(work), first: Cell::new(None) };
        budget.check()?;
        Ok(budget)
    }

    fn remember(&self, error: NativeSecurityError) -> NativeSecurityError {
        let first = self.first.get().unwrap_or(error);
        self.first.set(Some(first));
        first
    }

    pub(super) const fn first_failure(&self) -> Option<NativeSecurityError> { self.first.get() }
    pub(super) fn retain_failure(&self, error: NativeSecurityError) -> NativeSecurityError { self.remember(error) }
    pub(super) fn original_allowance(&self)
        -> Option<(Instant, OriginalWork<'_>)> {
        self.work.map(|work| (self.deadline, work))
    }

    fn original_failure(&self, error: &MigrationBootstrapFailure<OwnerError>) -> NativeSecurityError {
        self.remember(NativeSecurityError::OriginalContext { control: error.control, first: error.first_context })
    }

    pub(super) fn check(&self) -> Result<(), NativeSecurityError> {
        if let Some(first) = self.first.get() { return Err(first); }
        if let Some(work) = self.work {
            work(Work::Checkpoint).map_err(|first| self.remember(NativeSecurityError::OriginalContext {
                control: ControlError::ReadCancelled, first: Some(first),
            }))?;
        }
        if self.original.cancellation().is_cancelled() {
            return Err(self.remember(NativeSecurityError::Cancelled));
        }
        if Instant::now() >= self.deadline { return Err(self.remember(NativeSecurityError::DeadlineElapsed)); }
        Ok(())
    }

    pub(super) fn reserve(&self, unit: Work) -> Result<(), NativeSecurityError> {
        self.check()?;
        if let Some(work) = self.work {
            work(unit).map_err(|first| self.remember(NativeSecurityError::OriginalContext {
                control: ControlError::BudgetExceeded, first: Some(first),
            }))?;
        }
        self.check()
    }

    pub(super) fn metadata(&self, members: usize, text: usize) -> Result<(), NativeSecurityError> {
        let bytes = members.checked_mul(256).and_then(|n| n.checked_add(text))
            .and_then(|n| n.checked_add(1024)).ok_or(ControlError::BudgetExceeded)?;
        self.reserve(Work::Allocate(bytes))?;
        self.reserve(Work::Process(bytes))
    }

    pub(super) fn state(&self, state: &SecurityPolicyState) -> Result<(), NativeSecurityError> {
        let members = state.denied_memberships.len().checked_add(state.purged_memberships.len())
            .ok_or(ControlError::BudgetExceeded)?;
        self.metadata(members, state.security_domain_ref.as_str().len())
    }

    pub(super) fn descriptor(&self, value: &SecurityRestrictionMutation) -> Result<(), NativeSecurityError> {
        if let Some(state) = value.expected_state() { self.state(state)?; }
        self.state(value.replacement())?;
        self.metadata(value.required_dependents().len(), value.operation_id().as_str().len())
    }

    pub(super) fn command(&self, command: &SecurityRestriction) -> Result<(), NativeSecurityError> {
        let members = command.expected_live.denied_memberships.len()
            .checked_add(command.expected_live.purged_memberships.len())
            .and_then(|n| n.checked_add(command.next_live.denied_memberships.len()))
            .and_then(|n| n.checked_add(command.next_live.purged_memberships.len()))
            .and_then(|n| n.checked_add(command.required_dependents.len())).ok_or(ControlError::BudgetExceeded)?;
        self.metadata(members, command.operation_id.as_str().len() + command.security_domain_ref.as_str().len())
    }

    pub(super) fn read(&self, journal: &mut PersistentControlJournal, namespace: SourceNamespaceId)
        -> Result<SecurityRestrictionReadback, NativeSecurityError> {
        if let Some(work) = self.work {
            let callback = |unit| work(unit);
            journal.observe_security_restriction_with_original_context(namespace, self.deadline, &callback)
                .map(|(read, _)| read)
                .map_err(|error| self.original_failure(&error))
        } else { Ok(journal.read_security_restriction(namespace, &self.context()?)?) }
    }

    pub(super) fn observe(&self, journal: &mut PersistentControlJournal, namespace: SourceNamespaceId)
        -> Result<(SecurityRestrictionReadback, Option<SecurityRestrictionCommit>), NativeSecurityError> {
        if let Some(work) = self.work {
            let callback = |unit| work(unit);
            journal.observe_security_restriction_with_original_context(namespace, self.deadline, &callback)
                .map_err(|error| self.original_failure(&error))
        } else {
            let read = journal.read_security_restriction(namespace, &self.context()?)?;
            let committed = read.mutation().map(|mutation| self.recover(journal, mutation)).transpose()?.flatten();
            Ok((read, committed))
        }
    }

    pub(super) fn commit(&self, journal: &mut PersistentControlJournal, mutation: &SecurityRestrictionMutation)
        -> Result<SecurityRestrictionCommit, NativeSecurityError> {
        if let Some(work) = self.work {
            let callback = |unit| work(unit);
            journal.commit_security_restriction_with_original_context(mutation, self.deadline, &callback)
                .map_err(|error| self.original_failure(&error))
        } else { Ok(journal.commit_security_restriction(mutation, &self.context()?)?) }
    }

    pub(super) fn recover(&self, journal: &mut PersistentControlJournal, mutation: &SecurityRestrictionMutation)
        -> Result<Option<SecurityRestrictionCommit>, NativeSecurityError> {
        if let Some(work) = self.work {
            let callback = |unit| work(unit);
            if mutation.identity() != journal.identity() {
                let (_, committed) = journal.observe_security_restriction_with_original_context(
                    mutation.replacement().policy.namespace_id, self.deadline, &callback)
                    .map_err(|error| self.original_failure(&error))?;
                return match committed {
                    Some(committed) if committed.mutation() == mutation => Ok(Some(committed)),
                    _ => Err(ControlError::IdentityMismatch.into()),
                };
            }
            journal.recover_security_restriction_with_original_context(mutation, self.deadline, &callback)
                .map_err(|error| self.original_failure(&error))
        } else { Ok(journal.recover_security_restriction(mutation, &self.context()?)?) }
    }

    pub(super) fn publish(&self, journal: &PersistentControlJournal, publisher: &mut ControlSnapshotPublisher)
        -> Result<(), NativeSecurityError> {
        if let Some(work) = self.work {
            let callback = |unit| work(unit);
            journal.publish_direct_with_original_context(publisher, self.deadline, &callback)
                .map_err(|error| self.original_failure(&error))
        } else {
            journal.recover_snapshot_publication_with_context(publisher, &self.context()?)?
                .ok_or(ControlError::SnapshotPublicationFailed)?;
            Ok(())
        }
    }

    pub(super) fn invalidate<D: SecurityInvalidationSink<C>>(&self, dependents: &mut D,
        committed: &SecurityRestrictionCommit, receipt: &ReceiptRef, published: &LiveDenySnapshotRef,
    ) -> Result<BoundedList<SecurityDependentReceipt, MAX_SECURITY_DEPENDENTS>, NativeSecurityError> {
        self.descriptor(committed.mutation())?;
        if let Some(work) = self.work {
            dependents.invalidate_direct_with_original_context(committed, receipt, published,
                &self.context()?, self.deadline, work)
        } else { Ok(dependents.invalidate(committed, receipt, published, &self.context()?)?) }
    }

    pub(super) fn context(&self) -> Result<OperationContext<C>, NativeSecurityError> {
        self.check()?;
        // Round down, never give each stage a fresh original timeout. Less
        // than one millisecond is refused rather than widening the deadline.
        let left = self.deadline.saturating_duration_since(Instant::now()).as_millis();
        let left = u64::try_from(left).map_err(|_| NativeSecurityError::DeadlineElapsed)?;
        OperationContext::new(
            self.original.request_id(), left, self.original.cancellation().clone(),
            self.original.budget_ref().clone(),
        ).map_err(|_| NativeSecurityError::DeadlineElapsed)
    }
}

//! Native composition of the canonical access barrier and the existing journal.
//!
//! This is not a policy store. The barrier owns live transitions; redb owns the
//! full restart command. Dependent receipts must come from executed owner calls.

use core::fmt;

use search_access::{
    AccessCheckpoint, AccessError, AccessPermit, MAX_SECURITY_DEPENDENTS,
    RequestSecurityFence, SecurityDependentReceipt, SecurityMutationBarrier,
    SecurityMutationReceipt, SecurityRestriction,
};
use search_contracts::{
    BoundedList, BoundedSet, LiveDenySnapshotRef, OpaqueId, OpaqueRef, ReceiptRef,
    SecurityMutationPhase, SourceNamespaceId, SourceOwnerGeneration,
};
use search_control_redb::{
    ControlError, ControlSnapshotPublisher, JournalIdentity, PersistentControlJournal,
    access_policy::AccessPolicyJournalError,
    policy_codec::AccessPolicyRecord,
    security_restriction::{SecurityRestrictionCommit, SecurityRestrictionMutation},
};
use search_ports::{CancellationProbe, OperationContext};

mod budget;
mod effects;
mod mapping;
mod invalidation;

pub use invalidation::{
    HANDLE_SECURITY_DEPENDENT, SecurityDependentInvalidator, SecurityInvalidationRegistry,
};

use budget::MutationBudget;
use effects::{JournalEffects, publish_current};

/// Exact namespace/domain configuration supplied by the authenticated owner.
/// It must not be constructed from an untrusted query or a result handle.
#[derive(Clone, Eq, PartialEq)]
pub struct NativeSecurityBinding {
    pub namespace: SourceNamespaceId,
    pub source_owner: SourceOwnerGeneration,
    pub domain: OpaqueRef,
    pub dependents: BoundedSet<OpaqueId, MAX_SECURITY_DEPENDENTS>,
}

/// Real dependent-owner invalidation, under the same domain lock as serving.
///
/// Implementations must actually complete every configured owner operation,
/// preserve its idempotency identity, and return its exact receipts. There is
/// no default implementation and no success generated from a requested scope.
/// A failure after partial work is retried under the same native commit.
/// `mutation_receipt` addresses that verified native operation ledger entry.
pub trait SecurityInvalidationSink<C: CancellationProbe> {
    fn invalidate(
        &mut self,
        committed: &SecurityRestrictionCommit,
        mutation_receipt: &ReceiptRef,
        published: &LiveDenySnapshotRef,
        context: &OperationContext<C>,
    ) -> Result<BoundedList<SecurityDependentReceipt, MAX_SECURITY_DEPENDENTS>, AccessError>;
}

impl<C, F> SecurityInvalidationSink<C> for F
where
    C: CancellationProbe,
    F: FnMut(
        &SecurityRestrictionCommit,
        &ReceiptRef,
        &LiveDenySnapshotRef,
        &OperationContext<C>,
    ) -> Result<BoundedList<SecurityDependentReceipt, MAX_SECURITY_DEPENDENTS>, AccessError>,
{
    fn invalidate(
        &mut self,
        committed: &SecurityRestrictionCommit,
        mutation_receipt: &ReceiptRef,
        published: &LiveDenySnapshotRef,
        context: &OperationContext<C>,
    ) -> Result<BoundedList<SecurityDependentReceipt, MAX_SECURITY_DEPENDENTS>, AccessError> {
        self(committed, mutation_receipt, published, context)
    }
}

/// Content-free failure; a failed mutation/recovery never implies rollback.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeSecurityError {
    Access(AccessError),
    Journal(AccessPolicyJournalError),
    Cancelled,
    DeadlineElapsed,
}

impl fmt::Display for NativeSecurityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Access(error) => fmt::Display::fmt(error, f),
            Self::Journal(error) => fmt::Display::fmt(error, f),
            Self::Cancelled => f.write_str("DAEMON_SECURITY_CANCELLED"),
            Self::DeadlineElapsed => f.write_str("DAEMON_SECURITY_DEADLINE_ELAPSED"),
        }
    }
}

impl std::error::Error for NativeSecurityError {}
impl From<AccessError> for NativeSecurityError {
    fn from(error: AccessError) -> Self { Self::Access(error) }
}
impl From<AccessPolicyJournalError> for NativeSecurityError {
    fn from(error: AccessPolicyJournalError) -> Self { Self::Journal(error) }
}
impl From<ControlError> for NativeSecurityError {
    fn from(error: ControlError) -> Self {
        Self::Journal(AccessPolicyJournalError::Record(error))
    }
}
impl From<search_control_redb::ControlCallError> for NativeSecurityError {
    fn from(error: search_control_redb::ControlCallError) -> Self {
        Self::Journal(AccessPolicyJournalError::Call(error))
    }
}

// Native recovery material, not a second mutable access-policy owner. The
// descriptor is captured once before dispatch and is never rebased on retry.
struct PendingNative {
    policy: AccessPolicyRecord,
    descriptor: Option<SecurityRestrictionMutation>,
    expected_generation: Option<u64>,
    committed: Option<SecurityRestrictionCommit>,
}

/// Connects one canonical security barrier to one exact native journal/domain.
///
/// The outer daemon domain lock must cover serving, mutations and all dependent
/// callbacks. The journal and publisher are borrowed from existing owners, never
/// opened or cloned here. All operations on this domain must use this instance.
/// A snapshot publication alone cannot bypass pending dependent invalidation.
pub struct NativeSecurityDomain {
    binding: NativeSecurityBinding,
    identity: JournalIdentity,
    barrier: SecurityMutationBarrier,
    head: SecurityRestrictionCommit,
    pending: Option<PendingNative>,
}

impl fmt::Debug for NativeSecurityDomain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NativeSecurityDomain")
            .field("phase", &self.barrier.phase())
            .field("pending", &self.pending.is_some())
            .finish_non_exhaustive()
    }
}

impl NativeSecurityDomain {
    /// Restores a fully recorded domain, verifies the native ledger, publishes
    /// current disk state and executes every required dependent invalidation.
    /// No usable owner is returned before all these steps succeed.
    ///
    /// Missing records are errors, not empty restrictions. Initial full state
    /// must have been explicitly installed by the authoritative bootstrap owner.
    /// Old-owner descriptors are rejected; this is not implicit owner succession.
    pub fn restore<C, D>(
        binding: NativeSecurityBinding,
        journal: &mut PersistentControlJournal,
        publisher: &mut ControlSnapshotPublisher,
        dependents: &mut D,
        context: &OperationContext<C>,
    ) -> Result<Self, NativeSecurityError>
    where
        C: CancellationProbe + Clone,
        D: SecurityInvalidationSink<C>,
    {
        let budget = MutationBudget::new(context)?;
        let read = journal.read_security_restriction(binding.namespace, &budget.context()?)?;
        let descriptor = read.mutation().ok_or(AccessError::SecurityFailClosed)?.clone();
        mapping::validate_binding(&binding, &descriptor)?;
        let committed = journal
            .recover_security_restriction(&descriptor, &budget.context()?)?
            .ok_or(AccessError::SecurityFailClosed)?;
        let current = journal.read_security_restriction(binding.namespace, &budget.context()?)?;
        current.confirm_current(&committed)?;
        let replacement = descriptor.replacement();

        if descriptor.expected_state().is_none() {
            // An explicit initialization is not a fabricated restrictive
            // transition from an invented empty state. Replay its real effects
            // before exposing its recovered barrier, including after restart.
            let barrier = SecurityMutationBarrier::from_recovered_snapshot(
                binding.domain.clone(), replacement.policy.policy_revision,
                mapping::live(replacement), binding.dependents.clone(),
            )?;
            let published = publish_current(journal, publisher, &committed, &budget)?;
            let receipts = dependents.invalidate(
                &committed, &mapping::receipt_ref(&committed)?, &published, &budget.context()?,
            )?;
            mapping::validate_receipts(&committed, &published, &receipts)?;
            budget.check()?;
            return Ok(Self {
                binding, identity: journal.identity(), barrier, head: committed, pending: None,
            });
        }

        let command = mapping::command(&descriptor)?;
        let barrier = SecurityMutationBarrier::from_pending_restriction(
            command, Some(mapping::receipt_ref(&committed)?),
        )?;
        let mut owner = Self {
            binding, identity: journal.identity(), barrier,
            pending: Some(PendingNative {
                policy: replacement.policy,
                expected_generation: Some(committed.receipt().before_generation),
                descriptor: Some(descriptor), committed: Some(committed.clone()),
            }),
            head: committed,
        };
        owner.run(None, journal, publisher, dependents, &budget)?;
        Ok(owner)
    }

    /// Applies one server-compiled restriction. Extra policy metadata is frozen
    /// with the full command before native write dispatch, including on retry.
    /// Any uncertain effect leaves the canonical barrier closed for recovery.
    pub fn apply<C, D>(
        &mut self,
        command: SecurityRestriction,
        policy: AccessPolicyRecord,
        journal: &mut PersistentControlJournal,
        publisher: &mut ControlSnapshotPublisher,
        dependents: &mut D,
        context: &OperationContext<C>,
    ) -> Result<SecurityMutationReceipt, NativeSecurityError>
    where
        C: CancellationProbe + Clone,
        D: SecurityInvalidationSink<C>,
    {
        let budget = MutationBudget::new(context)?;
        self.require_journal(journal)?;
        mapping::validate_target(&self.binding, &command, &policy)?;
        if let Some(pending) = &self.pending {
            if pending.policy != policy {
                return Err(AccessError::SecurityOperationConflict.into());
            }
        } else {
            // Canonical retry receipts do not carry shadow/purge metadata.
            // Check that metadata here before accepting a completed replay.
            if self.head.mutation().operation_id() == &command.operation_id
                && (self.head.mutation().replacement().policy != policy
                    || mapping::command(self.head.mutation())? != command)
            {
                return Err(AccessError::SecurityOperationConflict.into());
            }
            self.pending = Some(PendingNative {
                policy, descriptor: None, expected_generation: None, committed: None,
            });
        }
        self.run(Some(command), journal, publisher, dependents, &budget)
    }

    /// Resolves the frozen pending operation; never replaces its generation,
    /// identity or command after an uncertain commit. Uses a fresh call budget.
    pub fn recover<C, D>(
        &mut self,
        journal: &mut PersistentControlJournal,
        publisher: &mut ControlSnapshotPublisher,
        dependents: &mut D,
        context: &OperationContext<C>,
    ) -> Result<SecurityMutationReceipt, NativeSecurityError>
    where
        C: CancellationProbe + Clone,
        D: SecurityInvalidationSink<C>,
    {
        let budget = MutationBudget::new(context)?;
        self.require_journal(journal)?;
        if self.pending.is_none() {
            return Err(AccessError::SecurityOperationConflict.into());
        }
        self.run(None, journal, publisher, dependents, &budget)
    }

    /// Runs a hot-path checkpoint without redb reads/writes or snapshot cloning.
    /// Current grant/binding/expiry validation remains mandatory in the caller.
    pub fn with_live_checkpoint<T>(
        &self,
        fence: &RequestSecurityFence,
        checkpoint: AccessCheckpoint,
        operation: impl FnOnce(AccessPermit) -> T,
    ) -> Result<T, AccessError> {
        if self.pending.is_some() { return Err(AccessError::SecurityFailClosed); }
        self.barrier.with_live_checkpoint(fence, checkpoint, operation)
    }

    /// Current content-free phase, not authorization or a durability receipt.
    #[must_use]
    pub const fn phase(&self) -> SecurityMutationPhase { self.barrier.phase() }

    fn require_journal(&self, journal: &PersistentControlJournal) -> Result<(), NativeSecurityError> {
        if self.identity != journal.identity() { return Err(ControlError::IdentityMismatch.into()); }
        Ok(())
    }

    fn run<C, D>(
        &mut self,
        command: Option<SecurityRestriction>,
        journal: &mut PersistentControlJournal,
        publisher: &mut ControlSnapshotPublisher,
        dependents: &mut D,
        budget: &MutationBudget<'_, C>,
    ) -> Result<SecurityMutationReceipt, NativeSecurityError>
    where
        C: CancellationProbe + Clone,
        D: SecurityInvalidationSink<C>,
    {
        let pending = self.pending.as_mut().ok_or(AccessError::SecurityOperationConflict)?;
        let mut effects = JournalEffects::new(
            &self.binding, &self.head, pending, journal, publisher, dependents, budget,
        );
        let result = match command {
            Some(command) => self.barrier.apply_security_mutation(command, &mut effects),
            None => self.barrier.recover_security_mutation(&mut effects),
        };
        let failure = effects.failure;
        drop(effects);
        // No fallible work follows canonical completion. A completed replay has
        // no new native commit; its frozen native metadata was checked in apply.
        match result {
            Ok(receipt) => {
                if let Some(committed) = self.pending.as_mut().and_then(|pending| pending.committed.take()) {
                    self.head = committed;
                }
                self.pending = None;
                Ok(receipt)
            }
            Err(error) => {
                if self.barrier.phase() == SecurityMutationPhase::Acknowledged {
                    // Rejected before the first effect; do not retain a false
                    // pending descriptor after ordinary input validation.
                    self.pending = None;
                }
                Err(failure.unwrap_or(NativeSecurityError::Access(error)))
            }
        }
    }
}

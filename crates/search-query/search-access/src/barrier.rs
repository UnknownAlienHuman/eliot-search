//! One domain's restrictive-mutation coordinator. Durable state and effects
//! belong to injected owners; pending or uncertain work closes every checkpoint.

use core::fmt;
use std::collections::BTreeSet;

use search_contracts::{
    AccessPolicyRevision, BoundedList, BoundedSet, LiveDenySnapshotRef, MAX_SET_ITEMS,
    OpaqueId, OpaqueRef, ReceiptRef, SecurityMutationPhase,
};

use crate::{AccessCheckpoint, AccessError, AccessPermit, LiveSecurityState, RequestSecurityFence};

/// Maximum dependent owners acknowledged by one restrictive mutation.
pub const MAX_SECURITY_DEPENDENTS: usize = 64;

/// Exact server-compiled membership restriction, not a client-authored policy.
///
/// Expected state and the complete required-owner set are part of the durable
/// operation identity. The control adapter must compare/store this whole value,
/// not just the operation ID, generation or a caller-supplied digest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecurityRestriction {
    pub operation_id: OpaqueId,
    pub security_domain_ref: OpaqueRef,
    pub expected_policy_revision: AccessPolicyRevision,
    pub policy_revision: AccessPolicyRevision,
    pub expected_live: LiveSecurityState,
    pub next_live: LiveSecurityState,
    pub required_dependents: BoundedSet<OpaqueId, MAX_SECURITY_DEPENDENTS>,
}

/// Exact durable readback, returned only after the guarded commit is known.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DurableSecurityRestriction {
    pub command: SecurityRestriction,
    pub receipt_ref: ReceiptRef,
}

/// One dependent owner's acknowledgement of this exact restriction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecurityDependentReceipt {
    pub dependent: OpaqueId,
    pub mutation_receipt_ref: ReceiptRef,
    pub live_snapshot_ref: LiveDenySnapshotRef,
    pub receipt_ref: ReceiptRef,
}

/// Historical completion evidence, never a reusable access permit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecurityMutationReceipt {
    pub operation_id: OpaqueId,
    pub mutation_receipt_ref: ReceiptRef,
    pub live_snapshot_ref: LiveDenySnapshotRef,
    pub dependent_receipts: BoundedList<SecurityDependentReceipt, MAX_SECURITY_DEPENDENTS>,
}

/// Effect boundary supplied by the control, snapshot and invalidation owners.
///
/// Adapters enforce bounded I/O, deadlines and cancellation. Any uncertain
/// effect returns `Err`, including a cancelled call that may have written.
/// Commit, publication and invalidation must be idempotent for the exact
/// operation. No implementation may construct success merely from its input.
pub trait SecurityMutationEffects {
    type Error;

    /// Atomically compare the complete expected domain state, reject conflicting
    /// operation-ID reuse, and durably store the restriction and operation record.
    /// Return only exact verified readback of the current authoritative head.
    fn commit_restriction(
        &mut self,
        command: &SecurityRestriction,
    ) -> Result<DurableSecurityRestriction, Self::Error>;

    /// Resolve a previous possible write before any retry. `Some` must be the
    /// exact operation AND current domain head; a historical/superseded commit
    /// is an error. `None` proves resolved absence with the exact expected head
    /// still current, not a timeout or an outstanding write. Otherwise fail.
    fn readback_restriction(
        &mut self,
        command: &SecurityRestriction,
    ) -> Result<Option<DurableSecurityRestriction>, Self::Error>;

    /// Publish the exact committed immutable restriction, rejecting rollback.
    fn publish_live_restriction(
        &mut self,
        committed: &DurableSecurityRestriction,
    ) -> Result<LiveDenySnapshotRef, Self::Error>;

    /// Invalidate every owner in `committed.command.required_dependents` and
    /// return their executed acknowledgements. Partial work is retryable under
    /// the same operation; a missing/stale receipt is never completion.
    fn invalidate_dependents(
        &mut self,
        committed: &DurableSecurityRestriction,
    ) -> Result<BoundedList<SecurityDependentReceipt, MAX_SECURITY_DEPENDENTS>, Self::Error>;
}

struct PendingRestriction {
    command: SecurityRestriction,
    committed_receipt_ref: Option<ReceiptRef>,
}

struct CompletedRestriction {
    command: SecurityRestriction,
    receipt: SecurityMutationReceipt,
}

/// Single mutable owner of one domain's live restriction transition.
///
/// The caller serializes all domain mutation and serving checkpoints through
/// this owner (for example, under its existing domain lock). There is no second
/// policy database, background worker, lock acquisition or unbounded history.
/// A pending operation blocks checkpoints even if an adapter panics. Process
/// recovery must restore pending work from durable control, not manufacture a
/// clean snapshot. A captured snapshot/receipt alone never authorizes output.
pub struct SecurityMutationBarrier {
    domain: OpaqueRef,
    policy_revision: AccessPolicyRevision,
    live: LiveSecurityState,
    required_dependents: BoundedSet<OpaqueId, MAX_SECURITY_DEPENDENTS>,
    phase: SecurityMutationPhase,
    pending: Option<PendingRestriction>,
    completed: Option<CompletedRestriction>,
}

impl fmt::Debug for SecurityMutationBarrier {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SecurityMutationBarrier")
            .field("phase", &self.phase)
            .field("blocked", &self.pending.is_some())
            .finish_non_exhaustive()
    }
}

impl SecurityMutationBarrier {
    /// Initializes an already-recovered domain. The integration owner must
    /// prove no unresolved durable mutation exists and that the supplied live
    /// snapshot is published. Required dependents are server configuration, not
    /// a choice made by the mutation requester. No effect is asserted here.
    pub fn from_recovered_snapshot(
        domain: OpaqueRef,
        policy_revision: AccessPolicyRevision,
        live: LiveSecurityState,
        required_dependents: BoundedSet<OpaqueId, MAX_SECURITY_DEPENDENTS>,
    ) -> Result<Self, AccessError> {
        validate_snapshot(&live)?;
        if required_dependents.is_empty() || live.fail_closed {
            return Err(AccessError::SecurityFailClosed);
        }
        Ok(Self {
            domain,
            policy_revision,
            live,
            required_dependents,
            phase: SecurityMutationPhase::Acknowledged,
            pending: None,
            completed: None,
        })
    }

    /// Restores unresolved work from the exact durable operation/journal input.
    /// Starts closed. A known commit reference cannot disappear or change on
    /// recovery; `None` means the previous commit outcome is still unknown.
    pub fn from_pending_restriction(
        command: SecurityRestriction,
        known_commit: Option<ReceiptRef>,
    ) -> Result<Self, AccessError> {
        validate_snapshot(&command.expected_live)?;
        validate_snapshot(&command.next_live)?;
        let mut owner = Self::from_recovered_snapshot(
            command.security_domain_ref.clone(),
            command.expected_policy_revision,
            command.expected_live.clone(),
            command.required_dependents.clone(),
        )?;
        owner.validate_command(&command)?;
        owner.pending = Some(PendingRestriction {
            command,
            committed_receipt_ref: known_commit,
        });
        owner.phase = SecurityMutationPhase::FailClosed;
        Ok(owner)
    }

    /// Current transition phase, without exposing denied memberships.
    #[must_use]
    pub const fn phase(&self) -> SecurityMutationPhase {
        self.phase
    }

    /// Borrows current state only when no mutation is pending. Consumers must
    /// still re-enter the owner at each later influence/disclosure checkpoint.
    pub fn live_snapshot(&self) -> Result<&LiveSecurityState, AccessError> {
        if self.pending.is_some() || self.phase != SecurityMutationPhase::Acknowledged {
            return Err(AccessError::SecurityFailClosed);
        }
        Ok(&self.live)
    }

    /// Holds a shared owner borrow across the checkpoint operation, excluding
    /// mutation through this owner until the callback returns. An outer domain
    /// lock must cover this call where the owner is shared between threads.
    /// This checks restrictive state, not grant signatures, expiry or transport.
    pub fn with_live_checkpoint<T>(
        &self,
        request: &RequestSecurityFence,
        checkpoint: AccessCheckpoint,
        operation: impl FnOnce(AccessPermit) -> T,
    ) -> Result<T, AccessError> {
        let live = self.live_snapshot()?;
        if request.memberships.len() > MAX_SET_ITEMS {
            return Err(AccessError::SecurityFailClosed);
        }
        let permit = crate::recheck_live_access(request, live, checkpoint)?;
        Ok(operation(permit))
    }

    /// Applies a new restriction, or reconciles the same pending operation.
    ///
    /// Order: block the domain, durable commit/readback, immutable publication,
    /// exact dependent acknowledgements, then expose completion. Every effect
    /// error or mismatched receipt retains pending identity and closes access.
    /// Retrying the last completed identical command returns its receipt; older
    /// stale commands are refused, not replayed over a newer security head.
    pub fn apply_security_mutation(
        &mut self,
        command: SecurityRestriction,
        effects: &mut impl SecurityMutationEffects,
    ) -> Result<SecurityMutationReceipt, AccessError> {
        if let Some(pending) = &self.pending {
            if pending.command != command {
                return Err(AccessError::SecurityOperationConflict);
            }
            return self.recover_security_mutation(effects);
        }
        if let Some(completed) = &self.completed {
            if completed.command.operation_id == command.operation_id {
                return if completed.command == command {
                    Ok(completed.receipt.clone())
                } else {
                    Err(AccessError::SecurityOperationConflict)
                };
            }
        }
        self.validate_command(&command)?;
        // Install before entering ANY adapter; unwind/uncertain return cannot
        // leave this owner advertising the old snapshot as safe to serve.
        self.pending = Some(PendingRestriction { command, committed_receipt_ref: None });
        self.phase = SecurityMutationPhase::Acquired;
        self.run_pending(effects, false)
    }

    /// Reconciles the exact pending identity. Readback always precedes another
    /// possible commit. Already-published restrictions are never rolled back;
    /// publication/invalidation replay is idempotent through the effect owners.
    pub fn recover_security_mutation(
        &mut self,
        effects: &mut impl SecurityMutationEffects,
    ) -> Result<SecurityMutationReceipt, AccessError> {
        if self.pending.is_none() {
            return Err(AccessError::SecurityOperationConflict);
        }
        self.run_pending(effects, true)
    }

    fn run_pending(
        &mut self,
        effects: &mut impl SecurityMutationEffects,
        recovering: bool,
    ) -> Result<SecurityMutationReceipt, AccessError> {
        let result = self.advance_pending(effects, recovering);
        if result.is_err() {
            self.phase = SecurityMutationPhase::FailClosed;
        }
        result
    }

    fn advance_pending(
        &mut self,
        effects: &mut impl SecurityMutationEffects,
        recovering: bool,
    ) -> Result<SecurityMutationReceipt, AccessError> {
        let pending = self.pending.as_ref().ok_or(AccessError::SecurityOperationConflict)?;
        let command = pending.command.clone();
        let known_commit = pending.committed_receipt_ref.clone();
        let observed = if recovering {
            effects.readback_restriction(&command).map_err(|_| AccessError::SecurityFailClosed)?
        } else {
            None
        };
        let committed = match observed {
            Some(committed) => committed,
            None if known_commit.is_some() => return Err(AccessError::SecurityFailClosed),
            None => effects
                .commit_restriction(&command)
                .map_err(|_| AccessError::SecurityFailClosed)?,
        };
        if committed.command != command
            || known_commit.as_ref().is_some_and(|reference| reference != &committed.receipt_ref)
        {
            return Err(AccessError::SecurityOperationConflict);
        }
        self.pending.as_mut().ok_or(AccessError::SecurityOperationConflict)?
            .committed_receipt_ref = Some(committed.receipt_ref.clone());
        self.phase = SecurityMutationPhase::DurableCommitted;

        let published = effects.publish_live_restriction(&committed)
            .map_err(|_| AccessError::SecurityFailClosed)?;
        let expected = LiveDenySnapshotRef {
            security_domain_ref: command.security_domain_ref.clone(),
            live_deny_generation: command.next_live.generation,
            snapshot_digest: command.next_live.snapshot_digest,
        };
        if published != expected {
            return Err(AccessError::SecurityOperationConflict);
        }
        self.phase = SecurityMutationPhase::LiveSnapshotPublished;

        let received = effects.invalidate_dependents(&committed)
            .map_err(|_| AccessError::SecurityFailClosed)?;
        validate_dependents(&command, &committed.receipt_ref, &published, &received)?;
        let mut ordered = received.into_vec();
        ordered.sort_by(|left, right| left.dependent.cmp(&right.dependent));
        let received = BoundedList::new(ordered).map_err(|_| AccessError::SecurityFailClosed)?;
        self.phase = SecurityMutationPhase::DependentsInvalidated;
        let receipt = SecurityMutationReceipt {
            operation_id: command.operation_id.clone(),
            mutation_receipt_ref: committed.receipt_ref,
            live_snapshot_ref: published,
            dependent_receipts: received,
        };
        // Prepare the retry receipt before releasing the pending fence. No
        // recoverable operation or adapter call occurs after this point.
        let completed = CompletedRestriction { command: command.clone(), receipt: receipt.clone() };
        self.policy_revision = command.policy_revision;
        self.live = command.next_live;
        self.completed = Some(completed);
        self.pending = None;
        self.phase = SecurityMutationPhase::Acknowledged;
        Ok(receipt)
    }

    fn validate_command(&self, command: &SecurityRestriction) -> Result<(), AccessError> {
        if command.security_domain_ref != self.domain
            || command.required_dependents != self.required_dependents
        {
            return Err(AccessError::SecurityOperationConflict);
        }
        if command.expected_policy_revision != self.policy_revision
            || command.expected_live != self.live
        {
            return Err(AccessError::SecurityFenceStale);
        }
        validate_snapshot(&command.next_live)?;
        if command.next_live.fail_closed {
            return Err(AccessError::SecurityFailClosed);
        }
        if command.next_live.generation <= self.live.generation
            || command.policy_revision.get() < self.policy_revision.get()
        {
            return Err(AccessError::SecurityGenerationRegression);
        }
        // This path is membership-restrictive only. Permissive policy changes
        // and recovery from an unknown initial snapshot require their own flow.
        if !self.live.denied_memberships.is_subset(&command.next_live.denied_memberships)
            || !self.live.purged_memberships.is_subset(&command.next_live.purged_memberships)
            || (self.live.denied_memberships == command.next_live.denied_memberships
                && self.live.purged_memberships == command.next_live.purged_memberships)
        {
            return Err(AccessError::SecurityOperationConflict);
        }
        Ok(())
    }
}

fn validate_snapshot(snapshot: &LiveSecurityState) -> Result<(), AccessError> {
    if snapshot.denied_memberships.len() > MAX_SET_ITEMS
        || snapshot.purged_memberships.len() > MAX_SET_ITEMS
    {
        return Err(AccessError::SecurityFailClosed);
    }
    Ok(())
}

fn validate_dependents(
    command: &SecurityRestriction,
    mutation_receipt: &ReceiptRef,
    published: &LiveDenySnapshotRef,
    received: &BoundedList<SecurityDependentReceipt, MAX_SECURITY_DEPENDENTS>,
) -> Result<(), AccessError> {
    if received.len() != command.required_dependents.len() {
        return Err(AccessError::SecurityFailClosed);
    }
    let mut seen = BTreeSet::new();
    let mut receipt_refs = BTreeSet::new();
    for receipt in received {
        if !command.required_dependents.contains(&receipt.dependent)
            || !seen.insert(&receipt.dependent)
            || !receipt_refs.insert(&receipt.receipt_ref)
            || receipt.mutation_receipt_ref != *mutation_receipt
            || receipt.live_snapshot_ref != *published
        {
            return Err(AccessError::SecurityOperationConflict);
        }
    }
    Ok(())
}

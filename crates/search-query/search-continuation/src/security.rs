//! Whole-window security invalidation with recoverable external cleanup.

use core::fmt;
use std::ops::Bound::{Excluded, Unbounded};

use search_contracts::{BoundedSet, MAX_SET_ITEMS, NonZeroRevision, ReceiptRef, SourceMembershipId};

use crate::{
    ContinuationEffect, ContinuationError, ContinuationId, ContinuationStore,
    CreateContinuationRequest, CreatedContinuation, InvalidationReason,
    LifecycleRecordStatus, StoredContinuation,
};

/// Complete membership population that influenced a continuation's plan.
///
/// Capture every retrieval/scoring/IDF/count/trace membership, not merely the
/// returned hits. The authenticated planner supplies this value; it is not a
/// grant and must not be constructed from client claims or inferred from tokens.
#[derive(Clone, Eq, PartialEq)]
pub struct ContinuationSecurityScope {
    memberships: BoundedSet<SourceMembershipId, MAX_SET_ITEMS>,
}

impl ContinuationSecurityScope {
    /// Captures one bounded nonempty population. Empty is not a proof that an
    /// existing ranking was unaffected; unrecorded populations remain unknown.
    pub fn new(
        memberships: BoundedSet<SourceMembershipId, MAX_SET_ITEMS>,
    ) -> Result<Self, ContinuationError> {
        if memberships.is_empty() {
            return Err(ContinuationError::InvalidLimits);
        }
        Ok(Self { memberships })
    }
}

impl fmt::Debug for ContinuationSecurityScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ContinuationSecurityScope")
            .field("membership_count", &self.memberships.len())
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub(super) struct PendingCleanup {
    operation_receipt: ReceiptRef,
    generation: u64,
}

impl fmt::Debug for PendingCleanup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PendingCleanup")
            .field("generation", &self.generation)
            .finish_non_exhaustive()
    }
}

/// Exact external cleanup requested for an already-terminal store record.
///
/// This is an obligation, not execution evidence. The resource owner must use
/// the original operation and immutable target for idempotent retry. References
/// must never be parsed into guessed native identities or widened deletion scopes.
pub struct ContinuationCleanup {
    continuation_id: ContinuationId,
    record_revision: u64,
    pending: PendingCleanup,
    effect: ContinuationEffect,
}

impl ContinuationCleanup {
    /// Exact record owning the pin or durable checkpoint.
    #[must_use]
    pub const fn continuation_id(&self) -> ContinuationId { self.continuation_id }
    /// Terminal record revision; it cannot change during this pass's borrow.
    #[must_use]
    pub const fn record_revision(&self) -> u64 { self.record_revision }
    /// Original verified security-operation reference, unchanged on retry.
    #[must_use]
    pub const fn operation_receipt(&self) -> &ReceiptRef { &self.pending.operation_receipt }
    /// Original live restriction generation; zero is valid for recorded bootstrap state.
    #[must_use]
    pub const fn generation(&self) -> u64 { self.pending.generation }
    /// Exact pin release or durable-checkpoint deletion. Never a renewal.
    #[must_use]
    pub const fn effect(&self) -> &ContinuationEffect { &self.effect }
}

impl fmt::Debug for ContinuationCleanup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ContinuationCleanup")
            .field("record_revision", &self.record_revision)
            .field("generation", &self.pending.generation)
            .finish_non_exhaustive()
    }
}

/// Progress in one bounded pass. Mutation and inspected-record counts are
/// separate so an unrelated or terminal prefix cannot bypass the work ceiling.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SecurityInvalidationProgress {
    /// Records inspected in this step.
    pub inspected: usize,
    /// Active records made terminal in this step.
    pub invalidated: usize,
    /// External cleanup must succeed before another step can run.
    pub cleanup_pending: bool,
    /// Every retained record was inspected and no cleanup remains in this pass.
    pub complete: bool,
}

/// Completed process-local invalidation and cleanup pass, not a durable receipt.
/// Only a completed pass can construct this value. Counters describe this attempt,
/// not records invalidated in an earlier interrupted attempt.
#[derive(Debug)]
pub struct SecurityInvalidationReceipt {
    generation: u64,
    inspected: usize,
    invalidated: usize,
    cleanups: usize,
}

impl SecurityInvalidationReceipt {
    /// Restriction generation inspected by the completed pass.
    #[must_use]
    pub const fn generation(&self) -> u64 { self.generation }
    /// Records visited during this attempt.
    #[must_use]
    pub const fn inspected(&self) -> usize { self.inspected }
    /// Active records made terminal during this attempt.
    #[must_use]
    pub const fn invalidated(&self) -> usize { self.invalidated }
    /// External cleanups acknowledged through successful owner callbacks.
    #[must_use]
    pub const fn cleanups(&self) -> usize { self.cleanups }
}

/// Exclusive, non-clonable continuation invalidation cursor.
///
/// The enclosing security domain must stay closed until `finish` succeeds.
/// Dropping a pass does not undo invalidation or erase pending cleanup: the
/// record keeps the original operation reference and cannot be compacted. Retry
/// the same operation from the beginning; external effects must be idempotent.
/// Different-operation retries cannot take over unresolved cleanup.
pub struct SecurityInvalidation<'store, 'scope> {
    store: &'store mut ContinuationStore,
    denied: &'scope BoundedSet<SourceMembershipId, MAX_SET_ITEMS>,
    purged: &'scope BoundedSet<SourceMembershipId, MAX_SET_ITEMS>,
    identity: PendingCleanup,
    after: Option<ContinuationId>,
    pending: Option<ContinuationCleanup>,
    complete: bool,
    inspected: usize,
    invalidated: usize,
    cleanups: usize,
}

impl ContinuationStore {
    /// Creates through the existing validated insertion path and binds the full
    /// planned influence population before returning the token. No second index
    /// or candidate copy is created. Durable restore must supply its original
    /// population; when unavailable, ordinary `create` remains conservative.
    pub fn create_with_security_scope(
        &mut self,
        request: CreateContinuationRequest,
        scope: ContinuationSecurityScope,
    ) -> Result<CreatedContinuation, ContinuationError> {
        let created = self.create(request)?;
        self.records.get_mut(&created.handle.continuation_id)
            .expect("record just inserted under exclusive store borrow")
            .security_scope = Some(scope);
        Ok(created)
    }

    /// Begins a bounded pass for a fully recorded native security operation.
    /// Any intersecting membership invalidates the entire window/replan, not
    /// just displayed candidates. Records without captured influence are treated
    /// as affected by any nonempty restriction, never certified disjoint.
    /// Empty recorded sets still resume any pending cleanup of this operation.
    pub fn begin_security_invalidation<'store, 'scope>(
        &'store mut self,
        denied: &'scope BoundedSet<SourceMembershipId, MAX_SET_ITEMS>,
        purged: &'scope BoundedSet<SourceMembershipId, MAX_SET_ITEMS>,
        generation: u64,
        operation_receipt: &ReceiptRef,
    ) -> SecurityInvalidation<'store, 'scope> {
        SecurityInvalidation {
            store: self, denied, purged,
            identity: PendingCleanup { operation_receipt: operation_receipt.clone(), generation },
            after: None, pending: None, complete: false,
            inspected: 0, invalidated: 0, cleanups: 0,
        }
    }
}

impl SecurityInvalidation<'_, '_> {
    /// Inspects at most the configured lifecycle batch and stops at the first
    /// cleanup obligation. Revisions and the exact effect are prepared before
    /// changing that record. A returned error never mutates a prefix of this step.
    pub fn advance(&mut self) -> Result<SecurityInvalidationProgress, ContinuationError> {
        if self.pending.is_some() {
            return Err(ContinuationError::InvalidTransition);
        }
        if self.complete {
            return Ok(progress(0, 0, false, true));
        }
        let limit = self.store.limits.max_lifecycle_batch;
        let start = self.after.map_or(Unbounded, Excluded);
        let mut last = self.after;
        let mut inspected = 0;
        let mut selected = None;
        for (id, record) in self.store.records.range((start, Unbounded)).take(limit) {
            inspected += 1;
            last = Some(*id);
            let reason = restriction_reason(record, self.denied, self.purged);
            if record.security_cleanup.is_some() || reason.is_some() {
                selected = Some((*id, reason));
                break;
            }
        }
        let invalidated = if let Some((id, reason)) = selected {
            let record = self.store.records.get_mut(&id)
                .ok_or(ContinuationError::InvalidTransition)?;
            if record.security_cleanup.as_ref().is_some_and(|pending| pending != &self.identity) {
                return Err(ContinuationError::OperationConflict);
            }
            let was_active = record.is_active();
            if was_active && record.last_invalidation_generation.is_some_and(|previous| {
                self.identity.generation < previous.get()
            }) {
                return Err(ContinuationError::OperationConflict);
            }
            // Bootstrap snapshots may have generation zero; the original
            // optional nonzero lifecycle field stays absent for that one case.
            let generation = NonZeroRevision::new(self.identity.generation).ok();
            let revision = if was_active { record.next_revision()? } else { record.revision };
            let cleanup = ContinuationCleanup {
                continuation_id: id, record_revision: revision,
                pending: self.identity.clone(), effect: record.cleanup_effect(),
            };
            let retained_pending = self.identity.clone();
            if was_active {
                let reason = reason.ok_or(ContinuationError::InvalidTransition)?;
                record.set_status(LifecycleRecordStatus::Revoked);
                record.terminal_reason = Some(reason);
                record.revision = revision;
                record.last_invalidation_generation = generation;
            }
            // The original target and operation survive cancellation, unwinding
            // and a dropped cursor. No fallible work follows the record mutation.
            record.security_cleanup = Some(retained_pending);
            self.pending = Some(cleanup);
            usize::from(was_active)
        } else {
            // At the exact batch boundary, one later step proves EOF; no extra
            // record is inspected outside the configured step budget.
            self.complete = inspected < limit;
            0
        };
        self.after = last;
        self.inspected += inspected;
        self.invalidated += invalidated;
        Ok(progress(inspected, invalidated, self.pending.is_some(), self.complete))
    }

    /// Runs the mandatory resource-owner callback for the exact pending effect.
    /// The owner must return success only after release/deletion or authoritative
    /// readback of its prior completion. Failure/unwind preserves the obligation;
    /// subsequent steps and `finish` remain unavailable until it succeeds.
    pub fn complete_cleanup(
        &mut self,
        execute: impl FnOnce(&ContinuationCleanup) -> Result<(), ContinuationError>,
    ) -> Result<(), ContinuationError> {
        let cleanup = self.pending.as_ref().ok_or(ContinuationError::InvalidTransition)?;
        let record = self.store.records.get_mut(&cleanup.continuation_id)
            .ok_or(ContinuationError::InvalidTransition)?;
        if record.is_active() || record.revision != cleanup.record_revision
            || record.security_cleanup.as_ref() != Some(&cleanup.pending)
            || record.cleanup_effect() != cleanup.effect
        {
            return Err(ContinuationError::InvalidTransition);
        }
        execute(cleanup)?;
        // The exclusive borrow excludes replacement/compaction during execution.
        // No fallible check after acknowledged external cleanup can lose its result.
        record.security_cleanup = None;
        self.pending = None;
        self.cleanups += 1;
        Ok(())
    }

    /// Produces completion only after EOF and every required cleanup callback.
    pub fn finish(self) -> Result<SecurityInvalidationReceipt, ContinuationError> {
        if !self.complete || self.pending.is_some() {
            return Err(ContinuationError::InvalidTransition);
        }
        Ok(SecurityInvalidationReceipt {
            generation: self.identity.generation, inspected: self.inspected,
            invalidated: self.invalidated, cleanups: self.cleanups,
        })
    }
}

fn restriction_reason(
    record: &StoredContinuation,
    denied: &BoundedSet<SourceMembershipId, MAX_SET_ITEMS>,
    purged: &BoundedSet<SourceMembershipId, MAX_SET_ITEMS>,
) -> Option<InvalidationReason> {
    let intersects = |restricted: &BoundedSet<SourceMembershipId, MAX_SET_ITEMS>| {
        record.security_scope.as_ref().map_or_else(
            || !restricted.is_empty(),
            |scope| scope.memberships.iter().any(|member| restricted.contains(member)),
        )
    };
    if intersects(purged) {
        Some(InvalidationReason::Purged)
    } else if intersects(denied) {
        Some(InvalidationReason::AccessRevoked)
    } else {
        None
    }
}

const fn progress(
    inspected: usize, invalidated: usize, cleanup_pending: bool, complete: bool,
) -> SecurityInvalidationProgress {
    SecurityInvalidationProgress { inspected, invalidated, cleanup_pending, complete }
}

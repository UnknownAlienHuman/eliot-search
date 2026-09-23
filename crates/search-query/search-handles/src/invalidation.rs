//! Bounded security invalidation over the existing handle owner.

use core::fmt;
use std::ops::Bound::{Excluded, Unbounded};

use search_contracts::{
    BoundedSet, HandleTokenDigest, MAX_LIST_ITEMS, MAX_SET_ITEMS, NonZeroRevision,
    SourceMembershipId,
};

use crate::{HandleError, HandleRecord, HandleRecordState, HandleStore};

/// Work completed by one bounded step, not acknowledgement of the whole scope.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HandleInvalidationProgress {
    /// Records inspected, including unrelated and terminal records.
    pub inspected: usize,
    /// Active records invalidated in this step.
    pub invalidated: usize,
    /// The pass is complete; an empty restriction set needs no record scan.
    pub complete: bool,
}

/// Completion of one actual full-store membership pass, not a durable receipt.
/// Only a completed scan constructs this value. The caller must keep its domain
/// barrier closed until all other dependent owners also acknowledge the mutation.
#[derive(Debug)]
pub struct HandleSecurityInvalidationReceipt {
    generation: u64,
    inspected: usize,
    invalidated: usize,
}

impl HandleSecurityInvalidationReceipt {
    /// Exact security generation applied to matching handles.
    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    /// Total records inspected during this pass.
    #[must_use]
    pub const fn inspected(&self) -> usize {
        self.inspected
    }

    /// Total active records invalidated during this pass.
    #[must_use]
    pub const fn invalidated(&self) -> usize {
        self.invalidated
    }
}

/// Non-clonable progress cursor borrowing the sole handle store and immutable
/// restriction sets. No token, target or record inventory is copied.
///
/// Holding this value excludes minting, expiry and other store mutations between
/// steps. Each step bounds inspected records, not merely matching records, so a
/// large unrelated or terminal prefix cannot monopolize a cancellation interval.
/// Dropping an incomplete pass does not roll back earlier steps. Retry the same
/// restriction while keeping the external domain closed; completed steps are
/// idempotent. This does not delete durable targets or release retention leases.
#[must_use]
pub struct HandleSecurityInvalidation<'a> {
    store: &'a mut HandleStore,
    denied: &'a BoundedSet<SourceMembershipId, MAX_SET_ITEMS>,
    purged: &'a BoundedSet<SourceMembershipId, MAX_SET_ITEMS>,
    generation: u64,
    cursor: Option<HandleTokenDigest>,
    complete: bool,
    inspected: usize,
    invalidated: usize,
}

impl fmt::Debug for HandleSecurityInvalidation<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HandleSecurityInvalidation")
            .field("generation", &self.generation)
            .field("complete", &self.complete)
            .finish_non_exhaustive()
    }
}

impl HandleStore {
    /// Begins a pass over both complete restriction sets without allocating their
    /// union. The same canonical membership ID selects retained and unsaved
    /// handles; unrelated handles keep their state, revision and token mapping.
    ///
    /// The caller must supply current server-owned sets under its domain barrier,
    /// then check cancellation/deadlines between calls to `advance`. Empty sets
    /// legitimately require no invalidation; they are never inferred from missing
    /// persistence. This operation performs no I/O and grants no read authority.
    pub fn begin_security_invalidation<'a>(
        &'a mut self,
        denied: &'a BoundedSet<SourceMembershipId, MAX_SET_ITEMS>,
        purged: &'a BoundedSet<SourceMembershipId, MAX_SET_ITEMS>,
        generation: u64,
    ) -> HandleSecurityInvalidation<'a> {
        HandleSecurityInvalidation {
            store: self,
            denied,
            purged,
            generation,
            cursor: None,
            complete: denied.is_empty() && purged.is_empty(),
            inspected: 0,
            invalidated: 0,
        }
    }
}

impl HandleSecurityInvalidation<'_> {
    /// Advances through at most the configured invalidation batch, capped by the
    /// shared item bound, plus one lookahead for completion. Revisions and totals
    /// are prepared before any mutation; a returned error leaves this step and
    /// cursor unchanged. Previously completed steps remain applied.
    pub fn advance(&mut self) -> Result<HandleInvalidationProgress, HandleError> {
        if self.complete {
            return Ok(HandleInvalidationProgress {
                inspected: 0,
                invalidated: 0,
                complete: true,
            });
        }
        let limit = self.store.policy.max_invalidate_batch.min(MAX_LIST_ITEMS);
        if limit == 0 {
            return Err(HandleError::InvalidPolicy);
        }
        let lower = self.cursor.map_or(Unbounded, Excluded);
        let mut records = self.store.records.range((lower, Unbounded));
        let mut prepared = Vec::new();
        let mut last = self.cursor;
        let mut inspected = 0_usize;
        for (digest, record) in records.by_ref().take(limit) {
            inspected += 1;
            last = Some(*digest);
            let membership = record.target.source_membership_id();
            if (self.denied.contains(&membership) || self.purged.contains(&membership))
                && let Some(revision) = prepare_revision(record, self.generation)?
            {
                prepared.push((*digest, revision));
            }
        }
        let complete = records.next().is_none();
        let invalidated = prepared.len();
        let total_inspected = self.inspected.checked_add(inspected)
            .ok_or(HandleError::InvalidationBudgetExceeded)?;
        let total_invalidated = self.invalidated.checked_add(invalidated)
            .ok_or(HandleError::InvalidationBudgetExceeded)?;
        apply_prepared(self.store, &prepared, self.generation);
        self.cursor = last;
        self.complete = complete;
        self.inspected = total_inspected;
        self.invalidated = total_invalidated;
        Ok(HandleInvalidationProgress { inspected, invalidated, complete })
    }

    /// Returns completion only after the whole pass. No store removal or external
    /// cleanup occurs here, and an incomplete cursor cannot become a receipt.
    pub fn finish(self) -> Result<HandleSecurityInvalidationReceipt, HandleError> {
        if !self.complete {
            return Err(HandleError::InvalidTransition);
        }
        Ok(HandleSecurityInvalidationReceipt {
            generation: self.generation,
            inspected: self.inspected,
            invalidated: self.invalidated,
        })
    }
}

// Shared by exact-scope invalidation and the resumable security pass. Terminal
// records need no new revision and do not consume a mutation slot. Do not lower
// their recorded invalidation generation when older work is replayed.
pub(super) fn prepare_revision(
    record: &HandleRecord,
    generation: u64,
) -> Result<Option<NonZeroRevision>, HandleError> {
    if record.state != HandleRecordState::Active {
        return Ok(None);
    }
    if generation < record.invalidation_generation {
        return Err(HandleError::InvalidTransition);
    }
    record.handle_revision.checked_next()
        .map(Some)
        .map_err(|_| HandleError::InvalidTransition)
}

// Exclusive ownership excludes removal between preparation and application.
// No recoverable work remains here; allocator panic is not transactional rollback.
pub(super) fn apply_prepared(
    store: &mut HandleStore,
    prepared: &[(HandleTokenDigest, NonZeroRevision)],
    generation: u64,
) {
    for (digest, revision) in prepared {
        let record = store.records.get_mut(digest).expect("prepared handle record");
        record.handle_revision = *revision;
        record.invalidation_generation = generation;
        record.state = HandleRecordState::Invalidated;
    }
}

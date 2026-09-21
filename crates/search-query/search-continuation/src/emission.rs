//! Exact candidate-selection and emission accounting for the existing owner.

use std::collections::BTreeSet;

use super::{
    Blake3Digest32, BoundedList, ContinuationError, ContinuationPayload, ContinuationPermit,
    ContinuationStore, EmissionReceipt, InvalidationReason, LifecycleRecordStatus,
    LiveContinuationState, MAX_LIST_ITEMS, UtcTimestamp, unique_fingerprints, validate_permit,
};

// This private selection cannot be supplied or widened by a package consumer.
// Sets originate only from a bounded resume window or a bounded durable binding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum EmissionSelection {
    Ephemeral(BTreeSet<Blake3Digest32>),
    DurableReplan { max_items: usize },
    DurableBatch(BTreeSet<Blake3Digest32>),
    Exhausted,
}

impl EmissionSelection {
    fn permits(
        &self,
        payload: &ContinuationPayload,
        proposed: &BTreeSet<Blake3Digest32>,
    ) -> bool {
        match (self, payload) {
            (Self::Ephemeral(selected), ContinuationPayload::Ephemeral { .. })
            | (Self::DurableBatch(selected), ContinuationPayload::DurableReplan) => {
                proposed.is_subset(selected)
            }
            _ => false,
        }
    }
}

struct PreparedEmission {
    proposed: BTreeSet<Blake3Digest32>,
    next_revision: u64,
    receipt: EmissionReceipt,
}

impl ContinuationStore {
    /// Binds verified durable-replan results to the original finite request.
    ///
    /// The caller must obtain these stable fingerprints from the accepted
    /// planner/executor and source validator under the original fence. This
    /// method freezes their identity; it does not prove external execution.
    /// Pending replan permits cannot commit any candidates. A bound permit
    /// cannot be rebound, and no record, pin, deadline or issued set changes.
    /// Before delivery, call [`Self::revalidate_emission`] with fresh live state.
    pub fn bind_durable_emission(
        &self,
        permit: &ContinuationPermit,
        live: &LiveContinuationState,
        now: &UtcTimestamp,
        selected: &BoundedList<Blake3Digest32, MAX_LIST_ITEMS>,
    ) -> Result<ContinuationPermit, ContinuationError> {
        let stored = self
            .records
            .get(&permit.continuation_id)
            .ok_or(ContinuationError::StalePermit)?;
        validate_permit(stored, permit)?;
        Self::revalidate(stored, live, now)?;
        let EmissionSelection::DurableReplan { max_items } = &permit.selection else {
            return Err(ContinuationError::StalePermit);
        };
        if !matches!(&stored.payload, ContinuationPayload::DurableReplan) {
            return Err(ContinuationError::DurabilityMismatch);
        }
        if selected.is_empty()
            || selected.len() > *max_items
            || selected.len() > self.limits.max_expansion_items
        {
            return Err(ContinuationError::InvalidLimits);
        }
        let selected = unique_fingerprints(selected.iter().copied())?;
        if selected.iter().any(|item| stored.issued.contains(item)) {
            return Err(ContinuationError::DuplicateCandidate);
        }
        let next_total = stored
            .issued
            .len()
            .checked_add(selected.len())
            .ok_or(ContinuationError::ResourceExhausted)?;
        if next_total > self.limits.max_issued_candidates {
            return Err(ContinuationError::ResourceExhausted);
        }
        let mut bound = permit.clone();
        bound.selection = EmissionSelection::DurableBatch(selected);
        Ok(bound)
    }

    /// Rechecks a bound emission immediately before delivery without mutation.
    ///
    /// Checks the exact record incarnation, selected fingerprints, revision,
    /// quotas, current authority, immutable fences, pin/job state and original
    /// expiry. Errors return no receipt and mark nothing issued. The caller
    /// must serialize this checkpoint with its live security/output barrier;
    /// this borrowed check is not an atomic socket-disclosure guarantee.
    pub fn revalidate_emission(
        &self,
        permit: &ContinuationPermit,
        emitted: &BoundedList<Blake3Digest32, MAX_LIST_ITEMS>,
        live: &LiveContinuationState,
        now: &UtcTimestamp,
    ) -> Result<(), ContinuationError> {
        let stored = self
            .records
            .get(&permit.continuation_id)
            .ok_or(ContinuationError::StalePermit)?;
        validate_permit(stored, permit)?;
        Self::revalidate(stored, live, now)?;
        self.prepare_emission(permit, emitted).map(|_| ())
    }

    /// Marks selected fingerprints issued only after successful client emission.
    ///
    /// A nonempty subset of the bound selection may be acknowledged. One
    /// successful commit invalidates every permit for that record revision,
    /// including clones and competing selections. Pending durable replans and
    /// exhausted windows are not emission permits. This is post-delivery
    /// accounting, not live authorization: call [`Self::revalidate_emission`]
    /// before delivery, and never infer successful output from this receipt.
    pub fn commit_emission(
        &mut self,
        permit: &ContinuationPermit,
        emitted: &BoundedList<Blake3Digest32, MAX_LIST_ITEMS>,
    ) -> Result<EmissionReceipt, ContinuationError> {
        let prepared = self.prepare_emission(permit, emitted)?;
        // Exclusive borrowing prevents changes since preparation. Every
        // recoverable failure, including receipt construction, precedes writes.
        let stored = self
            .records
            .get_mut(&permit.continuation_id)
            .ok_or(ContinuationError::StalePermit)?;
        stored.issued.extend(prepared.proposed);
        stored.revision = prepared.next_revision;
        if prepared.receipt.completed {
            stored.set_status(LifecycleRecordStatus::Revoked);
            stored.terminal_reason = Some(InvalidationReason::Completed);
        }
        Ok(prepared.receipt)
    }

    fn prepare_emission(
        &self,
        permit: &ContinuationPermit,
        emitted: &BoundedList<Blake3Digest32, MAX_LIST_ITEMS>,
    ) -> Result<PreparedEmission, ContinuationError> {
        if emitted.is_empty() || emitted.len() > self.limits.max_expansion_items {
            return Err(ContinuationError::InvalidLimits);
        }
        let proposed = unique_fingerprints(emitted.iter().copied())?;
        let stored = self
            .records
            .get(&permit.continuation_id)
            .ok_or(ContinuationError::StalePermit)?;
        validate_permit(stored, permit)?;
        if proposed.iter().any(|value| stored.issued.contains(value)) {
            return Err(ContinuationError::DuplicateCandidate);
        }
        if !permit.selection.permits(&stored.payload, &proposed) {
            return Err(ContinuationError::StalePermit);
        }
        let next_total = stored
            .issued
            .len()
            .checked_add(proposed.len())
            .ok_or(ContinuationError::ResourceExhausted)?;
        if next_total > self.limits.max_issued_candidates {
            return Err(ContinuationError::ResourceExhausted);
        }
        let next_revision = stored.next_revision()?;
        let completed = match &stored.payload {
            ContinuationPayload::Ephemeral { candidates, .. } => candidates.iter().all(|item| {
                stored.issued.contains(&item.fingerprint) || proposed.contains(&item.fingerprint)
            }),
            ContinuationPayload::DurableReplan => false,
        };
        let receipt = EmissionReceipt {
            continuation_id: stored.id(),
            emitted_count: emitted.len(),
            issued_total: next_total,
            completed,
            cleanup_effect: completed.then(|| stored.cleanup_effect()),
        };
        Ok(PreparedEmission { proposed, next_revision, receipt })
    }
}

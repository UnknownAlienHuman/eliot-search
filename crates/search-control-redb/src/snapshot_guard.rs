//! Publication fencing for the shared in-memory and disk-backed control APIs.
//!
//! The existing publisher retains the sole snapshot pointer. A disk publication
//! failure additionally suspends admission until fresh verified disk readback.

use std::sync::Arc;

use crate::{ControlCommitReceipt, ControlError, ControlJournal, ControlKey,
    ControlSnapshot, ControlValue, JournalIdentity, MutationId, SnapshotPublishReceipt};
use crate::reference;

#[path = "snapshot_guard_disk.rs"]
mod disk;

/// Process-local immutable snapshot publisher with monotone identity fences.
///
/// A disk-bound publisher accepts updates only from the journal's readback path.
/// `current()` is an admission view, not authorization: already returned Arcs and
/// cloned publishers are historical views, not live subscriptions to this owner.
#[derive(Clone, Debug, Default)]
pub struct ControlSnapshotPublisher {
    inner: reference::ControlSnapshotPublisher,
    current_operation: Option<MutationId>,
    disk: Option<disk::DiskPublicationState>,
}

impl ControlSnapshotPublisher {
    /// Creates an empty publisher without granting source or owner authority.
    #[must_use]
    pub const fn new() -> Self {
        Self { inner: reference::ControlSnapshotPublisher::new(), current_operation: None, disk: None }
    }

    /// Current snapshot for admission, or `None` while disk publication is unresolved.
    /// The last pointer is retained privately to enforce monotonic recovery.
    #[must_use]
    pub fn current(&self) -> Option<Arc<ControlSnapshot>> {
        if self.requires_recovery() { None } else { self.inner.current() }
    }

    /// Whether a failed/incomplete disk publication still blocks snapshot admission.
    #[must_use]
    pub fn requires_recovery(&self) -> bool {
        self.disk.as_ref().is_some_and(|state| state.suspended)
    }

    /// Publishes a caller-verified snapshot to an unbound low-level publisher.
    ///
    /// # Errors
    /// Rejects disk-bound publishers (use the actual journal), malformed receipts,
    /// foreign identities, regressions and equal-generation content/operation changes.
    /// A rejected low-level call leaves the prior pointer and admission state intact.
    pub fn publish_snapshot_after_commit(
        &mut self,
        commit: &ControlCommitReceipt,
        snapshot: ControlSnapshot,
    ) -> Result<SnapshotPublishReceipt, ControlError> {
        if self.disk.is_some() { return Err(ControlError::SnapshotPublicationFailed); }
        self.validate_publication(commit, &snapshot)?;
        let receipt = self.inner.publish_snapshot_after_commit(commit, snapshot)?;
        self.current_operation = Some(commit.operation_id);
        Ok(receipt)
    }

    /// Recovers a model journal's snapshot into an unbound low-level publisher.
    /// Disk journals cannot be replaced by a reference model, even after failure.
    ///
    /// # Errors
    /// Rejects disk bindings, unavailable/inconsistent models and identity regressions.
    pub fn recover_snapshot_publication(
        &mut self,
        journal: &ControlJournal,
    ) -> Result<SnapshotPublishReceipt, ControlError> {
        if self.disk.is_some() { return Err(ControlError::SnapshotPublicationFailed); }
        // ControlJournal mutations require &mut; no mutation can interleave with
        // these two read-only observations through this shared reference.
        let next = journal.read_snapshot()?;
        self.validate_next(next.identity, next.generation, &next.records)?;
        let same_generation = self.inner.current().is_some_and(|current| current.generation == next.generation);
        let receipt = self.inner.recover_snapshot_publication(journal)?;
        if !same_generation { self.current_operation = None; }
        Ok(receipt)
    }

    fn validate_publication(
        &self,
        commit: &ControlCommitReceipt,
        snapshot: &ControlSnapshot,
    ) -> Result<(), ControlError> {
        if commit.before_generation.checked_add(1) != Some(commit.after_generation)
            || snapshot.generation != commit.after_generation
            || commit.changed_keys.is_empty()
            || commit.changed_keys.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(ControlError::SnapshotPublicationFailed);
        }
        self.validate_next(snapshot.identity, snapshot.generation, &snapshot.records)?;
        if self.inner.current().is_some_and(|current| current.generation == snapshot.generation)
            && self.current_operation.is_some_and(|operation| operation != commit.operation_id)
        {
            return Err(ControlError::SnapshotPublicationFailed);
        }
        Ok(())
    }

    fn validate_next(
        &self,
        identity: JournalIdentity,
        generation: u64,
        records: &[(ControlKey, ControlValue)],
    ) -> Result<(), ControlError> {
        identity.validate()?;
        if records.windows(2).any(|pair| pair[0].0 >= pair[1].0)
            || records.iter().any(|(key, value)| key.as_bytes().is_empty() || value.is_empty())
            || (generation == 0 && !records.is_empty())
        {
            return Err(ControlError::SnapshotPublicationFailed);
        }
        // Never validate against the admission view: it is hidden during recovery.
        let Some(current) = self.inner.current() else { return Ok(()); };
        let stable_current = JournalIdentity { owner_epoch: identity.owner_epoch, ..current.identity };
        if stable_current != identity { return Err(ControlError::IdentityMismatch); }
        if identity.owner_epoch.get() < current.identity.owner_epoch.get()
            || generation < current.generation
            || (generation == current.generation && current.records.as_slice() != records)
        {
            return Err(ControlError::SnapshotPublicationFailed);
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "snapshot_guard_tests.rs"]
mod tests;

//! Publication fencing for the shared in-memory and disk-backed control APIs.
//!
//! The existing publisher retains the sole snapshot pointer. This boundary
//! rejects rollback and identity substitution before that pointer can change.

use std::sync::Arc;

use crate::{ControlCommitReceipt, ControlError, ControlJournal, ControlKey,
    ControlSnapshot, ControlValue, JournalIdentity, MutationId, SnapshotPublishReceipt};
use crate::reference;

/// Process-local immutable snapshot publisher with monotone identity fences.
///
/// The caller must first obtain exact committed readback. This publisher checks
/// ordering and consistency, not storage authenticity or client authorization.
/// A publisher belongs to one immutable installation/root/path/schema binding.
#[derive(Clone, Debug, Default)]
pub struct ControlSnapshotPublisher {
    inner: reference::ControlSnapshotPublisher,
    current_operation: Option<MutationId>,
}

impl ControlSnapshotPublisher {
    /// Creates an empty publisher without granting source or owner authority.
    #[must_use]
    pub const fn new() -> Self {
        Self { inner: reference::ControlSnapshotPublisher::new(), current_operation: None }
    }

    /// Returns the current immutable snapshot. A rejected publication preserves it.
    #[must_use]
    pub fn current(&self) -> Option<Arc<ControlSnapshot>> { self.inner.current() }

    /// Publishes a caller-verified snapshot without rolling back generation or owner.
    ///
    /// # Errors
    /// Rejects malformed receipts, foreign immutable bindings, older generations
    /// or owner epochs, and conflicting content/operation at the same generation.
    pub fn publish_snapshot_after_commit(
        &mut self,
        commit: &ControlCommitReceipt,
        snapshot: ControlSnapshot,
    ) -> Result<SnapshotPublishReceipt, ControlError> {
        if commit.before_generation.checked_add(1) != Some(commit.after_generation)
            || snapshot.generation != commit.after_generation
            || commit.changed_keys.is_empty()
            || commit.changed_keys.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(ControlError::SnapshotPublicationFailed);
        }
        self.validate_next(snapshot.identity, snapshot.generation, &snapshot.records)?;
        if self.current().is_some_and(|current| current.generation == snapshot.generation)
            && self.current_operation.is_some_and(|operation| operation != commit.operation_id)
        {
            return Err(ControlError::SnapshotPublicationFailed);
        }
        let receipt = self.inner.publish_snapshot_after_commit(commit, snapshot)?;
        self.current_operation = Some(commit.operation_id);
        Ok(receipt)
    }

    /// Recovers a model journal's current snapshot without replaying a mutation.
    /// Disk journals use their own exact-readback method and the same publication fence.
    ///
    /// # Errors
    /// Rejects unavailable or inconsistent journals and any identity/ordering regression.
    pub fn recover_snapshot_publication(
        &mut self,
        journal: &ControlJournal,
    ) -> Result<SnapshotPublishReceipt, ControlError> {
        // ControlJournal mutations require &mut; no mutation can interleave with
        // these two read-only observations through this shared reference.
        let next = journal.read_snapshot()?;
        self.validate_next(next.identity, next.generation, &next.records)?;
        let same_generation = self.current().is_some_and(|current| current.generation == next.generation);
        let receipt = self.inner.recover_snapshot_publication(journal)?;
        if !same_generation { self.current_operation = None; }
        Ok(receipt)
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
        let Some(current) = self.current() else { return Ok(()); };
        // A verified successor may advance its owner epoch, but not any stable
        // journal identity field. A new installation needs a new publisher.
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

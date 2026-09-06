//! Admission state for the existing snapshot pointer; never a second journal.

use super::{ControlCommitReceipt, ControlError, ControlSnapshot,
    ControlSnapshotPublisher, JournalIdentity, SnapshotPublishReceipt};

#[derive(Clone, Debug)]
pub(super) struct DiskPublicationState {
    identity: JournalIdentity,
    observed_generation: u64,
    pub(super) suspended: bool,
}

impl ControlSnapshotPublisher {
    // Read-only diagnostic binding. This does not publish, recover or grant admission.
    pub(crate) fn diagnostic_disk_identity(&self) -> Option<JournalIdentity> {
        self.disk.as_ref().map(|state| state.identity)
    }

    pub(crate) fn begin_disk_publication(&mut self, identity: JournalIdentity) -> Result<(), ControlError> {
        identity.validate()?;
        // A foreign/stale owner must not poison another publisher or clear its fence.
        if let Some(state) = &self.disk { require_successor(state.identity, identity)?; }
        let previous = self.inner.current();
        if let Some(current) = &previous { require_successor(current.identity, identity)?; }
        match &mut self.disk {
            Some(state) => {
                state.identity = identity;
                state.suspended = true;
            }
            None => self.disk = Some(DiskPublicationState {
                identity,
                observed_generation: previous.map_or(0, |current| current.generation),
                suspended: true,
            }),
        }
        Ok(())
    }

    pub(crate) fn observe_disk_generation(&mut self, generation: u64) -> Result<(), ControlError> {
        let state = self.disk.as_mut().ok_or(ControlError::SnapshotPublicationFailed)?;
        if !state.suspended || generation < state.observed_generation {
            return Err(ControlError::SnapshotPublicationFailed);
        }
        // This value comes from a verified journal header, never the caller's receipt.
        state.observed_generation = generation;
        Ok(())
    }

    pub(crate) fn publish_verified_disk_snapshot(
        &mut self,
        commit: &ControlCommitReceipt,
        snapshot: ControlSnapshot,
        before_publish: impl FnOnce() -> Result<(), ControlError>,
    ) -> Result<SnapshotPublishReceipt, ControlError> {
        self.validate_publication(commit, &snapshot)?;
        let state = self.disk.as_mut().ok_or(ControlError::SnapshotPublicationFailed)?;
        if !state.suspended || snapshot.identity != state.identity
            || snapshot.generation != state.observed_generation
        {
            return Err(ControlError::SnapshotPublicationFailed);
        }
        // All scans, content comparison and fallible preparation precede the last
        // cooperative checkpoint. A later cancellation cannot undo the pointer swap.
        before_publish()?;
        let receipt = self.inner.publish_snapshot_after_commit(commit, snapshot)?;
        self.current_operation = Some(commit.operation_id);
        state.suspended = false;
        Ok(receipt)
    }

    pub(crate) fn finish_verified_empty_disk(
        &mut self,
        identity: JournalIdentity,
        before_publish: impl FnOnce() -> Result<(), ControlError>,
    ) -> Result<(), ControlError> {
        self.validate_next(identity, 0, &[])?;
        let state = self.disk.as_mut().ok_or(ControlError::SnapshotPublicationFailed)?;
        if !state.suspended || state.identity != identity || state.observed_generation != 0 {
            return Err(ControlError::SnapshotPublicationFailed);
        }
        before_publish()?;
        // There is no mutation receipt for initialized generation zero. Do not
        // invent one or install an empty snapshot over a later committed view.
        state.suspended = false;
        Ok(())
    }
}

fn require_successor(previous: JournalIdentity, next: JournalIdentity) -> Result<(), ControlError> {
    if (JournalIdentity { owner_epoch: next.owner_epoch, ..previous }) != next {
        return Err(ControlError::IdentityMismatch);
    }
    if next.owner_epoch.get() < previous.owner_epoch.get() {
        return Err(ControlError::SnapshotPublicationFailed);
    }
    Ok(())
}

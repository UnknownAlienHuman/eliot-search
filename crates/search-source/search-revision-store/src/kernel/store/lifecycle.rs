//! Purge tombstone installation and exact lifecycle-authorized deletion.

use super::super::error::RevisionStoreError;
use super::super::model::{
    LifecycleDeletionPlan, ObjectDeletionReceipt, PurgeTombstone,
    PurgeTombstoneReceipt, RevisionState,
};
use super::support::tombstone_receipt;
use super::RevisionStore;

impl RevisionStore {
    /// Installs a purge tombstone at the admission boundary.
    ///
    /// The same tombstone reinstalls idempotently. A conflicting receipt for
    /// the same scope and generation, or any operation-identity reuse with a
    /// different payload, fails closed. There is no removal operation.
    pub fn install_purge_tombstone(
        &mut self,
        tombstone: PurgeTombstone,
    ) -> Result<PurgeTombstoneReceipt, RevisionStoreError> {
        if self
            .operations
            .iter()
            .any(|(operation_id, _, _)| operation_id == tombstone.operation.operation_id())
            || self
                .deletions
                .iter()
                .any(|(operation_id, _, _)| operation_id == tombstone.operation.operation_id())
        {
            return Err(RevisionStoreError::OperationConflict);
        }
        if let Some(existing) = self.tombstones.iter().find(|candidate| {
            candidate.operation.operation_id() == tombstone.operation.operation_id()
        }) {
            if existing.operation.request_digest() != tombstone.operation.request_digest()
                || *existing != tombstone
            {
                return Err(RevisionStoreError::OperationConflict);
            }
            return Ok(tombstone_receipt(existing, true));
        }
        if let Some(existing) = self.tombstones.iter().find(|candidate| {
            candidate.scope == tombstone.scope && candidate.generation == tombstone.generation
        }) {
            if existing.tombstone_receipt != tombstone.tombstone_receipt {
                return Err(RevisionStoreError::RevisionConflict);
            }
            return Ok(tombstone_receipt(existing, true));
        }
        if self.operation_count() >= self.limits.max_operations {
            return Err(RevisionStoreError::CapacityExceeded);
        }
        self.tombstones.push(tombstone.clone());
        Ok(tombstone_receipt(&tombstone, false))
    }

    /// Executes one exact bounded deletion under a lifecycle-owner plan.
    ///
    /// Only an `Active` record matching the exact target key and storage
    /// object is removed, and only with the plan receipt the lifecycle owner
    /// issued. Pending or unknown writes are never reported as deleted;
    /// the same plan replays idempotently while any operation reuse with a
    /// different plan conflicts.
    pub fn apply_exact_object_deletion(
        &mut self,
        plan: LifecycleDeletionPlan,
    ) -> Result<ObjectDeletionReceipt, RevisionStoreError> {
        if self
            .operations
            .iter()
            .any(|(operation_id, _, _)| operation_id == plan.operation.operation_id())
            || self.tombstones.iter().any(|tombstone| {
                tombstone.operation.operation_id() == plan.operation.operation_id()
            })
        {
            return Err(RevisionStoreError::OperationConflict);
        }
        if let Some((_, digest, receipt)) = self
            .deletions
            .iter()
            .find(|(operation_id, _, _)| operation_id == plan.operation.operation_id())
        {
            let bound = *digest == plan.operation.request_digest()
                && receipt.target == plan.target
                && receipt.target_storage_object_id == plan.target_storage_object_id
                && receipt.authority == plan.authority
                && receipt.plan_receipt == plan.plan_receipt;
            if !bound {
                return Err(RevisionStoreError::OperationConflict);
            }
            let mut replay = receipt.clone();
            replay.replayed = true;
            return Ok(replay);
        }
        if self.operation_count() >= self.limits.max_operations {
            return Err(RevisionStoreError::CapacityExceeded);
        }
        let record = match self.states.get(&plan.target) {
            None => return Err(RevisionStoreError::RevisionNotFound),
            Some(RevisionState::Pending(_) | RevisionState::OutcomeUnknown(_)) => {
                return Err(RevisionStoreError::OutcomeUnknown);
            }
            Some(RevisionState::Quarantined { .. }) => {
                return Err(RevisionStoreError::Quarantined);
            }
            Some(RevisionState::Active(record)) => record.clone(),
        };
        if record.key != plan.target || record.storage_object_id != plan.target_storage_object_id {
            return Err(RevisionStoreError::DeletionNotAuthorized);
        }
        self.states.remove(&plan.target);
        let receipt = ObjectDeletionReceipt {
            target: plan.target.clone(),
            target_storage_object_id: plan.target_storage_object_id.clone(),
            authority: plan.authority,
            plan_receipt: plan.plan_receipt.clone(),
            operation: plan.operation.clone(),
            replayed: false,
        };
        self.deletions.push((
            plan.operation.operation_id().clone(),
            plan.operation.request_digest(),
            receipt.clone(),
        ));
        Ok(receipt)
    }
}

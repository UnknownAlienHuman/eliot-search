//! Single-flight publication coordinator and linearization state machine.

use std::collections::BTreeSet;

mod abort;
mod restore;
pub use abort::{
    AbortControlCommitObservation, AbortFinalizationRequest, AbortedPublicationResolution,
};
pub use restore::PublicationRestoreInput;

use search_contracts::{Blake3Digest32, Epoch, OpaqueId, ReceiptRef};
use search_point_identity::PointId128;
use search_projection_planner::{ManifestDiff, ProjectionManifest, diff_manifests};

use crate::{
    AbandonFence, ClosureReceipt, CompensationPlan, CompensationReceipt, ControlCommitObservation,
    PreparedPublication, PublicationError, PublicationGuards, ReadbackVerified, RestorationReceipt,
    RetiredManifest, SnapshotPublishReceipt, StageReceipt, VisibleCommitReceipt,
};

/// Maximum exact points in one publication transaction.
pub const DEFAULT_MAX_PUBLICATION_POINTS: usize = 1_000_000;

/// Closed publication transaction phase.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublicationPhase {
    Prepared,
    IntentDurable,
    NewPointsAcknowledged,
    OldPointsClosedAcknowledged,
    ReadbackVerified,
    ControlCommitted,
    SnapshotPublished,
    Compensating,
    Aborted,
    PublicationBlocked,
}

/// Durable intent record prepared for an external control journal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DurableIntent {
    pub transaction_id: OpaqueId,
    pub target_epoch: Epoch,
    pub old_manifest_digest: Option<Blake3Digest32>,
    pub new_manifest_digest: Blake3Digest32,
    pub guards: PublicationGuards,
    pub persist_operation_id: OpaqueId,
    pub intent_receipt: ReceiptRef,
}

/// Complete in-flight publication transaction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicationTransaction {
    pub prepared: PreparedPublication,
    /// The actual visible epoch before reservation, which may precede skipped epochs.
    pub previous_visible_epoch: Epoch,
    pub target_epoch: Epoch,
    pub phase: PublicationPhase,
    pub durable_intent: Option<DurableIntent>,
    pub stage_receipt: Option<StageReceipt>,
    pub closure_receipt: Option<ClosureReceipt>,
    pub verified: Option<ReadbackVerified>,
    pub visible_commit: Option<VisibleCommitReceipt>,
    pub snapshot_receipt: Option<SnapshotPublishReceipt>,
    // Retain the caller's finite bound for recovery of this exact transaction.
    pub(crate) max_points: usize,
}

impl PublicationTransaction {
    #[must_use]
    pub const fn transaction_id(&self) -> &OpaqueId {
        &self.prepared.transaction_id
    }
}

/// Process-local single-flight coordinator. Deliberately not cloneable.
///
/// Its epoch floor is reconstructed from authoritative control history by the
/// composition owner. It is not another persistent ledger or ownership lease.
#[derive(Debug)]
pub struct PublicationCoordinator {
    visible_epoch: Epoch,
    last_reserved_epoch: Epoch,
    current_manifest: Option<ProjectionManifest>,
    current_manifest_digest: Option<Blake3Digest32>,
    active: Option<PublicationTransaction>,
    abort_finalization: abort::AbortFinalizationProgress,
    max_points: usize,
}

impl PublicationCoordinator {
    /// Creates a coordinator from an explicitly resolved control checkpoint.
    ///
    /// `last_reserved_epoch` includes aborted/abandoned reservations and must
    /// come from verified durable history, not be inferred from `VisibleEpoch`.
    /// A caller must resolve any outstanding intent before using this constructor.
    /// Initial empty history supplies zero for both epochs. These inputs are not
    /// evidence of root ownership, successful recovery or a qualified backend.
    pub fn new(
        visible_epoch: Epoch,
        last_reserved_epoch: Epoch,
        current_manifest: Option<ProjectionManifest>,
        current_manifest_digest: Option<Blake3Digest32>,
        max_points: usize,
    ) -> Result<Self, PublicationError> {
        if max_points == 0
            || last_reserved_epoch < visible_epoch
            || current_manifest.is_some() != current_manifest_digest.is_some()
        {
            return Err(PublicationError::InvalidPreparedPublication);
        }
        if let Some(manifest) = &current_manifest {
            validate_manifest(manifest, max_points)?;
        }
        Ok(Self {
            visible_epoch,
            last_reserved_epoch,
            current_manifest,
            current_manifest_digest,
            active: None,
            abort_finalization: abort::AbortFinalizationProgress::default(),
            max_points,
        })
    }

    #[must_use]
    pub const fn visible_epoch(&self) -> Epoch {
        self.visible_epoch
    }

    /// Highest consumed reservation in this coordinator, including aborted work.
    #[must_use]
    pub const fn last_reserved_epoch(&self) -> Epoch {
        self.last_reserved_epoch
    }

    #[must_use]
    pub const fn active(&self) -> Option<&PublicationTransaction> {
        self.active.as_ref()
    }

    #[must_use]
    pub const fn current_manifest(&self) -> Option<&ProjectionManifest> {
        self.current_manifest.as_ref()
    }

    /// Reserves the next unused epoch for one prepared transaction.
    /// A reservation is never returned to the pool, even after compensation.
    /// A physical point ID shared with the current manifest must retain its full
    /// immutable entry. Changed entries need distinct identities from the point
    /// owner; the coordinator never invents replacement IDs or mutates manifests.
    pub fn submit(&mut self, prepared: PreparedPublication) -> Result<Epoch, PublicationError> {
        if self.active.is_some() {
            return Err(PublicationError::PublicationBusy);
        }
        validate_manifest(&prepared.new_manifest, self.max_points)?;
        if let Some(old) = &prepared.old_manifest {
            validate_manifest(old, self.max_points)?;
        }
        if prepared.old_manifest != self.current_manifest
            || prepared.old_manifest_digest != self.current_manifest_digest
        {
            return Err(PublicationError::InvalidPreparedPublication);
        }
        if changed_point_id_is_reused(prepared.old_manifest.as_ref(), &prepared.new_manifest) {
            // Upserting a changed value under an old physical ID would destroy
            // the old epoch before commit. Closing that ID would also close the
            // newly staged point. Reject before consuming an epoch or slot.
            return Err(PublicationError::InvalidPreparedPublication);
        }
        let target_epoch = self
            .last_reserved_epoch
            .checked_next()
            .map_err(|_| PublicationError::ContractExhausted)?;
        self.active = Some(PublicationTransaction {
            prepared,
            previous_visible_epoch: self.visible_epoch,
            target_epoch,
            phase: PublicationPhase::Prepared,
            durable_intent: None,
            stage_receipt: None,
            closure_receipt: None,
            verified: None,
            visible_commit: None,
            snapshot_receipt: None,
            max_points: self.max_points,
        });
        self.last_reserved_epoch = target_epoch;
        Ok(target_epoch)
    }

    /// Accepts a durable intent record for the active transaction.
    pub fn persist_intent(
        &mut self,
        persist_operation_id: OpaqueId,
        intent_receipt: ReceiptRef,
    ) -> Result<DurableIntent, PublicationError> {
        let transaction = self.active_mut(PublicationPhase::Prepared)?;
        let intent = DurableIntent {
            transaction_id: transaction.prepared.transaction_id.clone(),
            target_epoch: transaction.target_epoch,
            old_manifest_digest: transaction.prepared.old_manifest_digest,
            new_manifest_digest: transaction.prepared.new_manifest_digest,
            guards: transaction.prepared.guards,
            persist_operation_id,
            intent_receipt,
        };
        transaction.durable_intent = Some(intent.clone());
        transaction.phase = PublicationPhase::IntentDurable;
        Ok(intent)
    }

    /// Accepts exact new-point staging and readback.
    pub fn stage_new_points(&mut self, receipt: StageReceipt) -> Result<(), PublicationError> {
        let transaction = self.active_mut(PublicationPhase::IntentDurable)?;
        verify_transaction_identity(transaction, &receipt.transaction_id, receipt.target_epoch)?;
        let expected = manifest_diff(transaction)?.create;
        let expected_ids = entry_ids(&expected);
        if !receipt.missing_ids.is_empty()
            || !receipt.unexpected_ids.is_empty()
            || receipt.staged_ids != expected_ids
        {
            return Err(PublicationError::StageReadbackMismatch);
        }
        transaction.stage_receipt = Some(receipt);
        transaction.phase = PublicationPhase::NewPointsAcknowledged;
        Ok(())
    }

    /// Accepts exact closure of retired point IDs.
    pub fn close_old_points(&mut self, receipt: ClosureReceipt) -> Result<(), PublicationError> {
        let transaction = self.active_mut(PublicationPhase::NewPointsAcknowledged)?;
        verify_transaction_identity(transaction, &receipt.transaction_id, receipt.target_epoch)?;
        let expected = manifest_diff(transaction)?.retire;
        let expected_ids = entry_ids(&expected);
        if !receipt.missing_ids.is_empty()
            || !receipt.unexpected_ids.is_empty()
            || receipt.closed_ids != expected_ids
        {
            return Err(PublicationError::ClosureReadbackMismatch);
        }
        transaction.closure_receipt = Some(receipt);
        transaction.phase = PublicationPhase::OldPointsClosedAcknowledged;
        Ok(())
    }

    /// Verifies staged and retired exact readback as one immutable proof.
    pub fn verify_readback(
        &mut self,
        staged_digest: Blake3Digest32,
        closure_digest: Blake3Digest32,
        retired_manifest_digest: Option<Blake3Digest32>,
    ) -> Result<ReadbackVerified, PublicationError> {
        let transaction = self.active_mut(PublicationPhase::OldPointsClosedAcknowledged)?;
        let stage = transaction
            .stage_receipt
            .as_ref()
            .ok_or(PublicationError::StageReadbackMismatch)?;
        let closure = transaction
            .closure_receipt
            .as_ref()
            .ok_or(PublicationError::ClosureReadbackMismatch)?;
        if stage.readback_digest != staged_digest || closure.readback_digest != closure_digest {
            return Err(PublicationError::StageReadbackMismatch);
        }
        let has_retired = !manifest_diff(transaction)?.retire.is_empty();
        if has_retired != retired_manifest_digest.is_some() {
            return Err(PublicationError::ClosureReadbackMismatch);
        }
        let verified = ReadbackVerified {
            transaction_id: transaction.prepared.transaction_id.clone(),
            target_epoch: transaction.target_epoch,
            staged_readback_digest: staged_digest,
            closure_readback_digest: closure_digest,
            new_manifest_digest: transaction.prepared.new_manifest_digest,
            retired_manifest_digest,
        };
        transaction.verified = Some(verified.clone());
        transaction.phase = PublicationPhase::ReadbackVerified;
        Ok(verified)
    }

    /// Accepts the guarded control compare-and-swap that linearizes visibility.
    pub fn commit_visible_epoch(
        &mut self,
        observation: ControlCommitObservation,
    ) -> Result<VisibleCommitReceipt, PublicationError> {
        let before_visible_epoch = self.visible_epoch;
        let transaction = self.active_mut(PublicationPhase::ReadbackVerified)?;
        if observation.before_visible_epoch != before_visible_epoch
            || observation.after_visible_epoch != transaction.target_epoch
            || observation.control_generation == 0
        {
            return Err(PublicationError::ControlConflict);
        }
        if observation.observed_guards != transaction.prepared.guards {
            return Err(PublicationError::GuardMismatch);
        }
        let verified = transaction
            .verified
            .as_ref()
            .ok_or(PublicationError::StageReadbackMismatch)?;
        let receipt = VisibleCommitReceipt {
            transaction_id: transaction.prepared.transaction_id.clone(),
            visible_epoch: transaction.target_epoch,
            visible_manifest_digest: transaction.prepared.new_manifest_digest,
            retired_manifest_digest: verified.retired_manifest_digest,
            control_generation: observation.control_generation,
            control_state_digest: observation.control_state_digest,
        };
        transaction.visible_commit = Some(receipt.clone());
        transaction.phase = PublicationPhase::ControlCommitted;
        self.visible_epoch = observation.after_visible_epoch;
        Ok(receipt)
    }

    /// Accepts immutable in-memory snapshot publication after control commit.
    pub fn publish_control_snapshot(
        &mut self,
        receipt: SnapshotPublishReceipt,
    ) -> Result<(), PublicationError> {
        let transaction = self.active_mut(PublicationPhase::ControlCommitted)?;
        let commit = transaction
            .visible_commit
            .as_ref()
            .ok_or(PublicationError::ControlConflict)?;
        if receipt.transaction_id != commit.transaction_id
            || receipt.visible_epoch != commit.visible_epoch
            || receipt.control_generation != commit.control_generation
        {
            return Err(PublicationError::SnapshotPublicationFailed);
        }
        transaction.snapshot_receipt = Some(receipt);
        transaction.phase = PublicationPhase::SnapshotPublished;
        Ok(())
    }

    /// Emits exact retired IDs only after visible control and snapshot commit.
    pub fn emit_retired_manifest(
        &self,
        manifest_digest: Blake3Digest32,
        publication_receipt: ReceiptRef,
    ) -> Result<Option<RetiredManifest>, PublicationError> {
        let transaction = self.active_at(PublicationPhase::SnapshotPublished)?;
        let retired = manifest_diff(transaction)?.retire;
        if retired.is_empty() {
            return Ok(None);
        }
        let verified = transaction
            .verified
            .as_ref()
            .ok_or(PublicationError::ClosureReadbackMismatch)?;
        if verified.retired_manifest_digest != Some(manifest_digest) {
            return Err(PublicationError::ClosureReadbackMismatch);
        }
        Ok(Some(RetiredManifest {
            collection_generation_id: transaction.prepared.collection_generation_id,
            retirement_epoch_exclusive: transaction.target_epoch,
            point_ids: entry_ids(&retired),
            manifest_digest,
            publication_receipt,
        }))
    }

    /// Completes the transaction and installs the new current manifest.
    pub fn complete(&mut self) -> Result<VisibleCommitReceipt, PublicationError> {
        // Validate before taking the sole active slot: an error must not release
        // serialization or discard the transaction needed by recovery.
        let receipt = self
            .active_at(PublicationPhase::SnapshotPublished)?
            .visible_commit
            .clone()
            .ok_or(PublicationError::ControlConflict)?;
        let transaction = self
            .active
            .take()
            .ok_or(PublicationError::InvalidTransition)?;
        self.current_manifest = Some(transaction.prepared.new_manifest);
        self.current_manifest_digest = Some(transaction.prepared.new_manifest_digest);
        Ok(receipt)
    }

    /// Begins exact compensation and returns both mutation directions.
    /// Retry while COMPENSATING returns the same complete plan.
    pub fn begin_compensation_plan(&mut self) -> Result<CompensationPlan, PublicationError> {
        let transaction = self
            .active
            .as_mut()
            .ok_or(PublicationError::InvalidTransition)?;
        if !matches!(
            transaction.phase,
            PublicationPhase::IntentDurable
                | PublicationPhase::NewPointsAcknowledged
                | PublicationPhase::OldPointsClosedAcknowledged
                | PublicationPhase::ReadbackVerified
                | PublicationPhase::Compensating
        ) {
            return Err(PublicationError::InvalidTransition);
        }
        let difference = manifest_diff(transaction)?;
        let plan = CompensationPlan {
            transaction_id: transaction.prepared.transaction_id.clone(),
            target_epoch: transaction.target_epoch,
            staged_ids: entry_ids(&difference.create),
            closed_ids: entry_ids(&difference.retire),
        };
        transaction.phase = PublicationPhase::Compensating;
        Ok(plan)
    }

    /// Compatibility projection for create-only callers. With retired points,
    /// `compensate_exact` refuses completion; use the full plan and restoration.
    pub fn begin_compensation(&mut self) -> Result<Vec<PointId128>, PublicationError> {
        Ok(self.begin_compensation_plan()?.staged_ids)
    }

    /// Completes compensation only when there were no old closures to restore.
    pub fn compensate_exact(
        &mut self,
        receipt: CompensationReceipt,
    ) -> Result<(), PublicationError> {
        self.finish_compensation(receipt, None)
    }

    /// Completes both exact staged-point removal and old-state restoration.
    /// Receipts must come from actual verified adapter readback. A timeout, a
    /// reference string or a list of IDs alone is not such evidence. When a point
    /// ID appears in both lists, restoration must follow removal and verify the
    /// original payload/vector state as well as its prior visibility bounds.
    pub fn compensate_and_restore(
        &mut self,
        receipt: CompensationReceipt,
        restoration: RestorationReceipt,
    ) -> Result<(), PublicationError> {
        self.finish_compensation(receipt, Some(restoration))
    }

    fn finish_compensation(
        &mut self,
        receipt: CompensationReceipt,
        restoration: Option<RestorationReceipt>,
    ) -> Result<(), PublicationError> {
        let transaction = self.active_mut(PublicationPhase::Compensating)?;
        let difference = manifest_diff(transaction)?;
        if receipt.transaction_id != transaction.prepared.transaction_id
            || receipt.target_epoch != transaction.target_epoch
            || !receipt.remaining_ids.is_empty()
            || receipt.compensated_ids != entry_ids(&difference.create)
        {
            return Err(PublicationError::CompensationIncomplete);
        }
        match &restoration {
            Some(restored)
                if restored.transaction_id == transaction.prepared.transaction_id
                    && restored.target_epoch == transaction.target_epoch
                    && restored.remaining_ids.is_empty()
                    && restored.restored_ids == entry_ids(&difference.retire) => {}
            None if difference.retire.is_empty() => {}
            _ => return Err(PublicationError::CompensationIncomplete),
        }
        transaction.phase = PublicationPhase::Aborted;
        // Keep only exact evidence references; large ID lists remain in manifests.
        self.abort_finalization.resolution = Some(AbortedPublicationResolution::Compensated {
            removal_readback: receipt.readback_receipt,
            restoration_readback: restoration.map(|value| value.readback_receipt),
        });
        Ok(())
    }

    /// Abandons only with complete membership exclusion, not merely point IDs.
    /// The control/access owner must verify and persist the fence before calling.
    pub fn abandon(&mut self, fence: &AbandonFence) -> Result<(), PublicationError> {
        let transaction = self
            .active
            .as_mut()
            .ok_or(PublicationError::InvalidTransition)?;
        if !matches!(
            transaction.phase,
            PublicationPhase::IntentDurable
                | PublicationPhase::NewPointsAcknowledged
                | PublicationPhase::OldPointsClosedAcknowledged
                | PublicationPhase::ReadbackVerified
                | PublicationPhase::Compensating
        ) {
            return Err(PublicationError::InvalidTransition);
        }
        let difference = manifest_diff(transaction)?;
        let affected = difference.create.iter().chain(&difference.retire);
        let point_ids = affected
            .clone()
            .map(|entry| entry.point_id)
            .collect::<BTreeSet<_>>();
        let memberships = affected
            .map(|entry| entry.projection_membership_id.clone())
            .collect::<BTreeSet<_>>();
        if fence.transaction_id != transaction.prepared.transaction_id
            || fence.target_epoch != transaction.target_epoch
            || fence.excluded_point_ids != point_ids
            || fence.excluded_projection_memberships != memberships
        {
            return Err(PublicationError::AbandonFenceMissing);
        }
        transaction.phase = PublicationPhase::Aborted;
        self.abort_finalization.resolution = Some(AbortedPublicationResolution::Excluded {
            exclusion_receipt: fence.exclusion_receipt.clone(),
            scope_digest: fence.excluded_scope_digest,
        });
        Ok(())
    }

    /// Blocks later publication until explicit repair.
    pub fn block_publication(&mut self) -> Result<(), PublicationError> {
        let transaction = self
            .active
            .as_mut()
            .ok_or(PublicationError::InvalidTransition)?;
        transaction.phase = PublicationPhase::PublicationBlocked;
        Ok(())
    }

    fn active_mut(
        &mut self,
        expected: PublicationPhase,
    ) -> Result<&mut PublicationTransaction, PublicationError> {
        let transaction = self
            .active
            .as_mut()
            .ok_or(PublicationError::InvalidTransition)?;
        if transaction.phase != expected {
            return Err(PublicationError::InvalidTransition);
        }
        Ok(transaction)
    }

    fn active_at(
        &self,
        expected: PublicationPhase,
    ) -> Result<&PublicationTransaction, PublicationError> {
        let transaction = self
            .active
            .as_ref()
            .ok_or(PublicationError::InvalidTransition)?;
        if transaction.phase != expected {
            return Err(PublicationError::InvalidTransition);
        }
        Ok(transaction)
    }
}

pub fn validate_manifest(
    manifest: &ProjectionManifest,
    max_points: usize,
) -> Result<(), PublicationError> {
    if manifest.canonical_bytes.is_empty() || manifest.entries.len() > max_points {
        return Err(PublicationError::InvalidPreparedPublication);
    }
    if manifest
        .entries
        .windows(2)
        .any(|pair| pair[0].point_id >= pair[1].point_id)
    {
        return Err(PublicationError::InvalidPreparedPublication);
    }
    Ok(())
}

/// Inputs must already have passed bounded, strictly sorted manifest validation.
/// A merge walk uses no cloned manifests, maps, ID sets or additional allocation.
/// Exact retained entries are allowed; even a digest-only change is not retained.
pub fn changed_point_id_is_reused(
    old: Option<&ProjectionManifest>,
    new: &ProjectionManifest,
) -> bool {
    let Some(old) = old else {
        return false;
    };
    let mut previous = old.entries.iter().peekable();
    for proposed in &new.entries {
        while previous
            .peek()
            .is_some_and(|entry| entry.point_id < proposed.point_id)
        {
            let _ = previous.next();
        }
        if let Some(existing) = previous.peek()
            && existing.point_id == proposed.point_id
        {
            if *existing != proposed {
                return true;
            }
            let _ = previous.next();
        }
    }
    false
}

fn manifest_diff(transaction: &PublicationTransaction) -> Result<ManifestDiff, PublicationError> {
    let empty = ProjectionManifest {
        entries: Vec::new(),
        canonical_bytes: b"eliot-search/empty-manifest/v1".to_vec(),
    };
    let old = transaction.prepared.old_manifest.as_ref().unwrap_or(&empty);
    diff_manifests(old, &transaction.prepared.new_manifest)
        .map_err(|_| PublicationError::InvalidPreparedPublication)
}

fn entry_ids(entries: &[search_projection_planner::ProjectionManifestEntry]) -> Vec<PointId128> {
    // The planner groups changed and removed old entries separately. Sort the
    // combined exact ID list; never rely on incidental diff grouping for receipts.
    let mut ids = entries
        .iter()
        .map(|entry| entry.point_id)
        .collect::<Vec<_>>();
    ids.sort_unstable();
    ids
}

fn verify_transaction_identity(
    transaction: &PublicationTransaction,
    transaction_id: &OpaqueId,
    target_epoch: Epoch,
) -> Result<(), PublicationError> {
    if transaction_id != &transaction.prepared.transaction_id
        || target_epoch != transaction.target_epoch
    {
        Err(PublicationError::OperationMismatch)
    } else {
        Ok(())
    }
}

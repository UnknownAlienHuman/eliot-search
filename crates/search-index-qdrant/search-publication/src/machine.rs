//! Single-flight publication coordinator and linearization state machine.

use std::collections::BTreeSet;

use search_contracts::{Blake3Digest32, Epoch, OpaqueId, ReceiptRef};
use search_point_identity::PointId128;
use search_projection_planner::{ManifestDiff, ProjectionManifest, diff_manifests};

use crate::{
    AbandonFence, ClosureReceipt, CompensationReceipt, ControlCommitObservation,
    PreparedPublication, PublicationError, PublicationGuards, ReadbackVerified,
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
    pub target_epoch: Epoch,
    pub phase: PublicationPhase,
    pub durable_intent: Option<DurableIntent>,
    pub stage_receipt: Option<StageReceipt>,
    pub closure_receipt: Option<ClosureReceipt>,
    pub verified: Option<ReadbackVerified>,
    pub visible_commit: Option<VisibleCommitReceipt>,
    pub snapshot_receipt: Option<SnapshotPublishReceipt>,
}

impl PublicationTransaction {
    #[must_use]
    pub const fn transaction_id(&self) -> &OpaqueId {
        &self.prepared.transaction_id
    }
}

/// Process-local single-flight coordinator.
#[derive(Clone, Debug)]
pub struct PublicationCoordinator {
    visible_epoch: Epoch,
    current_manifest: Option<ProjectionManifest>,
    current_manifest_digest: Option<Blake3Digest32>,
    active: Option<PublicationTransaction>,
    max_points: usize,
}

impl PublicationCoordinator {
    /// Creates an empty coordinator at an explicit visible epoch.
    pub fn new(
        visible_epoch: Epoch,
        current_manifest: Option<ProjectionManifest>,
        current_manifest_digest: Option<Blake3Digest32>,
        max_points: usize,
    ) -> Result<Self, PublicationError> {
        if max_points == 0
            || current_manifest.is_some() != current_manifest_digest.is_some()
        {
            return Err(PublicationError::InvalidPreparedPublication);
        }
        if let Some(manifest) = &current_manifest {
            validate_manifest(manifest, max_points)?;
        }
        Ok(Self {
            visible_epoch,
            current_manifest,
            current_manifest_digest,
            active: None,
            max_points,
        })
    }

    #[must_use]
    pub const fn visible_epoch(&self) -> Epoch {
        self.visible_epoch
    }

    #[must_use]
    pub const fn active(&self) -> Option<&PublicationTransaction> {
        self.active.as_ref()
    }

    #[must_use]
    pub const fn current_manifest(&self) -> Option<&ProjectionManifest> {
        self.current_manifest.as_ref()
    }

    /// Reserves the exact next epoch for one prepared transaction.
    pub fn submit(
        &mut self,
        prepared: PreparedPublication,
    ) -> Result<Epoch, PublicationError> {
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
        let target_epoch = self
            .visible_epoch
            .checked_next()
            .map_err(|_| PublicationError::ContractExhausted)?;
        self.active = Some(PublicationTransaction {
            prepared,
            target_epoch,
            phase: PublicationPhase::Prepared,
            durable_intent: None,
            stage_receipt: None,
            closure_receipt: None,
            verified: None,
            visible_commit: None,
            snapshot_receipt: None,
        });
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
    pub fn stage_new_points(
        &mut self,
        receipt: StageReceipt,
    ) -> Result<(), PublicationError> {
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
    pub fn close_old_points(
        &mut self,
        receipt: ClosureReceipt,
    ) -> Result<(), PublicationError> {
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
        if stage.readback_digest != staged_digest
            || closure.readback_digest != closure_digest
        {
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
        let transaction = self
            .active
            .take()
            .ok_or(PublicationError::InvalidTransition)?;
        if transaction.phase != PublicationPhase::SnapshotPublished {
            self.active = Some(transaction);
            return Err(PublicationError::InvalidTransition);
        }
        let receipt = transaction
            .visible_commit
            .clone()
            .ok_or(PublicationError::ControlConflict)?;
        self.current_manifest = Some(transaction.prepared.new_manifest);
        self.current_manifest_digest = Some(transaction.prepared.new_manifest_digest);
        Ok(receipt)
    }

    /// Begins exact compensation for an unresolved pre-control transaction.
    pub fn begin_compensation(&mut self) -> Result<Vec<PointId128>, PublicationError> {
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
        ) {
            return Err(PublicationError::InvalidTransition);
        }
        let ids = entry_ids(&manifest_diff(transaction)?.create);
        transaction.phase = PublicationPhase::Compensating;
        Ok(ids)
    }

    /// Accepts exact compensation and keeps the reserved epoch consumed.
    pub fn compensate_exact(
        &mut self,
        receipt: CompensationReceipt,
    ) -> Result<(), PublicationError> {
        let transaction = self.active_mut(PublicationPhase::Compensating)?;
        if receipt.transaction_id != transaction.prepared.transaction_id
            || !receipt.remaining_ids.is_empty()
            || receipt.compensated_ids != entry_ids(&manifest_diff(transaction)?.create)
        {
            return Err(PublicationError::CompensationIncomplete);
        }
        transaction.phase = PublicationPhase::Aborted;
        Ok(())
    }

    /// Abandons an exact transaction only after a complete exclusion fence.
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
        let affected = manifest_diff(transaction)?
            .create
            .into_iter()
            .chain(manifest_diff(transaction)?.retire)
            .map(|entry| entry.point_id)
            .collect::<BTreeSet<_>>();
        if fence.transaction_id != transaction.prepared.transaction_id
            || fence.target_epoch != transaction.target_epoch
            || fence.excluded_point_ids != affected
        {
            return Err(PublicationError::AbandonFenceMissing);
        }
        transaction.phase = PublicationPhase::Aborted;
        Ok(())
    }

    /// Removes an aborted transaction while preserving the consumed epoch.
    pub fn finalize_aborted(&mut self) -> Result<(), PublicationError> {
        let transaction = self
            .active
            .as_ref()
            .ok_or(PublicationError::InvalidTransition)?;
        if transaction.phase != PublicationPhase::Aborted {
            return Err(PublicationError::InvalidTransition);
        }
        self.active = None;
        Ok(())
    }

    /// Permanently blocks later publication until explicit repair.
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

fn validate_manifest(
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

fn manifest_diff(
    transaction: &PublicationTransaction,
) -> Result<ManifestDiff, PublicationError> {
    let empty = ProjectionManifest {
        entries: Vec::new(),
        canonical_bytes: b"eliot-search/empty-manifest/v1".to_vec(),
    };
    let old = transaction
        .prepared
        .old_manifest
        .as_ref()
        .unwrap_or(&empty);
    diff_manifests(old, &transaction.prepared.new_manifest)
        .map_err(|_| PublicationError::InvalidPreparedPublication)
}

fn entry_ids(
    entries: &[search_projection_planner::ProjectionManifestEntry],
) -> Vec<PointId128> {
    entries.iter().map(|entry| entry.point_id).collect()
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

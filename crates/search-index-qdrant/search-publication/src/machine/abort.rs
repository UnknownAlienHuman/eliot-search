//! Acknowledgement barrier for abnormal publication completion.
//! No method here executes a journal/index effect or authenticates a receipt producer.

use super::{
    Blake3Digest32, DurableIntent, Epoch, OpaqueId, PublicationCoordinator, PublicationError,
    PublicationGuards, PublicationPhase, ReceiptRef, SnapshotPublishReceipt,
};
use core::fmt;
use search_contracts::CollectionGenerationId;

/// Verified outcome references retained after the exact point/membership checks.
/// Large point sets stay in the original immutable manifests, not this control input.
#[derive(Clone, Eq, PartialEq)]
pub enum AbortedPublicationResolution {
    /// Both sides were verified; restoration is absent only when no old points required it.
    Compensated {
        /// Exact staged-point removal/exclusion readback.
        removal_readback: ReceiptRef,
        /// Exact old-state restoration readback, when supplied.
        restoration_readback: Option<ReceiptRef>,
    },
    /// Complete affected memberships were fenced before retrieval and IDF.
    Excluded {
        /// Durable effective exclusion receipt supplied by the access/control owner.
        exclusion_receipt: ReceiptRef,
        /// Exact excluded scope digest supplied by that owner.
        scope_digest: Blake3Digest32,
    },
}
impl fmt::Debug for AbortedPublicationResolution {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Compensated { .. } => "Compensated { readbacks: <redacted> }",
            Self::Excluded { .. } => "Excluded { evidence: <redacted> }",
        })
    }
}

/// Exact transient command latched by the sole coordinator before journal dispatch.
///
/// Copies may be inspected, but an edited copy cannot replace the latched request.
/// The journal adapter must validate live guards and record resolution plus consumed
/// epoch atomically, without changing visibility or clearing unrelated fences.
#[derive(Clone, Eq, PartialEq)]
pub struct AbortFinalizationRequest {
    /// Stable operation ID for this finalization, distinct from intent creation.
    pub operation_id: OpaqueId,
    /// Actual control generation expected by the guarded transaction.
    pub expected_control_generation: u64,
    /// Exact collection route; not a native collection name.
    pub collection_generation_id: CollectionGenerationId,
    /// Visibility that must remain unchanged by this operation.
    pub previous_visible_epoch: Epoch,
    /// Current guards to revalidate atomically, not replacements for original intent guards.
    pub current_guards: PublicationGuards,
    /// Original transaction, consumed target, manifest digests and durable operation binding.
    pub intent: DurableIntent,
    /// Original immutable preparation reference.
    pub preparation_receipt: ReceiptRef,
    /// Actual outcome references, captured only after successful coordinator validation.
    pub resolution: AbortedPublicationResolution,
}
impl fmt::Debug for AbortFinalizationRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AbortFinalizationRequest")
            .field(
                "expected_control_generation",
                &self.expected_control_generation,
            )
            .field("target_epoch", &self.intent.target_epoch)
            .field("resolution", &self.resolution)
            .finish_non_exhaustive()
    }
}

/// Exact authoritative journal readback, supplied by the owning adapter.
///
/// These fields cannot prove I/O occurred: composition must verify the producer
/// and the complete original request. This is not an adapter-created receipt.
#[derive(Clone, Eq, PartialEq)]
pub struct AbortControlCommitObservation {
    /// Complete command actually committed; compared to the coordinator's latched input.
    pub request: AbortFinalizationRequest,
    /// Observed route after commit.
    pub collection_generation_id: CollectionGenerationId,
    /// Observed visible epoch, which must not advance during an abort.
    pub visible_epoch: Epoch,
    /// Observed consumed reservation floor, which must retain the aborted target.
    pub last_reserved_epoch: Epoch,
    /// Guards actually observed by the commit, not echoed preparation values.
    pub observed_guards: PublicationGuards,
    /// Exact new control generation.
    pub control_generation: u64,
    /// Producer-computed digest of the committed control state.
    pub control_state_digest: Blake3Digest32,
    /// Immutable readback-verified finalization receipt.
    pub commit_receipt: ReceiptRef,
}
impl fmt::Debug for AbortControlCommitObservation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AbortControlCommitObservation")
            .field("control_generation", &self.control_generation)
            .field("visible_epoch", &self.visible_epoch)
            .finish_non_exhaustive()
    }
}

#[derive(Default)]
pub(super) struct AbortFinalizationProgress {
    pub(super) resolution: Option<AbortedPublicationResolution>,
    request: Option<AbortFinalizationRequest>,
    commit: Option<AbortControlCommitObservation>,
    snapshot: Option<SnapshotPublishReceipt>,
}

impl fmt::Debug for AbortFinalizationProgress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AbortFinalizationProgress")
            .field("resolved", &self.resolution.is_some())
            .field("prepared", &self.request.is_some())
            .field("committed", &self.commit.is_some())
            .field("snapshot_published", &self.snapshot.is_some())
            .finish()
    }
}

impl PublicationCoordinator {
    /// Prepares and latches one bounded exact finalization request after verified
    /// compensation/exclusion. Retrying unchanged returns the same command.
    /// The caller holds the live owner/recovery/admission barrier across journal
    /// commit, snapshot publication and final release. Observations are not grants.
    /// A different operation, generation or guard set requires explicit recovery,
    /// not an automatic reset that could conceal a prior unknown external write.
    ///
    /// # Errors
    /// Rejects an unresolved phase, missing outcome/intent, reused operation ID,
    /// regressed guards and exhausted control generation before changing the latch.
    pub fn prepare_abort_finalization(
        &mut self,
        operation_id: OpaqueId,
        expected_control_generation: u64,
        current_guards: PublicationGuards,
    ) -> Result<AbortFinalizationRequest, PublicationError> {
        let transaction = self.active_at(PublicationPhase::Aborted)?;
        let intent = transaction
            .durable_intent
            .as_ref()
            .ok_or(PublicationError::RecoveryBlocked)?;
        if operation_id == intent.persist_operation_id {
            return Err(PublicationError::OperationMismatch);
        }
        if expected_control_generation == 0 {
            return Err(PublicationError::ControlConflict);
        }
        expected_control_generation
            .checked_add(1)
            .ok_or(PublicationError::ContractExhausted)?;
        let old = intent.guards;
        if current_guards.owner_epoch < old.owner_epoch
            || current_guards.source_catalog_generation < old.source_catalog_generation
            || current_guards.membership_generation < old.membership_generation
            || current_guards.access_generation < old.access_generation
            || current_guards.shadow_generation < old.shadow_generation
            || current_guards.purge_generation < old.purge_generation
        {
            return Err(PublicationError::GuardMismatch);
        }
        if self.visible_epoch != transaction.previous_visible_epoch
            || self.last_reserved_epoch != transaction.target_epoch
        {
            return Err(PublicationError::EpochMismatch);
        }
        let request = AbortFinalizationRequest {
            operation_id,
            expected_control_generation,
            current_guards,
            collection_generation_id: transaction.prepared.collection_generation_id,
            previous_visible_epoch: transaction.previous_visible_epoch,
            intent: intent.clone(),
            preparation_receipt: transaction.prepared.preparation_receipt.clone(),
            resolution: self
                .abort_finalization
                .resolution
                .clone()
                .ok_or(PublicationError::RecoveryBlocked)?,
        };
        if let Some(previous) = &self.abort_finalization.request {
            if previous != &request {
                return Err(PublicationError::OperationMismatch);
            }
        } else {
            self.abort_finalization.request = Some(request.clone());
        }
        Ok(request)
    }

    /// Accepts exact durable finalization readback without releasing serialization.
    /// Lost replies retry this same operation; this method performs no journal write.
    ///
    /// # Errors
    /// Rejects changed request, route, epoch floor, guards, generation or conflicting
    /// repeat. Failure preserves all prior evidence and keeps the active slot occupied.
    pub fn acknowledge_abort_commit(
        &mut self,
        observed: AbortControlCommitObservation,
    ) -> Result<(), PublicationError> {
        self.active_at(PublicationPhase::Aborted)?;
        let request = self
            .abort_finalization
            .request
            .as_ref()
            .ok_or(PublicationError::RecoveryBlocked)?;
        if &observed.request != request {
            return Err(PublicationError::OperationMismatch);
        }
        let route_matches = observed.collection_generation_id == request.collection_generation_id;
        let observed_visible = observed.visible_epoch;
        let coordinator_visible = self.visible_epoch;
        let latched_previous = request.previous_visible_epoch;
        let visible_matches =
            observed_visible == coordinator_visible && observed_visible == latched_previous;
        let reservation_matches = observed.last_reserved_epoch == self.last_reserved_epoch
            && observed.last_reserved_epoch == request.intent.target_epoch;
        let generation_matches =
            request.expected_control_generation.checked_add(1) == Some(observed.control_generation);
        if !route_matches || !visible_matches || !reservation_matches || !generation_matches {
            return Err(PublicationError::ControlConflict);
        }
        if observed.observed_guards != request.current_guards {
            return Err(PublicationError::GuardMismatch);
        }
        if let Some(previous) = &self.abort_finalization.commit {
            if previous != &observed {
                return Err(PublicationError::OperationMismatch);
            }
        } else {
            self.abort_finalization.commit = Some(observed);
        }
        Ok(())
    }

    /// Accepts the current-process immutable snapshot after durable finalization.
    /// Snapshot and control-state digests have distinct producer-owned meanings;
    /// do not relabel one as the other. Exact repeats retain the original receipt.
    ///
    /// # Errors
    /// Missing commit, wrong transaction/visibility/generation or a conflicting
    /// snapshot receipt leaves admission blocked and retains the active operation.
    pub fn publish_abort_snapshot(
        &mut self,
        receipt: SnapshotPublishReceipt,
    ) -> Result<(), PublicationError> {
        self.active_at(PublicationPhase::Aborted)?;
        let commit = self
            .abort_finalization
            .commit
            .as_ref()
            .ok_or(PublicationError::RecoveryBlocked)?;
        if receipt.transaction_id != commit.request.intent.transaction_id
            || receipt.visible_epoch != commit.visible_epoch
            || receipt.control_generation != commit.control_generation
        {
            return Err(PublicationError::SnapshotPublicationFailed);
        }
        if let Some(previous) = &self.abort_finalization.snapshot {
            if previous != &receipt {
                return Err(PublicationError::SnapshotPublicationFailed);
            }
        } else {
            self.abort_finalization.snapshot = Some(receipt);
        }
        Ok(())
    }

    /// Releases a resolved slot only after exact durable commit and fresh snapshot
    /// acknowledgements. Does not advance visibility, lower the reservation floor,
    /// change the current manifest, remove fences, or perform an external mutation.
    ///
    /// # Errors
    /// A phase label or compensation/exclusion receipt alone is insufficient.
    /// Missing acknowledgements retain the active slot and all resolution evidence.
    pub fn finalize_aborted(&mut self) -> Result<(), PublicationError> {
        self.active_at(PublicationPhase::Aborted)?;
        if self.abort_finalization.commit.is_none() {
            return Err(PublicationError::RecoveryBlocked);
        }
        if self.abort_finalization.snapshot.is_none() {
            return Err(PublicationError::SnapshotPublicationFailed);
        }
        self.active = None;
        self.abort_finalization = AbortFinalizationProgress::default();
        Ok(())
    }
}

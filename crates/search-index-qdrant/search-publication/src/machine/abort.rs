//! Acknowledgement barrier for abnormal publication completion.
//! No method here executes a journal/index effect or authenticates a receipt producer.

use super::{
    Blake3Digest32, DurableIntent, Epoch, OpaqueId, PublicationCoordinator, PublicationEffectPort,
    PublicationError, PublicationGuards, PublicationPhase, PublicationReadbackPort,
    SnapshotPublishReceipt, effect_command,
};
use crate::model::{PublicationEffectKind, PublicationEffectOutcome};
use core::fmt;
use search_contracts::CollectionGenerationId;
use search_ports::{CancellationProbe, IdempotencyClass, MutationIdentity, OperationContext};

/// Verified outcome references retained after the exact point/membership checks.
/// Large point sets stay in the original immutable manifests, not this control input.
#[derive(Clone, Eq, PartialEq)]
pub enum AbortedPublicationResolution {
    /// Both sides were verified; restoration is absent only when no old points required it.
    Compensated {
        /// Stable owner effect identity whose complete readback was validated.
        operation_id: OpaqueId,
        /// Whether old closed points were restored as part of the same plan.
        restored_old_points: bool,
    },
    /// Complete affected memberships were fenced before retrieval and IDF.
    Excluded {
        /// Stable owner effect identity whose complete readback was validated.
        operation_id: OpaqueId,
        /// Complete excluded memberships read back from the access/control owner.
        excluded_projection_memberships: Vec<search_contracts::ProjectionMembershipId>,
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
    pub prepared_input_digest: Blake3Digest32,
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
    pub(crate) request: AbortFinalizationRequest,
    /// Observed route after commit.
    pub(crate) collection_generation_id: CollectionGenerationId,
    /// Observed visible epoch, which must not advance during an abort.
    pub(crate) visible_epoch: Epoch,
    /// Observed consumed reservation floor, which must retain the aborted target.
    pub(crate) last_reserved_epoch: Epoch,
    /// Guards actually observed by the commit, not echoed preparation values.
    pub(crate) observed_guards: PublicationGuards,
    /// Exact new control generation.
    pub(crate) control_generation: u64,
    /// Producer-computed digest of the committed control state.
    pub(crate) control_state_digest: Blake3Digest32,
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
            prepared_input_digest: transaction.prepared.prepared_input_digest,
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
    pub fn acknowledge_abort_commit<C, E, R>(
        &mut self,
        effects: &mut E,
        readback: &mut R,
        context: &OperationContext<C>,
    ) -> Result<(), PublicationError>
    where
        C: CancellationProbe,
        E: PublicationEffectPort<C>,
        R: PublicationReadbackPort<C>,
    {
        self.active_at(PublicationPhase::Aborted)?;
        let request = self
            .abort_finalization
            .request
            .clone()
            .ok_or(PublicationError::RecoveryBlocked)?;
        let proof = {
            let transaction = self
                .active
                .as_mut()
                .ok_or(PublicationError::InvalidTransition)?;
            let observed = execute_abort_effect(
                transaction,
                &request,
                effects,
                readback,
                context,
                PublicationEffectKind::CommitAbortFinalization,
            )?;
            let control = observed.control.ok_or(PublicationError::ControlConflict)?;
            if observed.outcome != PublicationEffectOutcome::Applied
                || observed.abort_request.as_ref() != Some(&request)
                || observed.binding.operation_id != request.operation_id
                || control.before_visible_epoch != request.previous_visible_epoch
                || control.after_visible_epoch != request.previous_visible_epoch
                || control.last_reserved_epoch != request.intent.target_epoch
                || control.before_control_generation != request.expected_control_generation
                || control.before_control_generation.checked_add(1)
                    != Some(control.after_control_generation)
                || control.collection_generation_id != request.collection_generation_id
                || control.previous_visible_manifest_digest != request.intent.old_manifest_digest
                || control.visible_manifest_digest != request.intent.old_manifest_digest
            {
                return Err(PublicationError::ControlConflict);
            }
            if control.guards != request.current_guards {
                return Err(PublicationError::GuardMismatch);
            }
            AbortControlCommitObservation {
                request,
                collection_generation_id: control.collection_generation_id,
                visible_epoch: control.after_visible_epoch,
                last_reserved_epoch: control.last_reserved_epoch,
                observed_guards: control.guards,
                control_generation: control.after_control_generation,
                control_state_digest: control.control_state_digest,
            }
        };
        if let Some(previous) = &self.abort_finalization.commit {
            if previous != &proof {
                return Err(PublicationError::OperationMismatch);
            }
        } else {
            self.abort_finalization.commit = Some(proof);
        }
        self.active
            .as_mut()
            .ok_or(PublicationError::InvalidTransition)?
            .pending_effect = None;
        Ok(())
    }

    /// Accepts the current-process immutable snapshot after durable finalization.
    /// Snapshot and control-state digests have distinct producer-owned meanings;
    /// do not relabel one as the other. Exact repeats retain the original receipt.
    ///
    /// # Errors
    /// Missing commit, wrong transaction/visibility/generation or a conflicting
    /// snapshot receipt leaves admission blocked and retains the active operation.
    pub fn publish_abort_snapshot<C, E, R>(
        &mut self,
        effects: &mut E,
        readback: &mut R,
        context: &OperationContext<C>,
    ) -> Result<(), PublicationError>
    where
        C: CancellationProbe,
        E: PublicationEffectPort<C>,
        R: PublicationReadbackPort<C>,
    {
        self.active_at(PublicationPhase::Aborted)?;
        let commit = self
            .abort_finalization
            .commit
            .as_ref()
            .ok_or(PublicationError::RecoveryBlocked)?
            .clone();
        let receipt = {
            let transaction = self
                .active
                .as_mut()
                .ok_or(PublicationError::InvalidTransition)?;
            let observed = execute_abort_effect(
                transaction,
                &commit.request,
                effects,
                readback,
                context,
                PublicationEffectKind::PublishAbortSnapshot,
            )?;
            let snapshot = observed
                .snapshot
                .ok_or(PublicationError::SnapshotPublicationFailed)?;
            if observed.outcome != PublicationEffectOutcome::Applied
                || observed.abort_request.as_ref() != Some(&commit.request)
                || snapshot.visible_epoch != commit.visible_epoch
                || snapshot.control_generation != commit.control_generation
                || snapshot.guards != commit.request.current_guards
                || snapshot.collection_generation_id != commit.collection_generation_id
                || snapshot.visible_manifest_digest != commit.request.intent.old_manifest_digest
            {
                return Err(PublicationError::SnapshotPublicationFailed);
            }
            SnapshotPublishReceipt {
                transaction_id: commit.request.intent.transaction_id.clone(),
                visible_epoch: snapshot.visible_epoch,
                control_generation: snapshot.control_generation,
                process_snapshot_id: snapshot.process_snapshot_id,
                snapshot_digest: snapshot.snapshot_digest,
            }
        };
        if let Some(previous) = &self.abort_finalization.snapshot {
            if previous != &receipt {
                return Err(PublicationError::SnapshotPublicationFailed);
            }
        } else {
            self.abort_finalization.snapshot = Some(receipt);
        }
        self.active
            .as_mut()
            .ok_or(PublicationError::InvalidTransition)?
            .pending_effect = None;
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

fn execute_abort_effect<C, E, R>(
    transaction: &mut super::PublicationTransaction,
    request: &AbortFinalizationRequest,
    effects: &mut E,
    readback: &mut R,
    context: &OperationContext<C>,
    effect: PublicationEffectKind,
) -> Result<crate::model::PublicationEffectReadback, PublicationError>
where
    C: CancellationProbe,
    E: PublicationEffectPort<C>,
    R: PublicationReadbackPort<C>,
{
    context
        .preflight()
        .map_err(|_| PublicationError::OperationContextInvalid)?;
    if transaction.original_context.as_ref()
        != Some(&super::OperationContextBinding::from_context(context))
    {
        return Err(PublicationError::OperationMismatch);
    }
    if transaction
        .pending_effect
        .is_some_and(|pending| pending != effect)
    {
        return Err(PublicationError::OutcomeUnknown);
    }
    let mut command = effect_command(transaction, effect, context)?;
    if effect == PublicationEffectKind::CommitAbortFinalization {
        command
            .binding
            .operation_id
            .clone_from(&request.operation_id);
    }
    command.abort_request = Some(request.clone());
    if transaction.pending_effect.is_none() {
        let identity = MutationIdentity::new(
            command.binding.operation_id.clone(),
            IdempotencyClass::RetrySameIdentity,
        );
        // Keep an interrupted abort mutation unresolved until exact readback.
        transaction.pending_effect = Some(effect);
        let _attempt = effects.apply(&command, &identity, context);
    }
    context
        .preflight()
        .map_err(|_| PublicationError::OperationContextInvalid)?;
    let observed = readback.readback(&command, context);
    if observed.effect != effect
        || observed.binding != command.binding
        || observed.abort_request.as_ref() != Some(request)
    {
        return Err(PublicationError::OperationMismatch);
    }
    match observed.outcome {
        PublicationEffectOutcome::Applied => {}
        PublicationEffectOutcome::NotApplied => {
            transaction.pending_effect = None;
            return Err(PublicationError::EffectNotObserved);
        }
        PublicationEffectOutcome::Conflict => {
            return Err(PublicationError::ControlConflict);
        }
        PublicationEffectOutcome::Unknown => {
            transaction.pending_effect = Some(effect);
            return Err(PublicationError::OutcomeUnknown);
        }
    }
    Ok(observed)
}

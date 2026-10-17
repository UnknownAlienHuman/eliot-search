//! Owner-readback hydration for the original publication operation.

use search_ports::{CancellationProbe, OperationContext};
use search_projection_planner::ProjectionBudget;

use crate::admission::{AdmissionPurpose, PublicationProposal, validate_prepared_generation};
use crate::model::{
    PublicationControlReadback, PublicationEffectBinding, PublicationEffectKind,
    PublicationEffectOutcome, PublicationEffectReadback, PublicationIntentReadback,
    PublicationPointExpectation,
};
use crate::{PublicationError, PublicationRecoveryDecision};

use super::{
    AbortControlCommitObservation, AbortFinalizationReadback, AbortFinalizationRequest,
    AbortedPublicationResolution,
    ClosureReceipt, DurableIntent, PublicationCoordinator, PublicationEffectCommand,
    PublicationEffectPreparationOwners, PublicationPhase, PublicationReadbackPort,
    PublicationTransaction, ReadbackVerified, SnapshotPublishReceipt, StageReceipt,
    VisibleCommitReceipt, effect_command, operation_id, point_readback_matches, readback_only,
    validate_control_generation_transition,
};

/// Restore request contains only the original canonical proposal. Durable
/// phase labels and all effect evidence are read back through owner ports.
pub struct PublicationRestoreInput<T> {
    /// Original immutable preparation token request; canonical admission runs again.
    pub prepared: T,
}

impl<T> PublicationRestoreInput<T> {
    /// Wraps the original proposal without accepting phase or proof claims.
    #[must_use]
    pub const fn new(prepared: T) -> Self {
        Self { prepared }
    }
}

impl PublicationCoordinator {
    /// Re-admits the original proposal and rebuilds its verified phase from
    /// exact owner readbacks. No effect is executed during hydration.
    #[allow(clippy::too_many_lines)]
    pub fn restore_inflight<P, C, R>(
        input: &PublicationRestoreInput<PublicationProposal<'_, P::Projection>>,
        owners: &mut P,
        snapshot: &P::Snapshot,
        budget: ProjectionBudget,
        readback: &mut R,
        context: &OperationContext<C>,
    ) -> Result<(Self, PublicationRecoveryDecision), PublicationError>
    where
        P: PublicationEffectPreparationOwners + ?Sized,
        C: CancellationProbe,
        R: PublicationReadbackPort<C>,
    {
        let prepared = validate_prepared_generation(
            owners,
            snapshot,
            &input.prepared,
            AdmissionPurpose::Restore,
            budget,
        )?;
        let max_points = budget
            .validate()
            .map_err(|_| PublicationError::BudgetExceeded)?
            .max_points;
        let expected_points =
            super::expected_point_sets(owners, &input.prepared, &prepared, budget)?;
        let observed = prepared.observed;
        if observed.current_guards.owner_epoch < prepared.guards.owner_epoch
            || observed.current_guards.source_catalog_generation
                < prepared.guards.source_catalog_generation
            || observed.current_guards.membership_generation < prepared.guards.membership_generation
            || observed.current_guards.access_generation < prepared.guards.access_generation
            || observed.current_guards.shadow_generation < prepared.guards.shadow_generation
            || observed.current_guards.purge_generation < prepared.guards.purge_generation
        {
            return Err(PublicationError::GuardMismatch);
        }
        if observed.target_epoch <= observed.previous_visible_epoch
            || observed.last_reserved_epoch != observed.target_epoch
        {
            return Err(PublicationError::EpochMismatch);
        }
        let committed = observed.current_visible_epoch == observed.target_epoch;
        let current_manifest = if committed {
            (!matches!(
                prepared.target,
                crate::admission::PublicationTarget::Retirement(_)
            ))
            .then(|| prepared.new_manifest.clone())
        } else {
            prepared.old_manifest.clone()
        };
        let current_manifest_digest = if committed {
            (!matches!(
                prepared.target,
                crate::admission::PublicationTarget::Retirement(_)
            ))
            .then_some(prepared.new_manifest_digest)
        } else {
            prepared.old_manifest_digest
        };
        let mut coordinator = Self {
            initialized: true,
            current_scope: Some(prepared.scope().clone()),
            visible_epoch: observed.current_visible_epoch,
            last_reserved_epoch: observed.last_reserved_epoch,
            current_manifest,
            current_manifest_digest,
            active: Some(PublicationTransaction {
                prepared,
                previous_visible_epoch: observed.previous_visible_epoch,
                target_epoch: observed.target_epoch,
                phase: PublicationPhase::Prepared,
                durable_intent: None,
                stage_receipt: None,
                closure_receipt: None,
                verified: None,
                visible_commit: None,
                snapshot_receipt: None,
                expected_points: Some(expected_points),
                pending_effect: None,
                original_context: None,
                max_points,
            }),
            abort_finalization: super::abort::AbortFinalizationProgress::default(),
            max_points,
        };
        let transaction = coordinator
            .active
            .as_mut()
            .ok_or(PublicationError::RecoveryBlocked)?;

        // Read the durable record using its original stable operation ID.
        let intent_command =
            effect_command(transaction, PublicationEffectKind::PersistIntent, context)?;
        context
            .preflight()
            .map_err(|_| PublicationError::OperationContextInvalid)?;
        let intent_observation = readback.readback(&intent_command, context);
        context
            .preflight()
            .map_err(|_| PublicationError::OperationContextInvalid)?;
        let intent = exact_intent_for_restore(&intent_command, &intent_observation)?;
        let recorded_phase = intent.recorded_phase;
        if matches!(recorded_phase, PublicationPhase::Prepared) {
            return Err(PublicationError::RecoveryBlocked);
        }
        transaction.durable_intent = Some(DurableIntent {
            transaction_id: transaction.prepared.transaction_id.clone(),
            target_epoch: transaction.target_epoch,
            old_manifest_digest: transaction.prepared.old_manifest_digest,
            new_manifest_digest: transaction.prepared.new_manifest_digest,
            guards: transaction.prepared.guards,
            persist_operation_id: intent.binding.operation_id.clone(),
            request_id: intent.binding.request_id,
            relative_deadline_ms: intent.binding.relative_deadline_ms,
            budget_ref: intent.binding.budget_ref.clone(),
            control_generation: intent.control_generation,
            control_state_digest: intent.control_state_digest,
        });
        transaction.phase = PublicationPhase::IntentDurable;

        if recorded_phase == PublicationPhase::Aborted {
            let (Some(request), Some(abort_binding)) = (
                intent_observation.abort_request,
                intent_observation.abort_binding,
            ) else {
                transaction.phase = PublicationPhase::PublicationBlocked;
                return Ok((coordinator, PublicationRecoveryDecision::PublicationBlocked));
            };
            return restore_aborted(
                coordinator,
                readback,
                context,
                &intent.binding,
                request,
                abort_binding,
            );
        }

        let (stage_command, stage) = readback_only(
            transaction,
            readback,
            context,
            PublicationEffectKind::StageNewPoints,
        )?;
        let mut stage_not_applied = false;
        let mut compensation_required = false;
        match stage.outcome {
            PublicationEffectOutcome::Applied => {
                exact_points(&stage_command, &stage)?;
                transaction.stage_receipt = Some(StageReceipt {
                    transaction_id: transaction.prepared.transaction_id.clone(),
                    target_epoch: transaction.target_epoch,
                    points: stage.points,
                });
                transaction.phase = PublicationPhase::NewPointsAcknowledged;
            }
            PublicationEffectOutcome::NotApplied => {
                exact_absence(&stage_command, &stage)?;
                stage_not_applied = true;
            }
            _ => return Err(PublicationError::RecoveryBlocked),
        }

        let (closure_command, closure) = readback_only(
            transaction,
            readback,
            context,
            PublicationEffectKind::CloseOldPoints,
        )?;
        match closure.outcome {
            PublicationEffectOutcome::Applied => {
                if transaction.stage_receipt.is_none() {
                    if transaction.prepared.difference.retire.is_empty() {
                        return Err(PublicationError::RecoveryBlocked);
                    }
                    exact_points(&closure_command, &closure)?;
                    transaction.closure_receipt = Some(ClosureReceipt {
                        transaction_id: transaction.prepared.transaction_id.clone(),
                        target_epoch: transaction.target_epoch,
                        points: closure.points,
                    });
                    transaction.phase = PublicationPhase::OldPointsClosedAcknowledged;
                    compensation_required = true;
                } else {
                    exact_points(&closure_command, &closure)?;
                    transaction.closure_receipt = Some(ClosureReceipt {
                        transaction_id: transaction.prepared.transaction_id.clone(),
                        target_epoch: transaction.target_epoch,
                        points: closure.points,
                    });
                    transaction.phase = PublicationPhase::OldPointsClosedAcknowledged;
                }
            }
            PublicationEffectOutcome::NotApplied => {
                exact_original_points(&closure_command, &closure)?;
                if phase_at_or_after(
                    recorded_phase,
                    PublicationPhase::OldPointsClosedAcknowledged,
                ) {
                    return Err(PublicationError::RecoveryBlocked);
                }
            }
            _ => return Err(PublicationError::RecoveryBlocked),
        }

        if stage_not_applied
            && !compensation_required
            && phase_at_or_after(recorded_phase, PublicationPhase::NewPointsAcknowledged)
        {
            return Err(PublicationError::RecoveryBlocked);
        }

        if transaction.closure_receipt.is_some() {
            let (verify_command, verified) = readback_only(
                transaction,
                readback,
                context,
                PublicationEffectKind::VerifyReadback,
            )?;
            match verified.outcome {
                PublicationEffectOutcome::Applied => {
                    exact_points(&verify_command, &verified)?;
                    let retired = (!transaction.prepared.difference.retire.is_empty())
                        .then_some(transaction.prepared.old_manifest_digest)
                        .flatten();
                    if !transaction.prepared.difference.retire.is_empty() && retired.is_none() {
                        return Err(PublicationError::RecoveryBlocked);
                    }
                    transaction.verified = Some(ReadbackVerified {
                        transaction_id: transaction.prepared.transaction_id.clone(),
                        target_epoch: transaction.target_epoch,
                        new_manifest_digest: transaction.prepared.new_manifest_digest,
                        retired_manifest_digest: retired,
                    });
                    transaction.phase = PublicationPhase::ReadbackVerified;
                }
                PublicationEffectOutcome::NotApplied
                    if phase_at_or_after(recorded_phase, PublicationPhase::ReadbackVerified)
                        && !compensation_required =>
                {
                    return Err(PublicationError::RecoveryBlocked);
                }
                PublicationEffectOutcome::NotApplied => {}
                _ => return Err(PublicationError::RecoveryBlocked),
            }
        } else if phase_at_or_after(recorded_phase, PublicationPhase::ReadbackVerified) {
            return Err(PublicationError::RecoveryBlocked);
        }

        // This fresh exact control readback accepts the lost-CAS-ack case even
        // when the durable phase still says ReadbackVerified.
        let (commit_command, commit_readback) = readback_only(
            transaction,
            readback,
            context,
            PublicationEffectKind::CommitVisibleEpoch,
        )?;
        let control = commit_readback
            .control
            .ok_or(PublicationError::ControlConflict)?;
        let intent_generation = transaction
            .durable_intent
            .as_ref()
            .ok_or(PublicationError::ControlConflict)?
            .control_generation;
        validate_control(
            transaction,
            &commit_command,
            &control,
            intent_generation,
            commit_readback.outcome,
        )?;
        let committed_now = commit_readback.outcome == PublicationEffectOutcome::Applied;
        if committed_now && compensation_required {
            return Err(PublicationError::RecoveryBlocked);
        }
        if matches!(
            recorded_phase,
            PublicationPhase::Aborted | PublicationPhase::PublicationBlocked
        ) {
            transaction.phase = PublicationPhase::PublicationBlocked;
            return Ok((coordinator, PublicationRecoveryDecision::PublicationBlocked));
        }
        if matches!(recorded_phase, PublicationPhase::Compensating) && !committed_now {
            transaction.phase = PublicationPhase::Compensating;
            return Ok((coordinator, PublicationRecoveryDecision::CompensateExact));
        }
        if committed_now {
            if transaction.verified.is_none() {
                return Err(PublicationError::RecoveryBlocked);
            }
            let verified = transaction
                .verified
                .as_ref()
                .ok_or(PublicationError::RecoveryBlocked)?;
            transaction.visible_commit = Some(VisibleCommitReceipt {
                transaction_id: transaction.prepared.transaction_id.clone(),
                visible_epoch: transaction.target_epoch,
                visible_manifest_digest: transaction.prepared.new_manifest_digest,
                retired_manifest_digest: verified.retired_manifest_digest,
                control_generation: control.after_control_generation,
                control_state_digest: control.control_state_digest,
            });
            transaction.phase = PublicationPhase::ControlCommitted;
            coordinator.visible_epoch = transaction.target_epoch;
        }

        if !committed_now {
            if phase_at_or_after(recorded_phase, PublicationPhase::ControlCommitted) {
                return Err(PublicationError::RecoveryBlocked);
            }
            if observed.current_guards != transaction.prepared.guards
                || compensation_required
                || matches!(recorded_phase, PublicationPhase::Compensating)
            {
                transaction.phase = PublicationPhase::Compensating;
                return Ok((coordinator, PublicationRecoveryDecision::CompensateExact));
            }
            if matches!(
                recorded_phase,
                PublicationPhase::Aborted | PublicationPhase::PublicationBlocked
            ) {
                transaction.phase = PublicationPhase::PublicationBlocked;
                return Ok((coordinator, PublicationRecoveryDecision::PublicationBlocked));
            }
            return Ok((coordinator, PublicationRecoveryDecision::Continue));
        }

        // A previous-process snapshot label is not proof; query this process's port.
        let (snapshot_command, snapshot_readback) = readback_only(
            transaction,
            readback,
            context,
            PublicationEffectKind::PublishControlSnapshot,
        )?;
        match snapshot_readback.outcome {
            PublicationEffectOutcome::Applied => {
                let snapshot = snapshot_readback
                    .snapshot
                    .ok_or(PublicationError::SnapshotPublicationFailed)?;
                let commit = transaction
                    .visible_commit
                    .as_ref()
                    .ok_or(PublicationError::ControlConflict)?;
                let expected_manifest = if matches!(
                    transaction.prepared.target,
                    crate::admission::PublicationTarget::Retirement(_)
                ) {
                    None
                } else {
                    Some(transaction.prepared.new_manifest_digest)
                };
                if snapshot_readback.binding != *snapshot_command.binding()
                    || snapshot.visible_epoch != commit.visible_epoch
                    || snapshot.control_generation != commit.control_generation
                    || snapshot.guards != transaction.prepared.guards
                    || snapshot.collection_generation_id
                        != transaction.prepared.collection_generation_id
                    || snapshot.visible_manifest_digest != expected_manifest
                {
                    return Err(PublicationError::SnapshotPublicationFailed);
                }
                transaction.snapshot_receipt = Some(SnapshotPublishReceipt {
                    transaction_id: transaction.prepared.transaction_id.clone(),
                    visible_epoch: snapshot.visible_epoch,
                    control_generation: snapshot.control_generation,
                    process_snapshot_id: snapshot.process_snapshot_id,
                    snapshot_digest: snapshot.snapshot_digest,
                });
                transaction.phase = PublicationPhase::SnapshotPublished;
                Ok((coordinator, PublicationRecoveryDecision::Continue))
            }
            PublicationEffectOutcome::NotApplied => {
                Ok((coordinator, PublicationRecoveryDecision::PublishSnapshot))
            }
            _ => Err(PublicationError::OutcomeUnknown),
        }
    }
}

#[allow(clippy::too_many_lines)]
fn restore_aborted<C, R>(
    mut coordinator: PublicationCoordinator,
    readback: &mut R,
    context: &OperationContext<C>,
    intent_binding: &PublicationEffectBinding,
    persisted: AbortFinalizationReadback,
    abort_binding: PublicationEffectBinding,
) -> Result<(PublicationCoordinator, PublicationRecoveryDecision), PublicationError>
where
    C: CancellationProbe,
    R: PublicationReadbackPort<C>,
{
    let transaction = coordinator
        .active
        .as_mut()
        .ok_or(PublicationError::RecoveryBlocked)?;
    // The external adapter supplies only data. Rebuild the command with the exact
    // original intent already validated above; never ask it to mint private proof.
    let request = AbortFinalizationRequest {
        operation_id: persisted.operation_id,
        expected_control_generation: persisted.expected_control_generation,
        collection_generation_id: persisted.collection_generation_id,
        previous_visible_epoch: persisted.previous_visible_epoch,
        current_guards: persisted.current_guards,
        prepared_input_digest: persisted.prepared_input_digest,
        resolution: persisted.resolution,
        intent: transaction
            .durable_intent
            .clone()
            .ok_or(PublicationError::RecoveryBlocked)?,
    };
    crate::recovery::validate_abort_finalization_request(transaction, &request)?;

    validate_abort_resolution_readback(transaction, readback, context, intent_binding, &request)?;

    let mut command =
        super::abort::abort_commit_command(transaction, &request, &abort_binding, context)?;
    command.abort_request = Some(request.clone());
    context
        .preflight()
        .map_err(|_| PublicationError::OperationContextInvalid)?;
    let observed = readback.readback(&command, context);
    context
        .preflight()
        .map_err(|_| PublicationError::OperationContextInvalid)?;
    if observed.effect != PublicationEffectKind::CommitAbortFinalization
        || observed.binding != abort_binding
        || observed.abort_request.as_ref() != Some(&AbortFinalizationReadback::from(&request))
        || observed.abort_binding.as_ref() != Some(&abort_binding)
        || observed.intent.is_some()
        || observed.snapshot.is_some()
        || observed.abandonment.is_some()
        || !observed.points.is_empty()
        || !observed.missing_point_ids.is_empty()
        || !observed.unexpected_point_ids.is_empty()
    {
        return Err(PublicationError::OperationMismatch);
    }
    let control = observed.control.ok_or(PublicationError::ControlConflict)?;
    let intent_generation = transaction
        .durable_intent
        .as_ref()
        .ok_or(PublicationError::ControlConflict)?
        .control_generation;
    match observed.outcome {
        PublicationEffectOutcome::Applied => {
            crate::recovery::validate_abort_control_readback(
                transaction,
                intent_generation,
                Some(&request),
                &control,
                PublicationEffectOutcome::Applied,
            )?;
            coordinator.visible_epoch = control.after_visible_epoch;
            coordinator.last_reserved_epoch = control.last_reserved_epoch;
            coordinator
                .current_manifest
                .clone_from(&transaction.prepared.old_manifest);
            coordinator.current_manifest_digest = transaction.prepared.old_manifest_digest;
            coordinator.abort_finalization.resolution = Some(request.resolution.clone());
            coordinator.abort_finalization.request = Some(request.clone());
            coordinator.abort_finalization.commit_binding = Some(abort_binding);
            coordinator.abort_finalization.commit = Some(AbortControlCommitObservation {
                request,
                collection_generation_id: control.collection_generation_id,
                visible_epoch: control.after_visible_epoch,
                last_reserved_epoch: control.last_reserved_epoch,
                observed_guards: control.guards,
                control_generation: control.after_control_generation,
                control_state_digest: control.control_state_digest,
            });
            transaction.phase = PublicationPhase::Aborted;
            transaction.pending_effect = None;
            transaction.original_context =
                Some(super::OperationContextBinding::from_context(context));
            Ok((
                coordinator,
                PublicationRecoveryDecision::PublishAbortSnapshot,
            ))
        }
        PublicationEffectOutcome::NotApplied => {
            crate::recovery::validate_abort_control_readback(
                transaction,
                intent_generation,
                Some(&request),
                &control,
                PublicationEffectOutcome::NotApplied,
            )?;
            coordinator.visible_epoch = control.after_visible_epoch;
            coordinator.last_reserved_epoch = control.last_reserved_epoch;
            coordinator
                .current_manifest
                .clone_from(&transaction.prepared.old_manifest);
            coordinator.current_manifest_digest = transaction.prepared.old_manifest_digest;
            coordinator.abort_finalization.resolution = Some(request.resolution.clone());
            coordinator.abort_finalization.request = Some(request);
            coordinator.abort_finalization.commit_binding = Some(abort_binding);
            coordinator.abort_finalization.retry_control = Some(control);
            coordinator.abort_finalization.commit = None;
            coordinator.abort_finalization.snapshot = None;
            transaction.phase = PublicationPhase::Aborted;
            transaction.pending_effect = None;
            transaction.original_context =
                Some(super::OperationContextBinding::from_context(context));
            Ok((
                coordinator,
                PublicationRecoveryDecision::PrepareAbortFinalization,
            ))
        }
        PublicationEffectOutcome::Conflict => Err(PublicationError::ControlConflict),
        PublicationEffectOutcome::Unknown => Err(PublicationError::OutcomeUnknown),
    }
}

fn validate_abort_resolution_readback<C, R>(
    transaction: &PublicationTransaction,
    readback: &mut R,
    context: &OperationContext<C>,
    intent_binding: &PublicationEffectBinding,
    request: &AbortFinalizationRequest,
) -> Result<(), PublicationError>
where
    C: CancellationProbe,
    R: PublicationReadbackPort<C>,
{
    let (effect, resolution_operation_id) = match &request.resolution {
        AbortedPublicationResolution::Compensated { operation_id, .. } => {
            (PublicationEffectKind::Compensate, operation_id)
        }
        AbortedPublicationResolution::Excluded { operation_id, .. } => {
            (PublicationEffectKind::Abandon, operation_id)
        }
    };
    let expected_operation_id = operation_id(transaction, effect)?;
    if resolution_operation_id != &expected_operation_id {
        return Err(PublicationError::OperationMismatch);
    }
    let command = original_context_command(
        transaction,
        effect,
        context,
        intent_binding,
        expected_operation_id,
    )?;
    context
        .preflight()
        .map_err(|_| PublicationError::OperationContextInvalid)?;
    let observed = readback.readback(&command, context);
    context
        .preflight()
        .map_err(|_| PublicationError::OperationContextInvalid)?;
    if observed.points.len() > transaction.max_points
        || observed.missing_point_ids.len() > transaction.max_points
        || observed.unexpected_point_ids.len() > transaction.max_points
    {
        return Err(PublicationError::BudgetExceeded);
    }
    if observed.effect != effect
        || observed.binding != command.binding
        || observed.outcome != PublicationEffectOutcome::Applied
        || !observed.unexpected_point_ids.is_empty()
        || observed.intent.is_some()
        || observed.control.is_some()
        || observed.snapshot.is_some()
        || observed.abort_request.is_some()
        || observed.abort_binding.is_some()
    {
        return Err(PublicationError::RecoveryBlocked);
    }
    match &request.resolution {
        AbortedPublicationResolution::Compensated {
            restored_old_points,
            ..
        } => {
            let expected_restore = !transaction.prepared.difference.retire.is_empty();
            if *restored_old_points != expected_restore
                || observed.abandonment.is_some()
                || !point_readback_matches(&command.points, &observed.points)
                || observed.missing_point_ids != command.required_absent_point_ids
            {
                return Err(PublicationError::CompensationIncomplete);
            }
        }
        AbortedPublicationResolution::Excluded {
            excluded_projection_memberships,
            ..
        } => {
            let difference = &transaction.prepared.difference;
            let mut point_ids = difference
                .create
                .iter()
                .chain(&difference.retire)
                .map(|entry| entry.point_id)
                .collect::<Vec<_>>();
            point_ids.sort_unstable();
            let mut memberships = difference
                .create
                .iter()
                .chain(&difference.retire)
                .map(|entry| entry.projection_membership_id)
                .collect::<Vec<_>>();
            memberships.sort_unstable();
            memberships.dedup();
            let fence = observed
                .abandonment
                .as_ref()
                .ok_or(PublicationError::AbandonFenceMissing)?;
            if fence.binding != command.binding
                || fence.excluded_point_ids != point_ids
                || fence.excluded_projection_memberships != memberships
                || excluded_projection_memberships != &memberships
                || !observed.points.is_empty()
                || !observed.missing_point_ids.is_empty()
            {
                return Err(PublicationError::AbandonFenceMissing);
            }
        }
    }
    Ok(())
}

fn original_context_command<C: CancellationProbe>(
    transaction: &PublicationTransaction,
    effect: PublicationEffectKind,
    context: &OperationContext<C>,
    original_intent_binding: &PublicationEffectBinding,
    operation_id: search_contracts::OpaqueId,
) -> Result<PublicationEffectCommand, PublicationError> {
    let mut command = effect_command(transaction, effect, context)?;
    command.binding.request_id = original_intent_binding.request_id;
    command.binding.relative_deadline_ms = original_intent_binding.relative_deadline_ms;
    command
        .binding
        .budget_ref
        .clone_from(&original_intent_binding.budget_ref);
    command.binding.operation_id = operation_id;
    Ok(command)
}

fn exact_intent_for_restore(
    command: &PublicationEffectCommand,
    observed: &PublicationEffectReadback,
) -> Result<PublicationIntentReadback, PublicationError> {
    if observed.effect != PublicationEffectKind::PersistIntent
        || observed.outcome != PublicationEffectOutcome::Applied
        || !observed.points.is_empty()
        || !observed.missing_point_ids.is_empty()
        || !observed.unexpected_point_ids.is_empty()
        || observed.control.is_some()
        || observed.snapshot.is_some()
        || observed.abandonment.is_some()
    {
        return Err(PublicationError::RecoveryBlocked);
    }
    let intent = observed
        .intent
        .as_ref()
        .ok_or(PublicationError::RecoveryBlocked)?;
    let mut expected_binding = command.binding().clone();
    expected_binding.request_id = intent.binding.request_id;
    expected_binding.relative_deadline_ms = intent.binding.relative_deadline_ms;
    expected_binding
        .budget_ref
        .clone_from(&intent.binding.budget_ref);
    if intent.binding != expected_binding
        || observed.binding != intent.binding
        || intent.control_generation == 0
        || intent.binding.relative_deadline_ms == 0
        || observed.abort_request.is_some() != observed.abort_binding.is_some()
        || (observed.abort_request.is_some() && intent.recorded_phase != PublicationPhase::Aborted)
    {
        return Err(PublicationError::OperationMismatch);
    }
    Ok(intent.clone())
}

fn exact_points(
    command: &PublicationEffectCommand,
    observed: &PublicationEffectReadback,
) -> Result<(), PublicationError> {
    if observed.binding != *command.binding()
        || !observed.missing_point_ids.is_empty()
        || !observed.unexpected_point_ids.is_empty()
        || !point_readback_matches(command.points(), &observed.points)
    {
        return Err(PublicationError::RecoveryBlocked);
    }
    Ok(())
}

fn exact_absence(
    command: &PublicationEffectCommand,
    observed: &PublicationEffectReadback,
) -> Result<(), PublicationError> {
    let expected_ids = command
        .points()
        .iter()
        .map(PublicationPointExpectation::point_id)
        .collect::<Vec<_>>();
    if observed.binding != *command.binding()
        || !observed.points.is_empty()
        || observed.missing_point_ids != expected_ids
        || !observed.unexpected_point_ids.is_empty()
    {
        return Err(PublicationError::RecoveryBlocked);
    }
    Ok(())
}

fn exact_original_points(
    command: &PublicationEffectCommand,
    observed: &PublicationEffectReadback,
) -> Result<(), PublicationError> {
    if observed.binding != *command.binding()
        || !observed.missing_point_ids.is_empty()
        || !observed.unexpected_point_ids.is_empty()
        || !point_readback_matches(command.original_points(), &observed.points)
    {
        return Err(PublicationError::RecoveryBlocked);
    }
    Ok(())
}

fn validate_control(
    transaction: &PublicationTransaction,
    command: &PublicationEffectCommand,
    control: &PublicationControlReadback,
    intent_generation: u64,
    outcome: PublicationEffectOutcome,
) -> Result<(), PublicationError> {
    let committed = outcome == PublicationEffectOutcome::Applied;
    let expected_manifest = if matches!(
        transaction.prepared.target,
        crate::admission::PublicationTarget::Retirement(_)
    ) {
        None
    } else {
        Some(transaction.prepared.new_manifest_digest)
    };
    let expected_current_manifest = if committed {
        expected_manifest
    } else {
        transaction.prepared.old_manifest_digest
    };
    let expected_visible = if committed {
        transaction.target_epoch
    } else {
        transaction.previous_visible_epoch
    };
    let expected_guards = if committed {
        transaction.prepared.guards
    } else {
        transaction.prepared.observed.current_guards
    };
    if command.effect() != PublicationEffectKind::CommitVisibleEpoch
        || control.before_visible_epoch != transaction.previous_visible_epoch
        || control.after_visible_epoch != expected_visible
        || control.last_reserved_epoch != transaction.target_epoch
        || control.guards != expected_guards
        || control.collection_generation_id != transaction.prepared.collection_generation_id
        || control.previous_visible_manifest_digest != transaction.prepared.old_manifest_digest
        || control.visible_manifest_digest != expected_current_manifest
        || validate_control_generation_transition(control, Some(intent_generation), outcome)
            .is_err()
    {
        return Err(PublicationError::ControlConflict);
    }
    Ok(())
}

fn phase_at_or_after(phase: PublicationPhase, threshold: PublicationPhase) -> bool {
    use PublicationPhase as P;
    let rank = |phase| match phase {
        P::Prepared | P::Compensating | P::Aborted | P::PublicationBlocked => 0,
        P::IntentDurable => 1,
        P::NewPointsAcknowledged => 2,
        P::OldPointsClosedAcknowledged => 3,
        P::ReadbackVerified => 4,
        P::ControlCommitted => 5,
        P::SnapshotPublished => 6,
    };
    rank(phase) >= rank(threshold)
}

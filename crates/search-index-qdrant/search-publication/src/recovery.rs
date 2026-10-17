//! Reconciliation of unknown publication effects through exact owner readback.

use search_ports::{CancellationProbe, OperationContext};

use crate::PublicationPointExpectation;
use crate::machine::{
    AbortFinalizationRequest, DurableIntent, PublicationPhase, PublicationReadbackPort,
    PublicationTransaction, point_readback_matches, readback_only,
    validate_control_generation_transition,
};
use crate::model::{
    ClosureReceipt, PublicationControlReadback, PublicationEffectKind, PublicationEffectOutcome,
    PublicationEffectReadback, PublicationGuards, PublicationIntentReadback,
    PublicationSnapshotReadback, ReadbackVerified, SnapshotPublishReceipt, StageReceipt,
    VisibleCommitReceipt,
};
use crate::{PublicationError, PublicationRecoveryDecision};

pub fn validate_abort_finalization_request(
    transaction: &PublicationTransaction,
    request: &AbortFinalizationRequest,
) -> Result<(), PublicationError> {
    let prepared = &transaction.prepared;
    let intent = &request.intent;
    let resolution_operation_id = match &request.resolution {
        crate::machine::AbortedPublicationResolution::Compensated { operation_id, .. }
        | crate::machine::AbortedPublicationResolution::Excluded { operation_id, .. } => {
            operation_id
        }
    };
    if request.operation_id == *intent.persist_operation_id()
        || &request.operation_id == resolution_operation_id
        || request.expected_control_generation == 0
        || request.expected_control_generation.checked_add(1).is_none()
        || request.collection_generation_id != prepared.scope().collection_generation_id
        || request.previous_visible_epoch != transaction.previous_visible_epoch
        || intent.transaction_id() != transaction.transaction_id()
        || intent.target_epoch() != transaction.target_epoch
        || intent.old_manifest_digest() != prepared.old_manifest_digest()
        || intent.new_manifest_digest() != prepared.new_manifest_digest()
        || intent.guards() != prepared.guards
        || request.prepared_input_digest != prepared.prepared_input_digest()
    {
        return Err(PublicationError::OperationMismatch);
    }
    if !abort_guards_are_current_or_newer(request.current_guards, intent.guards()) {
        return Err(PublicationError::GuardMismatch);
    }
    Ok(())
}

pub fn validate_abort_control_readback(
    transaction: &PublicationTransaction,
    intent_generation: u64,
    request: Option<&AbortFinalizationRequest>,
    control: &PublicationControlReadback,
    outcome: PublicationEffectOutcome,
) -> Result<(), PublicationError> {
    if let Some(request) = request {
        validate_abort_finalization_request(transaction, request)?;
    }
    let old_manifest = transaction.prepared.old_manifest_digest;
    if control.before_visible_epoch != transaction.previous_visible_epoch
        || control.after_visible_epoch != transaction.previous_visible_epoch
        || control.last_reserved_epoch != transaction.target_epoch
        || control.collection_generation_id != transaction.prepared.collection_generation_id
        || control.previous_visible_manifest_digest != old_manifest
        || control.visible_manifest_digest != old_manifest
    {
        return Err(PublicationError::ControlConflict);
    }
    if !abort_guards_are_current_or_newer(control.guards, transaction.prepared.guards) {
        return Err(PublicationError::GuardMismatch);
    }
    match outcome {
        PublicationEffectOutcome::Applied => {
            let request = request.ok_or(PublicationError::ControlConflict)?;
            if control.guards != request.current_guards {
                return Err(PublicationError::GuardMismatch);
            }
            if control.before_control_generation != request.expected_control_generation
                || control.before_control_generation.checked_add(1)
                    != Some(control.after_control_generation)
            {
                return Err(PublicationError::ControlConflict);
            }
        }
        PublicationEffectOutcome::NotApplied => {
            if control.before_control_generation != control.after_control_generation
                || control.before_control_generation < intent_generation
            {
                return Err(PublicationError::ControlConflict);
            }
        }
        PublicationEffectOutcome::Conflict => return Err(PublicationError::ControlConflict),
        PublicationEffectOutcome::Unknown => return Err(PublicationError::OutcomeUnknown),
    }
    validate_control_generation_transition(control, Some(intent_generation), outcome)
}

fn abort_guards_are_current_or_newer(
    current: PublicationGuards,
    original: PublicationGuards,
) -> bool {
    current.owner_epoch >= original.owner_epoch
        && current.source_catalog_generation >= original.source_catalog_generation
        && current.membership_generation >= original.membership_generation
        && current.access_generation >= original.access_generation
        && current.shadow_generation >= original.shadow_generation
        && current.purge_generation >= original.purge_generation
}

/// Reconcile a single active transaction without repeating any mutation.
/// The effect port is deliberately absent: unknown outcomes require readback
/// under the original phase operation identity before another attempt is allowed.
#[allow(clippy::too_many_lines)]
pub fn recover_active<C, R>(
    transaction: &mut PublicationTransaction,
    readback: &mut R,
    context: &OperationContext<C>,
) -> Result<(PublicationRecoveryDecision, Option<search_contracts::Epoch>), PublicationError>
where
    C: CancellationProbe,
    R: PublicationReadbackPort<C>,
{
    use PublicationEffectKind as Effect;
    use PublicationEffectOutcome as Outcome;
    use PublicationPhase as Phase;
    use PublicationRecoveryDecision as Decision;

    let pending_effect_for_phase = match transaction.phase {
        Phase::Prepared => Some(Effect::PersistIntent),
        Phase::IntentDurable => Some(Effect::StageNewPoints),
        Phase::NewPointsAcknowledged => Some(Effect::CloseOldPoints),
        Phase::OldPointsClosedAcknowledged => Some(Effect::VerifyReadback),
        Phase::ReadbackVerified => Some(Effect::CommitVisibleEpoch),
        Phase::ControlCommitted | Phase::SnapshotPublished => Some(Effect::PublishControlSnapshot),
        Phase::Compensating => Some(Effect::Compensate),
        Phase::Aborted | Phase::PublicationBlocked => None,
    };
    if transaction
        .pending_effect
        .is_some_and(|pending| Some(pending) != pending_effect_for_phase)
    {
        return Err(PublicationError::OutcomeUnknown);
    }

    let (intent_command, intent_observation) =
        readback_only(transaction, readback, context, Effect::PersistIntent)?;
    match intent_observation.outcome {
        Outcome::Applied => {
            let Ok(intent) = exact_intent(&intent_command, intent_observation) else {
                return Ok(blocked(transaction));
            };
            if let Some(retained) = &transaction.durable_intent {
                if intent.binding.operation_id != retained.persist_operation_id
                    || intent.binding.request_id != retained.request_id
                    || intent.binding.target_epoch != retained.target_epoch
                    || intent.binding.old_manifest_digest != retained.old_manifest_digest
                    || intent.binding.new_manifest_digest != retained.new_manifest_digest
                    || intent.binding.guards != retained.guards
                    || intent.control_generation != retained.control_generation
                    || intent.control_state_digest != retained.control_state_digest
                {
                    return Ok(blocked(transaction));
                }
            } else if transaction.phase == Phase::Prepared {
                transaction.durable_intent = Some(durable_intent(&transaction.prepared, &intent));
                transaction.pending_effect = None;
                transaction.phase = Phase::IntentDurable;
            } else {
                return Ok(blocked(transaction));
            }
        }
        Outcome::NotApplied
            if transaction.phase == Phase::Prepared && transaction.durable_intent.is_none() =>
        {
            if intent_observation.intent.is_some()
                || !intent_observation.points.is_empty()
                || !intent_observation.missing_point_ids.is_empty()
                || !intent_observation.unexpected_point_ids.is_empty()
            {
                return Ok(blocked(transaction));
            }
            let (stage_command, stage) =
                readback_only(transaction, readback, context, Effect::StageNewPoints)?;
            if stage.outcome != Outcome::NotApplied
                || exact_absence(&stage_command, &stage).is_err()
            {
                return Ok(blocked(transaction));
            }
            let (closure_command, closure) =
                readback_only(transaction, readback, context, Effect::CloseOldPoints)?;
            if !matches!(closure.outcome, Outcome::NotApplied | Outcome::Applied)
                || exact_original_points(&closure_command, &closure).is_err()
                || (!closure_command.points().is_empty() && closure.outcome != Outcome::NotApplied)
            {
                return Ok(blocked(transaction));
            }
            let (control_command, control_observation) =
                readback_only(transaction, readback, context, Effect::CommitVisibleEpoch)?;
            let Some(control) = control_observation.control else {
                return Ok(blocked(transaction));
            };
            if control_observation.outcome != Outcome::NotApplied
                || exact_control(transaction, &control_command, &control, Outcome::NotApplied)
                    .is_err()
            {
                return Ok(blocked(transaction));
            }
            transaction.pending_effect = None;
            return Ok((Decision::Continue, None));
        }
        _ => return Ok(blocked(transaction)),
    }

    if matches!(
        transaction.phase,
        Phase::ControlCommitted | Phase::SnapshotPublished
    ) {
        let (intent_command, intent_observation) =
            readback_only(transaction, readback, context, Effect::PersistIntent)?;
        if intent_observation.outcome != Outcome::Applied {
            return Ok(blocked(transaction));
        }
        let intent = exact_intent(&intent_command, intent_observation)?;
        let retained = transaction
            .durable_intent
            .as_ref()
            .ok_or(PublicationError::RecoveryBlocked)?;
        if intent.binding.operation_id != retained.persist_operation_id
            || intent.binding.request_id != retained.request_id
            || intent.binding.target_epoch != retained.target_epoch
            || intent.control_generation != retained.control_generation
            || intent.control_state_digest != retained.control_state_digest
        {
            return Ok(blocked(transaction));
        }
        for effect in [
            Effect::StageNewPoints,
            Effect::CloseOldPoints,
            Effect::VerifyReadback,
        ] {
            let (command, observed) = readback_only(transaction, readback, context, effect)?;
            if observed.outcome != Outcome::Applied || exact_points(&command, &observed).is_err() {
                return Ok(blocked(transaction));
            }
        }
        let (commit_command, commit_observation) =
            readback_only(transaction, readback, context, Effect::CommitVisibleEpoch)?;
        let Some(control) = commit_observation.control else {
            return Ok(blocked(transaction));
        };
        if commit_observation.outcome != Outcome::Applied
            || exact_control(transaction, &commit_command, &control, Outcome::Applied).is_err()
            || transaction.visible_commit.as_ref() != Some(&visible_receipt(transaction, control)?)
        {
            return Ok(blocked(transaction));
        }
    }
    loop {
        match transaction.phase {
            Phase::Prepared => {
                let (command, observed) =
                    readback_only(transaction, readback, context, Effect::PersistIntent)?;
                match observed.outcome {
                    Outcome::Applied => {
                        let Ok(intent) = exact_intent(&command, observed) else {
                            return Ok(blocked(transaction));
                        };
                        transaction.durable_intent =
                            Some(durable_intent(&transaction.prepared, &intent));
                        transaction.pending_effect = None;
                        transaction.phase = Phase::IntentDurable;
                    }
                    Outcome::NotApplied => return Ok((Decision::Continue, None)),
                    _ => return Ok(blocked(transaction)),
                }
            }
            Phase::IntentDurable => {
                let (command, observed) =
                    readback_only(transaction, readback, context, Effect::StageNewPoints)?;
                match classify_stage(&command, &observed) {
                    PointState::Complete => {
                        transaction.pending_effect = None;
                        transaction.stage_receipt = Some(StageReceipt {
                            transaction_id: transaction.prepared.transaction_id.clone(),
                            target_epoch: transaction.target_epoch,
                            points: observed.points,
                        });
                        transaction.phase = Phase::NewPointsAcknowledged;
                    }
                    PointState::Absent => {
                        transaction.pending_effect = None;
                        let (closure_command, closure) =
                            readback_only(transaction, readback, context, Effect::CloseOldPoints)?;
                        match classify_closure(&closure_command, &closure) {
                            PointState::Complete if closure_command.points().is_empty() => {
                                let (control_command, control_observation) = readback_only(
                                    transaction,
                                    readback,
                                    context,
                                    Effect::CommitVisibleEpoch,
                                )?;
                                let Some(control) = control_observation.control else {
                                    return Ok(blocked(transaction));
                                };
                                if control_observation.outcome != Outcome::NotApplied
                                    || exact_control(
                                        transaction,
                                        &control_command,
                                        &control,
                                        Outcome::NotApplied,
                                    )
                                    .is_err()
                                {
                                    return Ok(blocked(transaction));
                                }
                                return Ok((Decision::Continue, None));
                            }
                            PointState::Complete | PointState::Partial => {
                                transaction.phase = Phase::Compensating;
                                return Ok((Decision::CompensateExact, None));
                            }
                            PointState::Conflict => return Ok(blocked(transaction)),
                            PointState::Absent => {
                                let (control_command, control_observation) = readback_only(
                                    transaction,
                                    readback,
                                    context,
                                    Effect::CommitVisibleEpoch,
                                )?;
                                let Some(control) = control_observation.control else {
                                    return Ok(blocked(transaction));
                                };
                                if control_observation.outcome != Outcome::NotApplied
                                    || exact_control(
                                        transaction,
                                        &control_command,
                                        &control,
                                        Outcome::NotApplied,
                                    )
                                    .is_err()
                                {
                                    return Ok(blocked(transaction));
                                }
                                if transaction.prepared.observed.current_guards
                                    != transaction.prepared.guards
                                {
                                    transaction.phase = Phase::Compensating;
                                    return Ok((Decision::CompensateExact, None));
                                }
                                return Ok((Decision::Continue, None));
                            }
                        }
                    }
                    PointState::Partial => {
                        transaction.pending_effect = None;
                        transaction.phase = Phase::Compensating;
                        return Ok((Decision::CompensateExact, None));
                    }
                    PointState::Conflict => return Ok(blocked(transaction)),
                }
            }
            Phase::NewPointsAcknowledged => {
                let (command, observed) =
                    readback_only(transaction, readback, context, Effect::CloseOldPoints)?;
                match classify_closure(&command, &observed) {
                    PointState::Complete => {
                        transaction.pending_effect = None;
                        transaction.closure_receipt = Some(ClosureReceipt {
                            transaction_id: transaction.prepared.transaction_id.clone(),
                            target_epoch: transaction.target_epoch,
                            points: observed.points,
                        });
                        transaction.phase = Phase::OldPointsClosedAcknowledged;
                    }
                    PointState::Absent => {
                        transaction.pending_effect = None;
                        return Ok((Decision::Continue, None));
                    }
                    PointState::Partial => {
                        transaction.pending_effect = None;
                        transaction.phase = Phase::Compensating;
                        return Ok((Decision::CompensateExact, None));
                    }
                    PointState::Conflict => return Ok(blocked(transaction)),
                }
            }
            Phase::OldPointsClosedAcknowledged => {
                let (command, observed) =
                    readback_only(transaction, readback, context, Effect::VerifyReadback)?;
                match observed.outcome {
                    Outcome::Applied if exact_points(&command, &observed).is_ok() => {
                        transaction.pending_effect = None;
                        transaction.verified = Some(verified(transaction)?);
                        transaction.phase = Phase::ReadbackVerified;
                    }
                    Outcome::NotApplied => return Ok((Decision::Continue, None)),
                    _ => return Ok(blocked(transaction)),
                }
            }
            Phase::ReadbackVerified => {
                let (command, observed) =
                    readback_only(transaction, readback, context, Effect::CommitVisibleEpoch)?;
                match observed.outcome {
                    Outcome::Applied => {
                        let control = observed.control.ok_or(PublicationError::ControlConflict)?;
                        if exact_control(transaction, &command, &control, Outcome::Applied).is_err()
                        {
                            return Ok(blocked(transaction));
                        }
                        transaction.pending_effect = None;
                        transaction.visible_commit = Some(visible_receipt(transaction, control)?);
                        transaction.phase = Phase::ControlCommitted;
                        return Ok((Decision::PublishSnapshot, Some(transaction.target_epoch)));
                    }
                    Outcome::NotApplied => {
                        let control = observed.control.ok_or(PublicationError::ControlConflict)?;
                        if exact_control(transaction, &command, &control, Outcome::NotApplied)
                            .is_err()
                        {
                            return Ok(blocked(transaction));
                        }
                        transaction.pending_effect = None;
                        if transaction.prepared.observed.current_guards
                            != transaction.prepared.guards
                        {
                            transaction.phase = Phase::Compensating;
                            return Ok((Decision::CompensateExact, None));
                        }
                        return Ok((Decision::Continue, None));
                    }
                    _ => return Ok(blocked(transaction)),
                }
            }
            Phase::ControlCommitted | Phase::SnapshotPublished => {
                let (command, observed) = readback_only(
                    transaction,
                    readback,
                    context,
                    Effect::PublishControlSnapshot,
                )?;
                match observed.outcome {
                    Outcome::Applied => {
                        let snapshot = observed
                            .snapshot
                            .ok_or(PublicationError::SnapshotPublicationFailed)?;
                        if exact_snapshot(transaction, &command, &snapshot).is_err() {
                            return Ok(blocked(transaction));
                        }
                        transaction.pending_effect = None;
                        let receipt = snapshot_receipt(transaction, snapshot)?;
                        if transaction
                            .snapshot_receipt
                            .as_ref()
                            .is_some_and(|previous| previous != &receipt)
                        {
                            return Ok(blocked(transaction));
                        }
                        transaction.snapshot_receipt = Some(receipt);
                        transaction.phase = Phase::SnapshotPublished;
                        return Ok((Decision::Continue, None));
                    }
                    Outcome::NotApplied => {
                        transaction.pending_effect = None;
                        transaction.phase = Phase::ControlCommitted;
                        return Ok((Decision::PublishSnapshot, None));
                    }
                    _ => return Ok(blocked(transaction)),
                }
            }
            Phase::Compensating => return Ok((Decision::CompensateExact, None)),
            Phase::Aborted | Phase::PublicationBlocked => {
                return Ok((Decision::PublicationBlocked, None));
            }
        }
    }
}

/// Classifies one transaction using only owner readback. It does not mutate
/// the supplied snapshot; use `PublicationCoordinator::recover` to retain any
/// newly verified phase in the live coordinator.
pub fn recover<C, R>(
    transaction: &PublicationTransaction,
    readback: &mut R,
    context: &OperationContext<C>,
) -> Result<PublicationRecoveryDecision, PublicationError>
where
    C: CancellationProbe,
    R: PublicationReadbackPort<C>,
{
    let mut snapshot = transaction.clone();
    recover_active(&mut snapshot, readback, context).map(|(decision, _)| decision)
}

fn exact_intent(
    command: &crate::machine::PublicationEffectCommand,
    observed: PublicationEffectReadback,
) -> Result<PublicationIntentReadback, PublicationError> {
    if observed.binding != *command.binding()
        || observed.effect != PublicationEffectKind::PersistIntent
        || !observed.points.is_empty()
        || !observed.missing_point_ids.is_empty()
        || !observed.unexpected_point_ids.is_empty()
    {
        return Err(PublicationError::OperationMismatch);
    }
    let intent = observed.intent.ok_or(PublicationError::RecoveryBlocked)?;
    if intent.binding != *command.binding() || intent.control_generation == 0 {
        return Err(PublicationError::OperationMismatch);
    }
    Ok(intent)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PointState {
    Complete,
    Absent,
    Partial,
    Conflict,
}

fn classify_stage(
    command: &crate::machine::PublicationEffectCommand,
    observed: &PublicationEffectReadback,
) -> PointState {
    use PublicationEffectOutcome as O;
    if command.effect() != PublicationEffectKind::StageNewPoints
        || observed.binding != *command.binding()
        || !observed.unexpected_point_ids.is_empty()
        || !point_subset_matches(command.points(), &observed.points)
    {
        return PointState::Conflict;
    }
    let mut expected_ids = command
        .points()
        .iter()
        .map(PublicationPointExpectation::point_id)
        .collect::<Vec<_>>();
    expected_ids.sort_unstable();
    let mut actual_ids = observed
        .points
        .iter()
        .map(|point| point.point_id)
        .collect::<Vec<_>>();
    actual_ids.sort_unstable();
    if actual_ids.len() != observed.points.len() {
        return PointState::Conflict;
    }
    let mut missing = expected_ids
        .iter()
        .copied()
        .filter(|id| !actual_ids.contains(id))
        .collect::<Vec<_>>();
    missing.sort_unstable();
    if missing != observed.missing_point_ids {
        return PointState::Conflict;
    }
    if actual_ids == expected_ids && observed.outcome == O::Applied {
        PointState::Complete
    } else if actual_ids.is_empty() && missing == expected_ids && observed.outcome == O::NotApplied
    {
        PointState::Absent
    } else if actual_ids.iter().all(|id| expected_ids.contains(id)) {
        PointState::Partial
    } else {
        PointState::Conflict
    }
}

fn classify_closure(
    command: &crate::machine::PublicationEffectCommand,
    observed: &PublicationEffectReadback,
) -> PointState {
    use PublicationEffectOutcome as O;
    if command.effect() != PublicationEffectKind::CloseOldPoints
        || observed.binding != *command.binding()
        || !observed.missing_point_ids.is_empty()
        || !observed.unexpected_point_ids.is_empty()
        || observed.points.len() != command.original_points().len()
    {
        return PointState::Conflict;
    }
    let mut closed = 0usize;
    let mut original = 0usize;
    for actual in &observed.points {
        let Some(target) = command
            .points()
            .iter()
            .find(|point| point.point_id() == actual.point_id)
        else {
            return PointState::Conflict;
        };
        let Some(before) = command
            .original_points()
            .iter()
            .find(|point| point.point_id() == actual.point_id)
        else {
            return PointState::Conflict;
        };
        if point_subset_matches(std::slice::from_ref(target), std::slice::from_ref(actual)) {
            closed += 1;
        } else if point_subset_matches(std::slice::from_ref(before), std::slice::from_ref(actual)) {
            original += 1;
        } else {
            return PointState::Conflict;
        }
    }
    if closed == observed.points.len() && observed.outcome == O::Applied {
        PointState::Complete
    } else if original == observed.points.len() && observed.outcome == O::NotApplied {
        PointState::Absent
    } else if closed > 0 && original > 0 {
        PointState::Partial
    } else {
        PointState::Conflict
    }
}

fn point_subset_matches(
    expected: &[crate::PublicationPointExpectation],
    observed: &[crate::PublicationPointReadback],
) -> bool {
    observed.iter().all(|actual| {
        expected
            .iter()
            .find(|candidate| candidate.point_id() == actual.point_id)
            .is_some_and(|candidate| {
                point_readback_matches(
                    std::slice::from_ref(candidate),
                    std::slice::from_ref(actual),
                )
            })
    })
}

const fn blocked(
    transaction: &mut PublicationTransaction,
) -> (PublicationRecoveryDecision, Option<search_contracts::Epoch>) {
    transaction.phase = PublicationPhase::PublicationBlocked;
    (PublicationRecoveryDecision::PublicationBlocked, None)
}

fn durable_intent(
    prepared: &crate::PreparedGeneration,
    intent: &PublicationIntentReadback,
) -> DurableIntent {
    DurableIntent {
        transaction_id: prepared.transaction_id.clone(),
        target_epoch: prepared.target_epoch(),
        old_manifest_digest: prepared.old_manifest_digest,
        new_manifest_digest: prepared.new_manifest_digest,
        guards: prepared.guards,
        persist_operation_id: intent.binding.operation_id.clone(),
        request_id: intent.binding.request_id,
        control_generation: intent.control_generation,
        control_state_digest: intent.control_state_digest,
    }
}

fn verified(transaction: &PublicationTransaction) -> Result<ReadbackVerified, PublicationError> {
    let retired = (!transaction.prepared.difference.retire.is_empty())
        .then_some(transaction.prepared.old_manifest_digest)
        .flatten();
    if !transaction.prepared.difference.retire.is_empty() && retired.is_none() {
        return Err(PublicationError::ClosureReadbackMismatch);
    }
    Ok(ReadbackVerified {
        transaction_id: transaction.prepared.transaction_id.clone(),
        target_epoch: transaction.target_epoch,
        new_manifest_digest: transaction.prepared.new_manifest_digest,
        retired_manifest_digest: retired,
    })
}

fn exact_points(
    command: &crate::machine::PublicationEffectCommand,
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
    command: &crate::machine::PublicationEffectCommand,
    observed: &PublicationEffectReadback,
) -> Result<(), PublicationError> {
    let ids = command
        .points()
        .iter()
        .map(PublicationPointExpectation::point_id)
        .collect::<Vec<_>>();
    if observed.binding != *command.binding()
        || !observed.points.is_empty()
        || observed.missing_point_ids != ids
        || !observed.unexpected_point_ids.is_empty()
    {
        return Err(PublicationError::RecoveryBlocked);
    }
    Ok(())
}

fn exact_original_points(
    command: &crate::machine::PublicationEffectCommand,
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

fn exact_control(
    transaction: &PublicationTransaction,
    command: &crate::machine::PublicationEffectCommand,
    control: &PublicationControlReadback,
    outcome: PublicationEffectOutcome,
) -> Result<(), PublicationError> {
    if command.effect() != PublicationEffectKind::CommitVisibleEpoch
        || control.before_visible_epoch != transaction.previous_visible_epoch
        || control.after_visible_epoch
            != if outcome == PublicationEffectOutcome::Applied {
                transaction.target_epoch
            } else {
                transaction.previous_visible_epoch
            }
        || control.last_reserved_epoch != transaction.target_epoch
        || control.collection_generation_id != transaction.prepared.collection_generation_id
        || control.previous_visible_manifest_digest != transaction.prepared.old_manifest_digest
        || control.guards
            != if outcome == PublicationEffectOutcome::Applied {
                transaction.prepared.guards
            } else {
                transaction.prepared.observed.current_guards
            }
        || validate_control_generation_transition(
            control,
            transaction
                .durable_intent
                .as_ref()
                .map(|intent| intent.control_generation),
            outcome,
        )
        .is_err()
    {
        return Err(PublicationError::ControlConflict);
    }
    let expected_manifest = if outcome == PublicationEffectOutcome::Applied
        && !matches!(
            transaction.prepared.target,
            crate::admission::PublicationTarget::Retirement(_)
        ) {
        Some(transaction.prepared.new_manifest_digest)
    } else if outcome == PublicationEffectOutcome::NotApplied {
        transaction.prepared.old_manifest_digest
    } else {
        None
    };
    if control.visible_manifest_digest != expected_manifest {
        return Err(PublicationError::ControlConflict);
    }
    Ok(())
}

fn visible_receipt(
    transaction: &PublicationTransaction,
    control: PublicationControlReadback,
) -> Result<VisibleCommitReceipt, PublicationError> {
    let verified = transaction
        .verified
        .as_ref()
        .ok_or(PublicationError::StageReadbackMismatch)?;
    Ok(VisibleCommitReceipt {
        transaction_id: transaction.prepared.transaction_id.clone(),
        visible_epoch: transaction.target_epoch,
        visible_manifest_digest: transaction.prepared.new_manifest_digest,
        retired_manifest_digest: verified.retired_manifest_digest,
        control_generation: control.after_control_generation,
        control_state_digest: control.control_state_digest,
    })
}

fn exact_snapshot(
    transaction: &PublicationTransaction,
    command: &crate::machine::PublicationEffectCommand,
    snapshot: &PublicationSnapshotReadback,
) -> Result<(), PublicationError> {
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
    if command.effect() != PublicationEffectKind::PublishControlSnapshot
        || snapshot.visible_epoch != commit.visible_epoch
        || snapshot.control_generation != commit.control_generation
        || snapshot.guards != transaction.prepared.guards
        || snapshot.collection_generation_id != transaction.prepared.collection_generation_id
        || snapshot.visible_manifest_digest != expected_manifest
    {
        return Err(PublicationError::SnapshotPublicationFailed);
    }
    Ok(())
}

fn snapshot_receipt(
    transaction: &PublicationTransaction,
    snapshot: PublicationSnapshotReadback,
) -> Result<SnapshotPublishReceipt, PublicationError> {
    if snapshot.process_snapshot_id.as_str().is_empty() {
        return Err(PublicationError::SnapshotPublicationFailed);
    }
    Ok(SnapshotPublishReceipt {
        transaction_id: transaction.prepared.transaction_id.clone(),
        visible_epoch: snapshot.visible_epoch,
        control_generation: snapshot.control_generation,
        process_snapshot_id: snapshot.process_snapshot_id,
        snapshot_digest: snapshot.snapshot_digest,
    })
}

//! Fail-closed publication recovery decisions. No decision performs an effect.

use std::collections::BTreeSet;

use search_point_identity::PointId128;
use search_projection_planner::{ProjectionManifest, diff_manifests};

use crate::{
    PublicationError, PublicationPhase, PublicationRecoveryDecision,
    PublicationRecoveryObservation, PublicationTransaction,
};

/// Classifies a bounded observation already bound by its producing ports.
///
/// `Continue` means resume the normal verified steps, never skip readback or
/// authorize a commit. A bare durable-fence flag cannot establish membership/IDF
/// exclusion and never produces invalidation-only success. That path needs its
/// complete typed fence and authoritative control/index operation.
///
/// # Errors
/// Invalid/unbounded transaction manifests are errors. Contradictory or
/// insufficient external observations keep publication blocked.
pub fn recover(
    transaction: &PublicationTransaction,
    observation: &PublicationRecoveryObservation,
) -> Result<PublicationRecoveryDecision, PublicationError> {
    use PublicationRecoveryDecision as Decision;
    if transaction.max_points == 0
        || transaction.target_epoch <= transaction.previous_visible_epoch
        || transaction.prepared.old_manifest.is_some()
            != transaction.prepared.old_manifest_digest.is_some()
    {
        return Err(PublicationError::InvalidPreparedPublication);
    }
    crate::machine::validate_manifest(&transaction.prepared.new_manifest, transaction.max_points)?;
    let empty = ProjectionManifest {
        entries: Vec::new(),
        canonical_bytes: b"eliot-search/empty-manifest/v1".to_vec(),
    };
    let old = transaction.prepared.old_manifest.as_ref().unwrap_or(&empty);
    crate::machine::validate_manifest(old, transaction.max_points)?;
    if crate::machine::changed_point_id_is_reused(Some(old), &transaction.prepared.new_manifest) {
        // Legacy/conflicting plans can report every ID acknowledged while having
        // overwritten old-epoch content and closed the new version at its own
        // starting epoch. ID-only observations cannot prove either version safe.
        // Preserve the unresolved operation for explicit verified repair; neither
        // forward continuation nor snapshot publication is permitted here.
        return Ok(Decision::PublicationBlocked);
    }
    // Refuse oversized observation lists before building sets or manifest copies.
    if observation.staged_ids.len() > transaction.prepared.new_manifest.entries.len()
        || observation.closed_ids.len() > old.entries.len()
        || observation.abandon_fence_durable
    {
        return Ok(Decision::PublicationBlocked);
    }
    let difference = diff_manifests(old, &transaction.prepared.new_manifest)
        .map_err(|_| PublicationError::InvalidPreparedPublication)?;
    let expected_staged = difference
        .create
        .iter()
        .map(|entry| entry.point_id)
        .collect::<BTreeSet<_>>();
    let expected_closed = difference
        .retire
        .iter()
        .map(|entry| entry.point_id)
        .collect::<BTreeSet<_>>();
    let observed_staged = observation
        .staged_ids
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let observed_closed = observation
        .closed_ids
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if observed_staged.len() != observation.staged_ids.len()
        || observed_closed.len() != observation.closed_ids.len()
        || !observed_staged.is_subset(&expected_staged)
        || !observed_closed.is_subset(&expected_closed)
    {
        return Ok(Decision::PublicationBlocked);
    }
    // Neither an unrelated earlier/later epoch nor an abandoned epoch between
    // the old visible value and this target proves progress for this transaction.
    if observation.control_visible_epoch != transaction.previous_visible_epoch
        && observation.control_visible_epoch != transaction.target_epoch
    {
        return Ok(Decision::PublicationBlocked);
    }
    if !observation.intent_durable {
        return Ok(decide_without_durable_intent(
            transaction,
            observation,
            &observed_staged,
            &observed_closed,
        ));
    }
    let Some(intent) = &transaction.durable_intent else {
        return Ok(Decision::PublicationBlocked);
    };
    if intent.transaction_id != transaction.prepared.transaction_id
        || intent.target_epoch != transaction.target_epoch
        || intent.old_manifest_digest != transaction.prepared.old_manifest_digest
        || intent.new_manifest_digest != transaction.prepared.new_manifest_digest
        || intent.guards != transaction.prepared.guards
    {
        return Ok(Decision::PublicationBlocked);
    }
    let all_effects = observed_staged == expected_staged && observed_closed == expected_closed;
    if observation.control_visible_epoch == transaction.target_epoch {
        return Ok(decide_committed_epoch(
            transaction,
            observation,
            &expected_staged,
            &expected_closed,
            all_effects,
        ));
    }
    Ok(decide_pre_commit(
        transaction,
        observation,
        all_effects,
        &observed_staged,
        &observed_closed,
    ))
}

fn decide_without_durable_intent(
    transaction: &PublicationTransaction,
    observation: &PublicationRecoveryObservation,
    observed_staged: &BTreeSet<PointId128>,
    observed_closed: &BTreeSet<PointId128>,
) -> PublicationRecoveryDecision {
    use PublicationRecoveryDecision as Decision;
    if transaction.phase == PublicationPhase::Prepared
        && transaction.durable_intent.is_none()
        && observation.control_visible_epoch == transaction.previous_visible_epoch
        && !observation.snapshot_published
        && observed_staged.is_empty()
        && observed_closed.is_empty()
    {
        Decision::Continue
    } else {
        Decision::PublicationBlocked
    }
}

fn decide_pre_commit(
    transaction: &PublicationTransaction,
    observation: &PublicationRecoveryObservation,
    all_effects: bool,
    observed_staged: &BTreeSet<PointId128>,
    observed_closed: &BTreeSet<PointId128>,
) -> PublicationRecoveryDecision {
    use PublicationRecoveryDecision as Decision;
    if observation.snapshot_published
        || matches!(
            transaction.phase,
            PublicationPhase::Prepared
                | PublicationPhase::ControlCommitted
                | PublicationPhase::SnapshotPublished
                | PublicationPhase::Aborted
                | PublicationPhase::PublicationBlocked
        )
    {
        return Decision::PublicationBlocked;
    }
    if transaction.phase == PublicationPhase::Compensating {
        return Decision::CompensateExact;
    }
    if all_effects {
        return Decision::Continue;
    }
    // Old closures also require compensation. Seeing no staged new IDs cannot
    // justify ignoring old points that are still closed at a consumed epoch.
    if !observed_staged.is_empty()
        || !observed_closed.is_empty()
        || transaction.phase != PublicationPhase::IntentDurable
    {
        return Decision::CompensateExact;
    }
    Decision::Continue
}

fn decide_committed_epoch(
    transaction: &PublicationTransaction,
    observation: &PublicationRecoveryObservation,
    expected_staged: &BTreeSet<PointId128>,
    expected_closed: &BTreeSet<PointId128>,
    all_effects: bool,
) -> PublicationRecoveryDecision {
    use PublicationRecoveryDecision as Decision;
    if !matches!(
        transaction.phase,
        PublicationPhase::ControlCommitted | PublicationPhase::SnapshotPublished
    ) || !all_effects
        || !committed_binding_matches(transaction, expected_staged, expected_closed)
    {
        return Decision::PublicationBlocked;
    }
    if !observation.snapshot_published {
        return Decision::PublishSnapshot;
    }
    let Some(snapshot) = &transaction.snapshot_receipt else {
        return Decision::PublicationBlocked;
    };
    let Some(commit) = &transaction.visible_commit else {
        return Decision::PublicationBlocked;
    };
    if transaction.phase == PublicationPhase::SnapshotPublished
        && snapshot.transaction_id == commit.transaction_id
        && snapshot.visible_epoch == commit.visible_epoch
        && snapshot.control_generation == commit.control_generation
    {
        Decision::Continue
    } else {
        Decision::PublicationBlocked
    }
}

fn committed_binding_matches(
    transaction: &PublicationTransaction,
    expected_staged: &BTreeSet<PointId128>,
    expected_closed: &BTreeSet<PointId128>,
) -> bool {
    let (Some(commit), Some(verified), Some(stage), Some(closure)) = (
        &transaction.visible_commit,
        &transaction.verified,
        &transaction.stage_receipt,
        &transaction.closure_receipt,
    ) else {
        return false;
    };
    let id = &transaction.prepared.transaction_id;
    commit.transaction_id == *id
        && commit.visible_epoch == transaction.target_epoch
        && commit.visible_manifest_digest == transaction.prepared.new_manifest_digest
        && commit.retired_manifest_digest == verified.retired_manifest_digest
        && verified.retired_manifest_digest.is_some() != expected_closed.is_empty()
        && commit.control_generation != 0
        && verified.transaction_id == *id
        && verified.target_epoch == transaction.target_epoch
        && verified.new_manifest_digest == transaction.prepared.new_manifest_digest
        && verified.staged_readback_digest == stage.readback_digest
        && verified.closure_readback_digest == closure.readback_digest
        && stage.transaction_id == *id
        && stage.target_epoch == transaction.target_epoch
        && stage.missing_ids.is_empty()
        && stage.unexpected_ids.is_empty()
        && stage.staged_ids.iter().eq(expected_staged.iter())
        && closure.transaction_id == *id
        && closure.target_epoch == transaction.target_epoch
        && closure.missing_ids.is_empty()
        && closure.unexpected_ids.is_empty()
        && closure.closed_ids.iter().eq(expected_closed.iter())
}

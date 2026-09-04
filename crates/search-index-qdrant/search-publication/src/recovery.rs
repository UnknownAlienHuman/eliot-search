//! Fail-closed publication recovery decisions.

use std::collections::BTreeSet;

use search_projection_planner::{ProjectionManifest, diff_manifests};

use crate::{
    PublicationError, PublicationPhase, PublicationRecoveryDecision,
    PublicationRecoveryObservation, PublicationTransaction,
};

/// Classifies one complete authoritative recovery observation.
///
/// # Errors
///
/// Returns an error only when the transaction manifests themselves cannot be
/// diffed. Contradictory external evidence maps to `PublicationBlocked`.
pub fn recover(
    transaction: &PublicationTransaction,
    observation: &PublicationRecoveryObservation,
) -> Result<PublicationRecoveryDecision, PublicationError> {
    let empty = ProjectionManifest {
        entries: Vec::new(),
        canonical_bytes: b"eliot-search/empty-manifest/v1".to_vec(),
    };
    let old = transaction
        .prepared
        .old_manifest
        .as_ref()
        .unwrap_or(&empty);
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
    let observed_staged = observation.staged_ids.iter().copied().collect::<BTreeSet<_>>();
    let observed_closed = observation.closed_ids.iter().copied().collect::<BTreeSet<_>>();

    if observed_staged.len() != observation.staged_ids.len()
        || observed_closed.len() != observation.closed_ids.len()
        || !observed_staged.is_subset(&expected_staged)
        || !observed_closed.is_subset(&expected_closed)
    {
        return Ok(PublicationRecoveryDecision::PublicationBlocked);
    }

    if observation.control_visible_epoch == transaction.target_epoch {
        if transaction.phase == PublicationPhase::ControlCommitted
            || transaction.phase == PublicationPhase::SnapshotPublished
        {
            return Ok(if observation.snapshot_published {
                PublicationRecoveryDecision::Continue
            } else {
                PublicationRecoveryDecision::PublishSnapshot
            });
        }
        return Ok(PublicationRecoveryDecision::PublicationBlocked);
    }

    if !observation.intent_durable {
        return Ok(if observed_staged.is_empty() && observed_closed.is_empty() {
            PublicationRecoveryDecision::Continue
        } else {
            PublicationRecoveryDecision::PublicationBlocked
        });
    }

    if observed_staged == expected_staged && observed_closed == expected_closed {
        return Ok(PublicationRecoveryDecision::Continue);
    }
    if !observed_staged.is_empty() {
        return Ok(if observation.abandon_fence_durable {
            PublicationRecoveryDecision::CommitInvalidationOnly
        } else {
            PublicationRecoveryDecision::CompensateExact
        });
    }
    Ok(PublicationRecoveryDecision::Continue)
}

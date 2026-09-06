//! Rebuild the sole active coordinator slot from already verified recovery inputs.
//! This module performs no port call, reservation, durable write or index mutation.

use core::fmt;

use search_contracts::CollectionGenerationId;

use super::{
    ClosureReceipt, ControlCommitObservation, DurableIntent, Epoch, PreparedPublication,
    PublicationCoordinator, PublicationError, PublicationGuards, PublicationPhase,
    PublicationTransaction, ReadbackVerified, StageReceipt, changed_point_id_is_reused,
    validate_manifest,
};
use crate::{PublicationRecoveryDecision, PublicationRecoveryObservation, recover};

/// Complete transient input supplied by the recovery composition owner.
///
/// Read the journal checkpoint, retained preparation and bound index readback
/// under the live owner/recovery barrier. Verify their original IDs, manifest
/// bytes/digests, receipt references and collection route before constructing this
/// value. These public fields cannot authenticate a producer or an arbitrary
/// reference. This is not a new persisted checkpoint format or an ownership lease.
///
/// Snapshot acknowledgement is intentionally absent: a receipt from the previous
/// process cannot authorize admission in the restored process.
pub struct PublicationRestoreInput {
    /// Exact original preparation, including both immutable manifests.
    pub prepared: PreparedPublication,
    /// Authoritatively retained visible epoch immediately before this reservation.
    pub previous_visible_epoch: Epoch,
    /// Highest durable reservation; single-flight recovery requires the target itself.
    pub last_reserved_epoch: Epoch,
    /// Current route from the same authoritative control observation.
    pub collection_generation_id: CollectionGenerationId,
    /// Current live owner/source/membership/access/shadow/purge/profile guards.
    pub current_guards: PublicationGuards,
    /// Last recorded phase, not inferred from point counts or collection presence.
    pub phase: PublicationPhase,
    /// Exact retained intent and its original operation/receipt identities.
    pub intent: DurableIntent,
    /// Recorded stage acknowledgement, present only for the corresponding phase prefix.
    pub stage_receipt: Option<StageReceipt>,
    /// Recorded closure acknowledgement; never present without its stage predecessor.
    pub closure_receipt: Option<ClosureReceipt>,
    /// Recorded combined readback; its complete binding is revalidated.
    pub verified: Option<ReadbackVerified>,
    /// Actual verified control-commit observation, not reconstructed from a phase label.
    pub control_commit: Option<ControlCommitObservation>,
    /// Fresh transaction-bound recovery readback. `snapshot_published` must be false.
    pub observation: PublicationRecoveryObservation,
}

impl fmt::Debug for PublicationRestoreInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PublicationRestoreInput")
            .field("phase", &self.phase)
            .field("previous_visible_epoch", &self.previous_visible_epoch)
            .field("last_reserved_epoch", &self.last_reserved_epoch)
            .finish_non_exhaustive()
    }
}

impl PublicationCoordinator {
    /// Restores one in-flight transaction without calling `submit` or allocating
    /// another epoch. Returns the existing recovery decision and a non-Clone
    /// coordinator whose active slot remains occupied, including when blocked.
    ///
    /// Existing pure stage/closure/readback/commit validators rebuild the accepted
    /// receipt prefix; no external operation is re-executed. Recovery then checks
    /// fresh observed effects. Incomplete effects or changed pre-commit guards
    /// put the slot in COMPENSATING, preventing forward commit. Bare ABORTED and
    /// blocked records remain PUBLICATION_BLOCKED, not an empty ready coordinator.
    ///
    /// Prior SNAPSHOT_PUBLISHED becomes CONTROL_COMMITTED and requires a new
    /// current-process acknowledgement. Live restriction checks remain mandatory
    /// at actual publication/admission; this pure result grants no index access.
    ///
    /// # Errors
    /// Rejects missing/out-of-order receipt prefixes, foreign intent or route,
    /// invalid manifests, changed reuse of a physical point ID, stale owner,
    /// inconsistent epochs, oversized lists and prior-process snapshot claims.
    /// A rejected input never returns a partially usable coordinator.
    pub fn restore_inflight(
        input: PublicationRestoreInput,
        max_points: usize,
    ) -> Result<(Self, PublicationRecoveryDecision), PublicationError> {
        validate_input(&input, max_points)?;
        let phase = input.phase;
        let guards_changed = input.current_guards != input.prepared.guards;
        let target_epoch = input.intent.target_epoch;
        // Populate the original reservation directly. Lowering the floor and
        // calling submit would misuse a fresh-reservation operation for recovery.
        let mut coordinator = Self {
            visible_epoch: input.previous_visible_epoch,
            last_reserved_epoch: input.last_reserved_epoch,
            current_manifest: input.prepared.old_manifest.clone(),
            current_manifest_digest: input.prepared.old_manifest_digest,
            active: Some(PublicationTransaction {
                prepared: input.prepared,
                previous_visible_epoch: input.previous_visible_epoch,
                target_epoch,
                phase: PublicationPhase::IntentDurable,
                durable_intent: Some(input.intent),
                stage_receipt: None,
                closure_receipt: None,
                verified: None,
                visible_commit: None,
                snapshot_receipt: None,
                max_points,
            }),
            abort_finalization: super::abort::AbortFinalizationProgress::default(),
            max_points,
        };
        if let Some(stage) = input.stage_receipt {
            coordinator.stage_new_points(stage)?;
        }
        if let Some(closure) = input.closure_receipt {
            coordinator.close_old_points(closure)?;
        }
        if let Some(verified) = input.verified {
            let reconstructed = coordinator.verify_readback(
                verified.staged_readback_digest,
                verified.closure_readback_digest,
                verified.retired_manifest_digest,
            )?;
            if reconstructed != verified {
                return Err(PublicationError::RecoveryBlocked);
            }
        }
        if let Some(commit) = input.control_commit {
            coordinator.commit_visible_epoch(commit)?;
        }
        if coordinator.visible_epoch != input.observation.control_visible_epoch {
            return Err(PublicationError::ControlConflict);
        }
        let active = coordinator.active.as_mut().ok_or(PublicationError::RecoveryBlocked)?;
        match phase {
            PublicationPhase::Aborted | PublicationPhase::PublicationBlocked => {
                active.phase = PublicationPhase::PublicationBlocked;
            }
            PublicationPhase::Compensating => active.phase = PublicationPhase::Compensating,
            _ if active.visible_commit.is_none() && guards_changed => {
                // A successor owner may compensate the original epoch; it must
                // not silently replace old guards and continue forward staging.
                active.phase = PublicationPhase::Compensating;
            }
            _ => {}
        }
        let decision = recover(active, &input.observation)?;
        match decision {
            PublicationRecoveryDecision::CompensateExact => {
                active.phase = PublicationPhase::Compensating;
            }
            PublicationRecoveryDecision::PublicationBlocked => {
                active.phase = PublicationPhase::PublicationBlocked;
            }
            PublicationRecoveryDecision::CommitInvalidationOnly => {
                // Hydration cannot grant the still-separate invalidation protocol.
                active.phase = PublicationPhase::PublicationBlocked;
                return Ok((coordinator, PublicationRecoveryDecision::PublicationBlocked));
            }
            PublicationRecoveryDecision::Continue | PublicationRecoveryDecision::PublishSnapshot => {}
        }
        Ok((coordinator, decision))
    }
}

fn validate_input(input: &PublicationRestoreInput, max_points: usize) -> Result<(), PublicationError> {
    if max_points == 0
        || input.prepared.old_manifest.is_some() != input.prepared.old_manifest_digest.is_some()
    {
        return Err(PublicationError::InvalidPreparedPublication);
    }
    validate_manifest(&input.prepared.new_manifest, max_points)?;
    if let Some(old) = &input.prepared.old_manifest {
        validate_manifest(old, max_points)?;
    }
    if changed_point_id_is_reused(input.prepared.old_manifest.as_ref(), &input.prepared.new_manifest) {
        return Err(PublicationError::InvalidPreparedPublication);
    }
    if input.collection_generation_id != input.prepared.collection_generation_id
        || input.intent.transaction_id != input.prepared.transaction_id
        || input.intent.old_manifest_digest != input.prepared.old_manifest_digest
        || input.intent.new_manifest_digest != input.prepared.new_manifest_digest
        || input.intent.guards != input.prepared.guards
    {
        return Err(PublicationError::OperationMismatch);
    }
    if input.current_guards.owner_epoch < input.intent.guards.owner_epoch {
        return Err(PublicationError::GuardMismatch);
    }
    if input.last_reserved_epoch != input.intent.target_epoch
        || input.intent.target_epoch <= input.previous_visible_epoch
    {
        return Err(PublicationError::EpochMismatch);
    }
    if !input.observation.intent_durable || input.observation.snapshot_published {
        return Err(PublicationError::RecoveryBlocked);
    }
    let lengths = [
        input.observation.staged_ids.len(), input.observation.closed_ids.len(),
        input.stage_receipt.as_ref().map_or(0, |r| r.staged_ids.len()),
        input.stage_receipt.as_ref().map_or(0, |r| r.missing_ids.len()),
        input.stage_receipt.as_ref().map_or(0, |r| r.unexpected_ids.len()),
        input.closure_receipt.as_ref().map_or(0, |r| r.closed_ids.len()),
        input.closure_receipt.as_ref().map_or(0, |r| r.missing_ids.len()),
        input.closure_receipt.as_ref().map_or(0, |r| r.unexpected_ids.len()),
    ];
    if lengths.into_iter().any(|length| length > max_points) {
        return Err(PublicationError::BudgetExceeded);
    }
    // A recorded phase has one complete, ordered receipt prefix. Observed but
    // not acknowledged effects belong in `observation`, not invented receipts.
    let mask = u8::from(input.stage_receipt.is_some())
        | (u8::from(input.closure_receipt.is_some()) << 1)
        | (u8::from(input.verified.is_some()) << 2)
        | (u8::from(input.control_commit.is_some()) << 3);
    let valid = match input.phase {
        PublicationPhase::Prepared => false,
        PublicationPhase::IntentDurable => mask == 0,
        PublicationPhase::NewPointsAcknowledged => mask == 1,
        PublicationPhase::OldPointsClosedAcknowledged => mask == 3,
        PublicationPhase::ReadbackVerified => mask == 7,
        PublicationPhase::ControlCommitted | PublicationPhase::SnapshotPublished => mask == 15,
        PublicationPhase::Compensating | PublicationPhase::Aborted => matches!(mask, 0 | 1 | 3 | 7),
        PublicationPhase::PublicationBlocked => matches!(mask, 0 | 1 | 3 | 7 | 15),
    };
    if !valid { return Err(PublicationError::RecoveryBlocked); }
    Ok(())
}

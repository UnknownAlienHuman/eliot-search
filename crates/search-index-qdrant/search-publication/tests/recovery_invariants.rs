//! Canonical C08/C09/C13 inputs exercise pure publication decisions. Receipt,
//! journal and snapshot values remain synthetic phase observations, not I/O proof.

use search_contracts::{
    AccessPartitionId, Blake3Digest32, CollectionGenerationId, Epoch, InstallationIncarnationId,
    OpaqueId, OwnerEpoch, ProjectionMembershipId, ProjectionProfileSetId, ReceiptRef,
    ScoringDocumentId, ScoringPartitionId, SourceMembershipId, UnitId,
};
use search_point_identity::s11::PointId128;
use search_publication::*;
use std::collections::BTreeSet;

#[path = "support/publication.rs"]
mod publication_support;
use publication_support::{Fixture, PublicationCase, SubmitFixture, projection_fixture};

#[path = "recovery_invariants/abort_finalization.rs"]
mod abort_finalization;
#[path = "support/abort_finalization.rs"]
mod abort_support;
#[path = "recovery_invariants/identity_conflicts.rs"]
mod identity_conflicts;

fn id(name: &str) -> OpaqueId {
    OpaqueId::new(name).unwrap()
}
fn epoch(value: i64) -> Epoch {
    Epoch::new(value).unwrap()
}
const fn digest(value: u8) -> Blake3Digest32 {
    Blake3Digest32::from_bytes([value; 32])
}
fn reference(name: &str) -> ReceiptRef {
    ReceiptRef::new(format!("fixture:{name}")).unwrap()
}
const fn point(value: u8) -> PointId128 {
    PointId128::from_stored_bytes([value; 16])
}
fn guards() -> PublicationGuards {
    publication_support::guards()
}
fn manifest(points: &[u8]) -> Fixture {
    projection_fixture(points)
}
fn proposal(old: Option<Fixture>, new: Fixture, name: &str) -> PublicationCase {
    publication_support::proposal(old, new, name)
}
fn durable(old: Option<Fixture>, new: Fixture) -> PublicationCoordinator {
    let mut machine = PublicationCoordinator::new(32).unwrap();
    machine.submit_fixture(proposal(old, new, "tx-1")).unwrap();
    machine
        .persist_intent(id("persist-1"), reference("intent"))
        .unwrap();
    machine
}
fn compensation(plan: &CompensationPlan) -> CompensationReceipt {
    CompensationReceipt {
        transaction_id: plan.transaction_id.clone(),
        target_epoch: plan.target_epoch,
        compensated_ids: plan.staged_ids.clone(),
        remaining_ids: vec![],
        readback_receipt: reference("removed"),
    }
}
fn restoration(plan: &CompensationPlan) -> RestorationReceipt {
    RestorationReceipt {
        transaction_id: plan.transaction_id.clone(),
        target_epoch: plan.target_epoch,
        restored_ids: plan.closed_ids.clone(),
        remaining_ids: vec![],
        readback_receipt: reference("restored"),
    }
}
const fn observation(machine: &PublicationCoordinator) -> PublicationRecoveryObservation {
    let transaction = machine.active().unwrap();
    PublicationRecoveryObservation {
        intent_durable: transaction.durable_intent.is_some(),
        staged_ids: vec![],
        closed_ids: vec![],
        control_visible_epoch: transaction.previous_visible_epoch,
        snapshot_published: false,
        abandon_fence_durable: false,
    }
}
fn visible(
    machine: &mut PublicationCoordinator,
) -> (VisibleCommitReceipt, PublicationRecoveryObservation) {
    let transaction = machine.active().unwrap().clone();
    let difference = transaction.prepared.difference();
    let mut staged = difference
        .create
        .iter()
        .map(|entry| entry.point_id)
        .collect::<Vec<_>>();
    staged.sort();
    let mut closed = difference
        .retire
        .iter()
        .map(|entry| entry.point_id)
        .collect::<Vec<_>>();
    closed.sort();
    let tx = transaction.transaction_id().clone();
    machine
        .stage_new_points(StageReceipt {
            transaction_id: tx.clone(),
            target_epoch: transaction.target_epoch,
            staged_ids: staged.clone(),
            missing_ids: vec![],
            unexpected_ids: vec![],
            readback_digest: digest(3),
            mutation_receipt: reference("stage"),
        })
        .unwrap();
    machine
        .close_old_points(ClosureReceipt {
            transaction_id: tx,
            target_epoch: transaction.target_epoch,
            closed_ids: closed.clone(),
            missing_ids: vec![],
            unexpected_ids: vec![],
            readback_digest: digest(4),
            mutation_receipt: reference("close"),
        })
        .unwrap();
    machine
        .verify_readback(
            digest(3),
            digest(4),
            if closed.is_empty() {
                None
            } else {
                Some(digest(5))
            },
        )
        .unwrap();
    let receipt = machine
        .commit_visible_epoch(ControlCommitObservation {
            before_visible_epoch: transaction.previous_visible_epoch,
            after_visible_epoch: transaction.target_epoch,
            observed_guards: transaction.durable_intent.as_ref().unwrap().guards,
            control_generation: 9,
            control_state_digest: digest(6),
        })
        .unwrap();
    let observed = PublicationRecoveryObservation {
        intent_durable: true,
        staged_ids: staged,
        closed_ids: closed,
        control_visible_epoch: transaction.target_epoch,
        snapshot_published: false,
        abandon_fence_durable: false,
    };
    (receipt, observed)
}
fn snapshot(commit: &VisibleCommitReceipt) -> SnapshotPublishReceipt {
    SnapshotPublishReceipt {
        transaction_id: commit.transaction_id.clone(),
        visible_epoch: commit.visible_epoch,
        control_generation: commit.control_generation,
        snapshot_digest: digest(6),
    }
}

#[test]
fn aborted_reservations_are_never_reused_and_do_not_advance_visibility() {
    let mut machine = PublicationCoordinator::new(32).unwrap();
    assert_eq!(machine.visible_epoch(), None);
    assert_eq!(machine.last_reserved_epoch(), None);
    for expected in 1..=4 {
        assert_eq!(
            machine
                .submit_fixture(proposal(None, manifest(&[2]), &format!("tx-{expected}")))
                .unwrap(),
            epoch(expected)
        );
        machine
            .persist_intent(id(&format!("persist-{expected}")), reference("intent"))
            .unwrap();
        let plan = machine.begin_compensation_plan().unwrap();
        machine.compensate_exact(compensation(&plan)).unwrap();
        assert_eq!(
            machine.finalize_aborted(),
            Err(PublicationError::RecoveryBlocked)
        );
        abort_support::acknowledge(&mut machine);
        machine.finalize_aborted().unwrap();
        assert_eq!(machine.visible_epoch(), Some(epoch(0)));
        assert_eq!(machine.last_reserved_epoch(), Some(epoch(expected)));
        assert!(machine.active().is_none());
        assert!(machine.current_manifest().is_none());
    }
}

#[test]
fn reconstruction_requires_the_explicit_resolved_reservation_floor() {
    let mut machine = PublicationCoordinator::new(32).unwrap();
    let mut resumed = proposal(None, manifest(&[2]), "resumed");
    resumed.seed = publication_support::OwnerSeed {
        previous_visible: epoch(4),
        current_visible: epoch(4),
        last_reserved: epoch(9),
    };
    assert_eq!(machine.submit_fixture(resumed).unwrap(), epoch(10));
    assert_eq!(machine.active().unwrap().previous_visible_epoch, epoch(4));
    assert_eq!(machine.visible_epoch(), Some(epoch(4)));

    let mut inconsistent = proposal(None, manifest(&[3]), "inconsistent-floor");
    inconsistent.seed.previous_visible = epoch(4);
    inconsistent.seed.current_visible = epoch(4);
    inconsistent.seed.last_reserved = epoch(3);
    let mut uninitialized = PublicationCoordinator::new(32).unwrap();
    assert_eq!(
        uninitialized.submit_fixture(inconsistent),
        Err(PublicationError::EpochMismatch)
    );
    assert_eq!(uninitialized.visible_epoch(), None);
    assert_eq!(uninitialized.last_reserved_epoch(), None);
}

#[test]
fn exhausted_floor_does_not_wrap_or_consume_an_active_slot() {
    let mut machine = PublicationCoordinator::new(32).unwrap();
    let mut overflowing = proposal(None, manifest(&[2]), "overflow");
    overflowing.seed.last_reserved = epoch(search_contracts::MAX_QDRANT_EPOCH);
    assert_eq!(
        machine.submit_fixture(overflowing),
        Err(PublicationError::ContractExhausted)
    );
    assert!(machine.active().is_none());
    assert_eq!(machine.visible_epoch(), None);
    assert_eq!(machine.last_reserved_epoch(), None);
}

#[test]
fn invalid_and_competing_submissions_cannot_change_the_reservation_floor() {
    let mut machine = PublicationCoordinator::new(32).unwrap();
    let mut wrong_old = proposal(Some(manifest(&[1])), manifest(&[2]), "wrong-old");
    wrong_old.expected_manifest_override = publication_support::ManifestExpectation::Exact(None);
    wrong_old.seed.last_reserved = epoch(3);
    assert_eq!(
        machine.submit_fixture(wrong_old),
        Err(PublicationError::ControlConflict)
    );
    assert_eq!(machine.visible_epoch(), None);
    assert_eq!(machine.last_reserved_epoch(), None);
    let mut first = proposal(None, manifest(&[2]), "first");
    first.seed.last_reserved = epoch(3);
    machine.submit_fixture(first).unwrap();
    assert_eq!(
        machine.submit_fixture(proposal(None, manifest(&[3]), "competitor")),
        Err(PublicationError::PublicationBusy)
    );
    assert_eq!(machine.last_reserved_epoch(), Some(epoch(4)));
    assert_eq!(machine.active().unwrap().transaction_id(), &id("first"));
}

#[test]
fn compensation_requires_restoration_of_every_old_closed_point() {
    let old = manifest(&[1]);
    let new = manifest(&[2]);
    let mut machine = durable(Some(old.clone()), new.clone());
    let plan = machine.begin_compensation_plan().unwrap();
    assert_eq!(
        plan.staged_ids,
        new.entries
            .iter()
            .map(|entry| entry.point_id)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        plan.closed_ids,
        old.entries
            .iter()
            .map(|entry| entry.point_id)
            .collect::<Vec<_>>()
    );
    assert_eq!(machine.begin_compensation_plan().unwrap(), plan);
    assert_eq!(
        machine.compensate_exact(compensation(&plan)),
        Err(PublicationError::CompensationIncomplete)
    );
    assert_eq!(
        machine.finalize_aborted(),
        Err(PublicationError::InvalidTransition)
    );
    assert_eq!(
        machine.active().unwrap().phase,
        PublicationPhase::Compensating
    );
    machine
        .compensate_and_restore(compensation(&plan), restoration(&plan))
        .unwrap();
    abort_support::acknowledge(&mut machine);
    machine.finalize_aborted().unwrap();
    assert_eq!(machine.current_manifest(), Some(old.manifest()));
    assert_eq!(machine.visible_epoch(), Some(epoch(1)));
    assert_eq!(
        machine
            .submit_fixture(proposal(Some(old), manifest(&[3]), "next"))
            .unwrap(),
        epoch(3)
    );
}

#[test]
fn incomplete_wrong_epoch_or_foreign_restoration_never_finishes_compensation() {
    for case in 0..6 {
        let mut machine = durable(Some(manifest(&[1])), manifest(&[2]));
        let plan = machine.begin_compensation_plan().unwrap();
        let mut restore = restoration(&plan);
        match case {
            0 => restore.remaining_ids.push(point(1)),
            1 => restore.restored_ids.clear(),
            2 => restore.restored_ids.push(point(1)),
            3 => restore.restored_ids = vec![point(9)],
            4 => restore.target_epoch = epoch(3),
            _ => restore.transaction_id = id("foreign"),
        }
        assert_eq!(
            machine.compensate_and_restore(compensation(&plan), restore),
            Err(PublicationError::CompensationIncomplete)
        );
        assert_eq!(
            machine.active().unwrap().phase,
            PublicationPhase::Compensating
        );
    }
}

#[test]
fn compensation_receipt_is_bound_to_its_reservation_not_just_transaction_name() {
    let mut machine = durable(None, manifest(&[2]));
    let plan = machine.begin_compensation_plan().unwrap();
    let mut stale = compensation(&plan);
    stale.target_epoch = epoch(0);
    assert_eq!(
        machine.compensate_exact(stale),
        Err(PublicationError::CompensationIncomplete)
    );
    assert_eq!(
        machine.active().unwrap().phase,
        PublicationPhase::Compensating
    );
    let mut partial = compensation(&plan);
    partial.remaining_ids = vec![point(2)];
    assert_eq!(
        machine.compensate_exact(partial),
        Err(PublicationError::CompensationIncomplete)
    );
    machine.compensate_exact(compensation(&plan)).unwrap();
}

#[test]
fn changed_content_with_a_distinct_id_still_requires_two_sided_compensation() {
    let old = manifest(&[1]);
    let new = manifest(&[2]);
    let mut machine = durable(Some(old.clone()), new.clone());
    let plan = machine.begin_compensation_plan().unwrap();
    assert_eq!(
        plan.staged_ids,
        new.entries
            .iter()
            .map(|entry| entry.point_id)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        plan.closed_ids,
        old.entries
            .iter()
            .map(|entry| entry.point_id)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        machine.compensate_exact(compensation(&plan)),
        Err(PublicationError::CompensationIncomplete)
    );
    machine
        .compensate_and_restore(compensation(&plan), restoration(&plan))
        .unwrap();
}

#[test]
fn overlapping_changed_and_deleted_diff_is_rejected_before_publication() {
    let old = manifest(&[1, 2]);
    let mut new = manifest(&[2]);
    new.entries[0].payload_identity = digest(99);
    let mut machine = PublicationCoordinator::new(32).unwrap();
    let conflict = proposal(Some(old), new, "conflicting");
    assert_eq!(
        machine.submit_fixture(conflict),
        Err(PublicationError::InvalidPreparedPublication)
    );
    assert!(machine.active().is_none());
    assert_eq!(machine.last_reserved_epoch(), None);
    assert_eq!(machine.current_manifest(), None);
}

#[test]
fn point_only_or_incomplete_membership_fence_cannot_abandon() {
    let mut machine = durable(Some(manifest(&[1])), manifest(&[2]));
    let active = machine.active().unwrap();
    let affected_ids = active
        .prepared
        .difference()
        .create
        .iter()
        .chain(&active.prepared.difference().retire)
        .map(|entry| entry.point_id)
        .collect::<BTreeSet<_>>();
    let mut fence = AbandonFence {
        transaction_id: id("tx-1"),
        target_epoch: epoch(2),
        excluded_point_ids: affected_ids,
        excluded_projection_memberships: BTreeSet::new(),
        excluded_scope_digest: digest(8),
        exclusion_receipt: reference("exclusion"),
    };
    assert_eq!(
        machine.abandon(&fence),
        Err(PublicationError::AbandonFenceMissing)
    );
    fence
        .excluded_projection_memberships
        .insert(ProjectionMembershipId::from_bytes([1; 16]));
    assert_eq!(
        machine.abandon(&fence),
        Err(PublicationError::AbandonFenceMissing)
    );
    fence
        .excluded_projection_memberships
        .insert(ProjectionMembershipId::from_bytes([17; 16]));
    fence
        .excluded_projection_memberships
        .remove(&ProjectionMembershipId::from_bytes([1; 16]));
    machine.abandon(&fence).unwrap();
    abort_support::acknowledge(&mut machine);
    machine.finalize_aborted().unwrap();
    assert_eq!(machine.last_reserved_epoch(), Some(epoch(2)));
    assert_eq!(machine.visible_epoch(), Some(epoch(1)));
    assert_eq!(
        machine
            .submit_fixture(proposal(Some(manifest(&[1])), manifest(&[3]), "next"))
            .unwrap(),
        epoch(3)
    );
}

#[test]
fn closed_only_recovery_selects_compensation_not_optimistic_continue() {
    let machine = durable(Some(manifest(&[1])), manifest(&[2]));
    let mut seen = observation(&machine);
    seen.closed_ids
        .push(machine.active().unwrap().prepared.difference().retire[0].point_id);
    assert_eq!(
        recover(machine.active().unwrap(), &seen).unwrap(),
        PublicationRecoveryDecision::CompensateExact
    );
}

#[test]
fn unrelated_visible_epoch_including_a_skipped_epoch_blocks_recovery() {
    let mut machine = PublicationCoordinator::new(32).unwrap();
    let mut resume = proposal(None, manifest(&[2]), "resume");
    resume.seed.previous_visible = epoch(4);
    resume.seed.current_visible = epoch(4);
    resume.seed.last_reserved = epoch(9);
    machine.submit_fixture(resume).unwrap();
    machine
        .persist_intent(id("persist-resume"), reference("intent"))
        .unwrap();
    for wrong in [0, 3, 5, 9, 11] {
        let mut seen = observation(&machine);
        seen.control_visible_epoch = epoch(wrong);
        assert_eq!(
            recover(machine.active().unwrap(), &seen).unwrap(),
            PublicationRecoveryDecision::PublicationBlocked
        );
    }
}

#[test]
fn absent_or_contradictory_durable_intent_cannot_authorize_continuation() {
    let machine = durable(None, manifest(&[2]));
    let mut seen = observation(&machine);
    seen.intent_durable = false;
    assert_eq!(
        recover(machine.active().unwrap(), &seen).unwrap(),
        PublicationRecoveryDecision::PublicationBlocked
    );
    seen.intent_durable = true;
    for wrong in 0..3 {
        let mut transaction = machine.active().unwrap().clone();
        match wrong {
            0 => transaction.durable_intent = None,
            1 => transaction.durable_intent.as_mut().unwrap().target_epoch = epoch(2),
            _ => {
                transaction
                    .durable_intent
                    .as_mut()
                    .unwrap()
                    .guards
                    .access_generation += 1;
            }
        }
        assert_eq!(
            recover(&transaction, &seen).unwrap(),
            PublicationRecoveryDecision::PublicationBlocked
        );
    }
}

#[test]
fn a_boolean_fence_never_grants_invalidation_only_commit() {
    let machine = durable(Some(manifest(&[1])), manifest(&[2, 3]));
    let mut seen = observation(&machine);
    seen.staged_ids = vec![point(2)];
    seen.closed_ids = vec![point(1)];
    seen.abandon_fence_durable = true;
    assert_eq!(
        recover(machine.active().unwrap(), &seen).unwrap(),
        PublicationRecoveryDecision::PublicationBlocked
    );
}

#[test]
fn duplicate_unexpected_or_oversized_observed_id_lists_are_rejected() {
    let machine = durable(None, manifest(&[2, 3]));
    for ids in [
        vec![point(2), point(2)],
        vec![point(2), point(9)],
        vec![point(2); 3],
    ] {
        let mut seen = observation(&machine);
        seen.staged_ids = ids;
        assert_eq!(
            recover(machine.active().unwrap(), &seen).unwrap(),
            PublicationRecoveryDecision::PublicationBlocked
        );
    }
}

#[test]
fn committed_recovery_needs_complete_effects_and_bound_receipts() {
    let mut machine = durable(Some(manifest(&[1])), manifest(&[2]));
    let (_commit, seen) = visible(&mut machine);
    assert_eq!(
        recover(machine.active().unwrap(), &seen).unwrap(),
        PublicationRecoveryDecision::PublishSnapshot
    );
    let mut missing = seen.clone();
    missing.staged_ids.clear();
    assert_eq!(
        recover(machine.active().unwrap(), &missing).unwrap(),
        PublicationRecoveryDecision::PublicationBlocked
    );
    for wrong in 0..4 {
        let mut transaction = machine.active().unwrap().clone();
        match wrong {
            0 => transaction.visible_commit = None,
            1 => {
                transaction
                    .visible_commit
                    .as_mut()
                    .unwrap()
                    .control_generation = 0;
            }
            2 => transaction
                .stage_receipt
                .as_mut()
                .unwrap()
                .staged_ids
                .clear(),
            _ => transaction
                .closure_receipt
                .as_mut()
                .unwrap()
                .closed_ids
                .clear(),
        }
        assert_eq!(
            recover(&transaction, &seen).unwrap(),
            PublicationRecoveryDecision::PublicationBlocked
        );
    }
}

#[test]
fn snapshot_boolean_does_not_replace_exact_snapshot_acknowledgement() {
    let mut machine = durable(None, manifest(&[2]));
    let (commit, mut seen) = visible(&mut machine);
    seen.snapshot_published = true;
    assert_eq!(
        recover(machine.active().unwrap(), &seen).unwrap(),
        PublicationRecoveryDecision::PublicationBlocked
    );
    assert_eq!(machine.complete(), Err(PublicationError::InvalidTransition));
    assert!(machine.active().is_some());
    machine.publish_control_snapshot(snapshot(&commit)).unwrap();
    assert_eq!(
        recover(machine.active().unwrap(), &seen).unwrap(),
        PublicationRecoveryDecision::Continue
    );
    let mut stale = machine.active().unwrap().clone();
    stale.snapshot_receipt.as_mut().unwrap().control_generation += 1;
    assert_eq!(
        recover(&stale, &seen).unwrap(),
        PublicationRecoveryDecision::PublicationBlocked
    );
}

#[test]
fn committed_phase_with_old_visibility_is_a_contradiction() {
    let mut machine = durable(None, manifest(&[2]));
    let (_, mut seen) = visible(&mut machine);
    seen.control_visible_epoch = epoch(0);
    assert_eq!(
        recover(machine.active().unwrap(), &seen).unwrap(),
        PublicationRecoveryDecision::PublicationBlocked
    );
}

#[test]
fn compensation_phase_cannot_be_mistaken_for_forward_staging() {
    let mut machine = durable(None, manifest(&[2]));
    let mut seen = observation(&machine);
    seen.staged_ids
        .push(machine.active().unwrap().prepared.difference().create[0].point_id);
    let plan = machine.begin_compensation_plan().unwrap();
    assert_eq!(
        recover(machine.active().unwrap(), &seen).unwrap(),
        PublicationRecoveryDecision::CompensateExact
    );
    machine.compensate_exact(compensation(&plan)).unwrap();
    seen.staged_ids.clear();
    assert_eq!(
        recover(machine.active().unwrap(), &seen).unwrap(),
        PublicationRecoveryDecision::PublicationBlocked
    );
}

#[test]
fn prepared_without_effects_can_continue_but_unexplained_effects_cannot() {
    let mut machine = PublicationCoordinator::new(32).unwrap();
    machine
        .submit_fixture(proposal(None, manifest(&[2]), "prepared"))
        .unwrap();
    let mut seen = observation(&machine);
    assert_eq!(
        recover(machine.active().unwrap(), &seen).unwrap(),
        PublicationRecoveryDecision::Continue
    );
    seen.staged_ids.push(point(2));
    assert_eq!(
        recover(machine.active().unwrap(), &seen).unwrap(),
        PublicationRecoveryDecision::PublicationBlocked
    );
}

#[test]
fn completed_normal_publication_keeps_the_epoch_floor_aligned() {
    let mut machine = durable(None, manifest(&[2]));
    let (commit, _) = visible(&mut machine);
    machine.publish_control_snapshot(snapshot(&commit)).unwrap();
    machine.complete().unwrap();
    assert_eq!(machine.visible_epoch(), Some(epoch(1)));
    assert_eq!(machine.last_reserved_epoch(), Some(epoch(1)));
    assert_eq!(
        machine
            .submit_fixture(proposal(Some(manifest(&[2])), manifest(&[3]), "tx-2"))
            .unwrap(),
        epoch(2)
    );
}

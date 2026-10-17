//! Canonical C08/C09/C13 inputs exercise pure publication decisions. Receipt,
//! journal and snapshot values remain synthetic phase observations, not I/O proof.

use search_contracts::{
    AccessPartitionId, Blake3Digest32, CollectionGenerationId, Epoch, InstallationIncarnationId,
    OpaqueId, OwnerEpoch, ProjectionMembershipId, ProjectionProfileSetId, ScoringDocumentId,
    ScoringPartitionId, SourceMembershipId, UnitId,
};
use search_point_identity::s11::PointId128;
use search_publication::*;

#[path = "support/publication.rs"]
mod publication_support;
use publication_support::{
    Fixture, PhasePort, PublicationCase, PublicationRecoveryObservation, SubmitFixture,
    projection_fixture,
};

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
    publication_support::persist_intent(&mut machine, &mut PhasePort::default()).unwrap();
    machine
}
const fn observation(machine: &PublicationCoordinator) -> PublicationRecoveryObservation {
    publication_support::recovery_observation(machine)
}
fn recover(
    transaction: &PublicationTransaction,
    seen: &PublicationRecoveryObservation,
) -> Result<PublicationRecoveryDecision, PublicationError> {
    publication_support::recover_observation(transaction, seen)
}
fn visible(
    machine: &mut PublicationCoordinator,
) -> (VisibleCommitReceipt, PublicationRecoveryObservation) {
    let transaction = machine.active().unwrap().clone();
    let difference = transaction.prepared().difference();
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
    let mut port = PhasePort::default();
    publication_support::stage_new_points(machine, &mut port).unwrap();
    publication_support::close_old_points(machine, &mut port).unwrap();
    publication_support::verify_readback(machine, &mut port).unwrap();
    publication_support::commit_visible_epoch(machine, &mut port).unwrap();
    let receipt = machine.active().unwrap().visible_commit().unwrap().clone();
    let mut observed = observation(machine);
    observed.staged_ids = staged;
    observed.closed_ids = closed;
    observed.control_visible_epoch = transaction.target_epoch();
    (receipt, observed)
}

#[test]
fn unknown_stage_outcome_is_resolved_by_readback_without_replaying_the_mutation() {
    let mut machine = durable(None, manifest(&[2]));
    let mut port = PhasePort::default();
    port.readback.next_outcome = Some(PublicationEffectOutcome::Unknown);
    assert_eq!(
        publication_support::stage_new_points(&mut machine, &mut port),
        Err(PublicationError::OutcomeUnknown)
    );
    let first_id = port.effects.effect_ids[0].clone();
    assert_eq!(
        publication_support::stage_new_points(&mut machine, &mut port),
        Ok(())
    );
    assert_eq!(port.effects.effect_ids, vec![first_id]);
    assert_eq!(
        machine.active().unwrap().phase(),
        PublicationPhase::NewPointsAcknowledged
    );
}

#[test]
fn full_vector_readback_mismatch_blocks_stage_advancement() {
    let mut machine = durable(None, manifest(&[2]));
    let point_id = machine.active().unwrap().prepared().difference().create[0].point_id;
    let mut port = PhasePort::default();
    port.readback.corrupt_vector = Some((PublicationEffectKind::StageNewPoints, point_id));
    assert_eq!(
        publication_support::stage_new_points(&mut machine, &mut port),
        Err(PublicationError::StageReadbackMismatch)
    );
    assert_eq!(
        machine.active().unwrap().phase(),
        PublicationPhase::PublicationBlocked
    );
    assert_eq!(port.effects.effect_ids.len(), 1);
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
        publication_support::persist_intent(&mut machine, &mut PhasePort::default()).unwrap();
        machine.begin_compensation_plan().unwrap();
        publication_support::compensate_exact(&mut machine, &mut PhasePort::default()).unwrap();
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
    assert_eq!(machine.active().unwrap().previous_visible_epoch(), epoch(4));
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
        publication_support::compensate_exact(&mut machine, &mut PhasePort::default()),
        Err(PublicationError::CompensationIncomplete)
    );
    assert_eq!(
        machine.finalize_aborted(),
        Err(PublicationError::InvalidTransition)
    );
    assert_eq!(
        machine.active().unwrap().phase(),
        PublicationPhase::Compensating
    );
    publication_support::compensate_and_restore(&mut machine, &mut PhasePort::default()).unwrap();
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
        let mut port = PhasePort::default();
        match case {
            0 => {
                port.readback.missing_point_ids_override =
                    Some((PublicationEffectKind::Compensate, vec![point(9)]));
            }
            1 => port
                .readback
                .unexpected_point_ids_by_effect
                .push((PublicationEffectKind::Compensate, vec![point(9)])),
            2 => {
                let old_id = plan.closed_ids[0];
                port.readback.corrupt_point_payload =
                    Some((PublicationEffectKind::Compensate, old_id));
            }
            3 => {
                port.readback.binding_transaction_override =
                    Some((PublicationEffectKind::Compensate, id("foreign")));
            }
            4 => {
                port.readback.binding_target_epoch_override =
                    Some((PublicationEffectKind::Compensate, epoch(3)));
            }
            _ => {
                port.readback.binding_new_manifest_override =
                    Some((PublicationEffectKind::Compensate, digest(99)));
            }
        }
        assert!(publication_support::compensate_and_restore(&mut machine, &mut port).is_err());
        assert_eq!(
            machine.active().unwrap().phase(),
            if case >= 3 {
                PublicationPhase::PublicationBlocked
            } else {
                PublicationPhase::Compensating
            }
        );
    }
}

#[test]
fn compensation_receipt_is_bound_to_its_reservation_not_just_transaction_name() {
    let mut machine = durable(None, manifest(&[2]));
    let plan = machine.begin_compensation_plan().unwrap();
    let mut stale = PhasePort::default();
    stale.readback.binding_target_epoch_override =
        Some((PublicationEffectKind::Compensate, epoch(0)));
    assert_eq!(
        publication_support::compensate_exact(&mut machine, &mut stale),
        Err(PublicationError::OperationMismatch)
    );
    assert_eq!(
        machine.active().unwrap().phase(),
        PublicationPhase::PublicationBlocked
    );
    let _ = plan;
    let mut partial_machine = durable(None, manifest(&[2]));
    let partial_plan = partial_machine.begin_compensation_plan().unwrap();
    let mut partial = PhasePort::default();
    partial.readback.missing_point_ids_override =
        Some((PublicationEffectKind::Compensate, Vec::new()));
    assert_eq!(
        publication_support::compensate_exact(&mut partial_machine, &mut partial),
        Err(PublicationError::CompensationIncomplete)
    );
    let _ = partial_plan;
    publication_support::compensate_exact(&mut partial_machine, &mut PhasePort::default()).unwrap();
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
        publication_support::compensate_exact(&mut machine, &mut PhasePort::default()),
        Err(PublicationError::CompensationIncomplete)
    );
    publication_support::compensate_and_restore(&mut machine, &mut PhasePort::default()).unwrap();
}

#[test]
fn corrupted_old_canonical_manifest_is_rejected_before_publication() {
    let mut old = manifest(&[1, 2]);
    old.plan.manifest.canonical_bytes[0] ^= 1;
    let new = manifest(&[2, 3]);
    let mut machine = PublicationCoordinator::new(32).unwrap();
    assert_eq!(
        machine.submit_fixture(proposal(Some(old), new, "corrupted-old")),
        Err(PublicationError::InvalidPreparedPublication)
    );
    assert!(machine.active().is_none());
    assert_eq!(machine.last_reserved_epoch(), None);
    assert_eq!(machine.current_manifest(), None);
}

#[test]
fn point_only_or_incomplete_membership_fence_cannot_abandon() {
    let mut machine = durable(Some(manifest(&[1])), manifest(&[2]));
    let mut fence = PhasePort::default();
    fence.readback.abandon_memberships_override = Some(Vec::new());
    assert_eq!(
        publication_support::abandon(&mut machine, &mut fence),
        Err(PublicationError::AbandonFenceMissing)
    );
    fence.readback.abandon_memberships_override =
        Some(vec![ProjectionMembershipId::from_bytes([1; 16])]);
    assert_eq!(
        publication_support::abandon(&mut machine, &mut fence),
        Err(PublicationError::AbandonFenceMissing)
    );
    fence.readback.abandon_memberships_override =
        Some(vec![ProjectionMembershipId::from_bytes([17; 16])]);
    publication_support::abandon(&mut machine, &mut fence).unwrap();
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
        .push(machine.active().unwrap().prepared().difference().retire[0].point_id);
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
    publication_support::persist_intent(&mut machine, &mut PhasePort::default()).unwrap();
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
        let transaction = machine.active().unwrap();
        let mut port = publication_support::recovery_port(transaction, &seen);
        match wrong {
            0 => port.intent_operation_id_override = Some(id("foreign-operation")),
            1 => port.intent_target_epoch_override = Some(epoch(2)),
            _ => {
                let mut changed = guards();
                changed.access_generation += 1;
                port.intent_guards_override = Some(changed);
            }
        }
        assert_eq!(
            publication_support::recover_readback(transaction, &mut port).unwrap(),
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
        let transaction = machine.active().unwrap();
        let mut port = publication_support::recovery_port(transaction, &seen);
        match wrong {
            0 => port.outcomes.push((
                PublicationEffectKind::CommitVisibleEpoch,
                PublicationEffectOutcome::NotApplied,
            )),
            1 => port.control_generation_override = Some(0),
            2 => port.staged_ids = Some(Vec::new()),
            _ => port.closed_ids = Some(Vec::new()),
        }
        assert_eq!(
            publication_support::recover_readback(transaction, &mut port).unwrap(),
            PublicationRecoveryDecision::PublicationBlocked
        );
    }
}

#[test]
fn snapshot_requires_exact_current_process_readback() {
    let mut machine = durable(None, manifest(&[2]));
    let (_commit, mut seen) = visible(&mut machine);
    seen.snapshot_published = false;
    assert_eq!(
        recover(machine.active().unwrap(), &seen).unwrap(),
        PublicationRecoveryDecision::PublishSnapshot
    );
    assert_eq!(machine.complete(), Err(PublicationError::InvalidTransition));
    assert!(machine.active().is_some());
    publication_support::publish_control_snapshot(&mut machine, &mut PhasePort::default()).unwrap();
    seen.snapshot_published = true;
    assert_eq!(
        recover(machine.active().unwrap(), &seen).unwrap(),
        PublicationRecoveryDecision::Continue
    );
    let active = machine.active().unwrap();
    let mut stale = publication_support::recovery_port(active, &seen);
    stale.snapshot_generation_override = Some(10);
    assert_eq!(
        publication_support::recover_readback(active, &mut stale).unwrap(),
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
        .push(machine.active().unwrap().prepared().difference().create[0].point_id);
    machine.begin_compensation_plan().unwrap();
    assert_eq!(
        recover(machine.active().unwrap(), &seen).unwrap(),
        PublicationRecoveryDecision::CompensateExact
    );
    publication_support::compensate_exact(&mut machine, &mut PhasePort::default()).unwrap();
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
    let (_commit, _) = visible(&mut machine);
    publication_support::publish_control_snapshot(&mut machine, &mut PhasePort::default()).unwrap();
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

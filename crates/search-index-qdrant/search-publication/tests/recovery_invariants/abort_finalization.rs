//! Negative acknowledgement fixtures. Receipts are synthetic, not live I/O evidence.
use super::*;

fn compensated(old: bool) -> PublicationCoordinator {
    let mut machine = durable(old.then(|| manifest(&[1])), manifest(&[2]));
    let plan = machine.begin_compensation_plan().unwrap();
    if old { machine.compensate_and_restore(compensation(&plan), restoration(&plan)).unwrap(); }
    else { machine.compensate_exact(compensation(&plan)).unwrap(); }
    machine
}
fn request(machine: &mut PublicationCoordinator) -> AbortFinalizationRequest {
    machine.prepare_abort_finalization(id("finalize-1"), 40, guards()).unwrap()
}
fn complete(machine: &mut PublicationCoordinator, request: AbortFinalizationRequest) {
    let observed = abort_support::commit(request);
    machine.acknowledge_abort_commit(observed.clone()).unwrap();
    machine.publish_abort_snapshot(abort_support::snapshot(&observed)).unwrap();
    machine.finalize_aborted().unwrap();
}

#[test]
fn compensation_alone_cannot_release_the_slot_and_commit_alone_is_not_snapshot_publication() {
    let mut machine = compensated(true);
    let before = machine.active().unwrap().clone();
    assert_eq!(machine.finalize_aborted(), Err(PublicationError::RecoveryBlocked));
    assert_eq!(machine.active(), Some(&before));
    let command = request(&mut machine);
    assert_eq!(command.resolution, AbortedPublicationResolution::Compensated {
        removal_readback: reference("removed"), restoration_readback: Some(reference("restored")),
    });
    let observed = abort_support::commit(command.clone());
    assert_eq!(machine.publish_abort_snapshot(abort_support::snapshot(&observed)), Err(PublicationError::RecoveryBlocked));
    machine.acknowledge_abort_commit(observed.clone()).unwrap();
    assert_eq!(machine.finalize_aborted(), Err(PublicationError::SnapshotPublicationFailed));
    assert_eq!(machine.submit(prepared(Some(manifest(&[1])), manifest(&[3]), "early")), Err(PublicationError::PublicationBusy));
    assert_eq!(machine.prepare_abort_finalization(id("finalize-1"), 40, guards()).unwrap(), command);
    machine.publish_abort_snapshot(abort_support::snapshot(&observed)).unwrap();
    machine.finalize_aborted().unwrap();
    assert_eq!(machine.visible_epoch(), epoch(0));
    assert_eq!(machine.last_reserved_epoch(), epoch(1));
    assert_eq!(machine.current_manifest(), Some(&manifest(&[1])));
    assert_eq!(machine.submit(prepared(Some(manifest(&[1])), manifest(&[3]), "next")).unwrap(), epoch(2));
}

#[test]
fn create_only_compensation_retains_absent_restoration_instead_of_inventing_evidence() {
    let mut machine = compensated(false);
    let command = request(&mut machine);
    assert_eq!(command.resolution, AbortedPublicationResolution::Compensated {
        removal_readback: reference("removed"), restoration_readback: None,
    });
    assert_eq!(command.intent, machine.active().unwrap().durable_intent.clone().unwrap());
    assert_eq!(command.preparation_receipt, reference("preparation"));
    complete(&mut machine, command);
}

#[test]
fn complete_exclusion_keeps_its_exact_scope_evidence_and_requires_both_acknowledgements() {
    let mut machine = durable(Some(manifest(&[1])), manifest(&[2]));
    let fence = AbandonFence { transaction_id: id("tx-1"), target_epoch: epoch(1),
        excluded_point_ids: [point(1), point(2)].into_iter().collect(),
        excluded_projection_memberships: [id("membership-1"), id("membership-2")].into_iter().collect(),
        excluded_scope_digest: digest(8), exclusion_receipt: reference("complete-exclusion"),
    };
    machine.abandon(&fence).unwrap();
    assert_eq!(machine.finalize_aborted(), Err(PublicationError::RecoveryBlocked));
    let command = request(&mut machine);
    assert_eq!(command.resolution, AbortedPublicationResolution::Excluded {
        exclusion_receipt: reference("complete-exclusion"), scope_digest: digest(8),
    });
    complete(&mut machine, command);
    assert_eq!(machine.visible_epoch(), epoch(0));
    assert_eq!(machine.current_manifest(), Some(&manifest(&[1])));
}

#[test]
fn failed_or_missing_compensation_cannot_prepare_a_finalization_command() {
    let mut machine = durable(Some(manifest(&[1])), manifest(&[2]));
    assert!(machine.prepare_abort_finalization(id("finalize-1"), 40, guards()).is_err());
    let plan = machine.begin_compensation_plan().unwrap();
    assert_eq!(machine.compensate_exact(compensation(&plan)), Err(PublicationError::CompensationIncomplete));
    assert!(machine.prepare_abort_finalization(id("finalize-1"), 40, guards()).is_err());
    assert!(machine.finalize_aborted().is_err());
    machine.compensate_and_restore(compensation(&plan), restoration(&plan)).unwrap();
    let command = request(&mut machine);
    complete(&mut machine, command);
}

#[test]
fn exact_retries_are_nonmutating_but_operation_generation_and_guard_changes_do_not_reset_pending_work() {
    let mut machine = compensated(true);
    let original = request(&mut machine);
    assert_eq!(request(&mut machine), original);
    assert_eq!(machine.prepare_abort_finalization(id("another-op"), 40, guards()), Err(PublicationError::OperationMismatch));
    assert_eq!(machine.prepare_abort_finalization(id("finalize-1"), 41, guards()), Err(PublicationError::OperationMismatch));
    let mut changed = guards(); changed.access_generation += 1;
    assert_eq!(machine.prepare_abort_finalization(id("finalize-1"), 40, changed), Err(PublicationError::OperationMismatch));
    let observed = abort_support::commit(original.clone());
    machine.acknowledge_abort_commit(observed.clone()).unwrap();
    machine.acknowledge_abort_commit(observed.clone()).unwrap();
    let snapshot = abort_support::snapshot(&observed);
    machine.publish_abort_snapshot(snapshot.clone()).unwrap();
    machine.publish_abort_snapshot(snapshot).unwrap();
    assert_eq!(request(&mut machine), original);
    machine.finalize_aborted().unwrap();
}

#[test]
fn invalid_request_inputs_fail_before_latching_and_do_not_burn_a_control_operation() {
    for (op, generation, expected) in [
        (id("persist-1"), 40, PublicationError::OperationMismatch),
        (id("finalize-1"), 0, PublicationError::ControlConflict),
        (id("finalize-1"), u64::MAX, PublicationError::ContractExhausted),
    ] {
        let mut machine = compensated(false);
        assert_eq!(machine.prepare_abort_finalization(op, generation, guards()), Err(expected));
        let command = request(&mut machine);
        complete(&mut machine, command);
    }
}

#[test]
fn all_monotone_guard_regressions_are_rejected_without_replacing_prepared_guards() {
    for axis in 0..6 {
        let mut machine = if axis == 0 {
            let mut owner_input = prepared(None, manifest(&[2]), "tx-1");
            owner_input.guards.owner_epoch = OwnerEpoch::new(2).unwrap();
            let mut higher = PublicationCoordinator::new(epoch(0), epoch(0), None, None, 32).unwrap();
            higher.submit(owner_input).unwrap();
            higher.persist_intent(id("persist-1"), reference("intent")).unwrap();
            let plan = higher.begin_compensation_plan().unwrap();
            higher.compensate_exact(compensation(&plan)).unwrap();
            higher
        } else { compensated(false) };
        let mut current = guards();
        match axis {
            0 => {}
            1 => current.source_catalog_generation -= 1,
            2 => current.membership_generation -= 1,
            3 => current.access_generation -= 1,
            4 => current.shadow_generation -= 1,
            _ => current.purge_generation -= 1,
        }
        assert_eq!(machine.prepare_abort_finalization(id("finalize-1"), 40, current), Err(PublicationError::GuardMismatch));
        let original = machine.active().unwrap().prepared.guards;
        let command = machine.prepare_abort_finalization(id("finalize-1"), 40, original).unwrap();
        complete(&mut machine, command);
    }
}

#[test]
fn current_successor_guards_remain_separate_from_original_intent_guards() {
    let mut machine = compensated(true);
    let mut current = guards(); current.owner_epoch = OwnerEpoch::new(2).unwrap();
    current.access_generation += 1; current.purge_generation += 1; current.profile_digest = digest(99);
    let command = machine.prepare_abort_finalization(id("successor-finalize"), 40, current).unwrap();
    assert_eq!(command.current_guards, current);
    assert_eq!(command.intent.guards, guards());
    complete(&mut machine, command);
}

#[test]
fn changed_complete_request_cannot_be_acknowledged_under_the_latched_operation() {
    for case in 0..12 {
        let mut machine = compensated(true);
        let original = request(&mut machine);
        let mut observed = abort_support::commit(original.clone());
        match case {
            0 => observed.request.operation_id = id("foreign"),
            1 => observed.request.expected_control_generation += 1,
            2 => observed.request.intent.transaction_id = id("foreign"),
            3 => observed.request.intent.target_epoch = epoch(2),
            4 => observed.request.intent.persist_operation_id = id("foreign"),
            5 => observed.request.intent.intent_receipt = reference("foreign"),
            6 => observed.request.intent.new_manifest_digest = digest(99),
            7 => observed.request.intent.old_manifest_digest = None,
            8 => observed.request.intent.guards.shadow_generation += 1,
            9 => observed.request.preparation_receipt = reference("foreign"),
            10 => observed.request.resolution = AbortedPublicationResolution::Compensated {
                removal_readback: reference("removed"), restoration_readback: None,
            },
            _ => observed.request.resolution = AbortedPublicationResolution::Excluded {
                exclusion_receipt: reference("unrelated-exclusion"), scope_digest: digest(8),
            },
        }
        assert_eq!(machine.acknowledge_abort_commit(observed), Err(PublicationError::OperationMismatch));
        assert_eq!(request(&mut machine), original);
        assert_eq!(machine.finalize_aborted(), Err(PublicationError::RecoveryBlocked));
        complete(&mut machine, original);
    }
}

#[test]
fn actual_route_visibility_floor_and_generation_must_match_the_durable_command() {
    for case in 0..5 {
        let mut machine = compensated(false);
        let original = request(&mut machine);
        let mut observed = abort_support::commit(original.clone());
        match case {
            0 => observed.collection_generation_id = CollectionGenerationId::from_bytes([9; 16]),
            1 => observed.visible_epoch = epoch(1),
            2 => observed.last_reserved_epoch = epoch(0),
            3 => observed.control_generation = 40,
            _ => observed.control_generation = 42,
        }
        assert_eq!(machine.acknowledge_abort_commit(observed), Err(PublicationError::ControlConflict));
        complete(&mut machine, original);
    }
}

#[test]
fn every_observed_guard_axis_is_checked_at_acknowledgement() {
    for axis in 0..7 {
        let mut machine = compensated(false);
        let original = request(&mut machine);
        let mut observed = abort_support::commit(original.clone());
        match axis {
            0 => observed.observed_guards.owner_epoch = OwnerEpoch::new(2).unwrap(),
            1 => observed.observed_guards.source_catalog_generation += 1,
            2 => observed.observed_guards.membership_generation += 1,
            3 => observed.observed_guards.access_generation += 1,
            4 => observed.observed_guards.shadow_generation += 1,
            5 => observed.observed_guards.purge_generation += 1,
            _ => observed.observed_guards.profile_digest = digest(99),
        }
        assert_eq!(machine.acknowledge_abort_commit(observed), Err(PublicationError::GuardMismatch));
        complete(&mut machine, original);
    }
}

#[test]
fn a_conflicting_repeat_cannot_replace_an_already_accepted_commit() {
    let mut machine = compensated(false);
    let original = request(&mut machine);
    let observed = abort_support::commit(original.clone());
    machine.acknowledge_abort_commit(observed.clone()).unwrap();
    for case in 0..2 {
        let mut wrong = observed.clone();
        if case == 0 { wrong.commit_receipt = reference("other-receipt"); }
        else { wrong.control_state_digest = digest(99); }
        assert_eq!(machine.acknowledge_abort_commit(wrong), Err(PublicationError::OperationMismatch));
    }
    machine.publish_abort_snapshot(abort_support::snapshot(&observed)).unwrap();
    machine.finalize_aborted().unwrap();
}

#[test]
fn stale_or_foreign_snapshots_keep_the_active_slot_occupied() {
    let mut machine = compensated(false);
    let original = request(&mut machine);
    let observed = abort_support::commit(original);
    machine.acknowledge_abort_commit(observed.clone()).unwrap();
    let good = abort_support::snapshot(&observed);
    for case in 0..3 {
        let mut wrong = good.clone();
        match case { 0 => wrong.transaction_id = id("foreign"), 1 => wrong.visible_epoch = epoch(1),
            _ => wrong.control_generation -= 1, }
        assert_eq!(machine.publish_abort_snapshot(wrong), Err(PublicationError::SnapshotPublicationFailed));
        assert_eq!(machine.finalize_aborted(), Err(PublicationError::SnapshotPublicationFailed));
    }
    machine.publish_abort_snapshot(good.clone()).unwrap();
    let mut wrong = good; wrong.snapshot_digest = digest(99);
    assert_eq!(machine.publish_abort_snapshot(wrong), Err(PublicationError::SnapshotPublicationFailed));
    machine.finalize_aborted().unwrap();
}

#[test]
fn explicit_block_after_commit_cannot_be_cleared_by_an_acknowledgement() {
    let mut machine = compensated(false);
    let original = request(&mut machine);
    let observed = abort_support::commit(original);
    machine.acknowledge_abort_commit(observed.clone()).unwrap();
    machine.block_publication().unwrap();
    assert!(machine.publish_abort_snapshot(abort_support::snapshot(&observed)).is_err());
    assert!(machine.acknowledge_abort_commit(observed).is_err());
    assert!(machine.finalize_aborted().is_err());
    assert!(machine.active().is_some());
}

#[test]
fn prior_finalization_acknowledgements_are_not_carried_into_the_next_aborted_operation() {
    let mut machine = compensated(false);
    let original = request(&mut machine);
    let old_commit = abort_support::commit(original.clone());
    complete(&mut machine, original);
    machine.submit(prepared(None, manifest(&[3]), "next-transaction")).unwrap();
    machine.persist_intent(id("persist-2"), reference("intent-2")).unwrap();
    let plan = machine.begin_compensation_plan().unwrap();
    machine.compensate_exact(compensation(&plan)).unwrap();
    assert_eq!(machine.finalize_aborted(), Err(PublicationError::RecoveryBlocked));
    assert!(machine.acknowledge_abort_commit(old_commit.clone()).is_err());
    let next = machine.prepare_abort_finalization(id("finalize-2"), 50, guards()).unwrap();
    assert_eq!(machine.acknowledge_abort_commit(old_commit), Err(PublicationError::OperationMismatch));
    complete(&mut machine, next);
    assert_eq!(machine.last_reserved_epoch(), epoch(2));
    assert_eq!(machine.visible_epoch(), epoch(0));
}

#[test]
fn finalization_diagnostics_redact_all_receipt_and_operation_references() {
    let mut machine = compensated(true);
    let command = request(&mut machine);
    let observed = abort_support::commit(command.clone());
    let debug = format!("{command:?} {:?} {observed:?}", command.resolution);
    for secret in ["finalize-1", "tx-1", "persist-1", "fixture:removed", "fixture:restored",
        "fixture:intent", "fixture:preparation", "fixture:abort-commit-readback"] {
        assert!(!debug.contains(secret));
    }
}

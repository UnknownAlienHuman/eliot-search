//! Negative acknowledgement fixtures. Receipts are synthetic, not live I/O evidence.
use super::*;

fn compensated(old: bool) -> PublicationCoordinator {
    let mut machine = durable(old.then(|| manifest(&[1])), manifest(&[2]));
    machine.begin_compensation_plan().unwrap();
    if old {
        publication_support::compensate_and_restore(&mut machine, &mut PhasePort::default())
            .unwrap();
    } else {
        publication_support::compensate_exact(&mut machine, &mut PhasePort::default()).unwrap();
    }
    machine
}
fn request(machine: &mut PublicationCoordinator) -> AbortFinalizationRequest {
    machine
        .prepare_abort_finalization(id("finalize-1"), 40, guards())
        .unwrap()
}
fn complete(machine: &mut PublicationCoordinator, request: AbortFinalizationRequest) {
    let _ = request;
    let mut port = PhasePort::default();
    abort_support::commit(machine, &mut port).unwrap();
    abort_support::snapshot(machine, &mut port).unwrap();
    machine.finalize_aborted().unwrap();
}

#[test]
fn compensation_alone_cannot_release_the_slot_and_commit_alone_is_not_snapshot_publication() {
    let mut machine = compensated(true);
    let before = machine.active().unwrap().clone();
    assert_eq!(
        machine.finalize_aborted(),
        Err(PublicationError::RecoveryBlocked)
    );
    assert_eq!(machine.active(), Some(&before));
    let command = request(&mut machine);
    assert!(matches!(
        command.resolution,
        AbortedPublicationResolution::Compensated {
            restored_old_points: true,
            ..
        }
    ));
    assert_eq!(
        abort_support::snapshot(&mut machine, &mut PhasePort::default()),
        Err(PublicationError::RecoveryBlocked)
    );
    let mut port = PhasePort::default();
    abort_support::commit(&mut machine, &mut port).unwrap();
    assert_eq!(
        machine.finalize_aborted(),
        Err(PublicationError::SnapshotPublicationFailed)
    );
    assert_eq!(
        machine.submit_fixture(proposal(Some(manifest(&[1])), manifest(&[3]), "early")),
        Err(PublicationError::PublicationBusy)
    );
    assert_eq!(
        machine
            .prepare_abort_finalization(id("finalize-1"), 40, guards())
            .unwrap(),
        command
    );
    abort_support::snapshot(&mut machine, &mut port).unwrap();
    machine.finalize_aborted().unwrap();
    assert_eq!(machine.visible_epoch(), Some(epoch(1)));
    assert_eq!(machine.last_reserved_epoch(), Some(epoch(2)));
    assert_eq!(machine.current_manifest(), Some(manifest(&[1]).manifest()));
    assert_eq!(
        machine
            .submit_fixture(proposal(Some(manifest(&[1])), manifest(&[3]), "next"))
            .unwrap(),
        epoch(3)
    );
}

#[test]
fn create_only_compensation_retains_absent_restoration_instead_of_inventing_evidence() {
    let mut machine = compensated(false);
    let command = request(&mut machine);
    assert!(matches!(
        command.resolution,
        AbortedPublicationResolution::Compensated {
            restored_old_points: false,
            ..
        }
    ));
    assert_eq!(
        &command.intent,
        machine.active().unwrap().durable_intent().unwrap()
    );
    let prepared_input_digest: Blake3Digest32 = command.prepared_input_digest;
    assert_eq!(
        prepared_input_digest,
        machine.active().unwrap().prepared().prepared_input_digest()
    );
    complete(&mut machine, command);
}

#[test]
fn complete_exclusion_keeps_its_exact_scope_evidence_and_requires_both_acknowledgements() {
    let old = manifest(&[1]);
    let new = manifest(&[2]);
    let mut machine = durable(Some(old.clone()), new);
    publication_support::abandon(&mut machine, &mut PhasePort::default()).unwrap();
    assert_eq!(
        machine.finalize_aborted(),
        Err(PublicationError::RecoveryBlocked)
    );
    let command = request(&mut machine);
    assert!(matches!(&command.resolution,
        AbortedPublicationResolution::Excluded { excluded_projection_memberships, .. }
            if excluded_projection_memberships == &vec![ProjectionMembershipId::from_bytes([17; 16])]
    ));
    complete(&mut machine, command);
    assert_eq!(machine.visible_epoch(), Some(epoch(1)));
    assert_eq!(machine.current_manifest(), Some(old.manifest()));
}

#[test]
fn failed_or_missing_compensation_cannot_prepare_a_finalization_command() {
    let mut machine = durable(Some(manifest(&[1])), manifest(&[2]));
    assert!(
        machine
            .prepare_abort_finalization(id("finalize-1"), 40, guards())
            .is_err()
    );
    machine.begin_compensation_plan().unwrap();
    let mut incomplete = PhasePort::default();
    incomplete.readback.missing_point_ids_override =
        Some((PublicationEffectKind::Compensate, Vec::new()));
    assert_eq!(
        publication_support::compensate_and_restore(&mut machine, &mut incomplete),
        Err(PublicationError::CompensationIncomplete)
    );
    assert!(
        machine
            .prepare_abort_finalization(id("finalize-1"), 40, guards())
            .is_err()
    );
    assert!(machine.finalize_aborted().is_err());
    publication_support::compensate_and_restore(&mut machine, &mut PhasePort::default()).unwrap();
    let command = request(&mut machine);
    complete(&mut machine, command);
}

#[test]
fn exact_retries_are_nonmutating_but_operation_generation_and_guard_changes_do_not_reset_pending_work()
 {
    let mut machine = compensated(true);
    let original = request(&mut machine);
    assert_eq!(request(&mut machine), original);
    assert_eq!(
        machine.prepare_abort_finalization(id("another-op"), 40, guards()),
        Err(PublicationError::OperationMismatch)
    );
    assert_eq!(
        machine.prepare_abort_finalization(id("finalize-1"), 41, guards()),
        Err(PublicationError::OperationMismatch)
    );
    let mut changed = guards();
    changed.access_generation += 1;
    assert_eq!(
        machine.prepare_abort_finalization(id("finalize-1"), 40, changed),
        Err(PublicationError::OperationMismatch)
    );
    let mut port = PhasePort::default();
    abort_support::commit(&mut machine, &mut port).unwrap();
    abort_support::commit(&mut machine, &mut port).unwrap();
    abort_support::snapshot(&mut machine, &mut port).unwrap();
    abort_support::snapshot(&mut machine, &mut port).unwrap();
    assert_eq!(request(&mut machine), original);
    machine.finalize_aborted().unwrap();
}

#[test]
fn invalid_request_inputs_fail_before_latching_and_do_not_burn_a_control_operation() {
    for (op, generation, expected) in [
        (id("persist-1"), 40, PublicationError::OperationMismatch),
        (id("finalize-1"), 0, PublicationError::ControlConflict),
        (
            id("finalize-1"),
            u64::MAX,
            PublicationError::ContractExhausted,
        ),
    ] {
        let mut machine = compensated(false);
        assert_eq!(
            machine.prepare_abort_finalization(op, generation, guards()),
            Err(expected)
        );
        let command = request(&mut machine);
        complete(&mut machine, command);
    }
}

#[test]
fn all_monotone_guard_regressions_are_rejected_without_replacing_prepared_guards() {
    for axis in 0..6 {
        let mut machine = if axis == 0 {
            let mut owner_input = proposal(None, manifest(&[2]), "tx-1");
            owner_input.original_guards.owner_epoch = OwnerEpoch::new(8).unwrap();
            owner_input.current_guards = owner_input.original_guards;
            let mut higher = PublicationCoordinator::new(32).unwrap();
            higher.submit_fixture(owner_input).unwrap();
            publication_support::persist_intent(&mut higher, &mut PhasePort::default()).unwrap();
            higher.begin_compensation_plan().unwrap();
            publication_support::compensate_exact(&mut higher, &mut PhasePort::default()).unwrap();
            higher
        } else {
            compensated(false)
        };
        let mut current = guards();
        match axis {
            0 => {}
            1 => current.source_catalog_generation -= 1,
            2 => current.membership_generation -= 1,
            3 => current.access_generation -= 1,
            4 => current.shadow_generation -= 1,
            _ => current.purge_generation -= 1,
        }
        assert_eq!(
            machine.prepare_abort_finalization(id("finalize-1"), 40, current),
            Err(PublicationError::GuardMismatch)
        );
        let original = machine.active().unwrap().durable_intent().unwrap().guards();
        let command = machine
            .prepare_abort_finalization(id("finalize-1"), 40, original)
            .unwrap();
        complete(&mut machine, command);
    }
}

#[test]
fn current_successor_guards_remain_separate_from_original_intent_guards() {
    let mut machine = compensated(true);
    let mut current = guards();
    current.owner_epoch = OwnerEpoch::new(8).unwrap();
    current.access_generation += 1;
    current.purge_generation += 1;
    current.profile_digest = digest(99);
    let command = machine
        .prepare_abort_finalization(id("successor-finalize"), 40, current)
        .unwrap();
    assert_eq!(command.current_guards, current);
    assert_eq!(command.intent.guards(), guards());
    complete(&mut machine, command);
}

#[test]
fn changed_complete_request_cannot_be_acknowledged_under_the_latched_operation() {
    for case in 0..12 {
        let mut machine = compensated(true);
        let original = request(&mut machine);
        let mut port = PhasePort::default();
        match case {
            0 => {
                let mut wrong = original.clone();
                wrong.operation_id = id("foreign");
                port.readback.abort_request_override = Some(wrong);
            }
            1 => {
                let mut wrong = original.clone();
                wrong.expected_control_generation += 1;
                port.readback.abort_request_override = Some(wrong);
            }
            2 => {
                let mut wrong = original.clone();
                wrong.collection_generation_id = CollectionGenerationId::from_bytes([99; 16]);
                port.readback.abort_request_override = Some(wrong);
            }
            3 => {
                let mut wrong = original.clone();
                wrong.previous_visible_epoch = epoch(3);
                port.readback.abort_request_override = Some(wrong);
            }
            4 => {
                let mut wrong = original.clone();
                wrong.current_guards.access_generation += 1;
                port.readback.abort_request_override = Some(wrong);
            }
            5 => {
                let mut wrong = original.clone();
                wrong.prepared_input_digest = digest(99);
                port.readback.abort_request_override = Some(wrong);
            }
            6 => {
                let mut wrong = original.clone();
                wrong.prepared_input_digest = digest(98);
                port.readback.abort_request_override = Some(wrong);
            }
            7 => {
                let mut wrong = original.clone();
                wrong.resolution = AbortedPublicationResolution::Compensated {
                    operation_id: id("other-compensation"),
                    restored_old_points: false,
                };
                port.readback.abort_request_override = Some(wrong);
            }
            8 => {
                let mut wrong = original.clone();
                wrong.resolution = AbortedPublicationResolution::Excluded {
                    operation_id: id("other-exclusion"),
                    excluded_projection_memberships: Vec::new(),
                };
                port.readback.abort_request_override = Some(wrong);
            }
            9 => {
                port.readback.control_route_override =
                    Some(CollectionGenerationId::from_bytes([9; 16]))
            }
            10 => port.readback.control_after_epoch_override = Some(epoch(1)),
            _ => port.readback.control_generation_override = Some(40),
        }
        assert_eq!(
            abort_support::commit(&mut machine, &mut port),
            Err(if case <= 8 {
                PublicationError::OperationMismatch
            } else {
                PublicationError::ControlConflict
            })
        );
        assert_eq!(request(&mut machine), original);
        assert_eq!(
            machine.finalize_aborted(),
            Err(PublicationError::RecoveryBlocked)
        );
        complete(&mut machine, original);
    }
}

#[test]
fn actual_route_visibility_floor_and_generation_must_match_the_durable_command() {
    for case in 0..5 {
        let mut machine = compensated(false);
        let original = request(&mut machine);
        let mut port = PhasePort::default();
        match case {
            0 => {
                port.readback.control_route_override =
                    Some(CollectionGenerationId::from_bytes([9; 16]))
            }
            1 => port.readback.control_after_epoch_override = Some(epoch(1)),
            2 => port.readback.control_floor_override = Some(epoch(0)),
            3 => port.readback.control_generation_override = Some(40),
            _ => port.readback.control_generation_override = Some(42),
        }
        assert_eq!(
            abort_support::commit(&mut machine, &mut port),
            Err(PublicationError::ControlConflict)
        );
        complete(&mut machine, original);
    }
}

#[test]
fn every_observed_guard_axis_is_checked_at_acknowledgement() {
    for axis in 0..7 {
        let mut machine = compensated(false);
        let original = request(&mut machine);
        let mut port = PhasePort::default();
        let mut observed_guards = original.current_guards;
        match axis {
            0 => observed_guards.owner_epoch = OwnerEpoch::new(2).unwrap(),
            1 => observed_guards.source_catalog_generation += 1,
            2 => observed_guards.membership_generation += 1,
            3 => observed_guards.access_generation += 1,
            4 => observed_guards.shadow_generation += 1,
            5 => observed_guards.purge_generation += 1,
            _ => observed_guards.profile_digest = digest(99),
        }
        port.readback.control_guards_override = Some(observed_guards);
        assert_eq!(
            abort_support::commit(&mut machine, &mut port),
            Err(PublicationError::GuardMismatch)
        );
        complete(&mut machine, original);
    }
}

#[test]
fn a_conflicting_repeat_cannot_replace_an_already_accepted_commit() {
    let mut machine = compensated(false);
    let _original = request(&mut machine);
    let mut port = PhasePort::default();
    abort_support::commit(&mut machine, &mut port).unwrap();
    port.readback.control_state_digest_override = Some(digest(99));
    assert_eq!(
        abort_support::commit(&mut machine, &mut port),
        Err(PublicationError::OperationMismatch)
    );
    port.readback.control_state_digest_override = None;
    abort_support::snapshot(&mut machine, &mut port).unwrap();
    machine.finalize_aborted().unwrap();
}

#[test]
fn stale_or_foreign_snapshots_keep_the_active_slot_occupied() {
    let mut machine = compensated(false);
    let _original = request(&mut machine);
    let mut port = PhasePort::default();
    abort_support::commit(&mut machine, &mut port).unwrap();
    for case in 0..3 {
        port.readback.snapshot_visible_epoch_override = None;
        port.readback.snapshot_generation_override = None;
        port.readback.snapshot_route_override = None;
        match case {
            0 => port.readback.snapshot_visible_epoch_override = Some(epoch(1)),
            1 => port.readback.snapshot_generation_override = Some(40),
            _ => {
                port.readback.snapshot_route_override =
                    Some(CollectionGenerationId::from_bytes([9; 16]))
            }
        }
        assert_eq!(
            abort_support::snapshot(&mut machine, &mut port),
            Err(PublicationError::SnapshotPublicationFailed)
        );
        assert_eq!(
            machine.finalize_aborted(),
            Err(PublicationError::SnapshotPublicationFailed)
        );
    }
    port.readback.snapshot_visible_epoch_override = None;
    port.readback.snapshot_generation_override = None;
    port.readback.snapshot_route_override = None;
    abort_support::snapshot(&mut machine, &mut port).unwrap();
    port.readback.snapshot_digest_override = Some(digest(99));
    assert_eq!(
        abort_support::snapshot(&mut machine, &mut port),
        Err(PublicationError::SnapshotPublicationFailed)
    );
    port.readback.snapshot_digest_override = None;
    machine.finalize_aborted().unwrap();
}

#[test]
fn explicit_block_after_commit_cannot_be_cleared_by_an_acknowledgement() {
    let mut machine = compensated(false);
    let _original = request(&mut machine);
    let mut port = PhasePort::default();
    abort_support::commit(&mut machine, &mut port).unwrap();
    machine.block_publication().unwrap();
    assert!(abort_support::snapshot(&mut machine, &mut port).is_err());
    assert!(abort_support::commit(&mut machine, &mut port).is_err());
    assert!(machine.finalize_aborted().is_err());
    assert!(machine.active().is_some());
}

#[test]
fn prior_finalization_acknowledgements_are_not_carried_into_the_next_aborted_operation() {
    let mut machine = compensated(false);
    let original = request(&mut machine);
    let mut old_port = PhasePort::default();
    abort_support::commit(&mut machine, &mut old_port).unwrap();
    abort_support::snapshot(&mut machine, &mut old_port).unwrap();
    complete(&mut machine, original.clone());
    machine
        .submit_fixture(proposal(None, manifest(&[3]), "next-transaction"))
        .unwrap();
    publication_support::persist_intent(&mut machine, &mut PhasePort::default()).unwrap();
    machine.begin_compensation_plan().unwrap();
    publication_support::compensate_exact(&mut machine, &mut PhasePort::default()).unwrap();
    assert_eq!(
        machine.finalize_aborted(),
        Err(PublicationError::RecoveryBlocked)
    );
    let mut stale_port = PhasePort::default();
    stale_port.readback.abort_request_override = Some(original.clone());
    assert!(abort_support::commit(&mut machine, &mut stale_port).is_err());
    let next = machine
        .prepare_abort_finalization(id("finalize-2"), 50, guards())
        .unwrap();
    let mut stale_port = PhasePort::default();
    stale_port.readback.abort_request_override = Some(original);
    assert_eq!(
        abort_support::commit(&mut machine, &mut stale_port),
        Err(PublicationError::OperationMismatch)
    );
    complete(&mut machine, next);
    assert_eq!(machine.last_reserved_epoch(), Some(epoch(2)));
    assert_eq!(machine.visible_epoch(), Some(epoch(0)));
}

#[test]
fn finalization_diagnostics_redact_all_receipt_and_operation_references() {
    let mut machine = compensated(true);
    let command = request(&mut machine);
    let debug = format!("{command:?} {:?}", command.resolution);
    for secret in [
        "finalize-1",
        "tx-1",
        "persist-1",
        "fixture:removed",
        "fixture:restored",
        "fixture:intent",
        "fixture:preparation",
        "fixture:abort-commit-readback",
    ] {
        assert!(!debug.contains(secret));
    }
}

//! C08/C09/C13 inputs are real library products; phase receipts are synthetic
//! observations and prove no journal, Qdrant, native ownership, or crash behavior.

use search_contracts::{
    Blake3Digest32, CollectionGenerationId, Epoch, OpaqueId, OwnerEpoch, ReceiptRef,
};
use search_point_identity::s11::PointId128;
use search_publication::*;

#[path = "support/publication.rs"]
mod publication_support;
use publication_support::{
    Fixture, PublicationCase, RestoreScenario, SubmitFixture, projection_fixture,
    projection_fixture_at,
};

#[path = "support/abort_finalization.rs"]
mod abort_support;

fn id(value: &str) -> OpaqueId {
    OpaqueId::new(value).unwrap()
}
fn epoch(value: i64) -> Epoch {
    Epoch::new(value).unwrap()
}
const fn digest(value: u8) -> Blake3Digest32 {
    Blake3Digest32::from_bytes([value; 32])
}
fn reference(value: &str) -> ReceiptRef {
    ReceiptRef::new(format!("fixture:{value}")).unwrap()
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
fn snapshot(commit: &VisibleCommitReceipt) -> SnapshotPublishReceipt {
    SnapshotPublishReceipt {
        transaction_id: commit.transaction_id.clone(),
        visible_epoch: commit.visible_epoch,
        control_generation: commit.control_generation,
        snapshot_digest: commit.control_state_digest,
    }
}

/// Produce a canonical preparation and synthetic phase-observation prefix.
fn history(
    level: u8,
    replacement: bool,
) -> (
    PublicationCoordinator,
    Option<ControlCommitObservation>,
    PublicationCase,
) {
    let old = replacement.then(|| manifest(&[1]));
    let previous = if replacement { epoch(4) } else { epoch(0) };
    let floor = if replacement { epoch(8) } else { epoch(0) };
    let mut case = publication_support::proposal(old, manifest(&[2]), "original-transaction");
    case.seed = publication_support::OwnerSeed {
        previous_visible: previous,
        current_visible: previous,
        last_reserved: floor,
    };
    let mut machine = PublicationCoordinator::new(32).unwrap();
    machine.submit_fixture(case.clone()).unwrap();
    machine
        .persist_intent(
            id("original-intent-operation"),
            reference("original-intent"),
        )
        .unwrap();
    let transaction = machine.active().unwrap().clone();
    let difference = transaction.prepared.difference();
    let mut staged = difference
        .create
        .iter()
        .map(|entry| entry.point_id)
        .collect::<Vec<_>>();
    let mut closed = difference
        .retire
        .iter()
        .map(|entry| entry.point_id)
        .collect::<Vec<_>>();
    staged.sort();
    closed.sort();
    if level >= 1 {
        machine
            .stage_new_points(StageReceipt {
                transaction_id: transaction.transaction_id().clone(),
                target_epoch: transaction.target_epoch,
                staged_ids: staged,
                missing_ids: vec![],
                unexpected_ids: vec![],
                readback_digest: digest(3),
                mutation_receipt: reference("original-stage"),
            })
            .unwrap();
    }
    if level >= 2 {
        machine
            .close_old_points(ClosureReceipt {
                transaction_id: transaction.transaction_id().clone(),
                target_epoch: transaction.target_epoch,
                closed_ids: closed.clone(),
                missing_ids: vec![],
                unexpected_ids: vec![],
                readback_digest: digest(4),
                mutation_receipt: reference("original-closure"),
            })
            .unwrap();
    }
    if level >= 3 {
        machine
            .verify_readback(
                digest(3),
                digest(4),
                (!closed.is_empty()).then(|| digest(5)),
            )
            .unwrap();
    }
    let mut observation = None;
    if level >= 4 {
        let commit = ControlCommitObservation {
            before_visible_epoch: previous,
            after_visible_epoch: transaction.target_epoch,
            observed_guards: guards(),
            control_generation: 31,
            control_state_digest: digest(6),
        };
        let receipt = machine.commit_visible_epoch(commit).unwrap();
        observation = Some(commit);
        if level >= 5 {
            machine
                .publish_control_snapshot(snapshot(&receipt))
                .unwrap();
        }
    }
    (machine, observation, case)
}

fn input(level: u8, replacement: bool) -> RestoreScenario {
    let (machine, commit, case) = history(level, replacement);
    let transaction = machine.active().unwrap();
    RestoreScenario {
        case,
        previous_visible_epoch: transaction.previous_visible_epoch,
        last_reserved_epoch: machine.last_reserved_epoch().unwrap(),
        collection_generation_id: transaction.prepared.scope().collection_generation_id,
        current_guards: guards(),
        phase: transaction.phase,
        intent: transaction.durable_intent.clone().unwrap(),
        stage_receipt: transaction.stage_receipt.clone(),
        closure_receipt: transaction.closure_receipt.clone(),
        verified: transaction.verified.clone(),
        control_commit: commit,
        observation: PublicationRecoveryObservation {
            intent_durable: true,
            staged_ids: transaction
                .stage_receipt
                .as_ref()
                .map_or_else(Vec::new, |r| r.staged_ids.clone()),
            closed_ids: transaction
                .closure_receipt
                .as_ref()
                .map_or_else(Vec::new, |r| r.closed_ids.clone()),
            control_visible_epoch: machine.visible_epoch().unwrap(),
            snapshot_published: false,
            abandon_fence_durable: false,
        },
    }
}
fn restore(input: &RestoreScenario) -> (PublicationCoordinator, PublicationRecoveryDecision) {
    input.restore(32).unwrap()
}

#[test]
fn all_precommit_prefixes_restore_the_original_slot_and_original_operation_ids() {
    for level in 0..=3 {
        let (original, _, _) = history(level, false);
        let expected = original.active().unwrap();
        let (mut restored, decision) = restore(&input(level, false));
        let actual = restored.active().unwrap();
        assert_eq!(decision, PublicationRecoveryDecision::Continue);
        assert_eq!(actual.transaction_id(), expected.transaction_id());
        assert_eq!(
            actual.previous_visible_epoch,
            expected.previous_visible_epoch
        );
        assert_eq!(actual.target_epoch, expected.target_epoch);
        assert_eq!(
            actual.prepared.prepared_input_digest(),
            expected.prepared.prepared_input_digest()
        );
        assert_eq!(actual.phase, expected.phase);
        assert_eq!(actual.durable_intent, expected.durable_intent);
        assert_eq!(actual.stage_receipt, expected.stage_receipt);
        assert_eq!(actual.closure_receipt, expected.closure_receipt);
        assert_eq!(actual.verified, expected.verified);
        assert_eq!(restored.visible_epoch(), Some(epoch(0)));
        assert_eq!(restored.last_reserved_epoch(), Some(epoch(1)));
        assert_eq!(
            restored.submit_fixture(proposal(None, manifest(&[3]), "competing")),
            Err(PublicationError::PublicationBusy)
        );
        assert_eq!(restored.last_reserved_epoch(), Some(epoch(1)));
    }
}

#[test]
fn restored_verified_transaction_can_finish_through_the_existing_commit_and_snapshot_path() {
    let expected = projection_fixture_at(&[2], epoch(1)).plan.manifest;
    let (mut restored, _) = restore(&input(3, false));
    let commit = restored
        .commit_visible_epoch(ControlCommitObservation {
            before_visible_epoch: epoch(0),
            after_visible_epoch: epoch(1),
            observed_guards: guards(),
            control_generation: 31,
            control_state_digest: digest(6),
        })
        .unwrap();
    assert_eq!(
        restored.complete(),
        Err(PublicationError::InvalidTransition)
    );
    restored
        .publish_control_snapshot(snapshot(&commit))
        .unwrap();
    assert_eq!(restored.complete().unwrap(), commit);
    assert_eq!(restored.current_manifest(), Some(&expected));
    assert_eq!(restored.last_reserved_epoch(), Some(epoch(1)));
}

#[test]
fn both_committed_phases_need_a_new_current_process_snapshot_acknowledgement() {
    for level in [4, 5] {
        let (mut restored, decision) = restore(&input(level, false));
        assert_eq!(decision, PublicationRecoveryDecision::PublishSnapshot);
        assert_eq!(
            restored.active().unwrap().phase,
            PublicationPhase::ControlCommitted
        );
        assert!(restored.active().unwrap().snapshot_receipt.is_none());
        assert_eq!(
            restored.complete(),
            Err(PublicationError::InvalidTransition)
        );
        let commit = restored.active().unwrap().visible_commit.clone().unwrap();
        restored
            .publish_control_snapshot(snapshot(&commit))
            .unwrap();
        restored.complete().unwrap();
        assert_eq!(restored.visible_epoch(), Some(epoch(1)));
        assert!(restored.active().is_none());
    }
}

#[test]
fn a_previous_process_snapshot_flag_is_not_accepted_as_fresh_evidence() {
    let mut request = input(5, false);
    request.observation.snapshot_published = true;
    assert_eq!(
        request.restore(32).unwrap_err(),
        PublicationError::RecoveryBlocked
    );
}

#[test]
fn partial_effects_restore_two_sided_compensation_and_preserve_consumed_gaps() {
    let mut request = input(3, true);
    request.observation.staged_ids.clear(); // Old points remain closed.
    let (mut restored, decision) = restore(&request);
    assert_eq!(decision, PublicationRecoveryDecision::CompensateExact);
    assert_eq!(
        restored.active().unwrap().phase,
        PublicationPhase::Compensating
    );
    assert_eq!(restored.visible_epoch(), Some(epoch(4)));
    assert_eq!(restored.last_reserved_epoch(), Some(epoch(9)));
    let plan = restored.begin_compensation_plan().unwrap();
    assert_eq!(
        plan.staged_ids,
        manifest(&[2])
            .entries
            .iter()
            .map(|entry| entry.point_id)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        plan.closed_ids,
        manifest(&[1])
            .entries
            .iter()
            .map(|entry| entry.point_id)
            .collect::<Vec<_>>()
    );
    let removed = CompensationReceipt {
        transaction_id: plan.transaction_id.clone(),
        target_epoch: plan.target_epoch,
        compensated_ids: plan.staged_ids.clone(),
        remaining_ids: vec![],
        readback_receipt: reference("removed"),
    };
    assert_eq!(
        restored.compensate_exact(removed.clone()),
        Err(PublicationError::CompensationIncomplete)
    );
    restored
        .compensate_and_restore(
            removed,
            RestorationReceipt {
                transaction_id: plan.transaction_id,
                target_epoch: plan.target_epoch,
                restored_ids: plan.closed_ids,
                remaining_ids: vec![],
                readback_receipt: reference("restored"),
            },
        )
        .unwrap();
    assert_eq!(
        restored.finalize_aborted(),
        Err(PublicationError::RecoveryBlocked)
    );
    abort_support::acknowledge(&mut restored); // Synthetic acknowledgements; no I/O claim.
    restored.finalize_aborted().unwrap();
    let next = proposal(
        Some(manifest(&[1])),
        manifest(&[3]),
        "next-after-resolution",
    );
    assert_eq!(restored.submit_fixture(next).unwrap(), epoch(10));
}

#[test]
fn every_changed_live_guard_prevents_restored_forward_commit() {
    for axis in 0..7 {
        let mut request = input(3, false);
        match axis {
            0 => request.current_guards.owner_epoch = OwnerEpoch::new(8).unwrap(),
            1 => request.current_guards.source_catalog_generation += 1,
            2 => request.current_guards.membership_generation += 1,
            3 => request.current_guards.access_generation += 1,
            4 => request.current_guards.shadow_generation += 1,
            5 => request.current_guards.purge_generation += 1,
            _ => request.current_guards.profile_digest = digest(99),
        }
        let (mut restored, decision) = restore(&request);
        assert_eq!(decision, PublicationRecoveryDecision::CompensateExact);
        assert_eq!(
            restored.active().unwrap().phase,
            PublicationPhase::Compensating
        );
        assert_eq!(
            restored
                .active()
                .unwrap()
                .durable_intent
                .as_ref()
                .unwrap()
                .guards,
            guards()
        );
        assert_eq!(
            restored.commit_visible_epoch(ControlCommitObservation {
                before_visible_epoch: epoch(0),
                after_visible_epoch: epoch(1),
                observed_guards: guards(),
                control_generation: 31,
                control_state_digest: digest(6),
            }),
            Err(PublicationError::InvalidTransition)
        );
    }
}

#[test]
fn an_older_owner_or_foreign_collection_cannot_hydrate_the_transaction() {
    let mut old_owner = input(0, false);
    old_owner.current_guards.owner_epoch = OwnerEpoch::new(6).unwrap();
    assert_eq!(
        old_owner.restore(32).unwrap_err(),
        PublicationError::GuardMismatch
    );
    let mut route = input(0, false);
    route.collection_generation_id = CollectionGenerationId::from_bytes([2; 16]);
    assert_eq!(
        route.restore(32).unwrap_err(),
        PublicationError::OperationMismatch
    );
}

#[test]
fn a_new_owner_does_not_compensate_an_already_committed_visible_epoch() {
    let mut request = input(4, false);
    request.current_guards.owner_epoch = OwnerEpoch::new(8).unwrap();
    let (mut restored, decision) = restore(&request);
    assert_eq!(decision, PublicationRecoveryDecision::PublishSnapshot);
    assert_eq!(restored.visible_epoch(), Some(epoch(1)));
    assert!(restored.begin_compensation_plan().is_err());
    assert!(restored.complete().is_err());
}

#[test]
fn unresolved_target_must_equal_the_consumed_floor_not_visible_plus_one() {
    for floor in [0, 8, 10] {
        let mut request = input(0, true);
        request.last_reserved_epoch = epoch(floor);
        assert_eq!(
            request.restore(32).unwrap_err(),
            PublicationError::EpochMismatch
        );
    }
    let (restored, _) = restore(&input(0, true));
    assert_eq!(restored.active().unwrap().target_epoch, epoch(9));
    assert_eq!(restored.last_reserved_epoch(), Some(epoch(9)));
    let mut inverted = input(0, true);
    inverted.previous_visible_epoch = epoch(9);
    assert!(inverted.restore(32).is_err());
}

#[test]
fn missing_or_foreign_intent_bindings_return_no_coordinator() {
    for case in 0..6 {
        let mut request = input(0, false);
        match case {
            0 => request.observation.intent_durable = false,
            1 => request.intent.transaction_id = id("foreign"),
            2 => request.intent.new_manifest_digest = digest(99),
            3 => request.intent.old_manifest_digest = Some(digest(99)),
            4 => request.intent.guards.access_generation += 1,
            _ => request.phase = PublicationPhase::Prepared,
        }
        assert!(request.restore(32).is_err());
    }
}

#[test]
fn only_the_recorded_phases_complete_receipt_prefix_is_accepted() {
    let phases = [
        PublicationPhase::IntentDurable,
        PublicationPhase::NewPointsAcknowledged,
        PublicationPhase::OldPointsClosedAcknowledged,
        PublicationPhase::ReadbackVerified,
        PublicationPhase::ControlCommitted,
        PublicationPhase::SnapshotPublished,
    ];
    for (phase, valid_mask) in phases.into_iter().zip([0, 1, 3, 7, 15, 15]) {
        for mask in 0..16 {
            if mask == valid_mask {
                continue;
            }
            let mut request = input(4, false);
            request.phase = phase;
            if mask & 1 == 0 {
                request.stage_receipt = None;
            }
            if mask & 2 == 0 {
                request.closure_receipt = None;
            }
            if mask & 4 == 0 {
                request.verified = None;
            }
            if mask & 8 == 0 {
                request.control_commit = None;
            }
            assert_eq!(
                request.restore(32).unwrap_err(),
                PublicationError::RecoveryBlocked
            );
        }
    }
}

#[test]
fn receipt_ids_epochs_and_combined_digests_are_revalidated_by_the_existing_transitions() {
    for case in 0..12 {
        let mut request = input(4, true);
        match case {
            0 => request.stage_receipt.as_mut().unwrap().transaction_id = id("foreign"),
            1 => request.stage_receipt.as_mut().unwrap().target_epoch = epoch(8),
            2 => request
                .stage_receipt
                .as_mut()
                .unwrap()
                .staged_ids
                .push(point(2)),
            3 => request.closure_receipt.as_mut().unwrap().closed_ids.clear(),
            4 => request
                .closure_receipt
                .as_mut()
                .unwrap()
                .unexpected_ids
                .push(point(9)),
            5 => request.verified.as_mut().unwrap().staged_readback_digest = digest(99),
            6 => request.verified.as_mut().unwrap().transaction_id = id("foreign"),
            7 => request.verified.as_mut().unwrap().new_manifest_digest = digest(99),
            8 => request.verified.as_mut().unwrap().retired_manifest_digest = None,
            9 => request.control_commit.as_mut().unwrap().control_generation = 0,
            10 => {
                request
                    .control_commit
                    .as_mut()
                    .unwrap()
                    .observed_guards
                    .purge_generation += 1;
            }
            _ => request.control_commit.as_mut().unwrap().after_visible_epoch = epoch(10),
        }
        assert!(request.restore(32).is_err());
    }
}

#[test]
fn fresh_control_visibility_must_match_the_validated_committed_or_precommit_state() {
    for (level, wrong_epoch) in [(0, 1), (3, 1), (4, 0), (4, 2)] {
        let mut request = input(level, false);
        request.observation.control_visible_epoch = epoch(wrong_epoch);
        let expected = if wrong_epoch == 2 {
            PublicationError::EpochMismatch
        } else {
            PublicationError::ControlConflict
        };
        assert_eq!(request.restore(32).unwrap_err(), expected);
    }
}

#[test]
fn restored_aborted_and_blocked_records_keep_the_active_slot_unavailable() {
    for phase in [
        PublicationPhase::Aborted,
        PublicationPhase::PublicationBlocked,
    ] {
        let mut request = input(3, true);
        request.phase = phase;
        let (mut restored, decision) = restore(&request);
        assert_eq!(decision, PublicationRecoveryDecision::PublicationBlocked);
        assert_eq!(
            restored.active().unwrap().phase,
            PublicationPhase::PublicationBlocked
        );
        assert!(restored.finalize_aborted().is_err());
        assert!(restored.complete().is_err());
        assert_eq!(
            restored.submit_fixture(proposal(Some(manifest(&[1])), manifest(&[3]), "competing")),
            Err(PublicationError::PublicationBusy)
        );
        assert_eq!(restored.last_reserved_epoch(), Some(epoch(9)));
    }
}

#[test]
fn recorded_compensating_never_returns_to_forward_progress_even_when_all_ids_are_present() {
    let mut request = input(3, true);
    request.phase = PublicationPhase::Compensating;
    let (mut restored, decision) = restore(&request);
    assert_eq!(decision, PublicationRecoveryDecision::CompensateExact);
    assert_eq!(
        restored.active().unwrap().phase,
        PublicationPhase::Compensating
    );
    assert_eq!(
        restored.begin_compensation_plan().unwrap().closed_ids,
        manifest(&[1])
            .entries
            .iter()
            .map(|entry| entry.point_id)
            .collect::<Vec<_>>()
    );
}

#[test]
fn conflicting_physical_ids_and_unpaired_manifests_cannot_enter_rehydration() {
    let mut conflict = input(0, true);
    conflict.case.new.plan.manifest = conflict.case.old.as_ref().unwrap().plan.manifest.clone();
    conflict.case.new.plan.manifest.entries[0].payload_identity = digest(99);
    assert_eq!(
        conflict.restore(32).unwrap_err(),
        PublicationError::InvalidPreparedPublication
    );
    let mut unpaired = input(0, true);
    unpaired
        .case
        .old
        .as_mut()
        .unwrap()
        .plan
        .manifest
        .entries
        .pop();
    assert_eq!(
        unpaired.restore(32).unwrap_err(),
        PublicationError::InvalidPreparedPublication
    );
}

#[test]
fn finite_point_limits_cover_receipts_and_observations_before_reconstruction() {
    assert!(input(0, false).restore(0).is_err());
    for case in 0..8 {
        let mut request = input(4, true);
        let oversized = vec![point(9); 33];
        match case {
            0 => request.observation.staged_ids = oversized,
            1 => request.observation.closed_ids = oversized,
            2 => request.stage_receipt.as_mut().unwrap().staged_ids = oversized,
            3 => request.stage_receipt.as_mut().unwrap().missing_ids = oversized,
            4 => request.stage_receipt.as_mut().unwrap().unexpected_ids = oversized,
            5 => request.closure_receipt.as_mut().unwrap().closed_ids = oversized,
            6 => request.closure_receipt.as_mut().unwrap().missing_ids = oversized,
            _ => request.closure_receipt.as_mut().unwrap().unexpected_ids = oversized,
        }
        assert_eq!(
            request.restore(32).unwrap_err(),
            PublicationError::BudgetExceeded
        );
    }
}

#[test]
fn unverifiable_fences_and_incomplete_committed_readback_return_a_blocked_not_ready_slot() {
    for case in 0..2 {
        let mut request = input(4, true);
        if case == 0 {
            request.observation.abandon_fence_durable = true;
        } else {
            request.observation.staged_ids.clear();
        }
        let (mut restored, decision) = restore(&request);
        assert_eq!(decision, PublicationRecoveryDecision::PublicationBlocked);
        assert!(restored.active().is_some());
        assert!(restored.complete().is_err());
        assert!(restored.begin_compensation_plan().is_err());
        assert_eq!(restored.visible_epoch(), Some(epoch(9)));
    }
}

#[test]
fn restore_input_debug_does_not_print_manifests_or_receipt_identifiers() {
    let request = input(4, true);
    let debug = format!("{request:?}");
    for sentinel in [
        "original-transaction",
        "original-preparation",
        "original-intent-operation",
        "original-stage",
        "original-closure",
        "fixture:manifest-not-CAS-evidence",
        "source-member",
    ] {
        assert!(!debug.contains(sentinel));
    }
}

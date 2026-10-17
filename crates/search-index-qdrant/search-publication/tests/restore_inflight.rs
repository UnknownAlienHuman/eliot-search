//! C08/C09/C13 inputs are real library products; phase receipts are synthetic
//! observations and prove no journal, Qdrant, native ownership, or crash behavior.

use search_contracts::{Blake3Digest32, CollectionGenerationId, Epoch, OpaqueId, OwnerEpoch};
use search_point_identity::s11::PointId128;
use search_publication::*;

#[path = "support/publication.rs"]
mod publication_support;
use publication_support::{
    Fixture, PhasePort, PublicationCase, PublicationRecoveryObservation, RestoreScenario,
    SubmitFixture, SyntheticClosureReceipt, SyntheticControlCommit, SyntheticIntent,
    SyntheticStageReceipt, SyntheticVerified, projection_fixture, projection_fixture_at,
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
/// Produce a canonical preparation and synthetic phase-observation prefix.
fn history(
    level: u8,
    replacement: bool,
) -> (
    PublicationCoordinator,
    Option<SyntheticControlCommit>,
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
    publication_support::persist_intent(&mut machine, &mut PhasePort::default()).unwrap();
    let transaction = machine.active().unwrap().clone();
    let difference = transaction.prepared().difference();
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
    let mut port = PhasePort::default();
    if level >= 1 {
        publication_support::stage_new_points(&mut machine, &mut port).unwrap();
    }
    if level >= 2 {
        publication_support::close_old_points(&mut machine, &mut port).unwrap();
    }
    if level >= 3 {
        publication_support::verify_readback(&mut machine, &mut port).unwrap();
    }
    let control = if level >= 4 {
        publication_support::commit_visible_epoch(&mut machine, &mut port).unwrap();
        Some(SyntheticControlCommit {
            before_visible_epoch: previous,
            after_visible_epoch: transaction.target_epoch(),
            observed_guards: guards(),
            control_generation: 9,
            control_state_digest: digest(32),
        })
    } else {
        None
    };
    if level >= 5 {
        publication_support::publish_control_snapshot(&mut machine, &mut port).unwrap();
    }
    let _ = (staged, closed);
    (machine, control, case)
}

fn input(level: u8, replacement: bool) -> RestoreScenario {
    let (machine, commit, case) = history(level, replacement);
    let transaction = machine.active().unwrap();
    let difference = transaction.prepared().difference();
    let durable = transaction.durable_intent().unwrap();
    let stage_ids = difference
        .create
        .iter()
        .map(|entry| entry.point_id)
        .collect::<Vec<_>>();
    let closed_ids = difference
        .retire
        .iter()
        .map(|entry| entry.point_id)
        .collect::<Vec<_>>();
    let intent = SyntheticIntent {
        transaction_id: durable.transaction_id().clone(),
        target_epoch: durable.target_epoch(),
        old_manifest_digest: durable.old_manifest_digest(),
        new_manifest_digest: durable.new_manifest_digest(),
        guards: durable.guards(),
        persist_operation_id: durable.persist_operation_id().clone(),
    };
    let context = publication_support::operation_context();
    let prepared = transaction.prepared();
    let intent_binding = PublicationEffectBinding {
        transaction_id: durable.transaction_id().clone(),
        request_id: context.request_id(),
        relative_deadline_ms: context.relative_deadline_ms().get(),
        budget_ref: context.budget_ref().clone(),
        operation_id: durable.persist_operation_id().clone(),
        prepared_input_digest: prepared.prepared_input_digest(),
        scope: prepared.scope().clone(),
        target_epoch: transaction.target_epoch(),
        previous_visible_epoch: transaction.previous_visible_epoch(),
        last_reserved_epoch: machine.last_reserved_epoch().unwrap(),
        old_manifest_digest: prepared.old_manifest_digest(),
        old_manifest: prepared.old_manifest().cloned(),
        new_manifest_digest: prepared.new_manifest_digest(),
        new_manifest: prepared.new_manifest().clone(),
        difference: prepared.difference().clone(),
        guards: prepared.guards(),
    };
    RestoreScenario {
        case,
        previous_visible_epoch: transaction.previous_visible_epoch(),
        last_reserved_epoch: machine.last_reserved_epoch().unwrap(),
        collection_generation_id: transaction.prepared().scope().collection_generation_id,
        current_guards: guards(),
        phase: transaction.phase(),
        intent,
        intent_binding,
        stage_receipt: (level >= 1).then(|| SyntheticStageReceipt {
            transaction_id: transaction.transaction_id().clone(),
            target_epoch: transaction.target_epoch(),
            staged_ids: stage_ids.clone(),
            missing_ids: vec![],
            unexpected_ids: vec![],
            readback_digest: digest(3),
        }),
        closure_receipt: (level >= 2).then(|| SyntheticClosureReceipt {
            transaction_id: transaction.transaction_id().clone(),
            target_epoch: transaction.target_epoch(),
            closed_ids: closed_ids.clone(),
            missing_ids: vec![],
            unexpected_ids: vec![],
            readback_digest: digest(4),
        }),
        verified: (level >= 3).then(|| SyntheticVerified {
            transaction_id: transaction.transaction_id().clone(),
            target_epoch: transaction.target_epoch(),
            staged_readback_digest: digest(3),
            closure_readback_digest: digest(4),
            new_manifest_digest: transaction.prepared().new_manifest_digest(),
            retired_manifest_digest: (!closed_ids.is_empty())
                .then_some(durable.old_manifest_digest())
                .flatten(),
        }),
        control_commit: commit,
        observation: PublicationRecoveryObservation {
            intent_durable: true,
            staged_ids: if level >= 1 { stage_ids } else { Vec::new() },
            closed_ids: if level >= 2 { closed_ids } else { Vec::new() },
            control_visible_epoch: machine.visible_epoch().unwrap(),
            snapshot_published: false,
            abandon_fence_durable: false,
            missing_point_ids: Vec::new(),
            unexpected_point_ids: Vec::new(),
        },
        readback_script: publication_support::PhaseReadback::default(),
    }
}
fn restore(input: &RestoreScenario) -> (PublicationCoordinator, PublicationRecoveryDecision) {
    input.restore(32).unwrap()
}

#[test]
fn precommit_prefixes_restore_original_identity_and_advance_only_from_exact_readback() {
    for level in 0..=3 {
        let (original, _, _) = history(level, false);
        let expected = original.active().unwrap();
        let (mut restored, decision) = restore(&input(level, false));
        let actual = restored.active().unwrap();
        assert_eq!(decision, PublicationRecoveryDecision::Continue);
        assert_eq!(actual.transaction_id(), expected.transaction_id());
        assert_eq!(
            actual.previous_visible_epoch(),
            expected.previous_visible_epoch()
        );
        assert_eq!(actual.target_epoch(), expected.target_epoch());
        assert_eq!(
            actual.prepared().prepared_input_digest(),
            expected.prepared().prepared_input_digest()
        );
        assert_eq!(actual.durable_intent(), expected.durable_intent());
        if level == 0 {
            assert_eq!(actual.phase(), PublicationPhase::IntentDurable);
            assert!(actual.stage_receipt().is_none());
            assert!(actual.verified().is_none());
        } else {
            assert_eq!(actual.phase(), PublicationPhase::ReadbackVerified);
            assert!(actual.stage_receipt().is_some());
            assert!(actual.closure_receipt().is_some());
            assert!(actual.verified().is_some());
        }
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
    let commit =
        publication_support::commit_visible_epoch(&mut restored, &mut PhasePort::default())
            .unwrap();
    assert_eq!(
        restored.complete(),
        Err(PublicationError::InvalidTransition)
    );
    publication_support::publish_control_snapshot(&mut restored, &mut PhasePort::default())
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
            restored.active().unwrap().phase(),
            PublicationPhase::ControlCommitted
        );
        assert!(restored.active().unwrap().snapshot_receipt().is_none());
        assert_eq!(
            restored.complete(),
            Err(PublicationError::InvalidTransition)
        );
        publication_support::publish_control_snapshot(&mut restored, &mut PhasePort::default())
            .unwrap();
        restored.complete().unwrap();
        assert_eq!(restored.visible_epoch(), Some(epoch(1)));
        assert!(restored.active().is_none());
    }
}

#[test]
fn lost_visibility_ack_is_reconciled_from_the_original_readback_verified_prefix() {
    let mut request = input(3, false);
    request.phase = PublicationPhase::ReadbackVerified;
    request.control_commit = None; // The durable prefix is truthful; the CAS reply was lost.
    request.observation.control_visible_epoch = epoch(1);
    let (restored, decision) = restore(&request);
    assert_eq!(decision, PublicationRecoveryDecision::PublishSnapshot);
    assert_eq!(restored.visible_epoch(), Some(epoch(1)));
    let transaction = restored.active().unwrap();
    assert_eq!(transaction.phase(), PublicationPhase::ControlCommitted);
    assert!(transaction.visible_commit().is_some());
}

#[test]
fn unknown_stage_is_read_back_under_the_same_id_without_a_second_mutation() {
    let mut machine = PublicationCoordinator::new(32).unwrap();
    machine
        .submit_fixture(proposal(None, manifest(&[2]), "unknown-stage"))
        .unwrap();
    let mut port = PhasePort::default();
    publication_support::persist_intent(&mut machine, &mut port).unwrap();
    port.readback.next_outcome = Some(PublicationEffectOutcome::Unknown);
    assert_eq!(
        publication_support::stage_new_points(&mut machine, &mut port),
        Err(PublicationError::OutcomeUnknown)
    );
    let original_id = port.effects.effect_ids.last().unwrap().clone();
    port.readback.outcomes.push((
        PublicationEffectKind::CloseOldPoints,
        PublicationEffectOutcome::NotApplied,
    ));
    assert_eq!(
        machine
            .recover(
                &mut port.readback,
                &publication_support::operation_context()
            )
            .unwrap(),
        PublicationRecoveryDecision::Continue
    );
    assert_eq!(
        port.effects
            .effect_ids
            .iter()
            .filter(|id| **id == original_id)
            .count(),
        1
    );
    assert_eq!(
        machine.active().unwrap().phase(),
        PublicationPhase::NewPointsAcknowledged
    );
    assert_eq!(
        port.readback
            .readback_kinds
            .iter()
            .filter(|effect| **effect == PublicationEffectKind::StageNewPoints)
            .count(),
        2
    );
}

#[test]
fn cancellation_after_effect_dispatch_requires_readback_before_retry() {
    let mut machine = PublicationCoordinator::new(32).unwrap();
    machine
        .submit_fixture(proposal(None, manifest(&[2]), "cancel-after-dispatch"))
        .unwrap();
    let cancellation = publication_support::SharedCancellation::new(false);
    let context = publication_support::operation_context_with(cancellation.clone());
    let mut port = PhasePort::default();
    machine
        .persist_intent(&mut port.effects, &mut port.readback, &context)
        .unwrap();

    port.effects.cancel_after_apply = Some((
        PublicationEffectKind::StageNewPoints,
        cancellation.shared_flag(),
    ));
    assert_eq!(
        machine.stage_new_points(&mut port.effects, &mut port.readback, &context),
        Err(PublicationError::OperationContextInvalid)
    );
    assert_eq!(
        port.effects
            .effect_kinds
            .iter()
            .filter(|effect| **effect == PublicationEffectKind::StageNewPoints)
            .count(),
        1
    );

    let recovery_context = publication_support::operation_context_with(
        publication_support::SharedCancellation::new(false),
    );
    machine
        .stage_new_points(&mut port.effects, &mut port.readback, &recovery_context)
        .unwrap();
    assert_eq!(
        port.effects
            .effect_kinds
            .iter()
            .filter(|effect| **effect == PublicationEffectKind::StageNewPoints)
            .count(),
        1
    );
    assert_eq!(
        port.readback
            .readback_kinds
            .iter()
            .filter(|effect| **effect == PublicationEffectKind::StageNewPoints)
            .count(),
        1
    );
    assert_eq!(
        machine.active().unwrap().phase(),
        PublicationPhase::NewPointsAcknowledged
    );
}

#[test]
fn readback_compares_full_vectors_not_only_c13_vector_digests() {
    let mut machine = PublicationCoordinator::new(32).unwrap();
    machine
        .submit_fixture(proposal(None, manifest(&[2]), "vector-mismatch"))
        .unwrap();
    let point_id = machine.active().unwrap().prepared().difference().create[0].point_id;
    let mut port = PhasePort::default();
    publication_support::persist_intent(&mut machine, &mut port).unwrap();
    port.readback.corrupt_vector = Some((PublicationEffectKind::StageNewPoints, point_id));
    assert_eq!(
        publication_support::stage_new_points(&mut machine, &mut port),
        Err(PublicationError::StageReadbackMismatch)
    );
    assert_eq!(
        machine.active().unwrap().phase(),
        PublicationPhase::PublicationBlocked
    );
}

#[test]
fn readback_compares_the_full_validity_interval() {
    let mut machine = PublicationCoordinator::new(32).unwrap();
    machine
        .submit_fixture(proposal(None, manifest(&[2]), "interval-mismatch"))
        .unwrap();
    let point_id = machine.active().unwrap().prepared().difference().create[0].point_id;
    let mut port = PhasePort::default();
    publication_support::persist_intent(&mut machine, &mut port).unwrap();
    port.readback.corrupt_point_interval = Some((PublicationEffectKind::StageNewPoints, point_id));
    assert_eq!(
        publication_support::stage_new_points(&mut machine, &mut port),
        Err(PublicationError::StageReadbackMismatch)
    );
    assert_eq!(
        machine.active().unwrap().phase(),
        PublicationPhase::PublicationBlocked
    );
}

#[test]
fn compensation_membership_fence_and_abort_commit_require_owner_readback() {
    let mut compensating = PublicationCoordinator::new(32).unwrap();
    compensating
        .submit_fixture(proposal(Some(manifest(&[1])), manifest(&[2]), "compensate"))
        .unwrap();
    publication_support::persist_intent(&mut compensating, &mut PhasePort::default()).unwrap();
    compensating.begin_compensation_plan().unwrap();
    let mut incomplete = PhasePort::default();
    incomplete.readback.missing_point_ids_override =
        Some((PublicationEffectKind::Compensate, Vec::new()));
    assert_eq!(
        publication_support::compensate_and_restore(&mut compensating, &mut incomplete),
        Err(PublicationError::CompensationIncomplete)
    );
    publication_support::compensate_and_restore(&mut compensating, &mut PhasePort::default())
        .unwrap();
    assert_eq!(
        compensating.active().unwrap().phase(),
        PublicationPhase::Aborted
    );
    abort_support::acknowledge(&mut compensating);
    compensating.finalize_aborted().unwrap();

    let mut fenced = PublicationCoordinator::new(32).unwrap();
    fenced
        .submit_fixture(proposal(Some(manifest(&[1])), manifest(&[2]), "abandon"))
        .unwrap();
    publication_support::persist_intent(&mut fenced, &mut PhasePort::default()).unwrap();
    let mut incomplete_fence = PhasePort::default();
    incomplete_fence.readback.abandon_memberships_override = Some(Vec::new());
    assert_eq!(
        publication_support::abandon(&mut fenced, &mut incomplete_fence),
        Err(PublicationError::AbandonFenceMissing)
    );
    publication_support::abandon(&mut fenced, &mut PhasePort::default()).unwrap();
    abort_support::acknowledge(&mut fenced);
    fenced.finalize_aborted().unwrap();
}

#[test]
fn cancellation_after_abort_dispatch_requires_readback_before_retry() {
    let mut machine = PublicationCoordinator::new(32).unwrap();
    machine
        .submit_fixture(proposal(
            Some(manifest(&[1])),
            manifest(&[2]),
            "abort-cancel",
        ))
        .unwrap();
    publication_support::persist_intent(&mut machine, &mut PhasePort::default()).unwrap();
    machine.begin_compensation_plan().unwrap();
    publication_support::compensate_and_restore(&mut machine, &mut PhasePort::default()).unwrap();

    let target = machine.active().unwrap().target_epoch().get();
    let request_id = id(&format!("fixture:abort-cancel-{target}"));
    let current_guards = machine.active().unwrap().durable_intent().unwrap().guards();
    machine
        .prepare_abort_finalization(
            request_id,
            100 + u64::try_from(target).unwrap(),
            current_guards,
        )
        .unwrap();

    let cancellation = publication_support::SharedCancellation::new(false);
    let context = publication_support::operation_context_with(cancellation.clone());
    let mut port = PhasePort::default();
    port.effects.cancel_after_apply = Some((
        PublicationEffectKind::CommitAbortFinalization,
        cancellation.shared_flag(),
    ));
    assert_eq!(
        machine.acknowledge_abort_commit(&mut port.effects, &mut port.readback, &context),
        Err(PublicationError::OperationContextInvalid)
    );

    let recovery_context = publication_support::operation_context_with(
        publication_support::SharedCancellation::new(false),
    );
    machine
        .acknowledge_abort_commit(&mut port.effects, &mut port.readback, &recovery_context)
        .unwrap();
    assert_eq!(
        port.effects
            .effect_kinds
            .iter()
            .filter(|effect| **effect == PublicationEffectKind::CommitAbortFinalization)
            .count(),
        1
    );
    assert_eq!(
        port.readback
            .readback_kinds
            .iter()
            .filter(|effect| **effect == PublicationEffectKind::CommitAbortFinalization)
            .count(),
        1
    );
}

#[test]
fn a_previous_process_snapshot_flag_requires_a_fresh_snapshot_effect() {
    let mut request = input(5, false);
    request.observation.snapshot_published = true;
    let (restored, decision) = request.restore(32).unwrap();
    assert_eq!(decision, PublicationRecoveryDecision::PublishSnapshot);
    assert_eq!(
        restored.active().unwrap().phase(),
        PublicationPhase::ControlCommitted
    );
    assert!(restored.active().unwrap().snapshot_receipt().is_none());
}

#[test]
fn partial_effects_restore_two_sided_compensation_and_preserve_consumed_gaps() {
    let mut request = input(3, true);
    request.observation.staged_ids.clear(); // Old points remain closed.
    let (mut restored, decision) = restore(&request);
    assert_eq!(decision, PublicationRecoveryDecision::CompensateExact);
    assert_eq!(
        restored.active().unwrap().phase(),
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
    assert_eq!(
        publication_support::compensate_exact(&mut restored, &mut PhasePort::default()),
        Err(PublicationError::CompensationIncomplete)
    );
    publication_support::compensate_and_restore(&mut restored, &mut PhasePort::default()).unwrap();
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
            restored.active().unwrap().phase(),
            PublicationPhase::Compensating
        );
        assert_eq!(
            restored
                .active()
                .unwrap()
                .durable_intent()
                .unwrap()
                .guards(),
            guards()
        );
        assert_eq!(
            publication_support::commit_visible_epoch(&mut restored, &mut PhasePort::default()),
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
    route.readback_script.intent_route_generation_override =
        Some(CollectionGenerationId::from_bytes([2; 16]));
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
    assert_eq!(restored.active().unwrap().target_epoch(), epoch(9));
    assert_eq!(restored.last_reserved_epoch(), Some(epoch(9)));
    let mut inverted = input(0, true);
    inverted.previous_visible_epoch = epoch(9);
    assert!(inverted.restore(32).is_err());
}

#[test]
fn missing_or_foreign_intent_bindings_return_no_coordinator() {
    for case in 0..11 {
        let mut request = input(0, false);
        match case {
            0 => request.observation.intent_durable = false,
            1 => request.intent.transaction_id = id("foreign"),
            2 => request.intent.new_manifest_digest = digest(99),
            3 => request.intent.old_manifest_digest = Some(digest(99)),
            4 => request.intent.guards.access_generation += 1,
            5 => request.phase = PublicationPhase::Prepared,
            6 => request.readback_script.intent_deadline_override = Some(59_999),
            7 => {
                request.readback_script.intent_budget_ref_override =
                    Some(search_contracts::OpaqueRef::new("budget:foreign").unwrap());
            }
            8 => {
                request.readback_script.intent_route_generation_override =
                    Some(CollectionGenerationId::from_bytes([2; 16]));
            }
            9 => request.readback_script.intent_schema_identity_override = Some(digest(99)),
            _ => {
                request.readback_script.intent_request_id_override =
                    Some(search_contracts::RequestId::from_bytes([88; 16]));
            }
        }
        assert!(request.restore(32).is_err());
    }
}

#[test]
fn complete_owner_readback_advances_even_when_the_recorded_prefix_is_older() {
    let phases = [
        PublicationPhase::IntentDurable,
        PublicationPhase::NewPointsAcknowledged,
        PublicationPhase::OldPointsClosedAcknowledged,
        PublicationPhase::ReadbackVerified,
        PublicationPhase::ControlCommitted,
        PublicationPhase::SnapshotPublished,
    ];
    for phase in phases {
        let mut request = input(4, false);
        request.phase = phase;
        request.stage_receipt = None;
        request.closure_receipt = None;
        request.verified = None;
        request.control_commit = None;
        let (restored, decision) = request.restore(32).unwrap();
        assert_eq!(decision, PublicationRecoveryDecision::PublishSnapshot);
        assert_eq!(restored.visible_epoch(), Some(epoch(1)));
        assert_eq!(
            restored.active().unwrap().phase(),
            PublicationPhase::ControlCommitted
        );
    }
}

#[test]
fn receipt_ids_epochs_and_combined_digests_are_revalidated_by_the_existing_transitions() {
    for case in 0..15 {
        let mut request = input(4, true);
        let stage_id = request.case.new.plan.manifest.entries[0].point_id;
        let closure_id = request.case.old.as_ref().unwrap().plan.manifest.entries[0].point_id;
        match case {
            0 => {
                request.readback_script.binding_transaction_override =
                    Some((PublicationEffectKind::StageNewPoints, id("foreign")));
            }
            1 => {
                request.readback_script.binding_target_epoch_override =
                    Some((PublicationEffectKind::StageNewPoints, epoch(8)));
            }
            2 => request
                .readback_script
                .unexpected_point_ids_by_effect
                .push((PublicationEffectKind::StageNewPoints, vec![point(2)])),
            3 => {
                request.readback_script.missing_point_ids_override =
                    Some((PublicationEffectKind::CloseOldPoints, vec![closure_id]));
            }
            4 => request
                .readback_script
                .unexpected_point_ids_by_effect
                .push((PublicationEffectKind::CloseOldPoints, vec![point(9)])),
            5 => {
                request.readback_script.corrupt_vector =
                    Some((PublicationEffectKind::VerifyReadback, stage_id));
            }
            6 => {
                request.readback_script.binding_transaction_override =
                    Some((PublicationEffectKind::VerifyReadback, id("foreign")));
            }
            7 => {
                request.readback_script.binding_new_manifest_override =
                    Some((PublicationEffectKind::VerifyReadback, digest(99)));
            }
            8 => {
                request.readback_script.binding_old_manifest_override =
                    Some((PublicationEffectKind::VerifyReadback, None));
            }
            9 => request.readback_script.control_generation_override = Some(0),
            10 => {
                let mut wrong = guards();
                wrong.purge_generation += 1;
                request.readback_script.control_guards_override = Some(wrong);
            }
            11 => request.readback_script.control_after_epoch_override = Some(epoch(10)),
            12 => {
                request.readback_script.binding_schema_identity_override =
                    Some((PublicationEffectKind::StageNewPoints, digest(99)));
            }
            13 => {
                request.readback_script.binding_request_id_override = Some((
                    PublicationEffectKind::StageNewPoints,
                    search_contracts::RequestId::from_bytes([88; 16]),
                ));
            }
            _ => {
                request.readback_script.binding_difference_override = Some((
                    PublicationEffectKind::StageNewPoints,
                    search_projection_planner::ManifestDiff {
                        create: Vec::new(),
                        retain: Vec::new(),
                        retire: Vec::new(),
                    },
                ));
            }
        }
        assert!(request.restore(32).is_err());
    }
}

#[test]
fn fresh_control_visibility_must_match_the_validated_committed_or_precommit_state() {
    for (level, wrong_epoch) in [(0, 1), (3, 1), (4, 0), (4, 2)] {
        let mut request = input(level, false);
        request.observation.control_visible_epoch = epoch(wrong_epoch);
        match (level, wrong_epoch) {
            (3, 1) => {
                // Original durable phase is ReadbackVerified, but the exact
                // current control readback proves the original CAS committed.
                let (restored, decision) = request.restore(32).unwrap();
                assert_eq!(decision, PublicationRecoveryDecision::PublishSnapshot);
                assert_eq!(restored.visible_epoch(), Some(epoch(1)));
            }
            (0, 1) | (4, 0) => assert_eq!(
                request.restore(32).unwrap_err(),
                PublicationError::RecoveryBlocked
            ),
            _ => assert_eq!(
                request.restore(32).unwrap_err(),
                PublicationError::EpochMismatch
            ),
        }
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
            restored.active().unwrap().phase(),
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
        restored.active().unwrap().phase(),
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
fn a_boolean_fence_does_not_replace_exact_effect_readback() {
    let mut request = input(4, true);
    request.observation.abandon_fence_durable = true;
    let (restored, decision) = restore(&request);
    assert_eq!(decision, PublicationRecoveryDecision::PublishSnapshot);
    assert_eq!(restored.visible_epoch(), Some(epoch(9)));

    let mut incomplete = input(4, true);
    incomplete.observation.staged_ids.clear();
    assert_eq!(
        incomplete.restore(32).unwrap_err(),
        PublicationError::RecoveryBlocked
    );
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

//! Pure owner-port proofs for restart between abort control commit and snapshot.

use search_contracts::{Epoch, OpaqueId, OpaqueRef, ProjectionMembershipId, RequestId};
use search_ports::{FakeCancellation, OperationContext};
use search_publication::*;

#[path = "support/publication.rs"]
mod publication_support;
use publication_support::{
    PhasePort, PublicationCase, RestoreScenario, SubmitFixture, SyntheticIntent, operation_context,
    projection_fixture, proposal,
};

fn id(value: &str) -> OpaqueId {
    OpaqueId::new(value).unwrap()
}

fn epoch(value: i64) -> Epoch {
    Epoch::new(value).unwrap()
}

fn replacement_case() -> PublicationCase {
    proposal(
        Some(projection_fixture(&[1])),
        projection_fixture(&[2]),
        "abort-crash-recovery",
    )
}

fn fresh_recovery_context() -> OperationContext<FakeCancellation> {
    OperationContext::new(
        RequestId::from_bytes([91; 16]),
        30_000,
        FakeCancellation::new(false),
        OpaqueRef::new("budget:publication-recovery").unwrap(),
    )
    .unwrap()
}

fn aborted_scenario(
    commit_outcome: PublicationEffectOutcome,
) -> (
    RestoreScenario,
    AbortFinalizationRequest,
    PublicationEffectBinding,
) {
    aborted_scenario_with_resolution(commit_outcome, false)
}

fn abandoned_scenario(
    commit_outcome: PublicationEffectOutcome,
) -> (
    RestoreScenario,
    AbortFinalizationRequest,
    PublicationEffectBinding,
) {
    aborted_scenario_with_resolution(commit_outcome, true)
}

fn aborted_scenario_with_resolution(
    commit_outcome: PublicationEffectOutcome,
    abandon: bool,
) -> (
    RestoreScenario,
    AbortFinalizationRequest,
    PublicationEffectBinding,
) {
    let case = replacement_case();
    let mut coordinator = PublicationCoordinator::new(32).unwrap();
    coordinator.submit_fixture(case.clone()).unwrap();
    let mut original_port = PhasePort::default();
    publication_support::persist_intent(&mut coordinator, &mut original_port).unwrap();
    if abandon {
        publication_support::abandon(&mut coordinator, &mut original_port).unwrap();
    } else {
        coordinator.begin_compensation_plan().unwrap();
        publication_support::compensate_and_restore(&mut coordinator, &mut original_port).unwrap();
    }

    let target = coordinator.active().unwrap().target_epoch().get();
    let request = coordinator
        .prepare_abort_finalization(
            id("abort-commit-original"),
            40 + u64::try_from(target).unwrap(),
            publication_support::guards(),
        )
        .unwrap();
    original_port.readback.next_outcome = Some(commit_outcome);
    let mut current_guards = publication_support::guards();
    if commit_outcome == PublicationEffectOutcome::NotApplied {
        current_guards.source_catalog_generation += 1;
        original_port.readback.control_before_generation_override = Some(55);
        original_port.readback.control_generation_override = Some(55);
        original_port.readback.control_guards_override = Some(current_guards);
    }
    let commit_result = coordinator.acknowledge_abort_commit(
        &mut original_port.effects,
        &mut original_port.readback,
        &operation_context(),
    );
    match commit_outcome {
        PublicationEffectOutcome::Applied => commit_result.unwrap(),
        PublicationEffectOutcome::NotApplied => {
            assert_eq!(commit_result, Err(PublicationError::EffectNotObserved));
        }
        PublicationEffectOutcome::Conflict => {
            assert_eq!(commit_result, Err(PublicationError::ControlConflict));
        }
        PublicationEffectOutcome::Unknown => {
            assert_eq!(commit_result, Err(PublicationError::OutcomeUnknown));
        }
    }

    let transaction = coordinator.active().unwrap();
    let durable = transaction.durable_intent().unwrap();
    let intent_binding = original_port
        .effects
        .effect_bindings
        .iter()
        .find_map(|(effect, binding)| {
            (*effect == PublicationEffectKind::PersistIntent).then(|| binding.clone())
        })
        .unwrap();
    let abort_binding = original_port
        .effects
        .effect_bindings
        .iter()
        .find_map(|(effect, binding)| {
            (*effect == PublicationEffectKind::CommitAbortFinalization).then(|| binding.clone())
        })
        .unwrap();
    let intent = SyntheticIntent {
        transaction_id: durable.transaction_id().clone(),
        target_epoch: durable.target_epoch(),
        old_manifest_digest: durable.old_manifest_digest(),
        new_manifest_digest: durable.new_manifest_digest(),
        guards: durable.guards(),
        persist_operation_id: durable.persist_operation_id().clone(),
    };
    let mut scenario = RestoreScenario {
        case,
        previous_visible_epoch: transaction.previous_visible_epoch(),
        last_reserved_epoch: coordinator.last_reserved_epoch().unwrap(),
        collection_generation_id: transaction.prepared().scope().collection_generation_id,
        current_guards,
        phase: PublicationPhase::Aborted,
        intent,
        intent_binding,
        stage_receipt: None,
        closure_receipt: None,
        verified: None,
        control_commit: None,
        observation: publication_support::PublicationRecoveryObservation {
            intent_durable: true,
            staged_ids: Vec::new(),
            closed_ids: Vec::new(),
            control_visible_epoch: coordinator.visible_epoch().unwrap(),
            snapshot_published: false,
            abandon_fence_durable: false,
            missing_point_ids: Vec::new(),
            unexpected_point_ids: Vec::new(),
        },
        readback_script: publication_support::PhaseReadback::default(),
    };
    scenario.readback_script.recorded_phase = Some(PublicationPhase::Aborted);
    scenario.readback_script.abort_request_override = Some(request.clone());
    scenario.readback_script.abort_binding_override = Some(abort_binding.clone());
    scenario.readback_script.outcomes.push((
        PublicationEffectKind::CommitAbortFinalization,
        commit_outcome,
    ));
    if commit_outcome == PublicationEffectOutcome::NotApplied {
        scenario.readback_script.control_before_generation_override = Some(55);
        scenario.readback_script.control_generation_override = Some(55);
        scenario.readback_script.control_guards_override = Some(current_guards);
    }
    (scenario, request, abort_binding)
}

#[test]
fn restart_rehydrates_applied_abort_commit_and_publishes_only_a_fresh_snapshot() {
    let (scenario, request, original_binding) = aborted_scenario(PublicationEffectOutcome::Applied);
    let context = fresh_recovery_context();
    let (mut restored, decision) = scenario.restore_with_context(32, &context).unwrap();

    assert_eq!(decision, PublicationRecoveryDecision::PublishAbortSnapshot);
    assert_eq!(
        restored.active().unwrap().phase(),
        PublicationPhase::Aborted
    );
    assert_eq!(restored.visible_epoch(), Some(epoch(1)));
    assert_eq!(
        restored.last_reserved_epoch(),
        Some(request.intent.target_epoch())
    );
    assert_eq!(original_binding.operation_id, request.operation_id);

    let mut duplicate_ack = PhasePort::default();
    restored
        .acknowledge_abort_commit(
            &mut duplicate_ack.effects,
            &mut duplicate_ack.readback,
            &context,
        )
        .unwrap();
    assert!(duplicate_ack.effects.effect_kinds.is_empty());

    let mut current_process = PhasePort::default();
    restored
        .publish_abort_snapshot(
            &mut current_process.effects,
            &mut current_process.readback,
            &context,
        )
        .unwrap();
    assert_eq!(
        current_process.effects.effect_kinds,
        vec![PublicationEffectKind::PublishAbortSnapshot]
    );
    restored.finalize_aborted().unwrap();
    assert!(restored.active().is_none());
}

#[test]
fn exact_not_applied_readback_readdresses_only_from_fresh_owner_generation_and_guards() {
    let (scenario, original_request, _) = aborted_scenario(PublicationEffectOutcome::NotApplied);
    let context = fresh_recovery_context();
    let (mut restored, decision) = scenario.restore_with_context(32, &context).unwrap();

    assert_eq!(
        decision,
        PublicationRecoveryDecision::PrepareAbortFinalization
    );
    let mut fresh_guards = publication_support::guards();
    fresh_guards.source_catalog_generation += 1;
    let retry = restored
        .prepare_abort_finalization_after_recovery(id("abort-commit-retry"))
        .unwrap();
    assert_ne!(retry.operation_id, original_request.operation_id);
    assert_eq!(retry.expected_control_generation, 55);
    assert_eq!(retry.current_guards, fresh_guards);
    assert_eq!(retry.resolution, original_request.resolution);

    let mut current_process = PhasePort::default();
    restored
        .acknowledge_abort_commit(
            &mut current_process.effects,
            &mut current_process.readback,
            &context,
        )
        .unwrap();
    assert_eq!(
        current_process.effects.effect_kinds,
        vec![PublicationEffectKind::CommitAbortFinalization]
    );
    assert_eq!(current_process.effects.effect_ids, vec![retry.operation_id]);
}

#[test]
fn conflicting_durable_abort_binding_fails_closed_before_snapshot_publication() {
    let (mut scenario, _, _) = aborted_scenario(PublicationEffectOutcome::Applied);
    let mut bad_binding = scenario
        .readback_script
        .abort_binding_override
        .clone()
        .unwrap();
    bad_binding.target_epoch = epoch(99);
    scenario.readback_script.abort_binding_override = Some(bad_binding);

    assert!(matches!(
        scenario.restore_with_context(32, &fresh_recovery_context()),
        Err(PublicationError::OperationMismatch)
    ));
}

#[test]
fn conflicting_abort_commit_readback_keeps_the_publication_fence() {
    let (scenario, _, _) = aborted_scenario(PublicationEffectOutcome::Conflict);
    assert!(matches!(
        scenario.restore_with_context(32, &fresh_recovery_context()),
        Err(PublicationError::ControlConflict)
    ));
}

#[test]
fn unknown_abort_commit_readback_keeps_the_original_operation_unresolved() {
    let (scenario, request, binding) = aborted_scenario(PublicationEffectOutcome::Unknown);
    assert_eq!(binding.operation_id, request.operation_id);
    assert!(matches!(
        scenario.restore_with_context(32, &fresh_recovery_context()),
        Err(PublicationError::OutcomeUnknown)
    ));
}

#[test]
fn changed_compensation_binding_cannot_rehydrate_abort_authority() {
    let (mut scenario, _, _) = aborted_scenario(PublicationEffectOutcome::Applied);
    scenario.readback_script.binding_transaction_override = Some((
        PublicationEffectKind::Compensate,
        id("different-publication"),
    ));
    assert!(matches!(
        scenario.restore_with_context(32, &fresh_recovery_context()),
        Err(PublicationError::RecoveryBlocked)
    ));
}

#[test]
fn restart_revalidates_the_full_membership_exclusion_before_abort_commit() {
    let (scenario, _, _) = abandoned_scenario(PublicationEffectOutcome::Applied);
    let (restored, decision) = scenario
        .restore_with_context(32, &fresh_recovery_context())
        .unwrap();
    assert_eq!(decision, PublicationRecoveryDecision::PublishAbortSnapshot);
    assert_eq!(
        restored.active().unwrap().phase(),
        PublicationPhase::Aborted
    );
}

#[test]
fn changed_excluded_membership_readback_blocks_abort_commit_rehydration() {
    let (mut scenario, mut request, _) = abandoned_scenario(PublicationEffectOutcome::Applied);
    let operation_id = match &request.resolution {
        AbortedPublicationResolution::Excluded { operation_id, .. } => operation_id.clone(),
        AbortedPublicationResolution::Compensated { .. } => unreachable!(),
    };
    request.resolution = AbortedPublicationResolution::Excluded {
        operation_id,
        excluded_projection_memberships: vec![ProjectionMembershipId::from_bytes([0xfe; 16])],
    };
    scenario.readback_script.abort_request_override = Some(request);
    assert!(matches!(
        scenario.restore_with_context(32, &fresh_recovery_context()),
        Err(PublicationError::AbandonFenceMissing)
    ));
}

#[test]
fn cancellation_of_current_recovery_context_prevents_abort_readbacks() {
    let (scenario, _, _) = aborted_scenario(PublicationEffectOutcome::Applied);
    let context = publication_support::operation_context_with(
        publication_support::SharedCancellation::new(true),
    );
    assert!(matches!(
        scenario.restore_with_context(32, &context),
        Err(PublicationError::OperationContextInvalid)
    ));
}

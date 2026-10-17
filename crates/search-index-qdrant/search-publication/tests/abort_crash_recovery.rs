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

fn distinct_context(
    request: u8,
    relative_deadline_ms: u64,
    budget: &str,
) -> OperationContext<FakeCancellation> {
    OperationContext::new(
        RequestId::from_bytes([request; 16]),
        relative_deadline_ms,
        FakeCancellation::new(false),
        OpaqueRef::new(budget).unwrap(),
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

    let (scenario, abort_binding) = restore_scenario(
        case,
        &coordinator,
        &original_port,
        &request,
        current_guards,
        commit_outcome,
    );
    (scenario, request, abort_binding)
}

fn restore_scenario(
    case: PublicationCase,
    coordinator: &PublicationCoordinator,
    original_port: &PhasePort,
    request: &AbortFinalizationRequest,
    current_guards: PublicationGuards,
    commit_outcome: PublicationEffectOutcome,
) -> (RestoreScenario, PublicationEffectBinding) {
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
    scenario.readback_script.abort_request_override = Some(AbortFinalizationReadback::from(request));
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
    (scenario, abort_binding)
}

#[test]
fn external_adapter_restart_uses_only_plain_abort_data_and_a_fresh_snapshot() {
    let (mut scenario, private_request, abort_binding) =
        aborted_scenario(PublicationEffectOutcome::Applied);
    // Simulate losing every process-local request/proof. Only ordinary durable
    // observations may cross into this external adapter's new incarnation.
    drop(private_request);
    let resolution = scenario
        .readback_script
        .abort_request_override
        .take()
        .unwrap()
        .resolution;
    let persisted = AbortFinalizationReadback {
        operation_id: id("abort-commit-original"),
        expected_control_generation: 42,
        collection_generation_id: scenario.collection_generation_id,
        previous_visible_epoch: scenario.previous_visible_epoch,
        current_guards: scenario.current_guards,
        prepared_input_digest: abort_binding.prepared_input_digest,
        resolution,
    };
    scenario.readback_script.abort_request_override = Some(persisted.clone());
    let context = fresh_recovery_context();
    let (mut restored, decision) = scenario.restore_with_context(32, &context).unwrap();
    assert_eq!(decision, PublicationRecoveryDecision::PublishAbortSnapshot);
    assert_eq!(restored.visible_epoch(), Some(epoch(1)));
    assert_eq!(restored.last_reserved_epoch(), Some(epoch(2)));
    assert_eq!(
        restored.finalize_aborted(),
        Err(PublicationError::SnapshotPublicationFailed)
    );

    let mut process = PhasePort::default();
    process.readback.abort_request_override = Some(persisted);
    restored
        .publish_abort_snapshot(&mut process.effects, &mut process.readback, &context)
        .unwrap();
    assert_eq!(
        process.effects.effect_kinds,
        vec![PublicationEffectKind::PublishAbortSnapshot]
    );
    restored.finalize_aborted().unwrap();
    assert!(restored.active().is_none());
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
#[expect(
    clippy::too_many_lines,
    reason = "Keep the A/B/C abort-attempt restart trace in one focused owner case"
)]
fn replacement_abort_attempt_binding_survives_repeat_ack_and_crash_restore() {
    let intent_context = publication_support::operation_context();
    let attempt_context = distinct_context(92, 35_000, "budget:abort-attempt-b");
    let restart_context = distinct_context(93, 75_000, "budget:abort-recovery-c");
    assert_ne!(intent_context.request_id(), attempt_context.request_id());
    assert_ne!(
        intent_context.relative_deadline_ms(),
        attempt_context.relative_deadline_ms()
    );
    assert_ne!(intent_context.budget_ref(), attempt_context.budget_ref());
    assert_ne!(attempt_context.request_id(), restart_context.request_id());
    assert_ne!(
        attempt_context.relative_deadline_ms(),
        restart_context.relative_deadline_ms()
    );
    assert_ne!(attempt_context.budget_ref(), restart_context.budget_ref());

    let (scenario_a, original_request, original_binding) =
        aborted_scenario(PublicationEffectOutcome::NotApplied);
    assert_eq!(
        original_request.intent.request_id(),
        intent_context.request_id()
    );
    assert_eq!(original_binding.request_id, intent_context.request_id());
    assert_eq!(
        original_binding.relative_deadline_ms,
        intent_context.relative_deadline_ms().get()
    );
    assert_eq!(original_binding.budget_ref, *intent_context.budget_ref());

    let (mut after_not_applied, decision) = scenario_a
        .restore_with_context(32, &attempt_context)
        .unwrap();
    assert_eq!(
        decision,
        PublicationRecoveryDecision::PrepareAbortFinalization
    );
    let replacement = after_not_applied
        .prepare_abort_finalization_after_recovery(id("abort-commit-attempt-b"))
        .unwrap();
    assert_ne!(replacement.operation_id, original_request.operation_id);
    assert_eq!(replacement.intent.request_id(), intent_context.request_id());
    assert_eq!(replacement.expected_control_generation, 55);

    let mut attempt_b = PhasePort::default();
    after_not_applied
        .acknowledge_abort_commit(
            &mut attempt_b.effects,
            &mut attempt_b.readback,
            &attempt_context,
        )
        .unwrap();
    let attempt_binding = attempt_b
        .effects
        .effect_bindings
        .iter()
        .find_map(|(effect, binding)| {
            (*effect == PublicationEffectKind::CommitAbortFinalization).then(|| binding.clone())
        })
        .unwrap();
    assert_eq!(attempt_binding.operation_id, replacement.operation_id);
    assert_eq!(attempt_binding.request_id, attempt_context.request_id());
    assert_eq!(
        attempt_binding.relative_deadline_ms,
        attempt_context.relative_deadline_ms().get()
    );
    assert_eq!(attempt_binding.budget_ref, *attempt_context.budget_ref());
    assert_ne!(attempt_binding.request_id, original_binding.request_id);
    assert_ne!(
        attempt_binding.relative_deadline_ms,
        original_binding.relative_deadline_ms
    );
    assert_ne!(attempt_binding.budget_ref, original_binding.budget_ref);
    assert_eq!(
        attempt_b.effects.effect_kinds,
        vec![PublicationEffectKind::CommitAbortFinalization]
    );

    after_not_applied
        .acknowledge_abort_commit(
            &mut attempt_b.effects,
            &mut attempt_b.readback,
            &attempt_context,
        )
        .unwrap();
    assert_eq!(
        attempt_b.effects.effect_kinds,
        vec![PublicationEffectKind::CommitAbortFinalization]
    );

    let mut scenario_b = scenario_a;
    scenario_b.current_guards = replacement.current_guards;
    scenario_b.readback_script.abort_request_override = Some(AbortFinalizationReadback::from(&replacement));
    scenario_b.readback_script.abort_binding_override = Some(attempt_binding);
    scenario_b
        .readback_script
        .control_before_generation_override = Some(replacement.expected_control_generation);
    scenario_b.readback_script.control_generation_override = Some(
        replacement
            .expected_control_generation
            .checked_add(1)
            .unwrap(),
    );
    scenario_b.readback_script.control_guards_override = Some(replacement.current_guards);
    scenario_b
        .readback_script
        .outcomes
        .retain(|(effect, _)| *effect != PublicationEffectKind::CommitAbortFinalization);
    scenario_b.readback_script.outcomes.push((
        PublicationEffectKind::CommitAbortFinalization,
        PublicationEffectOutcome::Applied,
    ));

    let (mut after_crash, decision) = scenario_b
        .restore_with_context(32, &restart_context)
        .unwrap();
    assert_eq!(decision, PublicationRecoveryDecision::PublishAbortSnapshot);
    assert_eq!(
        after_crash.active().unwrap().phase(),
        PublicationPhase::Aborted
    );

    let mut restart_readback = PhasePort::default();
    after_crash
        .acknowledge_abort_commit(
            &mut restart_readback.effects,
            &mut restart_readback.readback,
            &restart_context,
        )
        .unwrap();
    assert!(restart_readback.effects.effect_kinds.is_empty());
    assert_eq!(
        restart_readback.readback.readback_ids,
        vec![replacement.operation_id]
    );

    let mut process_c = PhasePort::default();
    after_crash
        .publish_abort_snapshot(
            &mut process_c.effects,
            &mut process_c.readback,
            &restart_context,
        )
        .unwrap();
    assert_eq!(
        process_c.effects.effect_kinds,
        vec![PublicationEffectKind::PublishAbortSnapshot]
    );
    after_crash.finalize_aborted().unwrap();
}

#[test]
fn abort_finalization_pair_on_another_durable_phase_is_rejected() {
    for phase in [
        PublicationPhase::Prepared,
        PublicationPhase::IntentDurable,
        PublicationPhase::NewPointsAcknowledged,
        PublicationPhase::OldPointsClosedAcknowledged,
        PublicationPhase::ReadbackVerified,
        PublicationPhase::ControlCommitted,
        PublicationPhase::SnapshotPublished,
        PublicationPhase::Compensating,
        PublicationPhase::PublicationBlocked,
    ] {
        let (mut scenario, _, _) = aborted_scenario(PublicationEffectOutcome::Applied);
        scenario.phase = phase;
        scenario.readback_script.recorded_phase = Some(phase);
        assert!(
            matches!(
                scenario.restore_with_context(32, &fresh_recovery_context()),
                Err(PublicationError::OperationMismatch)
            ),
            "abort finalization must not be ignored at {phase:?}"
        );
    }
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
    scenario.readback_script.abort_request_override = Some(AbortFinalizationReadback::from(&request));
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

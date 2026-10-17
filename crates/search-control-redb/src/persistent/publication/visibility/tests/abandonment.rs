use super::*;
use crate::{
    AbortResolutionMetadataV1, ControlMutation, ControlWrite, MigrationBootstrapWork,
    PublicationAbandonFenceMembershipV1, PublicationAbandonFenceRequestV1,
    PublicationAbortFinalizationMetadataV1, PublicationAbortFinalizationRecordV1,
    PublicationIntentBindingMetadataV1, PublicationIntentBindingRecordV1,
};
use search_contracts::{
    OpaqueId, OpaqueRef, OwnerEpoch, PublicationReceiptId, RequestId, SourceRevisionId,
};
use std::time::{Duration, Instant};

fn durable_intent(journal: &mut PersistentControlJournal) -> PublicationIntent {
    durable_intent_with_guards(journal, state().guards)
}

fn durable_intent_with_guards(
    journal: &mut PersistentControlJournal,
    guards: PublicationGuards,
) -> PublicationIntent {
    let prepared = PublicationIntent {
        publication_intent_id: PublicationIntentId::from_bytes([33; 16]),
        target_epoch: Epoch::new(1).unwrap(),
        prepared_manifest_ref: reference("abandon-prepared-manifest"),
        owner_source_membership_access_guards: guards,
        state: PublicationIntentState::Prepared,
    };
    let original_operation_id = OpaqueId::new("operation:original-publication").unwrap();
    let binding = PublicationIntentBindingRecordV1::prepare_with_original_context(
        PublicationIntentBindingMetadataV1 {
            publication_intent_id: prepared.publication_intent_id,
            publication_receipt_id: PublicationReceiptId::from_bytes([34; 16]),
            target_epoch: prepared.target_epoch,
            prepared_manifest_reference: prepared.prepared_manifest_ref.clone(),
            guards: prepared.owner_source_membership_access_guards,
            c02_mutation_id: MutationId([3; 32]),
            c02_command_digest: digest(),
            expected_control_generation: 2,
            original_operation_id,
            original_request_id: RequestId::from_bytes([35; 16]),
            original_relative_deadline_ms: 60_000,
            original_budget_ref: OpaqueRef::new("budget:original-publication").unwrap(),
        },
        LIMITS,
        Instant::now() + Duration::from_secs(20),
        &|_: MigrationBootstrapWork| Ok::<(), ()>(()),
    )
    .unwrap();
    let update = PublicationIntentUpdate::begin(MutationId([3; 32]), digest(), 2, &prepared)
        .unwrap()
        .with_original_binding_v1(binding)
        .unwrap();
    journal
        .persist_publication_intent(&update, &context(false))
        .unwrap();
    update.intent().clone()
}

fn insert_previous_shadows(journal: &mut PersistentControlJournal) {
    let writes = [(member(1), shadow(9)), (member(2), shadow(10))]
        .into_iter()
        .map(|(membership, value)| ControlWrite {
            key: id_key(SHADOWS, membership.as_bytes(), LIMITS).unwrap(),
            value: codec::shadow(&value, LIMITS).unwrap(),
        })
        .collect();
    journal
        .transact(&ControlMutation::new(
            MutationId([2; 32]),
            digest(),
            1,
            writes,
            Vec::new(),
        ))
        .unwrap();
}

fn request(
    intent: PublicationIntent,
    mutation: u8,
    second_source_revision: u8,
) -> PublicationAbandonFenceRequestV1 {
    PublicationAbandonFenceRequestV1::new(
        OpaqueId::new("operation:abandon-one").unwrap(),
        MutationId([mutation; 32]),
        digest(),
        3,
        RequestId::from_bytes([44; 16]),
        60_000,
        OpaqueRef::new("budget:abandon-one").unwrap(),
        intent,
        state(),
        vec![
            PublicationAbandonFenceMembershipV1 {
                membership_id: member(1),
                source_revision_id: SourceRevisionId::from_bytes([41; 16]),
                previous_shadow: Some(shadow(9)),
            },
            PublicationAbandonFenceMembershipV1 {
                membership_id: member(2),
                source_revision_id: SourceRevisionId::from_bytes([second_source_revision; 16]),
                previous_shadow: Some(shadow(10)),
            },
        ],
    )
    .unwrap()
}

#[test]
#[allow(clippy::too_many_lines)] // One focused redb case covers unknown, reopen, replay, conflict and finalization.
fn abandonment_fence_unknown_reopens_replays_conflicts_and_is_retained_by_abort() {
    let scratch = Scratch::new();
    let mut journal = scratch.empty(PUBLICATION_VISIBILITY_SCHEMA_VERSION);
    journal
        .initialize_publication_visibility(state(), MutationId([1; 32]), digest(), &context(false))
        .unwrap();
    insert_previous_shadows(&mut journal);
    let intent = durable_intent(&mut journal);
    let fence_request = request(intent.clone(), 21, 42);

    let unknown = journal.abandon_publication_checked(
        &fence_request,
        super::super::super::super::Boundary::LostAcknowledgement,
        &super::super::super::super::Unscoped,
    );
    assert_eq!(unknown, Err(ControlError::CommitOutcomeUnknown));
    drop(journal);

    let mut journal = scratch.reopen();
    let recovered_receipt = match journal
        .recover_abandon_publication_checked(&fence_request, &super::super::super::super::Unscoped)
        .unwrap()
    {
        CommitRecoveryDecision::Committed(receipt) => receipt,
        decision => panic!("expected the original fence commit, got {decision:?}"),
    };
    assert_eq!(recovered_receipt.operation_id, fence_request.mutation_id());
    assert_eq!(recovered_receipt.after_generation, 4);
    let checkpoint = journal
        .read_checkpoint_checked(&super::super::super::super::Unscoped)
        .unwrap()
        .unwrap();
    let fence = checkpoint.abandonment_fence().unwrap().clone();
    assert_eq!(fence.membership_count(), 2);
    assert_eq!(
        fence
            .memberships()
            .map(|(membership, _, _, _)| membership)
            .collect::<Vec<_>>(),
        vec![member(1), member(2)]
    );
    let observed = journal
        .read_abandonment_fence_checked(&fence, &super::super::super::super::Unscoped)
        .unwrap();
    assert_eq!(observed.visibility().visible_epoch, Epoch::new(0).unwrap());
    assert_eq!(observed.visibility().guards.shadow_generation, 12);
    assert_eq!(
        observed.shadows().get(&member(1)),
        Some(&PublicationSourceShadow {
            source_revision_id: SourceRevisionId::from_bytes([41; 16]),
            fence_revision: NonZeroRevision::new(12).unwrap(),
        })
    );
    assert_eq!(
        observed.shadows().get(&member(2)),
        Some(&PublicationSourceShadow {
            source_revision_id: SourceRevisionId::from_bytes([42; 16]),
            fence_revision: NonZeroRevision::new(12).unwrap(),
        })
    );

    let replay = journal
        .abandon_publication_checked(
            &fence_request,
            super::super::super::super::Boundary::Normal,
            &super::super::super::super::Unscoped,
        )
        .unwrap();
    assert!(replay.receipt().replayed);

    let changed = request(intent.clone(), 21, 43);
    assert_eq!(
        journal.abandon_publication_checked(
            &changed,
            super::super::super::super::Boundary::Normal,
            &super::super::super::super::Unscoped,
        ),
        Err(ControlError::OperationConflict)
    );

    let binding = checkpoint.original_binding().unwrap();
    let metadata = PublicationAbortFinalizationMetadataV1 {
        abort_operation_id: OpaqueId::new("operation:abort-one").unwrap(),
        c02_mutation_id: MutationId([55; 32]),
        c02_command_digest: digest(),
        expected_control_generation: 4,
        abort_request_digest: digest(),
        publication_intent_id: intent.publication_intent_id,
        transaction_id: OpaqueId::new("transaction:abandon-one").unwrap(),
        collection_generation_id: state().collection_generation_id,
        projection_membership_id: member(1),
        schema_identity_digest: state().schema_identity_digest,
        target_epoch: intent.target_epoch,
        previous_visible_epoch: state().visible_epoch,
        old_manifest_digest: None,
        new_manifest_digest: digest(),
        old_manifest_reference: None,
        new_manifest_reference: intent.prepared_manifest_ref.clone(),
        scope_digest: digest(),
        prepared_input_digest: digest(),
        original_persist_operation_id: binding.metadata().original_operation_id.clone(),
        original_request_id: binding.metadata().original_request_id,
        original_relative_deadline_ms: binding.metadata().original_relative_deadline_ms,
        original_budget_ref: binding.metadata().original_budget_ref.clone(),
        abort_request_id: RequestId::from_bytes([44; 16]),
        abort_relative_deadline_ms: 60_000,
        abort_budget_ref: OpaqueRef::new("budget:abandon-one").unwrap(),
        original_control_generation: 3,
        original_control_state_digest: digest(),
        original_guards: intent.owner_source_membership_access_guards,
        current_guards: fence.fenced_visibility().guards,
        resolution_operation_id: OpaqueId::new("operation:abandon-one").unwrap(),
        resolution: AbortResolutionMetadataV1::Excluded {
            membership_count: fence.membership_count(),
            membership_digest: fence.membership_digest(),
        },
    };
    assert!(
        PublicationAbortFinalizationRecordV1::prepare_with_original_context(
            metadata.clone(),
            LIMITS,
            Instant::now() + Duration::from_secs(20),
            &|_: MigrationBootstrapWork| Ok::<(), ()>(()),
        )
        .is_err()
    );
    let abort_record = PublicationAbortFinalizationRecordV1::prepare_with_abandonment_fence_v1(
        metadata,
        fence.clone(),
        LIMITS,
        Instant::now() + Duration::from_secs(20),
        &|_: MigrationBootstrapWork| Ok::<(), ()>(()),
    )
    .unwrap();
    let abort = PublicationIntentUpdate::advance(
        MutationId([55; 32]),
        digest(),
        4,
        intent,
        PublicationIntentState::Aborted,
    )
    .unwrap()
    .with_abort_finalization_v1(abort_record)
    .unwrap();
    journal
        .persist_publication_intent(&abort, &context(false))
        .unwrap();
    drop(journal);

    let journal = scratch.reopen();
    let recovered = journal
        .read_checkpoint_checked(&super::super::super::super::Unscoped)
        .unwrap()
        .unwrap();
    assert_eq!(
        recovered.intent().unwrap().state,
        PublicationIntentState::Aborted
    );
    assert_eq!(
        recovered.abandonment_fence().unwrap().membership_digest(),
        fence.membership_digest()
    );
    assert_eq!(
        journal.control_snapshot(),
        Err(ControlError::SnapshotRebuildFailed)
    );
}

fn ready_with_original_binding(
    scratch: &Scratch,
) -> (PersistentControlJournal, VisibleEpochCommit) {
    ready_with_original_binding_for_state(scratch, PUBLICATION_VISIBILITY_SCHEMA_VERSION, state())
}

pub(super) fn ready_with_original_binding_for_owner(
    scratch: &Scratch,
    schema_version: u32,
    owner_epoch: OwnerEpoch,
) -> (PersistentControlJournal, VisibleEpochCommit) {
    let mut initial = state();
    initial.guards.owner_epoch = owner_epoch;
    ready_with_original_binding_for_state(scratch, schema_version, initial)
}

fn ready_with_original_binding_for_state(
    scratch: &Scratch,
    schema_version: u32,
    initial: PublicationVisibilityState,
) -> (PersistentControlJournal, VisibleEpochCommit) {
    let mut journal = scratch.empty_with_identity(identity_at_owner(
        schema_version,
        initial.guards.owner_epoch,
    ));
    if schema_version == crate::REGISTRY_JOURNAL_SCHEMA_VERSION {
        journal
            .initialize_operating_profile(
                crate::JournalProfileInitialization::Indexed(initial),
                MutationId([1; 32]),
                digest(),
                &context(false),
            )
            .unwrap();
    } else {
        journal
            .initialize_publication_visibility(
                initial,
                MutationId([1; 32]),
                digest(),
                &context(false),
            )
            .unwrap();
    }
    insert_previous_shadows(&mut journal);
    let mut intent = durable_intent_with_guards(&mut journal, initial.guards);
    let mut prior = None;
    for (id, phase) in [
        (34, PublicationIntentState::NewPointsAcknowledged),
        (35, PublicationIntentState::OldPointsClosedAcknowledged),
        (36, PublicationIntentState::ReadbackVerified),
    ] {
        let update = PublicationIntentUpdate::advance(
            MutationId([id; 32]),
            digest(),
            journal.verify().unwrap().generation,
            intent.clone(),
            phase,
        )
        .unwrap();
        prior = Some(
            journal
            .persist_publication_intent(&update, &context(false))
            .unwrap(),
        );
        intent = update.intent().clone();
    }
    let publication = VisibleEpochCommit::new(
        MutationId([7; 32]),
        digest(),
        prior.unwrap(),
        intent,
        initial,
        PublicationReceiptId::from_bytes([7; 16]),
        PublicationReadbackEvidence {
            exact_new_manifest_ref: reference("new-points"),
            exact_retired_manifest_ref: reference("retired-points"),
            readback_digest: digest(),
        },
        vec![PublicationManifestChange {
            membership_id: member(1),
            previous_manifest: None,
            next_manifest: Some(reference("new")),
            matching_shadow: Some(shadow(9)),
        }],
    )
    .unwrap();
    (journal, publication)
}

pub(super) fn durable_next_intent(
    journal: &mut PersistentControlJournal,
    publisher: &ControlSnapshotPublisher,
    previous_visibility: PublicationVisibilityState,
) -> PublicationIntent {
    let checkpoint = journal
        .read_checkpoint_checked(&super::super::super::super::Unscoped)
        .unwrap()
        .unwrap();
    let previous = checkpoint.intent().unwrap().clone();
    let previous_binding = checkpoint.original_binding().unwrap().clone();
    let expected_generation = checkpoint.generation();
    let prepared = PublicationIntent {
        publication_intent_id: PublicationIntentId::from_bytes([73; 16]),
        target_epoch: Epoch::new(2).unwrap(),
        prepared_manifest_ref: reference("abandon-next-epoch"),
        owner_source_membership_access_guards: previous_visibility.guards,
        state: PublicationIntentState::Prepared,
    };
    let binding = PublicationIntentBindingRecordV1::prepare_with_original_context(
        PublicationIntentBindingMetadataV1 {
            publication_intent_id: prepared.publication_intent_id,
            publication_receipt_id: PublicationReceiptId::from_bytes([74; 16]),
            target_epoch: prepared.target_epoch,
            prepared_manifest_reference: prepared.prepared_manifest_ref.clone(),
            guards: prepared.owner_source_membership_access_guards,
            c02_mutation_id: MutationId([75; 32]),
            c02_command_digest: digest(),
            expected_control_generation: expected_generation,
            original_operation_id: OpaqueId::new("operation:next-publication").unwrap(),
            original_request_id: RequestId::from_bytes([76; 16]),
            original_relative_deadline_ms: 60_000,
            original_budget_ref: OpaqueRef::new("budget:next-publication").unwrap(),
        },
        LIMITS,
        Instant::now() + Duration::from_secs(20),
        &|_: MigrationBootstrapWork| Ok::<(), ()>(()),
    )
    .unwrap();
    let successor = PublicationSuccessor::new(
        MutationId([75; 32]),
        digest(),
        expected_generation,
        previous,
        previous_visibility,
        &prepared,
    )
    .unwrap()
    .with_original_binding_v1(binding, previous_binding)
    .unwrap();
    journal
        .reserve_next_publication(&successor, publisher, &context(false))
        .unwrap();
    successor.intent().clone()
}

#[test]
fn committed_visibility_and_same_handle_abandonment_recovery_preserve_the_receipt() {
    let scratch = Scratch::new();
    let (mut journal, publication) = ready_with_original_binding(&scratch);
    let commit = apply(&mut journal, &publication).unwrap();
    let mut publisher = ControlSnapshotPublisher::new();
    journal
        .publish_committed_snapshot(&commit, &mut publisher)
        .unwrap();
    let previous_visibility = journal
        .read_publication_visibility(&context(false))
        .unwrap()
        .unwrap();
    assert_eq!(previous_visibility.visible_epoch, Epoch::new(1).unwrap());
    let receipt_id = previous_visibility.last_receipt.unwrap();

    let intent = durable_next_intent(&mut journal, &publisher, previous_visibility);
    let expected_generation = journal.verify().unwrap().generation;
    let request = PublicationAbandonFenceRequestV1::new(
        OpaqueId::new("operation:abandon-after-visible").unwrap(),
        MutationId([77; 32]),
        digest(),
        expected_generation,
        RequestId::from_bytes([78; 16]),
        60_000,
        OpaqueRef::new("budget:abandon-after-visible").unwrap(),
        intent,
        previous_visibility,
        vec![
            PublicationAbandonFenceMembershipV1 {
                membership_id: member(1),
                source_revision_id: SourceRevisionId::from_bytes([41; 16]),
                previous_shadow: None,
            },
            PublicationAbandonFenceMembershipV1 {
                membership_id: member(2),
                source_revision_id: SourceRevisionId::from_bytes([22; 16]),
                previous_shadow: Some(shadow(10)),
            },
        ],
    )
    .unwrap();
    let unknown = journal.abandon_publication_checked(
        &request,
        super::super::super::super::Boundary::LostAcknowledgement,
        &super::super::super::super::Unscoped,
    );
    assert_eq!(unknown, Err(ControlError::CommitOutcomeUnknown));
    assert!(journal.requires_recovery());

    let recovered = journal
        .recover_abandon_publication_checked(
            &request,
            &super::super::super::super::Unscoped,
        )
        .unwrap();
    assert!(matches!(recovered, CommitRecoveryDecision::Committed(_)));
    assert!(!journal.requires_recovery());

    let checkpoint = journal
        .read_checkpoint_checked(&super::super::super::super::Unscoped)
        .unwrap()
        .unwrap();
    assert_eq!(checkpoint.visibility().visible_epoch, Epoch::new(1).unwrap());
    assert_eq!(checkpoint.visibility().last_receipt, Some(receipt_id));
    assert_eq!(
        checkpoint.publication_receipt().unwrap().publication_receipt_id,
        receipt_id
    );
    assert_eq!(
        checkpoint.visibility().guards.shadow_generation,
        previous_visibility.guards.shadow_generation + 1
    );
    assert!(checkpoint.abandonment_fence().is_some());
}

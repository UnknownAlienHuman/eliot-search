//! One real-redb C02 canary. Opaque fixture bytes are not C07 I/O receipts.
use super::*;
use crate::{
    ControlPortCommand, JournalProfileInitialization, MigrationBootstrapWork as Work,
    PreparedArtifactSetMetadataV1, PreparedArtifactSetRecordV1, control_mutation_identity,
};
use search_contracts::{
    CanonicalValue as V, MaterializationId, OpaqueId, RepresentationId, SourceId,
    SourceNamespaceId, SourceRevisionRef, to_canonical_cbor,
};
use std::cell::Cell;
use std::time::{Duration, Instant};

fn metadata(byte: u8) -> PreparedArtifactSetMetadataV1 {
    let canonical = |n| to_canonical_cbor(&V::U64(n)).unwrap().as_slice().to_vec();
    PreparedArtifactSetMetadataV1 {
        operation_id: OpaqueId::new(format!("root:prepared-set-{byte}")).unwrap(),
        source_revision: SourceRevisionRef {
            source_namespace_id: SourceNamespaceId::from_bytes([40; 16]),
            source_id: SourceId::from_bytes([41; 16]),
            revision_id: SourceRevisionId::from_bytes([42; 16]),
            content_digest: digest(),
            byte_length: 123,
        },
        materialization_id: MaterializationId::from_bytes([byte; 16]),
        representation_id: RepresentationId::from_bytes([44; 16]),
        residency_canonical: canonical(45),
        object_reference_canonical: canonical(u64::from(byte)),
        stored_object_digest: Blake3Digest32::from_bytes([byte; 32]),
        stored_object_length: 456,
    }
}
fn phase_id(byte: u8) -> search_ports::MutationIdentity {
    control_mutation_identity(MutationId([byte; 32])).unwrap()
}
fn prepared(
    record: &PreparedArtifactSetRecordV1,
    visible: PublicationVisibilityState,
    byte: u8,
) -> PublicationIntent {
    PublicationIntent {
        publication_intent_id: PublicationIntentId::from_bytes([byte; 16]),
        target_epoch: visible.visible_epoch.checked_next().unwrap(),
        prepared_manifest_ref: record.reference().clone(),
        owner_source_membership_access_guards: visible.guards,
        state: PublicationIntentState::Prepared,
    }
}

#[test]
#[expect(clippy::too_many_lines, reason = "Keep the coherent real-journal publication and refusal sequence together")]
fn prepared_artifact_set_redb_canary() {
    let scratch = Scratch::new();
    let mut journal = scratch.empty(crate::REGISTRY_JOURNAL_SCHEMA_VERSION);
    journal
        .initialize_operating_profile(
            JournalProfileInitialization::Indexed(state()),
            MutationId([1; 32]),
            digest(),
            &context(false),
        )
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(60);
    let charged = Cell::new(0_usize);
    let work = |unit| -> Result<(), &'static str> {
        if let Work::Process(n) = unit {
            charged.set(charged.get() + n);
        }
        Ok(())
    };
    let record = PreparedArtifactSetRecordV1::prepare_with_original_context(
        metadata(43),
        LIMITS,
        deadline,
        &work,
    )
    .unwrap();
    let initial = prepared(&record, state(), 2);
    let begin = PublicationIntentUpdate::begin(MutationId([2; 32]), digest(), 1, &initial).unwrap();
    let unattached = ControlPortCommand::PublicationIntent(begin.clone());
    let begin = begin.with_prepared_artifact_set_v1(record.clone()).unwrap();
    let command = ControlPortCommand::PublicationIntent(begin.clone());
    let record_key = record.key(LIMITS).unwrap();
    // Even the low-level generic engine cannot manufacture or delete this profile.
    assert_eq!(
        journal.transact(&ControlMutation::new(
            MutationId([90; 32]),
            digest(),
            1,
            vec![ControlWrite {
                key: record_key.clone(),
                value: record.value(LIMITS, &Unscoped).unwrap()
            }],
            vec![]
        )),
        Err(ControlError::InvalidKey)
    );
    let mut publisher = ControlSnapshotPublisher::new();
    let mut bound = journal.bind_control_port::<Cancel>(&mut publisher).unwrap();
    assert_eq!(
        bound
            .transact_publication_with_original_context(&unattached, &phase_id(2), deadline, &work)
            .unwrap_err()
            .control,
        ControlError::InvalidValue
    );
    assert!(
        bound
            .read_prepared_artifact_set_with_original_context(record.reference(), deadline, &work)
            .is_err()
    );
    // A post-commit refusal remains unknown until recovery of the identical bytes.
    let boundaries = Cell::new(0);
    let refusing = |unit| -> Result<(), &'static str> {
        if matches!(unit, Work::Write(0)) {
            boundaries.set(boundaries.get() + 1);
        }
        if boundaries.get() >= 2 && matches!(unit, Work::Read(_)) {
            return Err("after-commit");
        }
        work(unit)
    };
    assert_eq!(
        bound
            .transact_publication_with_original_context(&command, &phase_id(2), deadline, &refusing)
            .unwrap_err()
            .control,
        ControlError::CommitOutcomeUnknown
    );
    let recovered = bound
        .recover_publication_with_original_context(&command, &phase_id(2), deadline, &work)
        .unwrap();
    let CommitRecoveryDecision::Committed(mut prior) = recovered else {
        panic!("expected committed exact recovery")
    };
    assert!(prior.replayed);
    let replay = bound
        .transact_publication_with_original_context(&command, &phase_id(2), deadline, &work)
        .unwrap();
    assert!(replay.replayed);
    let read = bound
        .read_prepared_artifact_set_with_original_context(record.reference(), deadline, &work)
        .unwrap();
    assert_eq!(read.generation(), prior.after_generation);
    assert_eq!(read.record().metadata(), &metadata(43));
    assert_eq!(read.record().reference(), &initial.prepared_manifest_ref);
    let mut different = metadata(43);
    different.stored_object_length += 1;
    let different = PreparedArtifactSetRecordV1::prepare_with_original_context(
        different, LIMITS, deadline, &work,
    )
    .unwrap();
    let wrong = PublicationIntentUpdate::begin(
        MutationId([2; 32]),
        digest(),
        1,
        &prepared(&different, state(), 2),
    )
    .unwrap()
    .with_prepared_artifact_set_v1(different)
    .unwrap();
    assert_eq!(
        bound
            .transact_publication_with_original_context(
                &ControlPortCommand::PublicationIntent(wrong),
                &phase_id(2),
                deadline,
                &work
            )
            .unwrap_err()
            .control,
        ControlError::OperationConflict
    );
    let mut intent = begin.intent().clone();
    for (id, phase) in [
        (3, PublicationIntentState::NewPointsAcknowledged),
        (4, PublicationIntentState::OldPointsClosedAcknowledged),
        (5, PublicationIntentState::ReadbackVerified),
    ] {
        let update = PublicationIntentUpdate::advance(
            MutationId([id; 32]),
            digest(),
            prior.after_generation,
            intent,
            phase,
        )
        .unwrap()
        .with_prepared_artifact_set_v1(record.clone())
        .unwrap();
        prior = bound
            .transact_publication_with_original_context(
                &ControlPortCommand::PublicationIntent(update.clone()),
                &phase_id(id),
                deadline,
                &work,
            )
            .unwrap();
        intent = update.intent().clone();
    }
    let request = VisibleEpochCommit::new(
        MutationId([6; 32]),
        digest(),
        prior,
        intent,
        state(),
        PublicationReceiptId::from_bytes([6; 16]),
        PublicationReadbackEvidence {
            exact_new_manifest_ref: reference("actual-new-point-manifest"),
            exact_retired_manifest_ref: reference("actual-empty-manifest"),
            readback_digest: digest(),
        },
        vec![PublicationManifestChange {
            membership_id: member(1),
            previous_manifest: None,
            next_manifest: Some(record.reference().clone()),
            matching_shadow: None,
        }],
    )
    .unwrap()
    .with_prepared_artifact_set_v1(record.clone())
    .unwrap();
    let commit = bound
        .commit_visible_epoch_with_original_context(&request, &phase_id(6), deadline, &work)
        .unwrap();
    bound
        .publish_committed_snapshot_with_original_context(&commit, deadline, &work)
        .unwrap();
    let snapshot = bound
        .read_publication_projection_snapshot_with_original_context(deadline, &work)
        .unwrap()
        .unwrap();
    assert_eq!(snapshot.visibility().visible_epoch.get(), 1);
    assert_eq!(
        snapshot.manifests().get(&member(1)),
        Some(record.reference())
    );
    assert_eq!(snapshot.prepared_artifact_sets().get(&member(1)), Some(&record));
    assert_eq!(snapshot.prepared_artifact_sets().len(), 1);
    // The successor retains a new actual record before any new visible-map change.
    let next_record = PreparedArtifactSetRecordV1::prepare_with_original_context(
        metadata(50),
        LIMITS,
        deadline,
        &work,
    )
    .unwrap();
    let head = bound
        .read_publication_intent_with_original_context(deadline, &work)
        .unwrap();
    let successor = PublicationSuccessor::new(
        MutationId([7; 32]),
        digest(),
        head.generation,
        head.intent.unwrap(),
        *snapshot.visibility(),
        &prepared(&next_record, *snapshot.visibility(), 7),
    )
    .unwrap()
    .with_prepared_artifact_set_v1(next_record.clone())
    .unwrap();
    let mut prior = bound
        .transact_publication_with_original_context(
            &ControlPortCommand::PublicationSuccessor(successor.clone()),
            &phase_id(7),
            deadline,
            &work,
        )
        .unwrap();
    assert_eq!(
        bound
            .read_prepared_artifact_set_with_original_context(
                next_record.reference(),
                deadline,
                &work
            )
            .unwrap()
            .record()
            .metadata(),
        &metadata(50)
    );
    // Preparing a successor cannot rewrite the already selected immutable image.
    assert_eq!(snapshot.prepared_artifact_sets().get(&member(1)), Some(&record));
    let mut intent = successor.intent().clone();
    for (id, phase) in [
        (8, PublicationIntentState::NewPointsAcknowledged),
        (9, PublicationIntentState::OldPointsClosedAcknowledged),
        (10, PublicationIntentState::ReadbackVerified),
    ] {
        let update = PublicationIntentUpdate::advance(
            MutationId([id; 32]),
            digest(),
            prior.after_generation,
            intent,
            phase,
        )
        .unwrap()
        .with_prepared_artifact_set_v1(next_record.clone())
        .unwrap();
        prior = bound
            .transact_publication_with_original_context(
                &ControlPortCommand::PublicationIntent(update.clone()),
                &phase_id(id),
                deadline,
                &work,
            )
            .unwrap();
        intent = update.intent().clone();
    }
    let next = VisibleEpochCommit::new(
        MutationId([11; 32]),
        digest(),
        prior.clone(),
        intent,
        *snapshot.visibility(),
        PublicationReceiptId::from_bytes([11; 16]),
        PublicationReadbackEvidence {
            exact_new_manifest_ref: reference("next-points"),
            exact_retired_manifest_ref: reference("prior-points"),
            readback_digest: digest(),
        },
        vec![PublicationManifestChange {
            membership_id: member(1),
            previous_manifest: Some(record.reference().clone()),
            next_manifest: Some(next_record.reference().clone()),
            matching_shadow: None,
        }],
    )
    .unwrap()
    .with_prepared_artifact_set_v1(next_record.clone())
    .unwrap();
    // A real guard/generation change between preparation and commit must refuse.
    let mut denied = *snapshot.visibility();
    denied.guards.purge_generation += 1;
    journal
        .transact(&ControlMutation::new(
            MutationId([80; 32]),
            digest(),
            prior.after_generation,
            vec![ControlWrite {
                key: key(STATE, LIMITS).unwrap(),
                value: codec::state(&denied, LIMITS).unwrap(),
            }],
            vec![],
        ))
        .unwrap();
    let mut bound = journal.bind_control_port::<Cancel>(&mut publisher).unwrap();
    assert_eq!(
        bound
            .commit_visible_epoch_with_original_context(&next, &phase_id(11), deadline, &work)
            .unwrap_err()
            .control,
        ControlError::TransactionConflict
    );
    let persisted = journal.verify().unwrap();
    assert_eq!(
        codec::read_state(persisted.get(&key(STATE, LIMITS).unwrap()).unwrap())
            .unwrap()
            .visible_epoch
            .get(),
        1
    );
    assert_eq!(
        codec::read_manifest(
            persisted
                .get(&id_key(MANIFESTS, member(1).as_bytes(), LIMITS).unwrap())
                .unwrap()
        )
        .unwrap(),
        *record.reference()
    );
    // Same-length valid metadata substitution is detected by the full-record reference.
    let original = persisted.get(&record_key).unwrap();
    let mut substituted = original.as_bytes().to_vec();
    *substituted.last_mut().unwrap() ^= 1;
    corrupt_value(
        &journal,
        &record_key,
        &ControlValue::new(ControlRecordClass::State, substituted, LIMITS).unwrap(),
    );
    assert!(journal.verify().is_err());
    corrupt_value(&journal, &record_key, original);
    let actual_identity = journal.identity();
    drop(journal);
    let mut reopened =
        PersistentControlJournal::open(scratch.file(), actual_identity, LIMITS).unwrap();
    let mut restored_publisher = ControlSnapshotPublisher::new();
    let restored = reopened
        .bind_control_port::<Cancel>(&mut restored_publisher)
        .unwrap();
    assert_eq!(
        restored
            .read_prepared_artifact_set_with_original_context(record.reference(), deadline, &work)
            .unwrap()
            .record()
            .metadata(),
        &metadata(43)
    );
    assert_eq!(
        restored
            .read_prepared_artifact_set_with_original_context(
                next_record.reference(),
                deadline,
                &work
            )
            .unwrap()
            .record()
            .metadata(),
        &metadata(50)
    );
    assert!(charged.get() > 0);
}

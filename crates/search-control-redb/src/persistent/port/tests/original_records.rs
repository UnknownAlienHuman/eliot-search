//! Real redb with original conditional-record accounting; no Native grant claim.

use super::*;
use crate::{ControlRecordCondition, JournalProfileInitialization, MigrationBootstrapWork as Work};
use crate::{CommitRecoveryDecision, RevisionDekCommand, REVISION_DEK_INTENT_KEY, REVISION_DEK_CANDIDATE_KEY, REVISION_DEK_CURRENT_KEY};
use crate::{RevisionDekSelectionCommand, REVISION_DEK_APPLICATION_INTENT_KEY};
use std::{
    cell::Cell,
    time::{Duration, Instant},
};

fn direct(scratch: &Scratch) -> PersistentControlJournal {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(scratch.path())
        .unwrap();
    let mut identity = identity3();
    identity.schema_version = crate::REGISTRY_JOURNAL_SCHEMA_VERSION;
    let mut journal = PersistentControlJournal::create(file, identity, LIMITS).unwrap();
    journal
        .initialize_operating_profile(
            JournalProfileInitialization::Direct,
            MutationId([90; 32]),
            digest(),
            &context(false),
        )
        .unwrap();
    journal
}
fn command() -> ConditionalControlMutation {
    // The actually accepted policy measured by the upstream owner is 82,255 bytes.
    // This owner tests opaque physical capacity, without importing policy meaning.
    let mut remaining = 82_255_usize;
    let writes: Vec<_> = (0..6)
        .map(|index| {
            let amount = remaining.min(16_384);
            remaining -= amount;
            ControlWrite {
                key: key(format!("original-inputs-test/{index}").as_bytes()),
                value: ControlValue::new(ControlRecordClass::State, vec![b'p'; amount], LIMITS)
                    .unwrap(),
            }
        })
        .collect();
    assert_eq!(remaining, 0);
    let conditions = writes
        .iter()
        .map(|entry| ControlRecordCondition::absent(entry.key.clone()))
        .collect();
    ConditionalControlMutation::new(
        ControlMutation::new(MutationId([91; 32]), digest(), 1, writes, vec![]),
        conditions,
    )
}
fn end() -> Instant {
    Instant::now() + Duration::from_secs(60)
}

#[test]
fn original_records_commit_all_six_values_once_without_advancing_source_registry_revision() {
    let scratch = Scratch::new();
    let mut journal = direct(&scratch);
    let mut publisher = ControlSnapshotPublisher::new();
    let command = command();
    let identity = control_mutation_identity(command.mutation().id()).unwrap();
    let remaining = Cell::new(8 * 1024 * 1024_usize);
    let work = |unit| {
        let bytes = match unit {
            Work::Read(n) | Work::Write(n) | Work::Allocate(n) | Work::Process(n) => n,
            _ => 0,
        };
        remaining.set(
            remaining
                .get()
                .checked_sub(bytes)
                .ok_or("original-allowance")?,
        );
        Ok::<_, &str>(())
    };
    let end = end();
    {
        let mut bound = journal.bind_control_port::<Cancel>(&mut publisher).unwrap();
        let receipt = bound
            .transact_records_with_original_context(&command, &identity, end, &work)
            .unwrap();
        assert_eq!(receipt.changed_keys.len(), 6);
        assert_eq!(receipt.after_generation, 2);
        let after_commit = remaining.get();
        let readback = bound
            .read_control_snapshot_with_original_context(end, &work)
            .unwrap();
        assert!(
            remaining.get() < after_commit,
            "readback uses the same borrowed allowance"
        );
        for entry in command.mutation().writes() {
            assert_eq!(readback.get(&entry.key), Some(&entry.value));
        }
        let registry = bound
            .read_registry_snapshot_with_original_context(end, &work)
            .unwrap();
        assert_eq!(registry.registry_revision(), 0);
        assert!(registry.states().is_empty());
        let replay = bound
            .transact_records_with_original_context(&command, &identity, end, &work)
            .unwrap();
        assert!(replay.replayed);
        assert_eq!(replay.after_generation, 2);
    }
    assert_eq!(journal.read_snapshot().unwrap().generation, 2);
}

#[test]
fn original_records_entry_cancel_deadline_and_reserved_keys_write_nothing() {
    let scratch = Scratch::new();
    let mut journal = direct(&scratch);
    let mut publisher = ControlSnapshotPublisher::new();
    let command = command();
    let identity = control_mutation_identity(command.mutation().id()).unwrap();
    let before = journal.read_snapshot().unwrap();
    {
        let mut bound = journal.bind_control_port::<Cancel>(&mut publisher).unwrap();
        let refused = bound
            .transact_records_with_original_context(&command, &identity, end(), &|_| {
                Err("original-cancel")
            })
            .unwrap_err();
        assert_eq!(refused.first_context, Some("original-cancel"));
        let expired = bound
            .transact_records_with_original_context(&command, &identity, Instant::now(), &|_| {
                Ok::<_, &str>(())
            })
            .unwrap_err();
        assert_eq!(expired.control, ControlError::BudgetExceeded);
        let mut reserved = command.mutation().writes().to_vec();
        reserved[0].key = key(b"collection_route/visibility/v1");
        let reserved = ConditionalControlMutation::new(
            ControlMutation::new(MutationId([92; 32]), digest(), 1, reserved, vec![]),
            vec![],
        );
        let identity = control_mutation_identity(reserved.mutation().id()).unwrap();
        assert_eq!(
            bound
                .transact_records_with_original_context(&reserved, &identity, end(), &|_| Ok::<
                    _,
                    &str,
                >(
                    ()
                ))
                .unwrap_err()
                .control,
            ControlError::InvalidKey
        );
    }
    assert_eq!(journal.read_snapshot().unwrap(), before);
}

#[test]
fn original_records_postdispatch_refusal_stays_unknown_until_identical_recovery() {
    let scratch = Scratch::new();
    let mut journal = direct(&scratch);
    let mut publisher = ControlSnapshotPublisher::new();
    let command = command();
    let identity = control_mutation_identity(command.mutation().id()).unwrap();
    {
        let mut bound = journal.bind_control_port::<Cancel>(&mut publisher).unwrap();
        let work = |unit| {
            if matches!(unit, Work::Write(n) if n > 0) {
                Err("original-stage-refusal")
            } else {
                Ok(())
            }
        };
        let error = bound
            .transact_records_with_original_context(&command, &identity, end(), &work)
            .unwrap_err();
        assert_eq!(error.control, ControlError::CommitOutcomeUnknown);
        assert_eq!(error.first_context, Some("original-stage-refusal"));
        let cancelled = bound
            .recover_records_with_original_context(&command, &identity, end(), &|_| {
                Err("recovery-cancel")
            })
            .unwrap_err();
        assert_eq!(cancelled.control, ControlError::CommitOutcomeUnknown);
        let recovered = bound
            .recover_records_with_original_context(&command, &identity, end(), &|_| {
                Ok::<_, &str>(())
            })
            .unwrap();
        assert_eq!(
            recovered,
            crate::CommitRecoveryDecision::NotCommittedRetrySameOperation
        );
    }
    assert!(!journal.requires_recovery());
    assert_eq!(journal.read_snapshot().unwrap().generation, 1);
}

// Structural C02 fixtures deliberately leave Core's four semantic fields opaque.
fn dek_value(marker: &str, codec: u64, kind: u64, evidence: u64) -> ControlValue {
    use search_contracts::{BoundedList, CanonicalText, CanonicalValue as V, to_canonical_cbor};
    let value = V::Array(BoundedList::new(vec![
        V::Text(CanonicalText::new(marker).unwrap()), V::U64(codec), V::U64(kind),
        V::Null, V::Null, V::Null, V::U64(evidence),
    ]).unwrap());
    ControlValue::new(ControlRecordClass::State, to_canonical_cbor(&value).unwrap().as_slice().to_vec(), LIMITS).unwrap()
}

fn dek_command(id: u8, generation: u64, candidate: bool) -> RevisionDekCommand {
    let intent = dek_value("eliot.revision-dek.record.v1", 1, 0, 0);
    let conditions = vec![
        if candidate { ControlRecordCondition::exact(key(REVISION_DEK_INTENT_KEY), intent.clone()) }
        else { ControlRecordCondition::absent(key(REVISION_DEK_INTENT_KEY)) },
        ControlRecordCondition::absent(key(REVISION_DEK_CANDIDATE_KEY)),
        ControlRecordCondition::absent(key(REVISION_DEK_CURRENT_KEY)),
    ];
    RevisionDekCommand::new(ConditionalControlMutation::new(ControlMutation::new(
        MutationId([id; 32]), digest(), generation, vec![ControlWrite {
            key: key(if candidate { REVISION_DEK_CANDIDATE_KEY } else { REVISION_DEK_INTENT_KEY }),
            value: if candidate { dek_value("eliot.revision-dek.record.v1", 1, 1, 0) } else { intent },
        }], vec![]), conditions)).unwrap()
}

fn altered_dek(command: &RevisionDekCommand, writes: Vec<ControlWrite>, deletes: Vec<ControlKey>,
    conditions: Vec<ControlRecordCondition>) -> Result<RevisionDekCommand, ControlError> {
    let mutation = command.command().mutation();
    RevisionDekCommand::new(ConditionalControlMutation::new(ControlMutation::new(
        mutation.id(), mutation.command_digest(), mutation.expected_generation(), writes, deletes), conditions))
}

#[test]
fn revision_dek_command_closes_keys_conditions_class_and_canonical_frame() {
    let intent = dek_command(91, 1, false);
    let candidate = dek_command(92, 2, true);
    let writes = intent.command().mutation().writes();
    let conditions = intent.command().conditions();
    for target in [REVISION_DEK_CURRENT_KEY, b"unrelated".as_slice(), b"secret/revision-dek/v1/unknown".as_slice()] {
        let mut changed = writes.to_vec();
        changed[0].key = key(target);
        assert_eq!(altered_dek(&intent, changed, vec![], conditions.to_vec()), Err(ControlError::InvalidKey));
    }
    assert_eq!(altered_dek(&intent, writes.to_vec(), vec![key(REVISION_DEK_CURRENT_KEY)], conditions.to_vec()), Err(ControlError::InvalidKey));
    for changed in [conditions[..2].to_vec(), vec![conditions[0].clone(); 3],
        conditions.iter().cloned().chain([ControlRecordCondition::absent(key(b"extra"))]).collect()] {
        assert!(altered_dek(&intent, writes.to_vec(), vec![], changed).is_err());
    }
    assert!(altered_dek(&candidate, candidate.command().mutation().writes().to_vec(), vec![], conditions.to_vec()).is_err());
    assert!(altered_dek(&intent, writes.to_vec(), vec![], candidate.command().conditions().to_vec()).is_err());
    for value in [
        dek_value("wrong-marker", 1, 0, 0),
        dek_value("eliot.revision-dek.record.v1", 2, 0, 0),
        dek_value("eliot.revision-dek.record.v1", 1, 1, 0),
        ControlValue::new(ControlRecordClass::Operation, writes[0].value.as_bytes().to_vec(), LIMITS).unwrap(),
        ControlValue::new(ControlRecordClass::State, writes[0].value.as_bytes().iter().copied().chain([0]).collect(), LIMITS).unwrap(),
    ] {
        let mut changed = writes.to_vec();
        changed[0].value = value;
        assert_eq!(altered_dek(&intent, changed, vec![], conditions.to_vec()), Err(ControlError::InvalidValue));
    }
}

#[test]
fn revision_dek_prefix_refuses_raw_and_bound_generic_writes_and_deletes() {
    let scratch = Scratch::new();
    let mut journal = direct(&scratch);
    let mut publisher = ControlSnapshotPublisher::new();
    let before = journal.read_snapshot().unwrap();
    let intent = dek_command(91, 1, false);
    for target in [REVISION_DEK_INTENT_KEY, REVISION_DEK_CANDIDATE_KEY, REVISION_DEK_CURRENT_KEY,
        b"secret/revision-dek/v1/unknown".as_slice()] {
        for deleting in [false, true] {
            let mutation = ControlMutation::new(MutationId([92; 32]), digest(), 1,
                if deleting { vec![] } else { vec![ControlWrite { key: key(target), value: intent.command().mutation().writes()[0].value.clone() }] },
                if deleting { vec![key(target)] } else { vec![] });
            let command = ConditionalControlMutation::new(mutation, vec![]);
            let identity = control_mutation_identity(command.mutation().id()).unwrap();
            assert_eq!(journal.transact_conditionally(command.clone(), &context(false)).unwrap_err().control_error(), ControlError::InvalidKey);
            let mut bound = journal.bind_control_port::<Cancel>(&mut publisher).unwrap();
            assert_eq!(bound.transact_records_with_original_context(&command, &identity, end(), &|_| Ok::<_, &str>(())).unwrap_err().control, ControlError::InvalidKey);
        }
    }
    assert_eq!(journal.read_snapshot().unwrap(), before);
}

#[test]
fn revision_dek_two_phases_readback_replay_and_exact_intent_without_current() {
    let scratch = Scratch::new();
    let mut journal = direct(&scratch);
    let mut publisher = ControlSnapshotPublisher::new();
    let intent = dek_command(91, 1, false);
    let candidate = dek_command(92, 2, true);
    let intent_id = control_mutation_identity(intent.command().mutation().id()).unwrap();
    let candidate_id = control_mutation_identity(candidate.command().mutation().id()).unwrap();
    let work = |_| Ok::<_, &str>(());
    let deadline = end();
    let mut bound = journal.bind_control_port::<Cancel>(&mut publisher).unwrap();
    assert!(!bound.revision_dek_operation_exists_with_original_context(intent.command().mutation().id(), deadline, &work).unwrap());
    assert_eq!(bound.transact_revision_dek_with_original_context(&intent, &intent_id, deadline, &work).unwrap().after_generation, 2);
    assert!(bound.revision_dek_operation_exists_with_original_context(intent.command().mutation().id(), deadline, &work).unwrap());
    assert!(!bound.revision_dek_operation_exists_with_original_context(candidate.command().mutation().id(), deadline, &work).unwrap());
    let mut wrong = candidate.command().conditions().to_vec();
    wrong[0] = ControlRecordCondition::exact(key(REVISION_DEK_INTENT_KEY), dek_value("eliot.revision-dek.record.v1", 1, 0, 1));
    let wrong = altered_dek(&candidate, candidate.command().mutation().writes().to_vec(), vec![], wrong).unwrap();
    assert_eq!(bound.transact_revision_dek_with_original_context(&wrong, &candidate_id, deadline, &work).unwrap_err().control, ControlError::GenerationMismatch);
    assert_eq!(bound.transact_revision_dek_with_original_context(&candidate, &candidate_id, deadline, &work).unwrap().after_generation, 3);
    let keys = [key(REVISION_DEK_INTENT_KEY), key(REVISION_DEK_CANDIDATE_KEY), key(REVISION_DEK_CURRENT_KEY)];
    let readback = bound.read_control_keys_with_original_context(&keys, deadline, &work).unwrap();
    assert_eq!(readback.get(&keys[0]), Some(&intent.command().mutation().writes()[0].value));
    assert_eq!(readback.get(&keys[1]), Some(&candidate.command().mutation().writes()[0].value));
    assert_eq!(readback.missing_keys(), &[keys[2].clone()]);
    let replay = bound.transact_revision_dek_with_original_context(&intent, &intent_id, deadline, &work).unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.after_generation, 2);
    assert!(matches!(bound.recover_revision_dek_with_original_context(&candidate, &candidate_id, deadline, &work).unwrap(), CommitRecoveryDecision::Committed(_)));
    assert_eq!(bound.read_control_keys_with_original_context(&keys, deadline, &work).unwrap().generation(), 3);
}

#[test]
fn revision_dek_original_refusal_unknown_and_identical_recovery_keep_fences() {
    let scratch = Scratch::new();
    let mut journal = direct(&scratch);
    let mut publisher = ControlSnapshotPublisher::new();
    let command = dek_command(91, 1, false);
    let identity = control_mutation_identity(command.command().mutation().id()).unwrap();
    let mut bound = journal.bind_control_port::<Cancel>(&mut publisher).unwrap();
    let cancelled = bound.transact_revision_dek_with_original_context(&command, &identity, end(), &|_| Err("entry-refusal")).unwrap_err();
    assert_eq!(cancelled.first_context, Some("entry-refusal"));
    assert_eq!(bound.transact_revision_dek_with_original_context(&command, &identity, Instant::now(), &|_| Ok::<_, &str>(())).unwrap_err().control, ControlError::BudgetExceeded);
    let error = bound.transact_revision_dek_with_original_context(&command, &identity, end(), &|unit|
        if matches!(unit, Work::Write(n) if n > 0) { Err("stage-refusal") } else { Ok(()) }).unwrap_err();
    assert_eq!(error.control, ControlError::CommitOutcomeUnknown);
    assert_eq!(error.first_context, Some("stage-refusal"));
    assert_eq!(bound.revision_dek_operation_exists_with_original_context(command.command().mutation().id(), end(), &|_| Ok::<_, &str>(())).unwrap_err().control, ControlError::StoreQuarantined);
    let foreign = dek_command(93, 1, false);
    let foreign_id = control_mutation_identity(foreign.command().mutation().id()).unwrap();
    assert_eq!(bound.recover_revision_dek_with_original_context(&foreign, &foreign_id, end(), &|_| Ok::<_, &str>(())).unwrap(), CommitRecoveryDecision::ConflictingInput);
    let cancelled = bound.recover_revision_dek_with_original_context(&command, &identity, end(), &|_| Err("recovery-refusal")).unwrap_err();
    assert_eq!(cancelled.control, ControlError::CommitOutcomeUnknown);
    assert_eq!(cancelled.first_context, Some("recovery-refusal"));
    assert_eq!(bound.recover_revision_dek_with_original_context(&command, &identity, end(), &|_| Ok::<_, &str>(())).unwrap(), CommitRecoveryDecision::NotCommittedRetrySameOperation);
    assert_eq!(bound.read_control_snapshot_with_original_context(end(), &|_| Ok::<_, &str>(())).unwrap().generation, 1);
}

#[test]
fn revision_dek_operation_lookup_is_read_only_and_never_treats_refusal_as_absence() {
    let scratch = Scratch::new();
    let mut journal = direct(&scratch);
    let mut publisher = ControlSnapshotPublisher::new();
    let before = journal.read_snapshot().unwrap();
    let bound = journal.bind_control_port::<Cancel>(&mut publisher).unwrap();
    let work = |_| Ok::<_, &str>(());
    let deadline = end();
    let slots = [key(REVISION_DEK_INTENT_KEY), key(REVISION_DEK_CANDIDATE_KEY), key(REVISION_DEK_CURRENT_KEY)];
    assert_eq!(bound.read_control_keys_with_original_context(&slots, deadline, &work).unwrap().missing_keys().len(), 3);
    // Even a different valid operation at the phase CID is a fence, never authority.
    assert!(bound.revision_dek_operation_exists_with_original_context(MutationId([90; 32]), deadline, &work).unwrap());
    assert!(!bound.revision_dek_operation_exists_with_original_context(MutationId([91; 32]), deadline, &work).unwrap());
    assert_eq!(bound.revision_dek_operation_exists_with_original_context(MutationId([91; 32]), deadline, &|_| Err("lookup-refusal")).unwrap_err().first_context, Some("lookup-refusal"));
    assert_eq!(bound.revision_dek_operation_exists_with_original_context(MutationId([91; 32]), Instant::now(), &work).unwrap_err().control, ControlError::BudgetExceeded);
    assert_eq!(bound.read_control_snapshot_with_original_context(deadline, &work).unwrap(), before);
    // Same row count, malformed operation bytes: never relabel corruption absent.
    let write = journal.database.begin_write().unwrap();
    {
        let mut operations = write.open_table(crate::persistent::OPERATIONS).unwrap();
        operations.insert(MutationId([90; 32]).0.as_slice(), b"invalid".as_slice()).unwrap();
    }
    write.commit().unwrap();
    let bound = journal.bind_control_port::<Cancel>(&mut publisher).unwrap();
    assert_eq!(bound.revision_dek_operation_exists_with_original_context(MutationId([90; 32]), deadline, &work).unwrap_err().control, ControlError::StoreCorrupt);
}

// C02 closes envelopes and CAS structure only; these opaque bodies are not Core
// records, applied backend proofs or Native qualification.
fn selection_array(values: Vec<search_contracts::CanonicalValue>) -> search_contracts::CanonicalValue {
    search_contracts::CanonicalValue::Array(search_contracts::BoundedList::new(values).unwrap())
}
fn selection_frame(marker: &str, codec: u64, kind: u64, body: search_contracts::CanonicalValue) -> ControlValue {
    use search_contracts::{CanonicalText, CanonicalValue as V, to_canonical_cbor};
    let value = selection_array(vec![V::Text(CanonicalText::new(marker).unwrap()), V::U64(codec), V::U64(kind), body]);
    ControlValue::new(ControlRecordClass::State, to_canonical_cbor(&value).unwrap().as_slice().to_vec(), LIMITS).unwrap()
}
fn selection_value(current: bool) -> ControlValue {
    use search_contracts::CanonicalValue as V;
    let intent = selection_array(vec![V::Null; 8]);
    let body = if current { selection_array(vec![intent, V::U64(1), V::U64(1), V::Null]) } else { intent };
    selection_frame("eliot.revision-dek.selection.v1", 1, u64::from(current), body)
}
fn selection_command(id: u8, generation: u64, current: bool) -> RevisionDekSelectionCommand {
    RevisionDekSelectionCommand::new(ConditionalControlMutation::new(ControlMutation::new(
        MutationId([id; 32]), digest(), generation, vec![ControlWrite {
            key: key(if current { REVISION_DEK_CURRENT_KEY } else { REVISION_DEK_APPLICATION_INTENT_KEY }),
            value: selection_value(current),
        }], vec![]), vec![
            ControlRecordCondition::exact(key(REVISION_DEK_INTENT_KEY), dek_value("eliot.revision-dek.record.v1", 1, 0, 0)),
            ControlRecordCondition::exact(key(REVISION_DEK_CANDIDATE_KEY), dek_value("eliot.revision-dek.record.v1", 1, 1, 0)),
            if current { ControlRecordCondition::exact(key(REVISION_DEK_APPLICATION_INTENT_KEY), selection_value(false)) }
            else { ControlRecordCondition::absent(key(REVISION_DEK_APPLICATION_INTENT_KEY)) },
            ControlRecordCondition::absent(key(REVISION_DEK_CURRENT_KEY)),
        ])).unwrap()
}
fn altered_selection(command: &RevisionDekSelectionCommand, writes: Vec<ControlWrite>, deletes: Vec<ControlKey>,
    conditions: Vec<ControlRecordCondition>) -> Result<RevisionDekSelectionCommand, ControlError> {
    let mutation = command.command().mutation();
    RevisionDekSelectionCommand::new(ConditionalControlMutation::new(ControlMutation::new(
        mutation.id(), mutation.command_digest(), mutation.expected_generation(), writes, deletes), conditions))
}

#[test]
fn revision_dek_selection_profiles_keep_candidate_commands_separate() {
    let application = selection_command(93, 3, false);
    let current = selection_command(94, 4, true);
    assert_eq!(application.command().conditions().len(), 4);
    assert_eq!(current.command().conditions().len(), 4);
    assert_eq!(current.command().mutation().writes()[0].key.as_bytes(), REVISION_DEK_CURRENT_KEY);
    assert_eq!(dek_command(92, 2, true).command().conditions().len(), 3);
    assert!(RevisionDekCommand::new(application.command().clone()).is_err());
    assert!(RevisionDekCommand::new(current.command().clone()).is_err());
    assert!(RevisionDekSelectionCommand::new(dek_command(92, 2, true).command().clone()).is_err());
}

#[test]
fn revision_dek_selection_closes_single_write_keys_deletes_and_four_conditions() {
    for command in [selection_command(93, 3, false), selection_command(94, 4, true)] {
        let writes = command.command().mutation().writes().to_vec();
        let conditions = command.command().conditions().to_vec();
        for target in [REVISION_DEK_INTENT_KEY, REVISION_DEK_CANDIDATE_KEY, b"unrelated".as_slice()] {
            let mut changed = writes.clone(); changed[0].key = key(target);
            assert!(altered_selection(&command, changed, vec![], conditions.clone()).is_err());
        }
        assert!(altered_selection(&command, vec![], vec![], conditions.clone()).is_err());
        assert!(altered_selection(&command, [writes.clone(), writes.clone()].concat(), vec![], conditions.clone()).is_err());
        assert!(altered_selection(&command, writes.clone(), vec![key(REVISION_DEK_CURRENT_KEY)], conditions.clone()).is_err());
        for index in 0..4 {
            let mut changed = conditions.clone(); changed.remove(index);
            assert!(altered_selection(&command, writes.clone(), vec![], changed).is_err());
        }
        let mut duplicate = conditions.clone(); duplicate[0] = duplicate[1].clone();
        assert!(altered_selection(&command, writes.clone(), vec![], duplicate).is_err());
        let mut present_current = conditions.clone();
        present_current[3] = ControlRecordCondition::exact(key(REVISION_DEK_CURRENT_KEY), selection_value(true));
        assert!(altered_selection(&command, writes, vec![], present_current).is_err());
    }
}

#[test]
fn revision_dek_selection_condition_values_require_exact_closed_predecessor_frames() {
    for current in [false, true] {
        let command = selection_command(93, 3, current);
        let writes = command.command().mutation().writes().to_vec();
        for index in 0..3 {
            let mut changed = command.command().conditions().to_vec();
            let condition_key = changed[index].key().clone();
            changed[index] = if index == 2 && !current {
                ControlRecordCondition::exact(condition_key, selection_value(false))
            } else { ControlRecordCondition::absent(condition_key) };
            assert!(altered_selection(&command, writes.clone(), vec![], changed).is_err());
        }
        for (index, value) in [(0, dek_value("eliot.revision-dek.record.v1", 1, 1, 0)),
            (1, dek_value("eliot.revision-dek.record.v1", 1, 0, 0))] {
            let mut changed = command.command().conditions().to_vec();
            changed[index] = ControlRecordCondition::exact(changed[index].key().clone(), value);
            assert!(altered_selection(&command, writes.clone(), vec![], changed).is_err());
        }
    }
    let command = selection_command(94, 4, true);
    let mut changed = command.command().conditions().to_vec();
    changed[2] = ControlRecordCondition::exact(key(REVISION_DEK_APPLICATION_INTENT_KEY), selection_value(true));
    assert!(altered_selection(&command, command.command().mutation().writes().to_vec(), vec![], changed).is_err());
}

#[test]
fn revision_dek_selection_requires_canonical_marker_codec_kind_and_nested_body_shapes() {
    use search_contracts::CanonicalValue as V;
    let intent = selection_command(93, 3, false);
    let current = selection_command(94, 4, true);
    let mut bad_values = vec![
        selection_frame("wrong-marker", 1, 0, selection_array(vec![V::Null; 8])),
        selection_frame("eliot.revision-dek.selection.v1", 2, 0, selection_array(vec![V::Null; 8])),
        selection_frame("eliot.revision-dek.selection.v1", 1, 2, selection_array(vec![V::Null; 8])),
        selection_frame("eliot.revision-dek.selection.v1", 1, 0, V::Null),
        selection_value(true),
    ];
    for size in [7, 9] { bad_values.push(selection_frame("eliot.revision-dek.selection.v1", 1, 0, selection_array(vec![V::Null; size]))); }
    let valid = selection_value(false);
    bad_values.push(ControlValue::new(ControlRecordClass::Operation, valid.as_bytes().to_vec(), LIMITS).unwrap());
    bad_values.push(ControlValue::new(ControlRecordClass::State, valid.as_bytes().iter().copied().chain([0]).collect(), LIMITS).unwrap());
    for value in bad_values {
        let mut changed = intent.command().mutation().writes().to_vec(); changed[0].value = value;
        assert!(altered_selection(&intent, changed, vec![], intent.command().conditions().to_vec()).is_err());
    }
    for body in [selection_array(vec![V::Null; 3]), selection_array(vec![V::Null; 5]),
        selection_array(vec![V::Null; 4]),
        selection_array(vec![selection_array(vec![V::Null; 7]), V::Null, V::Null, V::Null]),
        selection_array(vec![selection_array(vec![V::Null; 9]), V::Null, V::Null, V::Null])] {
        let mut changed = current.command().mutation().writes().to_vec();
        changed[0].value = selection_frame("eliot.revision-dek.selection.v1", 1, 1, body);
        assert!(altered_selection(&current, changed, vec![], current.command().conditions().to_vec()).is_err());
    }
}

#[test]
fn revision_dek_selection_receipts_reconstruct_original_cas_and_recover_exact_history() {
    use search_contracts::CanonicalValue as V;
    let scratch = Scratch::new();
    let mut journal = direct(&scratch);
    let mut publisher = ControlSnapshotPublisher::new();
    let work = |_| Ok::<_, &str>(());
    let deadline = end();
    let mut bound = journal.bind_control_port::<Cancel>(&mut publisher).unwrap();
    for command in [dek_command(91, 1, false), dek_command(92, 2, true)] {
        let identity = control_mutation_identity(command.command().mutation().id()).unwrap();
        bound.transact_revision_dek_with_original_context(&command, &identity, deadline, &work).unwrap();
    }
    let application = selection_command(93, 3, false);
    let current = selection_command(94, 4, true);
    let application_id = control_mutation_identity(application.command().mutation().id()).unwrap();
    let current_id = control_mutation_identity(current.command().mutation().id()).unwrap();
    assert_eq!(bound.revision_dek_selection_operation_with_original_context(MutationId([93; 32]), deadline, &work).unwrap(), None);
    assert_eq!(bound.revision_dek_selection_operation_with_original_context(MutationId([92; 32]), deadline, &work).unwrap_err().control, ControlError::OperationConflict);
    bound.transact_revision_dek_selection_with_original_context(&application, &application_id, deadline, &work).unwrap();
    let receipt = bound.revision_dek_selection_operation_with_original_context(MutationId([93; 32]), deadline, &work).unwrap().unwrap();
    assert_eq!(receipt.operation_id, application.command().mutation().id());
    assert_eq!(receipt.command_digest, application.command().mutation().command_digest());
    assert_eq!((receipt.before_generation, receipt.after_generation), (3, 4));
    let error = bound.transact_revision_dek_selection_with_original_context(&current, &current_id, deadline, &|unit|
        if matches!(unit, Work::Write(n) if n > 0) { Err("selection-stage-refusal") } else { Ok(()) }).unwrap_err();
    assert_eq!(error.control, ControlError::CommitOutcomeUnknown);
    assert_eq!(error.first_context, Some("selection-stage-refusal"));
    assert_eq!(bound.revision_dek_selection_operation_with_original_context(MutationId([94; 32]), deadline, &work).unwrap_err().control, ControlError::StoreQuarantined);
    assert_eq!(bound.recover_revision_dek_selection_with_original_context(&current, &current_id, deadline, &work).unwrap(), CommitRecoveryDecision::NotCommittedRetrySameOperation);
    bound.transact_revision_dek_selection_with_original_context(&current, &current_id, deadline, &work).unwrap();
    let stored = bound.revision_dek_selection_operation_with_original_context(MutationId([94; 32]), deadline, &work).unwrap().unwrap();
    assert_eq!((stored.before_generation, stored.after_generation), (4, 5));
    assert!(matches!(bound.recover_revision_dek_selection_with_original_context(&application, &application_id, deadline, &work).unwrap(), CommitRecoveryDecision::Committed(receipt) if receipt.replayed));
    // Same phase CID and even the same declared digest cannot replay changed bytes.
    let mut changed = application.command().mutation().writes().to_vec();
    let mut body = vec![V::Null; 8]; body[7] = V::U64(9);
    changed[0].value = selection_frame("eliot.revision-dek.selection.v1", 1, 0, selection_array(body));
    let changed = altered_selection(&application, changed, vec![], application.command().conditions().to_vec()).unwrap();
    assert_eq!(bound.transact_revision_dek_selection_with_original_context(&changed, &application_id, deadline, &work).unwrap_err().control, ControlError::OperationConflict);
    assert_eq!(bound.read_control_snapshot_with_original_context(deadline, &work).unwrap().generation, 5);
}

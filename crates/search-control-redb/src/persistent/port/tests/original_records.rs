//! Real redb with original conditional-record accounting; no Native grant claim.

use super::*;
use crate::{ControlRecordCondition, JournalProfileInitialization, MigrationBootstrapWork as Work};
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

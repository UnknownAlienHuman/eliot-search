//! Real checked-path/native-callback proofs. No native custody/admission is fabricated.
use super::*;
use crate::{JournalProfileInitialization, MigrationBootstrapWork as Work};
use std::cell::RefCell;
use std::time::{Duration, Instant};

fn deadline() -> Instant {
    Instant::now() + Duration::from_secs(60)
}
fn direct(scratch: &Scratch) -> PersistentControlJournal {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(scratch.0.join("control.redb"))
        .unwrap();
    let mut journal = PersistentControlJournal::create(file, identity(5), LIMITS).unwrap();
    journal
        .initialize_operating_profile(
            JournalProfileInitialization::Direct,
            MutationId([90; 32]),
            Blake3Digest32::from_bytes([91; 32]),
            &context(),
        )
        .unwrap();
    journal
}

#[test]
fn registry_original_context_accounts_large_records_and_shared_borrowed_allowance() {
    let scratch = Scratch::new();
    let mut journal = direct(&scratch);
    let state = vec![b'A'; 48 * 1024];
    let original = request("original-context", 1, 0, None, &state);
    let observed = RefCell::new(Vec::new());
    let remaining = Cell::new(8 * 1024 * 1024_usize);
    let work = |unit| {
        let bytes = match unit {
            Work::Read(n) | Work::Write(n) | Work::Allocate(n) | Work::Process(n) => n,
            _ => 0,
        };
        let next = remaining
            .get()
            .checked_sub(bytes)
            .ok_or("native-allowance")?;
        remaining.set(next);
        observed.borrow_mut().push(unit);
        Ok::<_, &str>(())
    };
    let end = deadline();
    let committed = journal
        .commit_registry_batch_with_original_context(&original, end, &work)
        .unwrap();
    let after_commit = remaining.get();
    assert!(matches!(
        journal
            .lookup_registry_operation_with_original_context(
                original.envelope().operation_id(),
                end,
                &work
            )
            .unwrap(),
        RegistryOperationLookup::Committed(_)
    ));
    let keys = vec![state_key()];
    let targets = journal
        .read_registry_operation_targets_with_original_context(
            original.envelope(),
            &keys,
            end,
            &work,
        )
        .unwrap();
    assert_eq!(targets.states()[0].canonical_bytes(), state);
    assert_eq!(
        journal
            .read_registry_snapshot_with_original_context(end, &work)
            .unwrap()
            .states()
            .len(),
        1
    );
    assert!(remaining.get() < after_commit);
    assert!(
        observed.borrow().contains(&Work::Read(state.len() + 13)),
        "full stored row, not a fixed 8 KiB charge"
    );
    assert!(
        observed
            .borrow()
            .iter()
            .any(|unit| matches!(unit, Work::Allocate(n) if *n >= state.len()))
    );
    let writes = journal.committed_writes();
    assert!(matches!(
        journal
            .recover_registry_batch_with_original_context(&original, end, &work)
            .unwrap(),
        CommitRecoveryDecision::Committed(_)
    ));
    assert_eq!(journal.committed_writes(), writes);
    assert_eq!(committed.receipt().after_generation, 2);
}

#[test]
fn registry_original_context_expired_deadline_and_first_refusal_cannot_be_absence() {
    let scratch = Scratch::new();
    let mut journal = direct(&scratch);
    let calls = Cell::new(0);
    let work = |_| {
        calls.set(calls.get() + 1);
        Ok::<_, &str>(())
    };
    let expired = Instant::now().checked_sub(Duration::from_secs(1)).expect("representable expired fixture deadline");
    let error = journal
        .lookup_registry_operation_with_original_context(
            &OpaqueId::new("absent").unwrap(),
            expired,
            &work,
        )
        .unwrap_err();
    assert_eq!(error.control, ControlError::BudgetExceeded);
    assert_eq!(calls.get(), 0);
    let refuse = |_| {
        calls.set(calls.get() + 1);
        if calls.get() == 1 {
            Err("first-native-refusal")
        } else {
            Ok(())
        }
    };
    let error = journal
        .commit_registry_batch_with_original_context(
            &request("refused", 1, 0, None, b"A"),
            deadline(),
            &refuse,
        )
        .unwrap_err();
    assert_eq!(error.control, ControlError::ReadCancelled);
    assert_eq!(error.first_context, Some("first-native-refusal"));
    assert_eq!(calls.get(), 1);
    assert_eq!(journal.read_snapshot().unwrap().generation, 1);
    assert!(!journal.requires_recovery());
}

#[test]
fn registry_original_context_target_refusal_is_not_partial_and_lookup_does_not_read_targets() {
    let scratch = Scratch::new();
    let mut journal = direct(&scratch);
    let original = request("large-read-refused", 1, 0, None, &vec![b'A'; 48 * 1024]);
    journal
        .commit_registry_batch(&original, &context())
        .unwrap();
    let work = |unit| {
        if matches!(unit, Work::Read(n) if n > 8192) {
            Err("native-record-read")
        } else {
            Ok(())
        }
    };
    assert!(matches!(
        journal
            .lookup_registry_operation_with_original_context(
                original.envelope().operation_id(),
                deadline(),
                &work
            )
            .unwrap(),
        RegistryOperationLookup::Committed(_)
    ));
    let target_error = journal
        .read_registry_operation_targets_with_original_context(
            original.envelope(),
            &[state_key()],
            deadline(),
            &work,
        )
        .unwrap_err();
    assert_eq!(target_error.control, ControlError::ReadCancelled);
    assert_eq!(target_error.first_context, Some("native-record-read"));
    let restore_error = journal
        .read_registry_snapshot_with_original_context(deadline(), &work)
        .unwrap_err();
    assert_eq!(restore_error.control, ControlError::ReadCancelled);
    assert_eq!(restore_error.first_context, Some("native-record-read"));
    assert!(!journal.requires_recovery());
}

#[test]
fn registry_original_context_allocation_refusal_precedes_large_copy_and_mutation() {
    let scratch = Scratch::new();
    let mut journal = direct(&scratch);
    let original = request("allocation-refused", 1, 0, None, &vec![b'A'; 48 * 1024]);
    let writes = Cell::new(0);
    let work = |unit| {
        if matches!(unit, Work::Write(_)) {
            writes.set(writes.get() + 1);
        }
        if matches!(unit, Work::Allocate(n) if n >= 48 * 1024) {
            Err("native-copy-capacity")
        } else {
            Ok(())
        }
    };
    let error = journal
        .commit_registry_batch_with_original_context(&original, deadline(), &work)
        .unwrap_err();
    assert_eq!(error.control, ControlError::ReadCancelled);
    assert_eq!(error.first_context, Some("native-copy-capacity"));
    assert_eq!(writes.get(), 0);
    assert!(!journal.requires_recovery());
    assert_eq!(journal.read_snapshot().unwrap().generation, 1);
}

#[test]
fn registry_original_context_post_dispatch_refusal_is_unknown_and_exact_recovery_does_not_write() {
    let scratch = Scratch::new();
    let mut journal = direct(&scratch);
    let original = request("dispatch-refused", 1, 0, None, b"A");
    let work = |unit| {
        if matches!(unit, Work::Write(n) if n > 0) {
            Err("native-stage-refusal")
        } else {
            Ok(())
        }
    };
    let error = journal
        .commit_registry_batch_with_original_context(&original, deadline(), &work)
        .unwrap_err();
    assert_eq!(error.control, ControlError::CommitOutcomeUnknown);
    assert_eq!(error.first_context, Some("native-stage-refusal"));
    assert!(journal.requires_recovery());
    // A separately admitted read-only recovery invocation gets no replacement mutation.
    let recovery = |_| Ok::<_, &str>(());
    let changed = request("dispatch-refused", 1, 0, None, b"B");
    assert_eq!(
        journal
            .recover_registry_batch_with_original_context(&changed, deadline(), &recovery)
            .unwrap(),
        CommitRecoveryDecision::ConflictingInput
    );
    assert!(journal.requires_recovery());
    assert_eq!(
        journal
            .recover_registry_batch_with_original_context(&original, deadline(), &recovery)
            .unwrap(),
        CommitRecoveryDecision::NotCommittedRetrySameOperation
    );
    assert!(!journal.requires_recovery());
    assert_eq!(journal.read_snapshot().unwrap().generation, 1);
}

#[test]
fn registry_original_context_bound_unknown_readback_keeps_same_publisher_fenced_through_recovery() {
    let scratch = Scratch::new();
    let mut journal = direct(&scratch);
    let original = request("bound-original", 1, 0, None, b"A");
    let mutation = control_mutation_identity(original.control_operation_id()).unwrap();
    let mut publisher = ControlSnapshotPublisher::new();
    journal
        .recover_snapshot_publication_with_context(&mut publisher, &context())
        .unwrap();
    let write_boundaries = Cell::new(0);
    let work = |unit| {
        if unit == Work::Write(0) {
            write_boundaries.set(write_boundaries.get() + 1);
        }
        if write_boundaries.get() >= 2 && matches!(unit, Work::Read(_)) {
            Err("native-post-commit-readback")
        } else {
            Ok(())
        }
    };
    {
        let mut bound = journal.bind_control_port::<Cancel>(&mut publisher).unwrap();
        let error = bound
            .commit_registry_batch_with_original_context(&original, &mutation, deadline(), &work)
            .unwrap_err();
        assert_eq!(error.control, ControlError::CommitOutcomeUnknown);
        assert_eq!(error.first_context, Some("native-post-commit-readback"));
        assert!(bound.current_snapshot().is_err());
        let recovery = |_| Ok::<_, &str>(());
        assert!(matches!(
            bound
                .recover_registry_batch_with_original_context(
                    &original,
                    &mutation,
                    deadline(),
                    &recovery
                )
                .unwrap(),
            CommitRecoveryDecision::Committed(_)
        ));
        assert!(
            bound.current_snapshot().is_err(),
            "recovery does not publish"
        );
        assert!(matches!(
            bound
                .lookup_registry_operation_with_original_context(
                    original.envelope().operation_id(),
                    deadline(),
                    &recovery
                )
                .unwrap(),
            RegistryOperationLookup::Committed(_)
        ));
        assert_eq!(
            bound
                .read_registry_operation_targets_with_original_context(
                    original.envelope(),
                    &[state_key()],
                    deadline(),
                    &recovery
                )
                .unwrap()
                .registry_revision(),
            1
        );
        assert_eq!(
            bound
                .read_registry_snapshot_with_original_context(deadline(), &recovery)
                .unwrap()
                .registry_revision(),
            1
        );
    }
    assert!(publisher.current().is_none());
    drop(journal);
    let reopened = scratch.reopen();
    assert_eq!(
        reopened
            .read_registry_snapshot_with_original_context(deadline(), &|_| Ok::<_, &str>(()))
            .unwrap()
            .registry_revision(),
        1
    );
    assert_eq!(reopened.committed_writes(), 0);
}

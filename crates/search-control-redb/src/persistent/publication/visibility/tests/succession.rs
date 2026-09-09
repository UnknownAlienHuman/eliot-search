//! Normal succession uses real redb plus the real disk-bound snapshot publisher.
use super::*;

fn committed(scratch: &Scratch) -> (PersistentControlJournal, ControlSnapshotPublisher) {
    let (mut journal, request) = scratch.ready();
    let receipt = apply(&mut journal, &request).unwrap();
    let mut publisher = ControlSnapshotPublisher::new();
    journal.publish_committed_snapshot(&receipt, &mut publisher).unwrap();
    (journal, publisher)
}
fn prepared(visibility: PublicationVisibilityState, id: u8) -> PublicationIntent {
    PublicationIntent {
        publication_intent_id: PublicationIntentId::from_bytes([id; 16]),
        target_epoch: visibility.visible_epoch.checked_next().unwrap(),
        prepared_manifest_ref: reference("successor-manifest-sentinel"),
        owner_source_membership_access_guards: visibility.guards,
        state: PublicationIntentState::Prepared,
    }
}
fn reserve_request(journal: &PersistentControlJournal, operation: u8, id: u8) -> PublicationSuccessor {
    let head = journal.read_publication_intent(&context(false)).unwrap();
    let visible = journal.read_publication_visibility(&context(false)).unwrap().unwrap();
    PublicationSuccessor::new(MutationId([operation; 32]), digest(), head.generation,
        head.intent.unwrap(), visible, &prepared(visible, id)).unwrap()
}
fn reserve(journal: &mut PersistentControlJournal, publisher: &ControlSnapshotPublisher,
    request: &PublicationSuccessor) -> Result<ControlCommitReceipt, ControlError> {
    journal.reserve_next_publication(request, publisher, &context(false))
        .map_err(crate::persistent::operation::ControlCallError::control_error)
}
fn finish_next(journal: &mut PersistentControlJournal, publisher: &mut ControlSnapshotPublisher,
    request: &PublicationSuccessor) {
    let mut receipt = reserve(journal, publisher, request).unwrap();
    let mut intent = request.intent().clone();
    for (id, phase) in [(11, PublicationIntentState::NewPointsAcknowledged),
        (12, PublicationIntentState::OldPointsClosedAcknowledged), (13, PublicationIntentState::ReadbackVerified)] {
        let update = PublicationIntentUpdate::advance(MutationId([id; 32]), digest(),
            receipt.after_generation, intent, phase).unwrap();
        receipt = journal.persist_publication_intent(&update, &context(false)).unwrap();
        intent = update.intent().clone();
    }
    let previous = journal.read_publication_visibility(&context(false)).unwrap().unwrap();
    let command = VisibleEpochCommit::new(MutationId([14; 32]), digest(), receipt,
        intent, previous, PublicationReceiptId::from_bytes([14; 16]),
        PublicationReadbackEvidence { exact_new_manifest_ref: reference("next-points"),
            exact_retired_manifest_ref: reference("prior-points"), readback_digest: digest() },
        vec![PublicationManifestChange { membership_id: member(1), previous_manifest: Some(reference("new")),
            next_manifest: Some(reference("next")), matching_shadow: None }]).unwrap();
    let commit = apply(journal, &command).unwrap();
    journal.publish_committed_snapshot(&commit, publisher).unwrap();
}

#[test]
fn reserve_changes_only_intent_and_preserves_visible_epoch_and_prior_receipts() {
    let scratch = Scratch::new(); let (mut journal, publisher) = committed(&scratch);
    let request = reserve_request(&journal, 10, 10);
    let before = journal.verify().unwrap();
    let commit = reserve(&mut journal, &publisher, &request).unwrap();
    assert_eq!(commit.changed_keys, vec![key(super::super::super::KEY, LIMITS).unwrap()]);
    let after = journal.verify().unwrap();
    assert_eq!(after.records.len(), before.records.len());
    for ((key, prior), (same_key, value)) in before.records.iter().zip(&after.records) {
        assert_eq!(key, same_key);
        if key.as_bytes() != super::super::super::KEY { assert_eq!(prior, value); }
    }
    assert_eq!(journal.read_publication_visibility(&context(false)).unwrap().unwrap().visible_epoch.get(), 1);
    assert_eq!(journal.load_unresolved_publication(&context(false)).unwrap(), Some(request.intent().clone()));
    assert_eq!(publisher.current().unwrap().generation, before.generation);
    assert!(journal.control_snapshot().is_err());
    assert!(!format!("{request:?}").contains("manifest-sentinel"));
}

#[test]
fn two_publications_then_third_reservation_keep_all_epochs_and_old_receipts() {
    let scratch = Scratch::new(); let (mut journal, mut publisher) = committed(&scratch);
    let second = reserve_request(&journal, 10, 10);
    finish_next(&mut journal, &mut publisher, &second);
    let visible = journal.read_publication_visibility(&context(false)).unwrap().unwrap();
    assert_eq!(visible.visible_epoch.get(), 2);
    let snapshot = journal.verify().unwrap();
    assert_eq!(snapshot.records.iter().filter(|(key, _)| key.as_bytes().starts_with(RECEIPTS)).count(), 2);
    let third = reserve_request(&journal, 20, 20);
    reserve(&mut journal, &publisher, &third).unwrap();
    assert_eq!(third.intent().target_epoch.get(), 3);
    assert_eq!(journal.read_publication_visibility(&context(false)).unwrap().unwrap(), visible);
    drop(journal);
    let reopened = scratch.reopen();
    assert_eq!(reopened.load_unresolved_publication(&context(false)).unwrap(), Some(third.intent().clone()));
}

#[test]
fn unresolved_aborted_or_invalidation_only_predecessors_cannot_be_replaced() {
    let scratch = Scratch::new(); let (journal, _) = committed(&scratch);
    let head = journal.read_publication_intent(&context(false)).unwrap();
    let visible = journal.read_publication_visibility(&context(false)).unwrap().unwrap();
    for state in [PublicationIntentState::Prepared, PublicationIntentState::IntentDurable,
        PublicationIntentState::NewPointsAcknowledged, PublicationIntentState::OldPointsClosedAcknowledged,
        PublicationIntentState::ReadbackVerified, PublicationIntentState::Compensating,
        PublicationIntentState::Aborted, PublicationIntentState::PublicationBlocked,
        PublicationIntentState::InvalidationOnlyCommitted, PublicationIntentState::Reclaimable] {
        let mut previous = head.intent.clone().unwrap(); previous.state = state;
        assert!(PublicationSuccessor::new(MutationId([10; 32]), digest(), head.generation,
            previous, visible, &prepared(visible, 10)).is_err(), "{state:?}");
    }
}

#[test]
fn constructor_rejects_skips_reused_identity_stale_guards_and_overflow() {
    let scratch = Scratch::new(); let (journal, _) = committed(&scratch);
    let head = journal.read_publication_intent(&context(false)).unwrap();
    let previous = head.intent.unwrap();
    let visible = journal.read_publication_visibility(&context(false)).unwrap().unwrap();
    for kind in 0..5 {
        let mut next = prepared(visible, 10);
        match kind {
            0 => next.target_epoch = Epoch::new(3).unwrap(),
            1 => next.publication_intent_id = previous.publication_intent_id,
            2 => next.owner_source_membership_access_guards.purge_generation += 1,
            3 => next.state = PublicationIntentState::IntentDurable,
            _ => next.target_epoch = visible.visible_epoch,
        }
        assert!(PublicationSuccessor::new(MutationId([10; 32]), digest(), head.generation,
            previous.clone(), visible, &next).is_err());
    }
    assert!(PublicationSuccessor::new(MutationId([10; 32]), digest(), u64::MAX,
        previous.clone(), visible, &prepared(visible, 10)).is_err());
    let mut exhausted = previous; exhausted.target_epoch = Epoch::new(i64::MAX - 1).unwrap();
    let mut full = visible; full.visible_epoch = exhausted.target_epoch;
    assert!(PublicationSuccessor::new(MutationId([10; 32]), digest(), 7,
        exhausted, full, &prepared(visible, 10)).is_err());
}

#[test]
fn unpublished_suspended_or_model_only_snapshot_cannot_authorize_reservation() {
    let scratch = Scratch::new(); let (mut journal, request) = scratch.ready();
    let commit = apply(&mut journal, &request).unwrap();
    let successor = reserve_request(&journal, 10, 10);
    let mut publisher = ControlSnapshotPublisher::new();
    let before = scratch.bytes();
    assert_eq!(reserve(&mut journal, &publisher, &successor), Err(ControlError::SnapshotPublicationFailed));
    publisher.publish_snapshot_after_commit(&commit, journal.control_snapshot().unwrap()).unwrap();
    assert_eq!(reserve(&mut journal, &publisher, &successor), Err(ControlError::SnapshotPublicationFailed));
    journal.publish_committed_snapshot(&commit, &mut publisher).unwrap();
    publisher.begin_disk_publication(identity()).unwrap();
    assert_eq!(reserve(&mut journal, &publisher, &successor), Err(ControlError::SnapshotPublicationFailed));
    assert_eq!(scratch.bytes(), before);
    journal.recover_snapshot_publication(&mut publisher).unwrap();
    reserve(&mut journal, &publisher, &successor).unwrap();
}

#[test]
fn competing_reservation_and_changed_replay_cannot_replace_an_active_intent() {
    let scratch = Scratch::new(); let (mut journal, publisher) = committed(&scratch);
    let first = reserve_request(&journal, 10, 10);
    let competitor = reserve_request(&journal, 20, 20);
    let changed = reserve_request(&journal, 10, 30);
    reserve(&mut journal, &publisher, &first).unwrap();
    assert_eq!(reserve(&mut journal, &publisher, &competitor), Err(ControlError::TransactionConflict));
    assert_eq!(reserve(&mut journal, &publisher, &changed), Err(ControlError::OperationConflict));
    assert_eq!(journal.load_unresolved_publication(&context(false)).unwrap(), Some(first.intent().clone()));
    assert!(reserve(&mut journal, &ControlSnapshotPublisher::new(), &first).unwrap().replayed);
}

#[test]
fn old_intent_identity_cannot_be_reused_after_another_publication_commits() {
    let scratch = Scratch::new(); let (mut journal, mut publisher) = committed(&scratch);
    let second = reserve_request(&journal, 10, 10);
    finish_next(&mut journal, &mut publisher, &second);
    let reused = reserve_request(&journal, 20, 3); // epoch one's retained identity
    let before = journal.verify().unwrap();
    assert_eq!(reserve(&mut journal, &publisher, &reused), Err(ControlError::OperationConflict));
    assert_eq!(journal.verify().unwrap(), before);
}

#[test]
fn lost_ack_reopens_and_recovers_the_original_request_without_reexecuting_it() {
    let scratch = Scratch::new(); let (mut journal, publisher) = committed(&scratch);
    let request = reserve_request(&journal, 10, 10);
    let changed = reserve_request(&journal, 10, 30);
    assert_eq!(journal.reserve_successor_checked(&request, &publisher, Boundary::LostAcknowledgement, &Unscoped),
        Err(ControlError::CommitOutcomeUnknown));
    assert!(journal.requires_recovery());
    assert!(matches!(journal.recover_publication_successor(&changed, &context(false)).unwrap(), CommitRecoveryDecision::ConflictingInput));
    assert!(journal.requires_recovery()); drop(journal);
    let mut journal = scratch.reopen();
    assert!(matches!(journal.recover_publication_successor(&request, &context(false)).unwrap(), CommitRecoveryDecision::Committed(_)));
    assert!(reserve(&mut journal, &ControlSnapshotPublisher::new(), &request).unwrap().replayed);
    assert_eq!(journal.committed_writes(), 0);
    assert_eq!(journal.read_publication_visibility(&context(false)).unwrap().unwrap().visible_epoch.get(), 1);
}

#[test]
fn interruption_before_or_during_write_preserves_exact_recovery_semantics() {
    let scratch = Scratch::new(); let (mut journal, publisher) = committed(&scratch);
    let request = reserve_request(&journal, 10, 10);
    assert!(journal.reserve_next_publication(&request, &publisher, &context(true)).is_err());
    assert!(!journal.requires_recovery());
    assert_eq!(journal.reserve_successor_checked(&request, &publisher, Boundary::Normal, &StopAt(Point::StageRecord)),
        Err(ControlError::CommitOutcomeUnknown));
    assert!(journal.requires_recovery());
    assert!(journal.recover_publication_successor(&request, &context(true)).is_err());
    assert!(journal.requires_recovery());
    assert!(matches!(journal.recover_publication_successor(&request, &context(false)).unwrap(),
        CommitRecoveryDecision::NotCommittedRetrySameOperation));
    reserve(&mut journal, &publisher, &request).unwrap();
}

#[test]
fn historical_replay_after_later_progress_and_owner_handoff_changes_nothing() {
    let scratch = Scratch::new(); let (mut journal, mut publisher) = committed(&scratch);
    let request = reserve_request(&journal, 10, 10);
    finish_next(&mut journal, &mut publisher, &request);
    let next_owner = JournalIdentity { owner_epoch: OwnerEpoch::new(2).unwrap(), ..identity() };
    let mut journal = journal.advance_owner(next_owner).unwrap();
    let before = journal.verify().unwrap();
    assert!(reserve(&mut journal, &ControlSnapshotPublisher::new(), &request).unwrap().replayed);
    assert!(matches!(journal.recover_publication_successor(&request, &context(false)).unwrap(), CommitRecoveryDecision::Committed(_)));
    assert_eq!(journal.verify().unwrap(), before);
}

#[test]
fn same_generation_corruption_cannot_use_an_older_published_snapshot() {
    let scratch = Scratch::new(); let (mut journal, publisher) = committed(&scratch);
    let request = reserve_request(&journal, 10, 10);
    let changed = id_key(MANIFESTS, member(1).as_bytes(), LIMITS).unwrap();
    // Both references encode to the same length; the corruption does not rely
    // on a counter/length mismatch being found first.
    corrupt_value(&journal, &changed, &codec::manifest(&reference("bad"), LIMITS).unwrap());
    assert_eq!(reserve(&mut journal, &publisher, &request), Err(ControlError::SnapshotPublicationFailed));
    assert_eq!(journal.read_publication_intent(&context(false)).unwrap().generation, 7);
}

#[test]
fn process_exit_before_and_after_reservation_never_reuses_a_committed_epoch() {
    use std::process::Command;
    use std::time::{Duration, Instant};
    for (mode, code, reserved) in [("before", 73, false), ("after", 74, true)] {
        let scratch = Scratch::new(); let (journal, _) = committed(&scratch);
        let request = reserve_request(&journal, 10, 10); drop(journal);
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args(["--ignored", "--exact", "persistent::publication::visibility::tests::succession::crash_child", "--nocapture"])
            .env("ELIOT_SUCCESSOR_CRASH_PATH", scratch.path()).env("ELIOT_SUCCESSOR_CRASH_MODE", mode).spawn().unwrap();
        let until = Instant::now() + Duration::from_secs(30);
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() { break status; }
            if Instant::now() >= until {
                let _ = child.kill(); let _ = child.wait(); panic!("successor crash fixture timed out");
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        assert_eq!(status.code(), Some(code));
        let mut journal = scratch.reopen();
        let head = journal.read_publication_intent(&context(false)).unwrap();
        assert_eq!(head.generation, if reserved { 8 } else { 7 });
        assert_eq!(journal.read_publication_visibility(&context(false)).unwrap().unwrap().visible_epoch.get(), 1);
        let mut publisher = ControlSnapshotPublisher::new();
        if !reserved { journal.recover_snapshot_publication(&mut publisher).unwrap(); }
        assert_eq!(reserve(&mut journal, &publisher, &request).unwrap().replayed, reserved);
        assert_eq!(journal.load_unresolved_publication(&context(false)).unwrap(), Some(request.intent().clone()));
    }
}

#[test]
#[ignore = "launched only by the bounded successor process-exit fixture"]
fn crash_child() {
    let Some(path) = std::env::var_os("ELIOT_SUCCESSOR_CRASH_PATH") else { return; };
    let file = OpenOptions::new().read(true).write(true).open(path).unwrap();
    let mut journal = PersistentControlJournal::open(file, identity(), LIMITS).unwrap();
    let request = reserve_request(&journal, 10, 10);
    let mut publisher = ControlSnapshotPublisher::new();
    journal.recover_snapshot_publication(&mut publisher).unwrap();
    let boundary = match std::env::var("ELIOT_SUCCESSOR_CRASH_MODE").unwrap().as_str() {
        "before" => Boundary::ExitBeforeCommit, "after" => Boundary::ExitAfterCommit,
        _ => panic!("invalid successor crash mode"),
    };
    let _ = journal.reserve_successor_checked(&request, &publisher, boundary, &Unscoped);
    panic!("successor crash checkpoint was not reached");
}

#[test]
fn one_deadline_includes_predecessor_inspection_and_reservation_dispatch() {
    use std::cell::Cell;
    use std::time::{Duration, Instant};
    let scratch = Scratch::new(); let (mut journal, publisher) = committed(&scratch);
    let request = reserve_request(&journal, 10, 10);
    let ctx = OperationContext::new(RequestId::from_bytes([1; 16]), 5, Cancel(false),
        OpaqueRef::new("budget:successor-deadline").unwrap()).unwrap();
    let start = Instant::now(); let calls = Cell::new(0_u64);
    let budget = Budget::with_clock(&ctx, || {
        let n = calls.get(); calls.set(n + 1); start + Duration::from_millis(n)
    });
    assert_eq!(journal.reserve_successor_checked(&request, &publisher, Boundary::Normal, &budget),
        Err(ControlError::BudgetExceeded));
    assert!(!journal.requires_recovery());
    assert_eq!(journal.read_publication_intent(&context(false)).unwrap().generation, 7);
}

#[test]
fn a_foreign_disk_publisher_or_stale_local_snapshot_cannot_reserve() {
    let scratch = Scratch::new(); let (mut journal, publisher) = committed(&scratch);
    let request = reserve_request(&journal, 10, 10);
    let foreign_root = Scratch::new();
    let file = OpenOptions::new().create_new(true).read(true).write(true).open(foreign_root.path()).unwrap();
    let foreign_identity = JournalIdentity { data_root_id: DataRootId::from_bytes([88; 16]), ..identity() };
    let mut foreign = PersistentControlJournal::create(file, foreign_identity, LIMITS).unwrap();
    let commit = foreign.initialize_publication_visibility(state(), MutationId([1; 32]), digest(), &context(false)).unwrap();
    let mut foreign_publisher = ControlSnapshotPublisher::new();
    foreign.publish_committed_snapshot(&commit, &mut foreign_publisher).unwrap();
    let before = journal.verify().unwrap();
    assert_eq!(reserve(&mut journal, &foreign_publisher, &request), Err(ControlError::SnapshotPublicationFailed));
    assert_eq!(journal.verify().unwrap(), before);
    journal.transact(&ControlMutation::new(MutationId([40; 32]), digest(), before.generation,
        vec![ControlWrite { key: key(b"technical-marker", LIMITS).unwrap(),
            value: ControlValue::new(ControlRecordClass::State, b"v1".to_vec(), LIMITS).unwrap() }], vec![])).unwrap();
    let current_request = reserve_request(&journal, 10, 10);
    assert_eq!(reserve(&mut journal, &publisher, &current_request), Err(ControlError::SnapshotPublicationFailed));
    assert!(journal.load_unresolved_publication(&context(false)).unwrap().is_none());
}

#[test]
fn owner_handoff_does_not_authorize_new_progress_with_old_owner_guards() {
    let scratch = Scratch::new(); let (journal, publisher) = committed(&scratch);
    let next_owner = JournalIdentity { owner_epoch: OwnerEpoch::new(2).unwrap(), ..identity() };
    let mut journal = journal.advance_owner(next_owner).unwrap();
    let request = reserve_request(&journal, 10, 10);
    let before = journal.verify().unwrap();
    assert_eq!(reserve(&mut journal, &publisher, &request), Err(ControlError::GenerationMismatch));
    assert_eq!(journal.verify().unwrap(), before);
    assert!(!journal.requires_recovery());
}
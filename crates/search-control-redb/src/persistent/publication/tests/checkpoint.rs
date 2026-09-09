//! Real journal reads with synthetic identities; not external-effect proof.
use super::*;
use search_contracts::CollectionGenerationId;

fn initialized(scratch: &Scratch) -> PersistentControlJournal {
    let file = OpenOptions::new().create_new(true).read(true).write(true).open(scratch.path()).unwrap();
    scratch.keep(&file);
    let mut journal = PersistentControlJournal::create(file,
        JournalIdentity { schema_version: PUBLICATION_VISIBILITY_SCHEMA_VERSION, ..identity() }, LIMITS).unwrap();
    let state = PublicationVisibilityState {
        collection_generation_id: CollectionGenerationId::from_bytes([31; 16]),
        schema_identity_digest: Blake3Digest32::from_bytes([32; 32]), visible_epoch: Epoch::new(0).unwrap(),
        guards: prepared().owner_source_membership_access_guards, last_receipt: None,
    };
    journal.initialize_publication_visibility(state, MutationId([21; 32]),
        Blake3Digest32::from_bytes([9; 32]), &context(false)).unwrap();
    journal
}
fn first() -> PublicationIntentUpdate {
    PublicationIntentUpdate::begin(MutationId([1; 32]), Blake3Digest32::from_bytes([9; 32]), 1, prepared()).unwrap()
}

#[test]
fn checkpoint_never_invents_a_route_for_legacy_or_uninitialized_state() {
    for version in [1, 2, 3] {
        let scratch = Scratch::new();
        let file = OpenOptions::new().create_new(true).read(true).write(true).open(scratch.path()).unwrap();
        scratch.keep(&file);
        let journal = PersistentControlJournal::create(file, JournalIdentity { schema_version: version, ..identity() }, LIMITS).unwrap();
        let before = scratch.bytes();
        let result = journal.read_publication_checkpoint(&context(false));
        if version == 3 { assert_eq!(result.unwrap(), None); }
        else { assert_eq!(result.unwrap_err().control_error(), ControlError::SchemaUnsupported); }
        assert_eq!(scratch.bytes(), before);
    }
}

#[test]
fn consumed_epoch_survives_restart_without_becoming_visible() {
    let scratch = Scratch::new(); let mut journal = initialized(&scratch);
    let empty = journal.read_publication_checkpoint(&context(false)).unwrap().unwrap();
    assert_eq!(empty.last_reserved_epoch().get(), 0); assert!(empty.intent().is_none());
    persist(&mut journal, &first());
    let expected = journal.read_publication_checkpoint(&context(false)).unwrap().unwrap();
    assert_eq!(expected.generation(), 2); assert_eq!(expected.visibility().visible_epoch.get(), 0);
    assert_eq!(expected.last_reserved_epoch().get(), 1);
    let identity = journal.identity(); drop(journal);
    let reopened = PersistentControlJournal::open(scratch.file(), identity, LIMITS).unwrap();
    assert_eq!(reopened.read_publication_checkpoint(&context(false)).unwrap().unwrap(), expected);
}

#[test]
fn aborted_state_keeps_consumed_epoch_and_cannot_clear_snapshot_admission_after_restart() {
    let scratch = Scratch::new(); let mut journal = initialized(&scratch);
    let mut update = first(); let mut receipt = persist(&mut journal, &update);
    for (id, state) in [(2, PublicationIntentState::Compensating), (3, PublicationIntentState::Aborted)] {
        update = advance(id, receipt.after_generation, update.intent(), state);
        receipt = persist(&mut journal, &update);
    }
    let identity = journal.identity(); drop(journal);
    let journal = PersistentControlJournal::open(scratch.file(), identity, LIMITS).unwrap();
    let checkpoint = journal.read_publication_checkpoint(&context(false)).unwrap().unwrap();
    assert_eq!(checkpoint.last_reserved_epoch().get(), 1);
    assert_eq!(checkpoint.visibility().visible_epoch.get(), 0);
    assert_eq!(checkpoint.intent().unwrap().state, PublicationIntentState::Aborted);
    assert_eq!(journal.load_unresolved_publication(&context(false)).unwrap().as_ref(), checkpoint.intent());
    let mut publisher = crate::ControlSnapshotPublisher::new();
    assert_eq!(journal.recover_snapshot_publication(&mut publisher), Err(ControlError::SnapshotRebuildFailed));
    assert!(publisher.current().is_none()); assert!(publisher.requires_recovery());
}

#[test]
fn lost_route_or_intent_cannot_reset_the_recovery_floor() {
    for deleted in [KEY, b"collection_route/visibility/v1".as_slice()] {
        let scratch = Scratch::new(); let mut journal = initialized(&scratch);
        persist(&mut journal, &first());
        journal.transact(ControlMutation::new(MutationId([40; 32]), Blake3Digest32::from_bytes([9; 32]), 2,
            vec![], vec![ControlKey::new(deleted.to_vec(), LIMITS).unwrap()])).unwrap();
        let before = scratch.bytes();
        assert!(journal.read_publication_checkpoint(&context(false)).is_err());
        assert_eq!(scratch.bytes(), before);
    }
}

#[test]
fn pending_write_requires_exact_recovery_before_checkpoint_and_handoff_preserves_floor() {
    let scratch = Scratch::new(); let mut journal = initialized(&scratch);
    let update = first();
    assert_eq!(journal.persist_intent_checked(&update, Boundary::LostAcknowledgement, &super::super::super::Unscoped),
        Err(ControlError::CommitOutcomeUnknown));
    assert!(journal.read_publication_checkpoint(&context(false)).is_err());
    assert!(journal.requires_recovery());
    assert!(matches!(journal.recover_publication_intent(&update, &context(false)).unwrap(), CommitRecoveryDecision::Committed(_)));
    let next = JournalIdentity { owner_epoch: OwnerEpoch::new(2).unwrap(), ..journal.identity() };
    let journal = journal.advance_owner(next).unwrap();
    let checkpoint = journal.read_publication_checkpoint(&context(false)).unwrap().unwrap();
    assert_eq!(checkpoint.identity(), next); assert_eq!(checkpoint.last_reserved_epoch().get(), 1);
    assert_eq!(checkpoint.intent().unwrap().owner_source_membership_access_guards.owner_epoch, OwnerEpoch::new(1).unwrap());
}

#[test]
fn checkpoint_is_read_only_redacted_and_one_snapshot_not_two_separate_observations() {
    let scratch = Scratch::new(); let mut journal = initialized(&scratch);
    persist(&mut journal, &first());
    let expected = journal.read_publication_checkpoint(&context(false)).unwrap().unwrap();
    let bytes = scratch.bytes(); let writes = journal.committed_writes();
    let reads = journal.work.snapshot_reads.load(Ordering::Relaxed);
    for _ in 0..10_000 { assert_eq!(journal.read_publication_checkpoint(&context(false)).unwrap().unwrap(), expected); }
    assert_eq!(journal.work.snapshot_reads.load(Ordering::Relaxed) - reads, 10_000);
    assert_eq!(journal.committed_writes(), writes); assert_eq!(scratch.bytes(), bytes);
    assert!(!format!("{expected:?}").contains("manifest-sentinel"));
}

#[test]
fn cancelled_or_expired_checkpoint_never_returns_partial_state() {
    use std::cell::Cell;
    use std::time::{Duration, Instant};
    let scratch = Scratch::new(); let mut journal = initialized(&scratch); persist(&mut journal, &first());
    assert!(journal.read_publication_checkpoint(&context(true)).is_err());
    assert_eq!(journal.read_checkpoint_checked(&StopAt(Point::ReadComplete)), Err(ControlError::ReadCancelled));
    let ctx = OperationContext::new(RequestId::from_bytes([1; 16]), 4, Cancel(false),
        OpaqueRef::new("budget:checkpoint-deadline").unwrap()).unwrap();
    let start = Instant::now(); let calls = Cell::new(0_u64);
    let budget = Budget::with_clock(&ctx, || {
        let n = calls.get(); calls.set(n + 1); start + Duration::from_millis(n)
    });
    assert_eq!(journal.read_checkpoint_checked(&budget), Err(ControlError::BudgetExceeded));
    assert!(!journal.requires_recovery()); assert_eq!(journal.verify().unwrap().generation, 2);
}

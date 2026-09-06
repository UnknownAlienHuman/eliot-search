//! Real redb fixtures; fixed identities/references are synthetic test data only.
use super::*;
use super::super::PublicationIntentUpdate;
use super::super::super::{JournalIdentity, RECORDS, encode_value, Unscoped};
use crate::ControlSnapshotPublisher;
use search_contracts::{DataRootId, InstallationIncarnationId, OpaqueRef, OwnerEpoch, PublicationIntentId, RequestId};
use search_ports::PackageOpaque;
use std::fs::{self, File, OpenOptions};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
mod succession;

const LIMITS: JournalLimits = JournalLimits::BASELINE;
static NEXT: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
struct Cancel(bool);
impl PackageOpaque for Cancel { fn owner_package(&self) -> &'static str { "search-control-redb" } }
impl CancellationProbe for Cancel { fn is_cancelled(&self) -> bool { self.0 } }
fn context(cancel: bool) -> OperationContext<Cancel> {
    OperationContext::new(RequestId::from_bytes([1; 16]), 60_000, Cancel(cancel),
        OpaqueRef::new("budget:visibility-tests").unwrap()).unwrap()
}
fn identity() -> JournalIdentity {
    JournalIdentity { installation_incarnation_id: InstallationIncarnationId::from_bytes([1; 16]),
        data_root_id: DataRootId::from_bytes([2; 16]), owner_epoch: OwnerEpoch::new(1).unwrap(),
        path_identity_digest: Blake3Digest32::from_bytes([3; 32]),
        schema_family_digest: Blake3Digest32::from_bytes([4; 32]), schema_version: PUBLICATION_VISIBILITY_SCHEMA_VERSION }
}
fn state() -> PublicationVisibilityState {
    PublicationVisibilityState { collection_generation_id: CollectionGenerationId::from_bytes([5; 16]),
        schema_identity_digest: Blake3Digest32::from_bytes([6; 32]), visible_epoch: Epoch::new(0).unwrap(),
        guards: PublicationGuards { owner_epoch: OwnerEpoch::new(1).unwrap(), source_catalog_generation: 3,
            membership_generation: 5, access_generation: 7, shadow_generation: 11, purge_generation: 13,
            profile_digest: Blake3Digest32::from_bytes([7; 32]) }, last_receipt: None }
}
fn digest() -> Blake3Digest32 { Blake3Digest32::from_bytes([9; 32]) }
fn reference(name: &str) -> ReceiptRef { ReceiptRef::new(format!("cas:{name}")).unwrap() }
fn member(id: u8) -> ProjectionMembershipId { ProjectionMembershipId::from_bytes([id; 16]) }
fn shadow(n: u64) -> PublicationSourceShadow {
    PublicationSourceShadow { source_revision_id: SourceRevisionId::from_bytes([22; 16]), fence_revision: NonZeroRevision::new(n).unwrap() }
}
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("eliot-visible-{}-{stamp}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&path).unwrap(); Self(path)
    }
    fn path(&self) -> PathBuf { self.0.join("control.redb") }
    fn file(&self) -> File { OpenOptions::new().read(true).write(true).open(self.path()).unwrap() }
    fn empty(&self, version: u32) -> PersistentControlJournal {
        let file = OpenOptions::new().create_new(true).read(true).write(true).open(self.path()).unwrap();
        PersistentControlJournal::create(file, JournalIdentity { schema_version: version, ..identity() }, LIMITS).unwrap()
    }
    fn reopen(&self) -> PersistentControlJournal { PersistentControlJournal::open(self.file(), identity(), LIMITS).unwrap() }
    fn ready(&self) -> (PersistentControlJournal, VisibleEpochCommit) {
        let mut journal = self.empty(PUBLICATION_VISIBILITY_SCHEMA_VERSION);
        journal.initialize_publication_visibility(state(), MutationId([1; 32]), digest(), &context(false)).unwrap();
        journal.transact(ControlMutation::new(MutationId([2; 32]), digest(), 1, vec![
            ControlWrite { key: id_key(MANIFESTS, member(1).as_bytes(), LIMITS).unwrap(), value: codec::manifest(&reference("old"), LIMITS).unwrap() },
            ControlWrite { key: id_key(SHADOWS, member(1).as_bytes(), LIMITS).unwrap(), value: codec::shadow(&shadow(1), LIMITS).unwrap() },
            ControlWrite { key: id_key(SHADOWS, member(2).as_bytes(), LIMITS).unwrap(), value: codec::shadow(&shadow(2), LIMITS).unwrap() },
        ], vec![])).unwrap();
        let prepared = PublicationIntent { publication_intent_id: PublicationIntentId::from_bytes([3; 16]),
            target_epoch: Epoch::new(1).unwrap(), prepared_manifest_ref: reference("prepared-manifest-sentinel"),
            owner_source_membership_access_guards: state().guards, state: PublicationIntentState::Prepared };
        let mut update = PublicationIntentUpdate::begin(MutationId([3; 32]), digest(), 2, prepared).unwrap();
        let mut prior = journal.persist_publication_intent(&update, &context(false)).unwrap();
        for (id, phase) in [(4, PublicationIntentState::NewPointsAcknowledged),
            (5, PublicationIntentState::OldPointsClosedAcknowledged), (6, PublicationIntentState::ReadbackVerified)] {
            update = PublicationIntentUpdate::advance(MutationId([id; 32]), digest(), prior.after_generation, update.intent().clone(), phase).unwrap();
            prior = journal.persist_publication_intent(&update, &context(false)).unwrap();
        }
        let request = make_request(prior, update.intent().clone());
        (journal, request)
    }
}
impl Drop for Scratch { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); } }
fn make_request(prior: ControlCommitReceipt, intent: PublicationIntent) -> VisibleEpochCommit {
    VisibleEpochCommit::new(MutationId([7; 32]), digest(), prior, intent, state(), PublicationReceiptId::from_bytes([7; 16]),
        PublicationReadbackEvidence { exact_new_manifest_ref: reference("new-points"), exact_retired_manifest_ref: reference("retired-points"), readback_digest: digest() },
        vec![PublicationManifestChange { membership_id: member(1), previous_manifest: Some(reference("old")),
            next_manifest: Some(reference("new")), matching_shadow: Some(shadow(1)) }]).unwrap()
}
fn apply(journal: &mut PersistentControlJournal, request: &VisibleEpochCommit) -> Result<ControlCommitReceipt, ControlError> {
    journal.commit_visible_epoch(request, &context(false)).map_err(|error| error.control_error())
}
fn corrupt_value(journal: &PersistentControlJournal, key: ControlKey, value: ControlValue) {
    let write = journal.database.begin_write().unwrap();
    { let mut table = write.open_table(RECORDS).unwrap();
      let bytes = encode_value(&value); table.insert(key.as_bytes(), bytes.as_slice()).unwrap(); }
    write.commit().unwrap();
}

#[test]
fn visibility_manifest_retirement_matching_shadow_and_receipts_commit_together() {
    let scratch = Scratch::new(); let (mut journal, request) = scratch.ready();
    assert!(journal.control_snapshot().is_err());
    let commit = apply(&mut journal, &request).unwrap();
    assert_eq!(commit.before_generation, 6); assert_eq!(commit.after_generation, 7);
    let result = journal.read_publication_visibility(&context(false)).unwrap().unwrap();
    assert_eq!(result.visible_epoch.get(), 1); assert_eq!(result.guards.shadow_generation, 12);
    let snapshot = journal.verify().unwrap();
    assert_eq!(codec::read_manifest(snapshot.get(&id_key(MANIFESTS, member(1).as_bytes(), LIMITS).unwrap()).unwrap()).unwrap(), reference("new"));
    assert_eq!(codec::read_manifest(snapshot.get(&retired_key(request.receipt.publication_receipt_id, member(1), LIMITS).unwrap()).unwrap()).unwrap(), reference("old"));
    assert!(snapshot.get(&id_key(SHADOWS, member(1).as_bytes(), LIMITS).unwrap()).is_none());
    assert_eq!(codec::read_shadow(snapshot.get(&id_key(SHADOWS, member(2).as_bytes(), LIMITS).unwrap()).unwrap()).unwrap(), shadow(2));
    assert!(journal.load_unresolved_publication(&context(false)).unwrap().is_none());
    let mut publisher = ControlSnapshotPublisher::new();
    assert!(publisher.current().is_none()); // durable commit is not snapshot acknowledgement
    journal.publish_committed_snapshot(&commit, &mut publisher).unwrap();
    assert_eq!(publisher.current().unwrap().generation, 7);
}

#[test]
fn all_seven_guard_axes_are_mandatory_and_compared_to_stored_state() {
    for axis in 0..7 {
        let scratch = Scratch::new(); let (mut journal, request) = scratch.ready();
        let mut actual = state();
        match axis {
            0 => actual.guards.owner_epoch = OwnerEpoch::new(2).unwrap(),
            1 => actual.guards.source_catalog_generation += 1,
            2 => actual.guards.membership_generation += 1,
            3 => actual.guards.access_generation += 1,
            4 => actual.guards.shadow_generation += 1,
            5 => actual.guards.purge_generation += 1,
            _ => actual.guards.profile_digest = Blake3Digest32::from_bytes([31; 32]),
        }
        // Simulate same-generation contradiction to discriminate guard checks
        // independently from the separate global-generation compare.
        corrupt_value(&journal, key(STATE, LIMITS).unwrap(), codec::state(&actual, LIMITS).unwrap());
        let before = fs::read(scratch.path()).unwrap();
        assert!(apply(&mut journal, &request).is_err(), "axis {axis}");
        assert_eq!(fs::read(scratch.path()).unwrap(), before);
        assert_eq!(journal.committed_writes(), 7); // create + six fixture commits only
    }
}

#[test]
fn stale_generation_and_forged_prior_receipt_cannot_finalize_publication() {
    let scratch = Scratch::new(); let (mut journal, request) = scratch.ready();
    let mut forged = request.clone(); forged.prior_commit.command_digest = Blake3Digest32::from_bytes([90; 32]);
    assert_eq!(apply(&mut journal, &forged), Err(ControlError::OperationConflict));
    journal.transact(ControlMutation::new(MutationId([20; 32]), digest(), 6, vec![ControlWrite {
        key: key(b"unrelated", LIMITS).unwrap(), value: ControlValue::new(ControlRecordClass::State, b"fixture".to_vec(), LIMITS).unwrap(),
    }], vec![])).unwrap();
    assert_eq!(apply(&mut journal, &request), Err(ControlError::TransactionConflict));
    assert_eq!(journal.read_publication_visibility(&context(false)).unwrap().unwrap().visible_epoch.get(), 0);
}

#[test]
fn newer_or_unexpected_shadow_and_wrong_manifest_preserve_the_whole_batch() {
    for kind in 0..3 {
        let scratch = Scratch::new(); let (mut journal, mut request) = scratch.ready();
        match kind {
            0 => request.changes[0].matching_shadow = Some(shadow(99)),
            1 => request.changes[0].matching_shadow = None,
            _ => request.changes[0].previous_manifest = Some(reference("wrong")),
        }
        let before = journal.verify().unwrap();
        assert_eq!(apply(&mut journal, &request), Err(ControlError::GenerationMismatch));
        assert_eq!(journal.verify().unwrap(), before);
        assert!(!journal.requires_recovery());
    }
}

#[test]
fn duplicate_empty_skipped_or_incomplete_commands_are_rejected_before_dispatch() {
    let scratch = Scratch::new(); let (mut journal, request) = scratch.ready();
    for kind in 0..5 {
        let mut bad = request.clone();
        match kind {
            0 => bad.changes.push(bad.changes[0].clone()),
            1 => bad.changes.clear(),
            2 => bad.intent.target_epoch = Epoch::new(2).unwrap(),
            3 => bad.intent.state = PublicationIntentState::NewPointsAcknowledged,
            _ => bad.changes[0].next_manifest = bad.changes[0].previous_manifest.clone(),
        }
        assert!(apply(&mut journal, &bad).is_err());
    }
    assert_eq!(journal.verify().unwrap().generation, 6);
}

#[test]
fn lost_ack_requires_exact_command_and_survives_reopen_without_second_commit() {
    let scratch = Scratch::new(); let (mut journal, request) = scratch.ready();
    assert_eq!(journal.commit_visibility_checked(&request, Boundary::LostAcknowledgement, &Unscoped), Err(ControlError::CommitOutcomeUnknown));
    assert!(journal.requires_recovery());
    let mut changed = request.clone(); changed.receipt.readback_digest = Blake3Digest32::from_bytes([42; 32]);
    assert!(matches!(journal.recover_visible_epoch_commit(&changed, &context(false)).unwrap(), CommitRecoveryDecision::ConflictingInput));
    assert!(journal.requires_recovery()); drop(journal);
    let mut journal = scratch.reopen();
    assert!(matches!(journal.recover_visible_epoch_commit(&request, &context(false)).unwrap(), CommitRecoveryDecision::Committed(_)));
    assert!(apply(&mut journal, &request).unwrap().replayed);
    assert_eq!(journal.committed_writes(), 0);
    assert_eq!(journal.verify().unwrap().generation, 7);
}

struct StopAt(Point);
impl Check for StopAt {
    fn check(&self, point: Point) -> Result<(), ControlError> {
        if point == self.0 { Err(ControlError::ReadCancelled) } else { Ok(()) }
    }
}
#[test]
fn cancellation_before_and_after_dispatch_has_distinct_recovery_semantics() {
    let scratch = Scratch::new(); let (mut journal, request) = scratch.ready();
    assert!(journal.commit_visible_epoch(&request, &context(true)).is_err());
    assert!(!journal.requires_recovery());
    assert_eq!(journal.commit_visibility_checked(&request, Boundary::Normal, &StopAt(Point::StageRecord)), Err(ControlError::CommitOutcomeUnknown));
    assert!(journal.requires_recovery());
    assert!(journal.recover_visible_epoch_commit(&request, &context(true)).is_err());
    assert!(journal.requires_recovery());
    assert!(matches!(journal.recover_visible_epoch_commit(&request, &context(false)).unwrap(), CommitRecoveryDecision::NotCommittedRetrySameOperation));
    assert_eq!(journal.verify().unwrap().generation, 6);
    apply(&mut journal, &request).unwrap();
}

#[test]
fn publication_failure_keeps_admission_closed_until_exact_committed_snapshot() {
    let scratch = Scratch::new(); let (mut journal, request) = scratch.ready();
    let mut publisher = ControlSnapshotPublisher::new();
    assert!(journal.publish_committed_snapshot(&request.prior_commit, &mut publisher).is_err());
    assert!(publisher.requires_recovery());
    let commit = apply(&mut journal, &request).unwrap();
    assert!(publisher.current().is_none());
    assert!(journal.publish_committed_snapshot_with_context(&commit, &mut publisher, &context(true)).is_err());
    assert!(publisher.current().is_none());
    journal.recover_snapshot_publication(&mut publisher).unwrap();
    assert_eq!(publisher.current().unwrap().generation, 7);
}

#[test]
fn missing_receipt_or_visibility_and_fake_committed_intent_fail_reopen() {
    for kind in 0..3 {
        let scratch = Scratch::new(); let (mut journal, request) = scratch.ready();
        if kind < 2 {
            apply(&mut journal, &request).unwrap();
            let removed = if kind == 0 { key(STATE, LIMITS).unwrap() }
                else { id_key(RECEIPTS, request.receipt.publication_receipt_id.as_bytes(), LIMITS).unwrap() };
            journal.transact(ControlMutation::new(MutationId([21; 32]), digest(), 7, vec![], vec![removed])).unwrap();
        } else {
            let mut committed = request.intent.clone(); committed.state = PublicationIntentState::ControlCommitted;
            corrupt_value(&journal, key(super::super::KEY, LIMITS).unwrap(), intent_codec::encode(&committed, LIMITS).unwrap());
        }
        assert!(journal.read_publication_visibility(&context(false)).is_err());
        assert!(journal.load_unresolved_publication(&context(false)).is_err());
        drop(journal);
        assert!(PersistentControlJournal::open(scratch.file(), identity(), LIMITS).is_err());
    }
}

#[test]
fn older_owner_cannot_finalize_but_historical_replay_survives_verified_handoff() {
    for already_committed in [false, true] {
        let scratch = Scratch::new(); let (mut journal, request) = scratch.ready();
        if already_committed { apply(&mut journal, &request).unwrap(); }
        let mut journal = journal.advance_owner(JournalIdentity { owner_epoch: OwnerEpoch::new(2).unwrap(), ..identity() }).unwrap();
        if already_committed { assert!(apply(&mut journal, &request).unwrap().replayed); }
        else { assert_eq!(apply(&mut journal, &request), Err(ControlError::GenerationMismatch)); }
    }
}

#[test]
fn strict_visibility_codecs_reject_every_truncation_trailing_bytes_and_wrong_class() {
    let scratch = Scratch::new(); let (_journal, request) = scratch.ready();
    let values = [codec::state(&state(), LIMITS).unwrap(), codec::receipt(&request, LIMITS).unwrap(),
        codec::manifest(&reference("valid"), LIMITS).unwrap(), codec::shadow(&shadow(1), LIMITS).unwrap()];
    for (kind, value) in values.iter().enumerate() {
        let valid = |candidate: &ControlValue| match kind {
            0 => codec::read_state(candidate).is_ok(), 1 => codec::read_receipt(candidate).is_ok(),
            2 => codec::read_manifest(candidate).is_ok(), _ => codec::read_shadow(candidate).is_ok(),
        };
        assert!(valid(value));
        for n in 1..value.len() {
            assert!(!valid(&ControlValue::new(value.class(), value.as_bytes()[..n].to_vec(), LIMITS).unwrap()));
        }
        let mut extra = value.as_bytes().to_vec(); extra.push(0);
        assert!(!valid(&ControlValue::new(value.class(), extra, LIMITS).unwrap()));
        assert!(!valid(&ControlValue::new(ControlRecordClass::Identity, value.as_bytes().to_vec(), LIMITS).unwrap()));
    }
}

#[test]
fn schema_three_is_explicit_and_previous_schemas_are_not_rewritten() {
    for version in [1, 2] {
        let scratch = Scratch::new(); let mut journal = scratch.empty(version);
        let before = fs::read(scratch.path()).unwrap();
        assert_eq!(journal.initialize_publication_visibility(state(), MutationId([1; 32]), digest(), &context(false))
            .unwrap_err().control_error(), ControlError::SchemaUnsupported);
        assert!(!journal.quarantined); assert_eq!(fs::read(scratch.path()).unwrap(), before);
        assert_eq!(journal.verify().unwrap().generation, 0);
    }
}

#[test]
fn write_budget_and_shadow_counter_exhaustion_do_not_partially_publish() {
    let scratch = Scratch::new(); let (mut journal, request) = scratch.ready();
    journal.limits.max_mutation_items = 10;
    assert_eq!(apply(&mut journal, &request), Err(ControlError::BudgetExceeded));
    assert!(!journal.requires_recovery()); journal.limits = LIMITS;
    let mut exhausted = request.clone(); exhausted.previous.guards.shadow_generation = u64::MAX;
    exhausted.intent.owner_source_membership_access_guards.shadow_generation = u64::MAX;
    assert_eq!(apply(&mut journal, &exhausted), Err(ControlError::GenerationExhausted));
    assert_eq!(journal.verify().unwrap().generation, 6);
}

#[test]
fn visibility_reads_are_nonmutating_and_command_debug_redacts_refs() {
    let scratch = Scratch::new(); let (mut journal, request) = scratch.ready(); apply(&mut journal, &request).unwrap();
    let before = fs::read(scratch.path()).unwrap(); let writes = journal.committed_writes();
    for _ in 0..10_000 { assert_eq!(journal.read_publication_visibility(&context(false)).unwrap().unwrap().visible_epoch.get(), 1); }
    assert_eq!(fs::read(scratch.path()).unwrap(), before); assert_eq!(journal.committed_writes(), writes);
    assert!(!format!("{request:?} {:?}", request.changes).contains("cas:"));
}

#[test]
fn process_exit_at_visibility_boundary_recovers_exact_prior_or_complete_new_state() {
    use std::process::Command; use std::time::{Duration, Instant};
    for (mode, code, committed) in [("before", 73, false), ("after", 74, true)] {
        let scratch = Scratch::new(); let (journal, request) = scratch.ready(); drop(journal);
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args(["--ignored", "--exact", "persistent::publication::visibility::tests::crash_child", "--nocapture"])
            .env("ELIOT_VISIBILITY_CRASH_PATH", scratch.path()).env("ELIOT_VISIBILITY_CRASH_MODE", mode).spawn().unwrap();
        let deadline = Instant::now() + Duration::from_secs(30);
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() { break status; }
            if Instant::now() >= deadline { let _ = child.kill(); let _ = child.wait(); panic!("visibility child timed out"); }
            std::thread::sleep(Duration::from_millis(10));
        };
        assert_eq!(status.code(), Some(code));
        let mut journal = scratch.reopen();
        assert_eq!(journal.read_publication_visibility(&context(false)).unwrap().unwrap().visible_epoch.get(), i64::from(committed));
        assert_eq!(apply(&mut journal, &request).unwrap().replayed, committed);
        assert_eq!(journal.verify().unwrap().generation, 7);
    }
}
#[test]
#[ignore = "invoked by the bounded process-exit fixture"]
fn crash_child() {
    let Some(path) = std::env::var_os("ELIOT_VISIBILITY_CRASH_PATH") else { return; };
    let file = OpenOptions::new().read(true).write(true).open(path).unwrap();
    let mut journal = PersistentControlJournal::open(file, identity(), LIMITS).unwrap();
    let intent = journal.load_unresolved_publication(&context(false)).unwrap().unwrap();
    let prior = { let read = journal.database.begin_read().unwrap(); let header = journal.header_from(&read).unwrap();
        operation_from(&read, MutationId([6; 32]), &header, LIMITS).unwrap().unwrap().receipt };
    let request = make_request(prior, intent);
    let boundary = match std::env::var("ELIOT_VISIBILITY_CRASH_MODE").unwrap().as_str() {
        "before" => Boundary::ExitBeforeCommit, "after" => Boundary::ExitAfterCommit, _ => panic!("invalid mode"),
    };
    let _ = journal.commit_visibility_checked(&request, boundary, &Unscoped);
    panic!("expected crash boundary");
}

#[test]
fn visibility_state_encoding_matches_independent_golden_bytes() {
    let hex = "454c49564953303105050505050505050505050505050505060606060606060606060606060606060606060606060606060606060606060600000000000000000000000000000001000000000000000300000000000000050000000000000007000000000000000b000000000000000d070707070707070707070707070707070707070707070707070707070707070700";
    let expected = (0..hex.len()).step_by(2).map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap()).collect::<Vec<_>>();
    assert_eq!(codec::state(&state(), LIMITS).unwrap().as_bytes(), expected.as_slice());
    assert_eq!(expected.len(), 145);
}

#[test]
fn an_intent_cannot_reserve_a_skipped_epoch_or_echo_stale_guards() {
    for bad_epoch in [false, true] {
        let scratch = Scratch::new(); let mut journal = scratch.empty(PUBLICATION_VISIBILITY_SCHEMA_VERSION);
        journal.initialize_publication_visibility(state(), MutationId([1; 32]), digest(), &context(false)).unwrap();
        let mut guards = state().guards;
        if !bad_epoch { guards.access_generation += 1; }
        let prepared = PublicationIntent { publication_intent_id: PublicationIntentId::from_bytes([3; 16]),
            target_epoch: Epoch::new(if bad_epoch { 2 } else { 1 }).unwrap(),
            prepared_manifest_ref: reference("prepared"), owner_source_membership_access_guards: guards,
            state: PublicationIntentState::Prepared };
        let update = PublicationIntentUpdate::begin(MutationId([2; 32]), digest(), 1, prepared).unwrap();
        assert_eq!(journal.persist_publication_intent(&update, &context(false)).unwrap_err().control_error(), ControlError::GenerationMismatch);
        assert_eq!(journal.verify().unwrap().generation, 1);
    }
}

#[test]
fn interruption_after_durable_commit_recovers_without_reapplying_old_shadows() {
    let scratch = Scratch::new(); let (mut journal, request) = scratch.ready();
    assert_eq!(journal.commit_visibility_checked(&request, Boundary::Normal, &StopAt(Point::AfterCommit)), Err(ControlError::CommitOutcomeUnknown));
    assert!(journal.requires_recovery());
    assert!(matches!(journal.recover_visible_epoch_commit(&request, &context(false)).unwrap(), CommitRecoveryDecision::Committed(_)));
    assert_eq!(journal.read_publication_visibility(&context(false)).unwrap().unwrap().guards.shadow_generation, 12);
    assert!(apply(&mut journal, &request).unwrap().replayed);
    assert_eq!(journal.read_publication_visibility(&context(false)).unwrap().unwrap().guards.shadow_generation, 12);
}

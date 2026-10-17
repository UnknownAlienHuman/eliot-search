//! Real redb fixtures; fixed identities/references are synthetic test data only.
use super::*;
use super::super::PublicationIntentUpdate;
use super::super::super::{JournalIdentity, RECORDS, encode_value, Unscoped};
use crate::policy_codec::AccessPolicyRecord;
use crate::security_restriction::{MAX_RESTRICTION_DEPENDENTS, SecurityPolicyState};
use crate::{
    ControlSnapshotPublisher, RegistryMutationRequest,
    RegistryStateChange, RegistryStateFamily, RegistryStateKey, control_mutation_identity,
};
use search_contracts::{AccessPolicyRevision, BoundedSet, DataRootId, InstallationIncarnationId,
    OpaqueId, OpaqueRef, OwnerEpoch, PublicationIntentId, PurgeFenceRevision, RequestId,
    ShadowFenceRevision, SourceNamespaceId, SourceOwnerGeneration};
use search_ports::PackageOpaque;
use std::cell::{Cell, RefCell};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
mod succession;
mod projection_snapshot;
mod prepared_artifacts;
mod preparation_input;
mod abandonment;

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
fn identity_at_owner(version: u32, owner_epoch: OwnerEpoch) -> JournalIdentity {
    JournalIdentity { schema_version: version, owner_epoch, ..identity() }
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
fn current_owner_security_state(namespace: SourceNamespaceId) -> SecurityPolicyState {
    SecurityPolicyState {
        policy: AccessPolicyRecord {
            namespace_id: namespace,
            owner_generation: SourceOwnerGeneration::from_bytes([23; 32]),
            policy_revision: AccessPolicyRevision::new(11),
            live_deny_generation: 7,
            shadow_fence_revision: ShadowFenceRevision::new(12),
            purge_fence_revision: PurgeFenceRevision::new(13),
            policy_digest: Blake3Digest32::from_bytes([24; 32]),
        },
        security_domain_ref: OpaqueRef::new("domain:c01-guard-refresh").unwrap(),
        snapshot_digest: Blake3Digest32::from_bytes([25; 32]),
        denied_memberships: BoundedSet::empty(),
        purged_memberships: BoundedSet::empty(),
        fail_closed: false,
    }
}
fn current_owner_security_owners() -> BoundedSet<OpaqueId, MAX_RESTRICTION_DEPENDENTS> {
    BoundedSet::from_items([OpaqueId::new("owner:c01-guard-refresh").unwrap()]).unwrap()
}
struct Scratch {
    root: PathBuf,
    handle: RefCell<Option<File>>,
}
impl Scratch {
    fn keep(&self, file: &File) { *self.handle.borrow_mut() = Some(file.try_clone().unwrap()); }
    /// Exact database bytes, read through a handle duplicated from the one redb
    /// owns. redb holds an exclusive byte-range lock for the life of the database;
    /// on Windows an unrelated `fs::read` of the same path fails with
    /// `ERROR_LOCK_VIOLATION`. A duplicated handle shares that lock ownership, so
    /// this reads the same bytes without unlocking, dropping or reopening.
    fn bytes(&self) -> Vec<u8> {
        let mut guard = self.handle.borrow_mut();
        let file = guard.as_mut().expect("scratch file handle");
        file.seek(SeekFrom::Start(0)).unwrap();
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer).unwrap();
        buffer
    }
    fn new() -> Self {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("eliot-visible-{}-{stamp}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&path).unwrap(); Self { root: path, handle: RefCell::new(None) }
    }
    fn path(&self) -> PathBuf { self.root.join("control.redb") }
    fn file(&self) -> File { OpenOptions::new().read(true).write(true).open(self.path()).unwrap() }
    fn empty(&self, version: u32) -> PersistentControlJournal {
        self.empty_with_identity(JournalIdentity { schema_version: version, ..identity() })
    }
    fn empty_with_identity(&self, identity: JournalIdentity) -> PersistentControlJournal {
        let file = OpenOptions::new().create_new(true).read(true).write(true).open(self.path()).unwrap();
        self.keep(&file);
        PersistentControlJournal::create(file, identity, LIMITS).unwrap()
    }
    fn reopen(&self) -> PersistentControlJournal { PersistentControlJournal::open(self.file(), identity(), LIMITS).unwrap() }
    fn ready(&self) -> (PersistentControlJournal, VisibleEpochCommit) {
        self.ready_for_owner(OwnerEpoch::new(1).unwrap())
    }
    fn ready_for_owner(&self, owner_epoch: OwnerEpoch) -> (PersistentControlJournal, VisibleEpochCommit) {
        self.ready_for_owner_with_schema(PUBLICATION_VISIBILITY_SCHEMA_VERSION, owner_epoch)
    }
    fn ready_for_owner_with_schema(
        &self,
        schema_version: u32,
        owner_epoch: OwnerEpoch,
    ) -> (PersistentControlJournal, VisibleEpochCommit) {
        let mut initial = state();
        initial.guards.owner_epoch = owner_epoch;
        let mut journal = self.empty_with_identity(identity_at_owner(schema_version, owner_epoch));
        if schema_version == crate::REGISTRY_JOURNAL_SCHEMA_VERSION {
            journal.initialize_operating_profile(
                crate::JournalProfileInitialization::Indexed(initial),
                MutationId([1; 32]),
                digest(),
                &context(false),
            ).unwrap();
        } else {
            journal.initialize_publication_visibility(initial, MutationId([1; 32]), digest(), &context(false)).unwrap();
        }
        journal.transact(&ControlMutation::new(MutationId([2; 32]), digest(), 1, vec![
            ControlWrite { key: id_key(MANIFESTS, member(1).as_bytes(), LIMITS).unwrap(), value: codec::manifest(&reference("old"), LIMITS).unwrap() },
            ControlWrite { key: id_key(SHADOWS, member(1).as_bytes(), LIMITS).unwrap(), value: codec::shadow(&shadow(1), LIMITS).unwrap() },
            ControlWrite { key: id_key(SHADOWS, member(2).as_bytes(), LIMITS).unwrap(), value: codec::shadow(&shadow(2), LIMITS).unwrap() },
        ], vec![])).unwrap();
        let prepared = PublicationIntent { publication_intent_id: PublicationIntentId::from_bytes([3; 16]),
            target_epoch: Epoch::new(1).unwrap(), prepared_manifest_ref: reference("prepared-manifest-sentinel"),
            owner_source_membership_access_guards: initial.guards, state: PublicationIntentState::Prepared };
        let mut update = PublicationIntentUpdate::begin(MutationId([3; 32]), digest(), 2, &prepared).unwrap();
        let mut prior = journal.persist_publication_intent(&update, &context(false)).unwrap();
        for (id, phase) in [(4, PublicationIntentState::NewPointsAcknowledged),
            (5, PublicationIntentState::OldPointsClosedAcknowledged), (6, PublicationIntentState::ReadbackVerified)] {
            update = PublicationIntentUpdate::advance(MutationId([id; 32]), digest(), prior.after_generation, update.intent().clone(), phase).unwrap();
            prior = journal.persist_publication_intent(&update, &context(false)).unwrap();
        }
        let request = make_request_with_previous(prior, update.intent().clone(), initial);
        (journal, request)
    }
}
impl Drop for Scratch { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.root); } }
fn make_request(prior: ControlCommitReceipt, intent: PublicationIntent) -> VisibleEpochCommit {
    make_request_with_previous(prior, intent, state())
}
fn make_request_with_previous(
    prior: ControlCommitReceipt,
    intent: PublicationIntent,
    previous: PublicationVisibilityState,
) -> VisibleEpochCommit {
    VisibleEpochCommit::new(MutationId([7; 32]), digest(), prior, intent, previous, PublicationReceiptId::from_bytes([7; 16]),
        PublicationReadbackEvidence { exact_new_manifest_ref: reference("new-points"), exact_retired_manifest_ref: reference("retired-points"), readback_digest: digest() },
        vec![PublicationManifestChange { membership_id: member(1), previous_manifest: Some(reference("old")),
            next_manifest: Some(reference("new")), matching_shadow: Some(shadow(1)) }]).unwrap()
}
fn apply(journal: &mut PersistentControlJournal, request: &VisibleEpochCommit) -> Result<ControlCommitReceipt, ControlError> {
    journal.commit_visible_epoch(request, &context(false)).map_err(crate::persistent::operation::ControlCallError::control_error)
}
fn corrupt_value(journal: &PersistentControlJournal, key: &ControlKey, value: &ControlValue) {
    let write = journal.database.begin_write().unwrap();
    { let mut table = write.open_table(RECORDS).unwrap();
      let bytes = encode_value(value); table.insert(key.as_bytes(), bytes.as_slice()).unwrap(); }
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
        corrupt_value(&journal, &key(STATE, LIMITS).unwrap(), &codec::state(&actual, LIMITS).unwrap());
        let before = scratch.bytes();
        assert!(apply(&mut journal, &request).is_err(), "axis {axis}");
        assert_eq!(scratch.bytes(), before);
        assert_eq!(journal.committed_writes(), 7); // create + six fixture commits only
    }
}

#[test]
fn stale_generation_and_forged_prior_receipt_cannot_finalize_publication() {
    let scratch = Scratch::new(); let (mut journal, request) = scratch.ready();
    let mut forged = request.clone(); forged.prior_commit.command_digest = Blake3Digest32::from_bytes([90; 32]);
    assert_eq!(apply(&mut journal, &forged), Err(ControlError::OperationConflict));
    journal.transact(&ControlMutation::new(MutationId([20; 32]), digest(), 6, vec![ControlWrite {
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
            journal.transact(&ControlMutation::new(MutationId([21; 32]), digest(), 7, vec![], vec![removed])).unwrap();
        } else {
            let mut committed = request.intent.clone(); committed.state = PublicationIntentState::ControlCommitted;
            corrupt_value(&journal, &key(super::super::KEY, LIMITS).unwrap(), &intent_codec::encode(&committed, LIMITS).unwrap());
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
        let before = scratch.bytes();
        assert_eq!(journal.initialize_publication_visibility(state(), MutationId([1; 32]), digest(), &context(false))
            .unwrap_err().control_error(), ControlError::SchemaUnsupported);
        assert!(!journal.quarantined); assert_eq!(scratch.bytes(), before);
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
    let before = scratch.bytes(); let writes = journal.committed_writes();
    for _ in 0..10_000 { assert_eq!(journal.read_publication_visibility(&context(false)).unwrap().unwrap().visible_epoch.get(), 1); }
    assert_eq!(scratch.bytes(), before); assert_eq!(journal.committed_writes(), writes);
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
        let update = PublicationIntentUpdate::begin(MutationId([2; 32]), digest(), 1, &prepared).unwrap();
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

#[test]
#[allow(clippy::too_many_lines)] // One finite guard-refresh, receipt-preservation and abandonment case.
fn publication_guard_image_regression_refresh_then_abandonment_preserves_receipt() {
    use std::time::{Duration, Instant};

    let mut case = guard_image_refresh_case();
    let deadline = Instant::now() + Duration::from_secs(30);
    let work = |_unit: crate::MigrationBootstrapWork| Ok::<(), ()>(());
    let mutation_identity = control_mutation_identity(case.request.operation_id()).unwrap();
    let readback = {
        let mut bound = case
            .journal
            .bind_control_port_with_original_context::<Cancel, _>(
                &mut case.publisher,
                deadline,
                &work,
            )
            .unwrap();
        bound
            .transact_publication_guard_image_with_original_context(
                &case.request,
                &mutation_identity,
                deadline,
                &work,
            )
            .unwrap()
    };
    let committed = readback.checkpoint();
    assert_eq!(
        committed.generation(),
        case.request.expected_generation() + 1
    );
    assert_eq!(committed.identity(), case.next_identity);
    assert_eq!(
        committed.visibility().guards.owner_epoch,
        case.next_identity.owner_epoch
    );
    assert_eq!(
        committed.visibility().visible_epoch,
        case.before.visible_epoch
    );
    assert_eq!(committed.visibility().last_receipt, Some(case.old_receipt));
    assert_eq!(
        committed.visibility().collection_generation_id,
        case.before.collection_generation_id
    );
    assert_eq!(
        committed.visibility().schema_identity_digest,
        case.before.schema_identity_digest
    );
    assert_eq!(committed.visibility().guards.source_catalog_generation, 17);
    assert_eq!(committed.visibility().guards.membership_generation, 29);

    case.journal
        .recover_snapshot_publication(&mut case.publisher)
        .unwrap()
        .unwrap();
    let previous_visibility = case
        .journal
        .read_publication_visibility(&context(false))
        .unwrap()
        .unwrap();
    let intent =
        abandonment::durable_next_intent(&mut case.journal, &case.publisher, previous_visibility);
    let fence_request = crate::PublicationAbandonFenceRequestV1::new(
        OpaqueId::new("operation:guard-image-abandon").unwrap(),
        MutationId([92; 32]),
        digest(),
        case.journal.verify().unwrap().generation,
        RequestId::from_bytes([93; 16]),
        60_000,
        OpaqueRef::new("budget:guard-image-abandon").unwrap(),
        intent,
        previous_visibility,
        vec![crate::PublicationAbandonFenceMembershipV1 {
            membership_id: member(2),
            source_revision_id: SourceRevisionId::from_bytes([22; 16]),
            previous_shadow: Some(shadow(10)),
        }],
    )
    .unwrap();
    case.journal
        .abandon_publication_checked(
            &fence_request,
            super::super::super::Boundary::Normal,
            &Unscoped,
        )
        .unwrap();
    let fenced = case
        .journal
        .read_checkpoint_checked(&Unscoped)
        .unwrap()
        .unwrap();
    assert_eq!(fenced.visibility().last_receipt, Some(case.old_receipt));
    assert_eq!(
        fenced.publication_receipt().unwrap().publication_receipt_id,
        case.old_receipt
    );
    assert_eq!(
        fenced.visibility().guards.shadow_generation,
        previous_visibility.guards.shadow_generation + 1
    );
    assert_eq!(
        fenced.abandonment_fence().unwrap().previous_visibility(),
        &previous_visibility
    );
}

struct GuardImageRefreshCase {
    journal: PersistentControlJournal,
    publisher: ControlSnapshotPublisher,
    request: PublicationGuardImageRequestV1,
    before: PublicationVisibilityState,
    next_identity: JournalIdentity,
    old_receipt: PublicationReceiptId,
    _scratch: Scratch,
}

fn guard_image_refresh_case() -> GuardImageRefreshCase {
    use std::time::{Duration, Instant};

    let scratch = Scratch::new();
    let old_epoch = OwnerEpoch::new(3).unwrap();
    let current_epoch = OwnerEpoch::new(4).unwrap();
    let (mut journal, visibility_request) =
        abandonment::ready_with_original_binding_for_owner(
            &scratch,
            crate::REGISTRY_JOURNAL_SCHEMA_VERSION,
            old_epoch,
        );
    apply(&mut journal, &visibility_request).unwrap();
    let old_visibility = journal.read_publication_checkpoint(&context(false)).unwrap().unwrap();
    let old_receipt = old_visibility.visibility().last_receipt.unwrap();
    let next_identity = identity_at_owner(crate::REGISTRY_JOURNAL_SCHEMA_VERSION, current_epoch);
    let mut journal = journal.advance_owner(next_identity).unwrap();
    let current_checkpoint = journal.read_publication_checkpoint(&context(false)).unwrap().unwrap();
    let previous = *current_checkpoint.visibility();
    assert_eq!(previous.guards.owner_epoch, old_epoch);
    assert_eq!(current_checkpoint.identity().owner_epoch, next_identity.owner_epoch);

    let registry_request = RegistryMutationRequest::new(
        OpaqueId::new("registry:c01-member-setup").unwrap(),
        current_checkpoint.generation(),
        0,
        digest(),
        b"owner:c01-member-setup".to_vec(),
        vec![
            RegistryStateChange::new(
                RegistryStateKey::new(RegistryStateFamily::Source, b"source".to_vec(), LIMITS).unwrap(),
                None,
                b"source-state".to_vec(),
                LIMITS,
            ).unwrap(),
            RegistryStateChange::new(
                RegistryStateKey::new(RegistryStateFamily::Membership, b"membership".to_vec(), LIMITS).unwrap(),
                None,
                b"membership-state".to_vec(),
                LIMITS,
            ).unwrap(),
        ],
        LIMITS,
    ).unwrap();
    journal.commit_registry_batch(&registry_request, &context(false)).unwrap();

    let namespace = SourceNamespaceId::from_bytes([26; 16]);
    let security_before = journal.read_security_restriction(namespace, &context(false)).unwrap();
    let security_mutation = security_before.initialize(
        OpaqueId::new("security:c01-current-owner").unwrap(),
        digest(),
        current_owner_security_state(namespace),
        current_owner_security_owners(),
    ).unwrap();
    journal.commit_security_restriction(&security_mutation, &context(false)).unwrap();

    let registry = journal.read_registry_snapshot(&context(false)).unwrap();
    let security = journal.read_security_restriction(namespace, &context(false)).unwrap();
    let checkpoint = journal.read_publication_checkpoint(&context(false)).unwrap().unwrap();
    assert_eq!(registry.identity(), next_identity);
    assert_eq!(security.identity(), next_identity);
    assert_eq!(checkpoint.identity(), next_identity);
    assert_eq!(checkpoint.visibility().guards.owner_epoch, old_epoch);

    let mut next_guards = previous.guards;
    next_guards.owner_epoch = current_epoch;
    next_guards.source_catalog_generation = 17;
    next_guards.membership_generation = 29;
    let deadline = Instant::now() + Duration::from_secs(30);
    let work = |_unit: crate::MigrationBootstrapWork| Ok::<(), ()>(());
    let mut publisher = ControlSnapshotPublisher::new();
    let request = {
        let bound = journal
            .bind_control_port_with_original_context::<Cancel, _>(&mut publisher, deadline, &work)
            .unwrap();
        bound
            .prepare_publication_guard_image_with_original_context(
                &checkpoint,
                &registry,
                std::slice::from_ref(&security),
                next_guards,
                MutationId([91; 32]),
                digest(),
                deadline,
                &work,
            )
            .unwrap()
    };
    GuardImageRefreshCase {
        journal,
        publisher,
        request,
        before: previous,
        next_identity,
        old_receipt,
        _scratch: scratch,
    }
}

#[test]
fn publication_guard_image_regression_late_refusal_is_unknown_with_context_cause() {
    use std::time::{Duration, Instant};

    let mut baseline = guard_image_refresh_case();
    let baseline_checkpoints = Cell::new(0_usize);
    let count_checkpoints = |_: crate::MigrationBootstrapWork| {
        baseline_checkpoints.set(baseline_checkpoints.get() + 1);
        Ok::<(), &'static str>(())
    };
    let identity = control_mutation_identity(baseline.request.operation_id()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    {
        let mut bound = baseline
            .journal
            .bind_control_port_with_original_context::<Cancel, _>(
                &mut baseline.publisher,
                deadline,
                &count_checkpoints,
            )
            .unwrap();
        bound
            .transact_publication_guard_image_with_original_context(
                &baseline.request,
                &identity,
                deadline,
                &count_checkpoints,
            )
            .unwrap();
    }
    let refuse_on = baseline_checkpoints.get();
    assert!(refuse_on > 0);

    let mut target = guard_image_refresh_case();
    let observed_checkpoints = Cell::new(0_usize);
    let refusal = "late guard-image context refusal";
    let refuse_at_final_check = |_: crate::MigrationBootstrapWork| {
        let current = observed_checkpoints.get() + 1;
        observed_checkpoints.set(current);
        if current == refuse_on {
            Err(refusal)
        } else {
            Ok(())
        }
    };
    let identity = control_mutation_identity(target.request.operation_id()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    let error = {
        let mut bound = target
            .journal
            .bind_control_port_with_original_context::<Cancel, _>(
                &mut target.publisher,
                deadline,
                &refuse_at_final_check,
            )
            .unwrap();
        bound
            .transact_publication_guard_image_with_original_context(
                &target.request,
                &identity,
                deadline,
                &refuse_at_final_check,
            )
            .unwrap_err()
    };
    assert_eq!(error.control, ControlError::CommitOutcomeUnknown);
    assert_eq!(error.first_context, Some(refusal));
    assert_eq!(observed_checkpoints.get(), refuse_on);
    let checkpoint = target
        .journal
        .read_checkpoint_checked(&Unscoped)
        .unwrap()
        .unwrap();
    assert_eq!(
        checkpoint.generation(),
        target.request.expected_generation() + 1
    );
    assert_eq!(checkpoint.visibility(), target.request.next_visibility());
}

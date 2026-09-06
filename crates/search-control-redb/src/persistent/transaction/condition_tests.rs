//! Conditions exercise the real disk engine. Synthetic keys/values are technical
//! fixture data, not an authorization or full production-schema qualification.

use super::*;
use crate::{ConditionalControlMutation, ControlRecordClass, ControlValue, ControlWrite,
    CommitRecoveryDecision, JournalIdentity, JournalLimits, MutationId};
use crate::persistent::operation::{Budget, Unscoped};
use search_contracts::{Blake3Digest32, DataRootId, InstallationIncarnationId,
    OpaqueRef, OwnerEpoch, RequestId};
use search_ports::{CancellationProbe, OperationContext, PackageOpaque, PortErrorKind, PortRetryability};
use std::cell::Cell;
use std::fs::{self, File, OpenOptions};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const LIMITS: JournalLimits = JournalLimits::BASELINE;
static NEXT: AtomicU64 = AtomicU64::new(0);

fn identity() -> JournalIdentity {
    JournalIdentity {
        installation_incarnation_id: InstallationIncarnationId::from_bytes([1; 16]),
        data_root_id: DataRootId::from_bytes([2; 16]),
        owner_epoch: OwnerEpoch::new(1).unwrap(),
        path_identity_digest: Blake3Digest32::from_bytes([3; 32]),
        schema_family_digest: Blake3Digest32::from_bytes([4; 32]),
        schema_version: 1,
    }
}

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("eliot-control-conditions-{}-{timestamp}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn path(&self) -> PathBuf { self.0.join("control.redb") }
    fn file(&self) -> File { OpenOptions::new().read(true).write(true).open(self.path()).unwrap() }
    fn create(&self) -> PersistentControlJournal {
        let file = OpenOptions::new().create_new(true).read(true).write(true).open(self.path()).unwrap();
        PersistentControlJournal::create(file, identity(), LIMITS).unwrap()
    }
    fn open(&self) -> PersistentControlJournal {
        PersistentControlJournal::open(self.file(), identity(), LIMITS).unwrap()
    }
}
impl Drop for Scratch {
    fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); }
}

fn key(bytes: &[u8]) -> ControlKey { ControlKey::new(bytes.to_vec(), LIMITS).unwrap() }
fn value(bytes: &[u8]) -> ControlValue {
    ControlValue::new(ControlRecordClass::State, bytes.to_vec(), LIMITS).unwrap()
}
fn put(id: u8, generation: u64, name: &[u8], bytes: &[u8]) -> ControlMutation {
    ControlMutation::new(MutationId([id; 32]), Blake3Digest32::from_bytes([9; 32]), generation,
        vec![ControlWrite { key: key(name), value: value(bytes) }], vec![])
}
fn absent(name: &[u8]) -> ControlRecordCondition { ControlRecordCondition::absent(key(name)) }
fn exact(name: &[u8], bytes: &[u8]) -> ControlRecordCondition {
    ControlRecordCondition::exact(key(name), value(bytes))
}
fn command(mutation: ControlMutation, conditions: Vec<ControlRecordCondition>) -> ConditionalControlMutation {
    ConditionalControlMutation::new(mutation, conditions)
}

#[derive(Debug)]
struct Cancellation(bool);
impl PackageOpaque for Cancellation {
    fn owner_package(&self) -> &'static str { "search-control-redb" }
}
impl CancellationProbe for Cancellation {
    fn is_cancelled(&self) -> bool { self.0 }
}
fn context(cancelled: bool) -> OperationContext<Cancellation> {
    OperationContext::new(RequestId::from_bytes([7; 16]), 10_000, Cancellation(cancelled),
        OpaqueRef::new("budget:conditions-test").unwrap()).unwrap()
}

fn apply(journal: &mut PersistentControlJournal, request: ConditionalControlMutation) -> Result<ControlCommitReceipt, ControlError> {
    journal.transact_conditionally(request, &context(false)).map_err(|error| error.control_error())
}
fn recover(journal: &mut PersistentControlJournal, request: &ConditionalControlMutation) -> CommitRecoveryDecision {
    journal.recover_conditional_transaction(request, &context(false)).unwrap()
}

#[test]
fn conditional_insert_then_update_is_atomic_and_replay_does_not_recheck_prestate() {
    let scratch = Scratch::new();
    let mut journal = scratch.create();
    let first = command(put(1, 0, b"state", b"READY"), vec![absent(b"state")]);
    assert!(!apply(&mut journal, first.clone()).unwrap().replayed);
    assert!(apply(&mut journal, first).unwrap().replayed);
    let second = command(put(2, 1, b"state", b"STOPPED"), vec![exact(b"state", b"READY")]);
    apply(&mut journal, second.clone()).unwrap();
    assert!(apply(&mut journal, second.clone()).unwrap().replayed);
    assert!(matches!(recover(&mut journal, &second), CommitRecoveryDecision::Committed(_)));
    assert_eq!(journal.verify().unwrap().records, vec![(key(b"state"), value(b"STOPPED"))]);
}

#[test]
fn false_exact_or_absence_precondition_has_no_record_receipt_or_disk_write() {
    let scratch = Scratch::new();
    let mut journal = scratch.create();
    journal.transact(put(1, 0, b"state", b"READY")).unwrap();
    let before = journal.verify().unwrap();
    let disk = fs::read(scratch.path()).unwrap();
    let writes = journal.committed_writes();
    for condition in [absent(b"state"), exact(b"state", b"WRONG"), exact(b"missing", b"READY")] {
        let request = command(put(2, 1, b"target", b"new"), vec![condition]);
        assert_eq!(apply(&mut journal, request.clone()), Err(ControlError::GenerationMismatch));
        assert!(matches!(recover(&mut journal, &request), CommitRecoveryDecision::NotCommittedRetrySameOperation));
        assert_eq!(journal.verify().unwrap(), before);
        assert_eq!(journal.committed_writes(), writes);
        assert!(!journal.requires_recovery());
        assert_eq!(fs::read(scratch.path()).unwrap(), disk);
    }
}

#[test]
fn identical_value_bytes_with_a_different_record_class_do_not_match() {
    let scratch = Scratch::new();
    let mut journal = scratch.create();
    journal.transact(put(1, 0, b"guard", b"READY")).unwrap();
    let wrong_class = ControlValue::new(ControlRecordClass::Identity, b"READY".to_vec(), LIMITS).unwrap();
    let request = command(put(2, 1, b"target", b"new"),
        vec![ControlRecordCondition::exact(key(b"guard"), wrong_class)]);
    assert_eq!(apply(&mut journal, request), Err(ControlError::GenerationMismatch));
    assert_eq!(journal.verify().unwrap().generation, 1);
}

#[test]
fn conditions_can_read_untouched_keys_but_do_not_appear_as_changed_keys() {
    let scratch = Scratch::new();
    let mut journal = scratch.create();
    journal.transact(put(1, 0, b"guard", b"READY")).unwrap();
    let receipt = apply(&mut journal, command(put(2, 1, b"target", b"new"),
        vec![exact(b"guard", b"READY"), absent(b"denied")])).unwrap();
    assert_eq!(receipt.changed_keys, vec![key(b"target")]);
    assert_eq!(journal.verify().unwrap().get(&key(b"guard")), Some(&value(b"READY")));
}

#[test]
fn guarded_delete_replays_without_requiring_the_deleted_prestate_again() {
    let scratch = Scratch::new();
    let mut journal = scratch.create();
    journal.transact(put(1, 0, b"state", b"READY")).unwrap();
    let mutation = ControlMutation::new(MutationId([2; 32]), Blake3Digest32::from_bytes([9; 32]), 1,
        vec![], vec![key(b"state")]);
    let request = command(mutation, vec![exact(b"state", b"READY")]);
    apply(&mut journal, request.clone()).unwrap();
    assert!(apply(&mut journal, request).unwrap().replayed);
    assert!(journal.verify().unwrap().records.is_empty());
}

#[test]
fn guard_aba_cannot_bypass_the_global_expected_generation() {
    let scratch = Scratch::new();
    let mut journal = scratch.create();
    journal.transact(put(1, 0, b"guard", b"A")).unwrap();
    let stale = command(put(9, 1, b"target", b"new"), vec![exact(b"guard", b"A")]);
    journal.transact(put(2, 1, b"guard", b"B")).unwrap();
    journal.transact(put(3, 2, b"guard", b"A")).unwrap();
    assert_eq!(apply(&mut journal, stale), Err(ControlError::TransactionConflict));
    assert!(journal.verify().unwrap().get(&key(b"target")).is_none());
}

#[test]
fn changing_adding_or_removing_a_condition_cannot_replay_the_same_operation() {
    let scratch = Scratch::new();
    let mut journal = scratch.create();
    let mutation = put(1, 0, b"state", b"READY");
    apply(&mut journal, command(mutation.clone(), vec![absent(b"guard")])).unwrap();
    for conditions in [vec![], vec![absent(b"different")], vec![exact(b"guard", b"READY")],
        vec![absent(b"guard"), absent(b"extra")]] {
        let request = command(mutation.clone(), conditions);
        assert_eq!(apply(&mut journal, request.clone()), Err(ControlError::OperationConflict));
        assert!(matches!(recover(&mut journal, &request), CommitRecoveryDecision::ConflictingInput));
    }
    assert_eq!(journal.transact(mutation), Err(ControlError::OperationConflict));
    assert_eq!(journal.verify().unwrap().generation, 1);
}

#[test]
fn reordered_conditions_replay_and_empty_conditions_keep_legacy_identity() {
    let scratch = Scratch::new();
    let mut journal = scratch.create();
    let mutation = put(1, 0, b"state", b"READY");
    apply(&mut journal, command(mutation.clone(), vec![absent(b"a"), absent(b"b")])).unwrap();
    assert!(apply(&mut journal, command(mutation, vec![absent(b"b"), absent(b"a")])).unwrap().replayed);
    let legacy = put(2, 1, b"second", b"value");
    journal.transact(legacy.clone()).unwrap();
    assert!(apply(&mut journal, command(legacy.clone(), vec![])).unwrap().replayed);
    assert!(matches!(journal.recover_transaction(&legacy).unwrap(), CommitRecoveryDecision::Committed(_)));
}

#[test]
fn duplicate_condition_keys_are_rejected_even_when_the_values_are_equal() {
    let scratch = Scratch::new();
    let mut journal = scratch.create();
    for conditions in [vec![absent(b"guard"), absent(b"guard")],
        vec![absent(b"guard"), exact(b"guard", b"value")]] {
        assert_eq!(apply(&mut journal, command(put(1, 0, b"state", b"READY"), conditions)),
            Err(ControlError::DuplicateMutationKey));
    }
    assert_eq!(journal.verify().unwrap().generation, 0);
}

#[test]
fn guard_only_commands_do_not_manufacture_an_empty_commit() {
    let scratch = Scratch::new();
    let mut journal = scratch.create();
    let mutation = ControlMutation::new(MutationId([1; 32]), Blake3Digest32::from_bytes([9; 32]), 0, vec![], vec![]);
    assert_eq!(apply(&mut journal, command(mutation, vec![absent(b"guard")])), Err(ControlError::BudgetExceeded));
    assert_eq!(journal.verify().unwrap().generation, 0);
}

#[test]
fn expected_values_and_combined_work_are_bounded_before_database_dispatch() {
    let mutation = put(1, 0, b"state", b"READY");
    let conditions = vec![exact(b"a", b"12"), exact(b"b", b"34")];
    let check = || Ok(());
    assert_eq!(validate_conditions(&mutation, &conditions,
        JournalLimits { max_mutation_items: 2, ..LIMITS }, check), Err(ControlError::BudgetExceeded));
    assert_eq!(validate_conditions(&mutation, &conditions,
        JournalLimits { max_total_value_bytes: 3, ..LIMITS }, check), Err(ControlError::BudgetExceeded));
    assert_eq!(validate_conditions(&mutation, &conditions,
        JournalLimits { max_value_bytes: 1, ..LIMITS }, check), Err(ControlError::InvalidValue));
    assert_eq!(validate_conditions(&mutation, &[absent(b"long-key")],
        JournalLimits { max_key_bytes: 4, ..LIMITS }, check), Err(ControlError::InvalidKey));
    assert!(validate_conditions(&mutation, &conditions,
        JournalLimits { max_mutation_items: 3, max_total_value_bytes: 4, ..LIMITS }, check).is_ok());
}

#[test]
fn conditions_are_checked_again_in_the_actual_write_transaction() {
    let scratch = Scratch::new();
    let mut journal = scratch.create();
    journal.transact(put(1, 0, b"guard", b"READY")).unwrap();
    let conditions = [exact(b"guard", b"READY")];
    {
        let read = journal.database.begin_read().unwrap();
        let table = read.open_table(RECORDS).unwrap();
        verify_conditions(&journal, &table, &conditions, &Unscoped, Point::PlanRecord).unwrap();
    }
    let write = journal.database.begin_write().unwrap();
    {
        let mut table = write.open_table(RECORDS).unwrap();
        // The pre-read is not evidence for this transaction's changed state.
        let replacement = encode_value(&value(b"OTHER"));
        table.insert(b"guard".as_slice(), replacement.as_slice()).unwrap();
        assert_eq!(verify_conditions(&journal, &table, &conditions, &Unscoped, Point::StageRecord),
            Err(ControlError::GenerationMismatch));
    }
    write.abort().unwrap();
    assert_eq!(journal.verify().unwrap().get(&key(b"guard")), Some(&value(b"READY")));
}

struct StopAt(Point);
impl Check for StopAt {
    fn check(&self, point: Point) -> Result<(), ControlError> {
        if point == self.0 { Err(ControlError::ReadCancelled) } else { Ok(()) }
    }
}

#[test]
fn pre_cancelled_and_guard_validation_interruptions_do_not_dispatch_a_write() {
    let scratch = Scratch::new();
    let mut journal = scratch.create();
    let request = command(put(1, 0, b"state", b"READY"), vec![absent(b"guard")]);
    let error = journal.transact_conditionally(request.clone(), &context(true)).unwrap_err();
    assert_eq!(error.kind(), PortErrorKind::CancelledBeforeSideEffect);
    assert_eq!(error.retryability(), PortRetryability::SameIdentity);
    assert_eq!(journal.transact_conditionally_checked(request, Boundary::Normal, &StopAt(Point::PlanRecord)),
        Err(ControlError::ReadCancelled));
    assert!(!journal.requires_recovery());
    assert_eq!(journal.verify().unwrap().generation, 0);
}

#[test]
fn cancellation_during_guard_staging_requires_exact_conditional_recovery() {
    let scratch = Scratch::new();
    let mut journal = scratch.create();
    let request = command(put(1, 0, b"state", b"READY"), vec![absent(b"guard")]);
    assert_eq!(journal.transact_conditionally_checked(request.clone(), Boundary::Normal, &StopAt(Point::StageRecord)),
        Err(ControlError::CommitOutcomeUnknown));
    assert!(journal.requires_recovery());
    assert!(matches!(journal.recover_transaction(request.mutation()).unwrap(), CommitRecoveryDecision::ConflictingInput));
    assert!(journal.requires_recovery());
    let error = journal.recover_conditional_transaction(&request, &context(true)).unwrap_err();
    assert_eq!(error.kind(), PortErrorKind::OutcomeUnknown);
    assert!(journal.requires_recovery());
    assert!(matches!(recover(&mut journal, &request), CommitRecoveryDecision::NotCommittedRetrySameOperation));
    assert!(!journal.requires_recovery());
    apply(&mut journal, request).unwrap();
}

#[test]
fn lost_acknowledgement_survives_reopen_and_cannot_be_recovered_with_stripped_conditions() {
    let scratch = Scratch::new();
    let request = command(put(1, 0, b"state", b"READY"), vec![absent(b"state")]);
    {
        let mut journal = scratch.create();
        assert_eq!(journal.transact_conditionally_checked(request.clone(), Boundary::LostAcknowledgement, &Unscoped),
            Err(ControlError::CommitOutcomeUnknown));
    }
    let mut journal = scratch.open();
    assert!(matches!(journal.recover_transaction(request.mutation()).unwrap(), CommitRecoveryDecision::ConflictingInput));
    assert!(matches!(recover(&mut journal, &request), CommitRecoveryDecision::Committed(_)));
    assert!(apply(&mut journal, request).unwrap().replayed);
    assert_eq!(journal.committed_writes(), 0);
    assert_eq!(journal.verify().unwrap().generation, 1);
}

#[test]
fn historical_replay_and_owner_handoff_preserve_the_original_conditional_identity() {
    let scratch = Scratch::new();
    let mut journal = scratch.create();
    let request = command(put(1, 0, b"state", b"READY"), vec![absent(b"state"), absent(b"guard")]);
    apply(&mut journal, request.clone()).unwrap();
    journal.transact(put(2, 1, b"state", b"STOPPED")).unwrap();
    journal.transact(put(3, 2, b"guard", b"CHANGED")).unwrap();
    let next = JournalIdentity { owner_epoch: OwnerEpoch::new(2).unwrap(), ..identity() };
    let mut journal = journal.advance_owner(next).unwrap();
    assert!(apply(&mut journal, request.clone()).unwrap().replayed);
    assert!(matches!(recover(&mut journal, &request), CommitRecoveryDecision::Committed(_)));
    assert_eq!(journal.verify().unwrap().get(&key(b"state")), Some(&value(b"STOPPED")));
    assert_eq!(journal.verify().unwrap().generation, 3);
}

#[test]
fn malformed_stored_guard_is_corruption_not_a_normal_precondition_failure() {
    let scratch = Scratch::new();
    let mut journal = scratch.create();
    journal.transact(put(1, 0, b"guard", b"READY")).unwrap();
    let write = journal.database.begin_write().unwrap();
    {
        let mut table = write.open_table(RECORDS).unwrap();
        table.insert(b"guard".as_slice(), b"\xffREADY".as_slice()).unwrap();
    }
    write.commit().unwrap();
    let request = command(put(2, 1, b"state", b"new"), vec![exact(b"guard", b"READY")]);
    assert_eq!(apply(&mut journal, request), Err(ControlError::ForbiddenControlPayload));
    assert_eq!(journal.read_snapshot(), Err(ControlError::StoreQuarantined));
}

#[test]
fn guard_lookup_cost_is_independent_of_unrelated_record_count() {
    let scratch = Scratch::new();
    let mut journal = scratch.create();
    let writes = (0..128).map(|n| ControlWrite { key: key(format!("unrelated-{n:03}").as_bytes()), value: value(b"value") }).collect();
    journal.transact(ControlMutation::new(MutationId([1; 32]), Blake3Digest32::from_bytes([9; 32]), 0, writes, vec![])).unwrap();
    let before_points = journal.work.point_reads.load(Ordering::Relaxed);
    let before_snapshots = journal.work.snapshot_reads.load(Ordering::Relaxed);
    apply(&mut journal, command(put(2, 1, b"target", b"new"),
        vec![absent(b"target"), exact(b"unrelated-050", b"value")])).unwrap();
    // Each condition is read twice; the untouched condition is checked after
    // commit too. One write is planned and verified once.
    assert_eq!(journal.work.point_reads.load(Ordering::Relaxed) - before_points, 7);
    assert_eq!(journal.work.snapshot_reads.load(Ordering::Relaxed), before_snapshots);
}

#[test]
fn conditional_work_uses_one_deadline_including_validation_and_fingerprinting() {
    let scratch = Scratch::new();
    let mut journal = scratch.create();
    let context = OperationContext::new(RequestId::from_bytes([8; 16]), 5, Cancellation(false),
        OpaqueRef::new("budget:conditions-deadline").unwrap()).unwrap();
    let origin = Instant::now();
    let ticks = Cell::new(0_u64);
    let budget = Budget::with_clock(&context, || {
        let tick = ticks.get();
        ticks.set(tick + 1);
        origin + Duration::from_millis(tick)
    });
    let request = command(put(1, 0, b"target", b"new"), vec![absent(b"a"), absent(b"b")]);
    assert_eq!(journal.transact_conditionally_checked(request, Boundary::Normal, &budget),
        Err(ControlError::BudgetExceeded));
    assert!(!journal.requires_recovery());
    assert_eq!(journal.verify().unwrap().generation, 0);
}

#[test]
fn condition_debug_never_exposes_key_or_expected_value_sentinels() {
    let request = command(put(1, 0, b"write-key-secret", b"write-value-secret"),
        vec![exact(b"condition-key-secret", b"condition-value-secret")]);
    let debug = format!("{request:?}");
    for secret in ["write-key-secret", "write-value-secret", "condition-key-secret", "condition-value-secret"] {
        assert!(!debug.contains(secret));
    }
}

#[test]
fn conditional_fingerprint_has_an_independent_known_answer_and_no_empty_guard_delta() {
    let base = [0x42; 32];
    let conditions = [absent(b"a"), exact(b"b", b"READY")];
    let expected: [u8; 32] = [0xeb, 0x1e, 0xe6, 0x6c, 0x24, 0x5d, 0x57, 0x0c, 0xa8, 0x85, 0xb4, 0x0b, 0x58, 0x93, 0x3c, 0x4d, 0x64, 0x51, 0x97, 0xdb, 0x18, 0xc5, 0xb3, 0xaa, 0x7d, 0x73, 0x74, 0x77, 0x3b, 0x85, 0xad, 0x33];
    assert_eq!(bind_conditions(base, &conditions, || Ok(())).unwrap(), expected);
    assert_eq!(bind_conditions(base, &[conditions[1].clone(), conditions[0].clone()], || Ok(())).unwrap(), expected);
    assert_eq!(bind_conditions(base, &[], || Ok(())).unwrap(), base);
    assert_ne!(bind_conditions(base, &[absent(b"a")], || Ok(())).unwrap(), expected);
}

#[test]
fn current_receipt_checks_untouched_conditions_but_not_obsolete_prestate() {
    for recover_only in [false, true] {
        let scratch = Scratch::new();
        let mut journal = scratch.create();
        journal.transact(put(1, 0, b"guard", b"READY")).unwrap();
        let request = command(put(2, 1, b"target", b"new"), vec![exact(b"guard", b"READY")]);
        apply(&mut journal, request.clone()).unwrap();
        let write = journal.database.begin_write().unwrap();
        {
            let mut table = write.open_table(RECORDS).unwrap();
            let tampered = encode_value(&value(b"WRONG")); // same class, length and counters
            table.insert(b"guard".as_slice(), tampered.as_slice()).unwrap();
        }
        write.commit().unwrap();
        if recover_only {
            assert!(matches!(recover(&mut journal, &request), CommitRecoveryDecision::PartialOrCorruptQuarantine));
        } else {
            assert_eq!(apply(&mut journal, request), Err(ControlError::StoreCorrupt));
        }
        assert_eq!(journal.read_snapshot(), Err(ControlError::StoreQuarantined));
    }
}

#[test]
fn failed_batch_condition_preserves_every_write_and_delete_target() {
    let scratch = Scratch::new();
    let mut journal = scratch.create();
    journal.transact(put(1, 0, b"a", b"old")).unwrap();
    journal.transact(put(2, 1, b"b", b"old")).unwrap();
    let before = journal.verify().unwrap();
    let mutation = ControlMutation::new(MutationId([3; 32]), Blake3Digest32::from_bytes([9; 32]), 2,
        vec![ControlWrite { key: key(b"a"), value: value(b"new") }], vec![key(b"b")]);
    let request = command(mutation, vec![exact(b"a", b"old"), exact(b"b", b"stale")]);
    assert_eq!(apply(&mut journal, request.clone()), Err(ControlError::GenerationMismatch));
    assert_eq!(journal.verify().unwrap(), before);
    assert!(matches!(recover(&mut journal, &request), CommitRecoveryDecision::NotCommittedRetrySameOperation));
}

#[test]
fn conditional_fingerprint_distinguishes_every_class_and_length_delimited_value() {
    let mut fingerprints = std::collections::BTreeSet::new();
    for class in [ControlRecordClass::Identity, ControlRecordClass::Revision, ControlRecordClass::State,
        ControlRecordClass::Receipt, ControlRecordClass::Operation, ControlRecordClass::Snapshot,
        ControlRecordClass::Migration] {
        let condition = ControlRecordCondition::exact(key(b"a"),
            ControlValue::new(class, b"bc".to_vec(), LIMITS).unwrap());
        fingerprints.insert(bind_conditions([0x42; 32], &[condition], || Ok(())).unwrap());
    }
    assert_eq!(fingerprints.len(), 7);
    let first = bind_conditions([0x42; 32], &[exact(b"a", b"bc")], || Ok(())).unwrap();
    let second = bind_conditions([0x42; 32], &[exact(b"ab", b"c")], || Ok(())).unwrap();
    assert_ne!(first, second);
}

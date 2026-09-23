//! Native temporary-redb scenarios. Synthetic identities do not qualify the
//! Windows root-owner guard, media durability or live authorization wiring.

use super::*;
use search_contracts::{
    AccessPolicyRevision, DataRootId, InstallationIncarnationId, OpaqueRef, OwnerEpoch,
    PurgeFenceRevision, RequestId, ShadowFenceRevision, SourceOwnerGeneration,
};
use search_ports::PackageOpaque;
use std::fs::{self, OpenOptions};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Clone, Debug)]
struct Cancel(bool);
impl PackageOpaque for Cancel {
    fn owner_package(&self) -> &'static str { "search-control-redb" }
}
impl CancellationProbe for Cancel {
    fn is_cancelled(&self) -> bool { self.0 }
}

fn context(cancelled: bool) -> OperationContext<Cancel> {
    OperationContext::new(
        RequestId::from_bytes([1; 16]), 60_000, Cancel(cancelled),
        OpaqueRef::new("budget:policy-native-test").unwrap(),
    ).unwrap()
}

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
        static NEXT: AtomicU64 = AtomicU64::new(0);
        for _ in 0..128 {
            let path = std::env::temp_dir().join(format!(
                "eliot-policy-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed),
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => panic!("create native fixture: {error}"),
            }
        }
        panic!("fixture collision budget exhausted");
    }
    fn create(&self, limits: JournalLimits) -> PersistentControlJournal {
        let file = OpenOptions::new().read(true).write(true).create_new(true)
            .open(self.0.join("control.redb")).unwrap();
        PersistentControlJournal::create(file, identity(), limits).unwrap()
    }
    fn open(&self) -> PersistentControlJournal {
        let file = OpenOptions::new().read(true).write(true)
            .open(self.0.join("control.redb")).unwrap();
        PersistentControlJournal::open(file, identity(), JournalLimits::BASELINE).unwrap()
    }
}
impl Drop for Scratch {
    fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); }
}

fn record(generation: u64) -> AccessPolicyRecord {
    AccessPolicyRecord {
        namespace_id: SourceNamespaceId::from_bytes([7; 16]),
        owner_generation: SourceOwnerGeneration::from_bytes([8; 32]),
        policy_revision: AccessPolicyRevision::new(generation),
        live_deny_generation: generation,
        shadow_fence_revision: ShadowFenceRevision::new(generation),
        purge_fence_revision: PurgeFenceRevision::new(generation),
        policy_digest: Blake3Digest32::from_bytes([9; 32]),
    }
}

fn update(id: u8, generation: u64, previous: Option<AccessPolicyRecord>, next: AccessPolicyRecord)
    -> AccessPolicyMutation
{
    // Digest is a fixture; the journal binds the actual descriptor independently.
    AccessPolicyMutation::new(identity(), MutationId([id; 32]), Blake3Digest32::from_bytes([10; 32]),
        generation, previous, next).unwrap()
}

#[test]
fn native_initialize_replace_and_reopen_preserve_exact_policy() {
    let scratch = Scratch::new();
    let mut journal = scratch.create(JournalLimits::BASELINE);
    let namespace = record(1).namespace_id;
    let empty = journal.read_access_policy(namespace, &context(false)).unwrap();
    assert!(empty.record().is_none());
    assert_eq!(empty.generation(), 0);
    let initial = update(1, 0, None, record(1));
    journal.commit_access_policy(&initial, &context(false)).unwrap();
    let change = update(2, 1, Some(record(1)), record(2));
    let receipt = journal.commit_access_policy(&change, &context(false)).unwrap();
    let observed = journal.read_access_policy(namespace, &context(false)).unwrap();
    assert_eq!(observed.identity(), identity());
    assert_eq!(observed.record(), Some(&record(2)));
    observed.confirm_commit(&receipt).unwrap();
    drop(journal);
    let mut journal = scratch.open();
    let recovered = journal.recover_access_policy(&change, &context(false)).unwrap();
    let receipt = recovered.expect("lost committed policy");
    journal.read_access_policy(namespace, &context(false)).unwrap()
        .confirm_commit(&receipt).unwrap();
}

#[test]
fn exact_prestate_not_just_generation_guards_replacement() {
    let scratch = Scratch::new();
    let mut journal = scratch.create(JournalLimits::BASELINE);
    journal.commit_access_policy(&update(1, 0, None, record(1)), &context(false)).unwrap();
    let before = journal.read_snapshot().unwrap();
    let writes = journal.committed_writes();
    for previous in [None, Some(record(99))] {
        let error = journal.commit_access_policy(&update(2, 1, previous, record(2)), &context(false)).unwrap_err();
        assert!(matches!(error, AccessPolicyJournalError::Call(call)
            if call.control_error() == ControlError::GenerationMismatch));
        assert_eq!(journal.read_snapshot().unwrap(), before);
        assert_eq!(journal.committed_writes(), writes);
    }
    let mut foreign = record(1);
    foreign.namespace_id = SourceNamespaceId::from_bytes([99; 16]);
    assert_eq!(AccessPolicyMutation::new(identity(), MutationId([3; 32]),
        Blake3Digest32::from_bytes([10; 32]), 1, Some(foreign), record(2)), Err(ControlError::IdentityMismatch));
}

#[test]
fn exact_replay_does_not_write_and_changed_conditions_conflict() {
    let scratch = Scratch::new();
    let mut journal = scratch.create(JournalLimits::BASELINE);
    let initial = update(1, 0, None, record(1));
    let first = journal.commit_access_policy(&initial, &context(false)).unwrap();
    let writes = journal.committed_writes();
    let replay = journal.commit_access_policy(&initial, &context(false)).unwrap();
    assert!(replay.receipt().replayed);
    assert_eq!(replay.receipt().after_generation, first.receipt().after_generation);
    assert_eq!(journal.committed_writes(), writes);
    let conflict = update(1, 0, Some(record(1)), record(1));
    assert_eq!(journal.recover_access_policy(&conflict, &context(false)),
        Err(AccessPolicyJournalError::Record(ControlError::OperationConflict)));
    assert_eq!(journal.committed_writes(), writes);
}

#[test]
fn historical_and_aba_receipts_are_not_current_policy_readback() {
    let scratch = Scratch::new();
    let mut journal = scratch.create(JournalLimits::BASELINE);
    let initial = update(1, 0, None, record(1));
    journal.commit_access_policy(&initial, &context(false)).unwrap();
    journal.commit_access_policy(&update(2, 1, Some(record(1)), record(2)), &context(false)).unwrap();
    // The storage owner does not decide policy monotonicity. Deliberately return
    // the same bytes to prove journal generations still expose an ABA change.
    journal.commit_access_policy(&update(3, 2, Some(record(2)), record(1)), &context(false)).unwrap();
    let old = journal.recover_access_policy(&initial, &context(false)).unwrap()
        .expect("historical receipt missing");
    let now = journal.read_access_policy(record(1).namespace_id, &context(false)).unwrap();
    assert_eq!(now.record(), Some(&record(1)));
    assert_eq!(now.confirm_commit(&old), Err(ControlError::TransactionConflict));
}

#[test]
fn native_read_rejects_malformed_foreign_and_wrong_class_rows_without_writes() {
    for case in 0..3 {
        let scratch = Scratch::new();
        let mut journal = scratch.create(JournalLimits::BASELINE);
        let mut stored = record(1);
        if case == 0 { stored.namespace_id = SourceNamespaceId::from_bytes([99; 16]); }
        let bytes = if case == 1 { vec![0xff; 124] } else { encode_access_policy(&stored) };
        let class = if case == 2 { ControlRecordClass::Receipt } else { ControlRecordClass::State };
        let mutation = ControlMutation::new(MutationId([1; 32]), Blake3Digest32::from_bytes([10; 32]), 0,
            vec![ControlWrite { key: policy_key(record(1).namespace_id).unwrap(),
                value: ControlValue::new(class, bytes, JournalLimits::BASELINE).unwrap() }], Vec::new());
        journal.transact(&mutation).unwrap();
        let writes = journal.committed_writes();
        assert_eq!(journal.read_access_policy(record(1).namespace_id, &context(false)),
            Err(AccessPolicyJournalError::Record(ControlError::StoreCorrupt)));
        assert!(journal.read_access_policy(SourceNamespaceId::from_bytes([66; 16]), &context(false))
            .unwrap().record().is_none());
        assert_eq!(journal.committed_writes(), writes);
    }
}

#[test]
fn foreign_owner_and_actual_journal_limits_deny_before_commit() {
    let scratch = Scratch::new();
    let mut journal = scratch.create(JournalLimits::BASELINE);
    let mut foreign = update(1, 0, None, record(1));
    foreign.identity.owner_epoch = OwnerEpoch::new(2).unwrap();
    let before = journal.committed_writes();
    for error in [journal.commit_access_policy(&foreign, &context(false)).unwrap_err(),
        journal.recover_access_policy(&foreign, &context(false)).unwrap_err()]
    {
        assert_eq!(error, AccessPolicyJournalError::Record(ControlError::IdentityMismatch));
    }
    assert_eq!(journal.committed_writes(), before);
    let narrow = Scratch::new();
    let mut journal = narrow.create(JournalLimits { max_value_bytes: 123, ..JournalLimits::BASELINE });
    let before = journal.committed_writes();
    assert!(journal.commit_access_policy(&update(1, 0, None, record(1)), &context(false)).is_err());
    assert_eq!(journal.committed_writes(), before);
}

#[test]
fn cancellation_remains_typed_and_recovery_does_not_initialize_absence() {
    let scratch = Scratch::new();
    let mut journal = scratch.create(JournalLimits::BASELINE);
    let mutation = update(1, 0, None, record(1));
    let writes = journal.committed_writes();
    let error = journal.commit_access_policy(&mutation, &context(true)).unwrap_err();
    assert!(matches!(error, AccessPolicyJournalError::Call(call)
        if call.interruption() == Some(crate::ControlInterruption::Cancelled)));
    assert!(journal.recover_access_policy(&mutation, &context(false)).unwrap().is_none());
    assert!(journal.read_access_policy(record(1).namespace_id, &context(false)).unwrap().record().is_none());
    assert_eq!(journal.committed_writes(), writes);
}

#[test]
fn receipt_confirmation_checks_identity_keys_and_exact_generations() {
    let scratch = Scratch::new();
    let mut journal = scratch.create(JournalLimits::BASELINE);
    let mutation = update(1, 0, None, record(1));
    let receipt = journal.commit_access_policy(&mutation, &context(false)).unwrap();
    let observed = journal.read_access_policy(record(1).namespace_id, &context(false)).unwrap();
    for case in 0..5 {
        let mut changed = receipt.clone();
        match case {
            0 => changed.receipt.operation_id = MutationId([99; 32]),
            1 => changed.receipt.command_digest = Blake3Digest32::from_bytes([99; 32]),
            2 => changed.receipt.before_generation = 2,
            3 => changed.receipt.after_generation = 2,
            _ => changed.receipt.changed_keys.clear(),
        }
        assert_eq!(observed.confirm_commit(&changed), Err(ControlError::OperationConflict));
    }
    let mut foreign = receipt.clone();
    foreign.mutation.identity.data_root_id = DataRootId::from_bytes([99; 16]);
    assert_eq!(observed.confirm_commit(&foreign), Err(ControlError::IdentityMismatch));
}

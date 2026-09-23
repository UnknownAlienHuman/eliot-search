//! Temporary native redb and synthetic codec fixtures, not platform/power-loss
//! qualification or evidence that a live restriction has been published.

use super::*;
use crate::access_policy::AccessPolicyMutation;
use search_contracts::{
    AccessPolicyRevision, DataRootId, InstallationIncarnationId, OwnerEpoch,
    PurgeFenceRevision, RequestId, ShadowFenceRevision, SourceOwnerGeneration,
};
use search_ports::PackageOpaque;
use std::fs::{self, OpenOptions};
use std::path::PathBuf;
use std::sync::{Arc, atomic::{AtomicBool, AtomicU64, Ordering}};

#[derive(Clone, Debug, Default)]
struct Cancel(Arc<AtomicBool>);
impl PackageOpaque for Cancel {
    fn owner_package(&self) -> &'static str { "search-control-redb" }
}
impl CancellationProbe for Cancel {
    fn is_cancelled(&self) -> bool { self.0.load(Ordering::SeqCst) }
}
fn context(cancel: &Cancel) -> OperationContext<Cancel> {
    OperationContext::new(RequestId::from_bytes([1; 16]), 30_000, cancel.clone(),
        OpaqueRef::new("budget:security-record-fixture").unwrap()).unwrap()
}
fn identity() -> JournalIdentity {
    JournalIdentity {
        installation_incarnation_id: InstallationIncarnationId::from_bytes([1; 16]),
        data_root_id: DataRootId::from_bytes([2; 16]), owner_epoch: OwnerEpoch::new(1).unwrap(),
        path_identity_digest: Blake3Digest32::from_bytes([3; 32]),
        schema_family_digest: Blake3Digest32::from_bytes([4; 32]), schema_version: 1,
    }
}
fn state() -> SecurityPolicyState {
    SecurityPolicyState {
        policy: AccessPolicyRecord {
            namespace_id: SourceNamespaceId::from_bytes([5; 16]),
            owner_generation: SourceOwnerGeneration::from_bytes([6; 32]),
            policy_revision: AccessPolicyRevision::new(11), live_deny_generation: 7,
            shadow_fence_revision: ShadowFenceRevision::new(8),
            purge_fence_revision: PurgeFenceRevision::new(9), policy_digest: Blake3Digest32::from_bytes([10; 32]),
        },
        security_domain_ref: OpaqueRef::new("domain:fixture").unwrap(),
        snapshot_digest: Blake3Digest32::from_bytes([12; 32]),
        denied_memberships: BoundedSet::from_items([1_u128, 2].map(|id| SourceMembershipId::from_bytes(id.to_be_bytes()))).unwrap(),
        purged_memberships: BoundedSet::from_items([SourceMembershipId::from_bytes(3_u128.to_be_bytes())]).unwrap(),
        fail_closed: false,
    }
}
fn owners() -> BoundedSet<OpaqueId, MAX_RESTRICTION_DEPENDENTS> {
    BoundedSet::from_items(["continuation", "handle"].map(|id| OpaqueId::new(id).unwrap())).unwrap()
}
fn op(value: &str) -> OpaqueId { OpaqueId::new(value).unwrap() }
fn digest() -> Blake3Digest32 { Blake3Digest32::from_bytes([13; 32]) }

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        for _ in 0..128 {
            let path = std::env::temp_dir().join(format!("eliot-security-record-{}-{}",
                std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => panic!("fixture directory: {error}"),
            }
        }
        panic!("fixture collision limit");
    }
    fn create(&self, limits: JournalLimits) -> PersistentControlJournal {
        let file = OpenOptions::new().read(true).write(true).create_new(true).open(self.0.join("control.redb")).unwrap();
        PersistentControlJournal::create(file, identity(), limits).unwrap()
    }
    fn reopen(&self) -> PersistentControlJournal {
        let file = OpenOptions::new().read(true).write(true).open(self.0.join("control.redb")).unwrap();
        PersistentControlJournal::open(file, identity(), JournalLimits::BASELINE).unwrap()
    }
}
impl Drop for Scratch {
    fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); }
}
fn initial(journal: &mut PersistentControlJournal) -> SecurityRestrictionCommit {
    let read = journal.read_security_restriction(state().policy.namespace_id, &context(&Cancel::default())).unwrap();
    let command = read.initialize(op("operation:one"), digest(), state(), owners()).unwrap();
    journal.commit_security_restriction(&command, &context(&Cancel::default())).unwrap()
}
fn raw_write(journal: &mut PersistentControlJournal, writes: Vec<ControlWrite>, id: u8) {
    let generation = journal.read_snapshot().unwrap().generation;
    journal.transact(&ControlMutation::new(MutationId([id; 32]), digest(), generation, writes, Vec::new())).unwrap();
}

#[test]
fn reopen_recovers_the_complete_original_command_without_rewriting() {
    let scratch = Scratch::new();
    let mut journal = scratch.create(JournalLimits::BASELINE);
    let first = initial(&mut journal);
    let read = journal.read_security_restriction(state().policy.namespace_id, &context(&Cancel::default())).unwrap();
    let mut next = state();
    next.policy.live_deny_generation += 1;
    next.policy.policy_revision = AccessPolicyRevision::new(12);
    next.denied_memberships.insert(SourceMembershipId::from_bytes(4_u128.to_be_bytes())).unwrap();
    next.snapshot_digest = Blake3Digest32::from_bytes([14; 32]);
    let command = read.prepare_restriction(&first, op("operation:two"), digest(), next.clone(), owners()).unwrap();
    let committed = journal.commit_security_restriction(&command, &context(&Cancel::default())).unwrap();
    let expected_native = command.native_command().unwrap();
    drop(command);
    drop(journal);

    let mut journal = scratch.reopen();
    let before = journal.committed_writes();
    let read = journal.read_security_restriction(next.policy.namespace_id, &context(&Cancel::default())).unwrap();
    let original = read.mutation().unwrap();
    assert_eq!(original.expected_state(), Some(&state()));
    assert_eq!(original.replacement(), &next);
    assert_eq!(original.required_dependents(), &owners());
    assert_eq!(original.operation_id(), &op("operation:two"));
    assert_eq!(original.native_command().unwrap(), expected_native);
    let recovered = journal.recover_security_restriction(original, &context(&Cancel::default())).unwrap().unwrap();
    assert_eq!(recovered.receipt().operation_id, committed.receipt().operation_id);
    journal.read_security_restriction(next.policy.namespace_id, &context(&Cancel::default())).unwrap()
        .confirm_current(&recovered).unwrap();
    assert_eq!(journal.committed_writes(), before);
}

#[test]
fn codec_is_lossless_versioned_and_rejects_every_truncated_prefix() {
    let scratch = Scratch::new();
    let mut journal = scratch.create(JournalLimits::BASELINE);
    let committed = initial(&mut journal);
    let command = committed.mutation();
    let encoded = codec::encode(command).unwrap();
    assert_eq!(encoded.len(), 440);
    assert_eq!(format!("{:x}", Sha256::digest(&encoded)), "9787e3051350e7d661a5561a32f31e22e6aaa6f4e36d1aca92afe44554b264aa");
    let decoded = codec::decode(&encoded).unwrap();
    assert_eq!(&decoded, command);
    assert_eq!(decoded.native_command().unwrap(), command.native_command().unwrap());
    for length in 0..encoded.len() {
        assert!(codec::decode(&encoded[..length]).is_err(), "accepted prefix {length}");
    }
    let mut trailing = encoded.clone(); trailing.push(0);
    assert!(codec::decode(&trailing).is_err());
    for offset in [0, 11] {
        let mut bad = encoded.clone(); bad[offset] ^= 0xff;
        assert!(codec::decode(&bad).is_err());
    }
}

#[test]
fn noncanonical_sets_flags_utf8_and_counts_never_decode() {
    let scratch = Scratch::new();
    let mut journal = scratch.create(JournalLimits::BASELINE);
    let committed = initial(&mut journal);
    let encoded = codec::encode(committed.mutation()).unwrap();
    let pattern = [2_u32.to_be_bytes().as_slice(), &1_u128.to_be_bytes(), &2_u128.to_be_bytes()].concat();
    let offset = encoded.windows(pattern.len()).position(|bytes| bytes == pattern).unwrap();
    let mut duplicate = encoded.clone();
    duplicate[offset + 20..offset + 36].copy_from_slice(&1_u128.to_be_bytes());
    assert!(codec::decode(&duplicate).is_err());
    let mut reversed = encoded.clone();
    reversed[offset + 4..offset + 20].copy_from_slice(&2_u128.to_be_bytes());
    reversed[offset + 20..offset + 36].copy_from_slice(&1_u128.to_be_bytes());
    assert!(codec::decode(&reversed).is_err());
    let mut count = encoded.clone(); count[offset..offset + 4].copy_from_slice(&u32::MAX.to_be_bytes());
    assert!(codec::decode(&count).is_err());
    let mut flag = encoded.clone(); flag[offset - 1] = 2;
    assert!(codec::decode(&flag).is_err());
    let text = b"operation:one";
    let location = encoded.windows(text.len()).position(|bytes| bytes == text).unwrap();
    let mut utf8 = encoded.clone(); utf8[location] = 0xff;
    assert!(codec::decode(&utf8).is_err());
    assert!(codec::decode(&vec![0; MAX_RESTRICTION_RECORD_BYTES + 1]).is_err());
}

#[test]
fn same_operation_replay_is_exact_and_changed_inputs_conflict() {
    let scratch = Scratch::new();
    let mut journal = scratch.create(JournalLimits::BASELINE);
    let read = journal.read_security_restriction(state().policy.namespace_id, &context(&Cancel::default())).unwrap();
    let command = read.initialize(op("operation:one"), digest(), state(), owners()).unwrap();
    journal.commit_security_restriction(&command, &context(&Cancel::default())).unwrap();
    let writes = journal.committed_writes();
    assert!(journal.commit_security_restriction(&command, &context(&Cancel::default())).unwrap().receipt().replayed);
    for field in 0..5 {
        let mut changed = command.clone();
        match field {
            0 => changed.replacement.snapshot_digest = Blake3Digest32::from_bytes([77; 32]),
            1 => changed.replacement.fail_closed = true,
            2 => changed.replacement.denied_memberships.insert(SourceMembershipId::from_bytes([44; 16])).unwrap(),
            3 => changed.dependents.insert(op("other-owner")).unwrap(),
            _ => changed.command_digest = Blake3Digest32::from_bytes([78; 32]),
        }
        assert_eq!(changed.native_id(), command.native_id());
        assert!(journal.commit_security_restriction(&changed, &context(&Cancel::default())).is_err());
    }
    assert_eq!(journal.committed_writes(), writes);
    let read = journal.read_security_restriction(state().policy.namespace_id, &context(&Cancel::default())).unwrap();
    assert_eq!(read.mutation(), Some(&command));
}

#[test]
fn stale_snapshot_refuses_both_writes_and_omits_an_operation_receipt() {
    let scratch = Scratch::new();
    let mut journal = scratch.create(JournalLimits::BASELINE);
    let read = journal.read_security_restriction(state().policy.namespace_id, &context(&Cancel::default())).unwrap();
    let command = read.initialize(op("operation:one"), digest(), state(), owners()).unwrap();
    raw_write(&mut journal, vec![ControlWrite {
        key: ControlKey::new(b"unrelated".to_vec(), JournalLimits::BASELINE).unwrap(),
        value: ControlValue::new(ControlRecordClass::State, b"READY".to_vec(), JournalLimits::BASELINE).unwrap(),
    }], 88);
    let before = journal.read_snapshot().unwrap();
    assert!(journal.commit_security_restriction(&command, &context(&Cancel::default())).is_err());
    assert_eq!(journal.read_snapshot().unwrap(), before);
    assert!(journal.recover_security_restriction(&command, &context(&Cancel::default())).unwrap().is_none());
    assert!(journal.read_security_restriction(state().policy.namespace_id, &context(&Cancel::default())).unwrap().mutation().is_none());
}

#[test]
fn current_confirmation_survives_unrelated_writes_but_rejects_domain_aba() {
    let scratch = Scratch::new();
    let mut journal = scratch.create(JournalLimits::BASELINE);
    let first = initial(&mut journal);
    raw_write(&mut journal, vec![ControlWrite {
        key: ControlKey::new(b"unrelated".to_vec(), JournalLimits::BASELINE).unwrap(),
        value: ControlValue::new(ControlRecordClass::State, b"READY".to_vec(), JournalLimits::BASELINE).unwrap(),
    }], 87);
    let read = journal.read_security_restriction(state().policy.namespace_id, &context(&Cancel::default())).unwrap();
    read.confirm_current(&first).unwrap();
    let mut newer = state(); newer.policy.live_deny_generation += 1;
    let second = read.prepare_restriction(&first, op("operation:two"), digest(), newer, owners()).unwrap();
    let second = journal.commit_security_restriction(&second, &context(&Cancel::default())).unwrap();
    let read = journal.read_security_restriction(state().policy.namespace_id, &context(&Cancel::default())).unwrap();
    // The persistence layer stores captured data, not permissive-policy authority.
    let aba = read.prepare_restriction(&second, op("operation:three"), digest(), state(), owners()).unwrap();
    let latest = journal.commit_security_restriction(&aba, &context(&Cancel::default())).unwrap();
    let read = journal.read_security_restriction(state().policy.namespace_id, &context(&Cancel::default())).unwrap();
    read.confirm_current(&latest).unwrap();
    assert_eq!(read.confirm_current(&first), Err(ControlError::TransactionConflict));
    assert!(read.prepare_restriction(&first, op("operation:four"), digest(), state(), owners()).is_err());
}

#[test]
fn metadata_only_initialization_is_explicit_and_preserves_existing_policy() {
    let scratch = Scratch::new();
    let mut journal = scratch.create(JournalLimits::BASELINE);
    let old = AccessPolicyMutation::new(identity(), MutationId([90; 32]), digest(), 0, None, state().policy).unwrap();
    journal.commit_access_policy(&old, &context(&Cancel::default())).unwrap();
    let read = journal.read_security_restriction(state().policy.namespace_id, &context(&Cancel::default())).unwrap();
    assert!(read.mutation().is_none());
    let mut changed = state(); changed.policy.live_deny_generation += 1;
    assert_eq!(read.initialize(op("init"), digest(), changed, owners()), Err(ControlError::GenerationMismatch));
    let command = read.initialize(op("init"), digest(), state(), owners()).unwrap();
    let commit = journal.commit_security_restriction(&command, &context(&Cancel::default())).unwrap();
    let read = journal.read_security_restriction(state().policy.namespace_id, &context(&Cancel::default())).unwrap();
    read.confirm_current(&commit).unwrap();
    assert_eq!(read.initialize(op("again"), digest(), state(), owners()), Err(ControlError::OperationConflict));
}

#[test]
fn missing_native_receipt_cannot_be_replaced_by_a_well_formed_command_row() {
    let scratch = Scratch::new();
    let mut journal = scratch.create(JournalLimits::BASELINE);
    let read = journal.read_security_restriction(state().policy.namespace_id, &context(&Cancel::default())).unwrap();
    let command = read.initialize(op("operation:one"), digest(), state(), owners()).unwrap();
    // Write valid-looking rows under a DIFFERENT operation, bypassing the typed
    // API to simulate semantic corruption. The saved original operation is absent.
    raw_write(&mut journal, command.native_command().unwrap().mutation().writes().to_vec(), 91);
    let read = journal.read_security_restriction(state().policy.namespace_id, &context(&Cancel::default())).unwrap();
    assert!(journal.recover_security_restriction(read.mutation().unwrap(), &context(&Cancel::default())).unwrap().is_none());
    assert!(read.initialize(op("replace-corruption"), digest(), state(), owners()).is_err());
    assert!(journal.commit_security_restriction(&command, &context(&Cancel::default())).is_err());
}

#[test]
fn malformed_or_incoherent_pairs_are_never_repaired_by_a_read() {
    for mode in 0..4 {
        let scratch = Scratch::new();
        let mut journal = scratch.create(JournalLimits::BASELINE);
        let commit = initial(&mut journal);
        let namespace = state().policy.namespace_id;
        let write = match mode {
            0 => ControlWrite {
                key: key(RESTRICTION_PREFIX, namespace).unwrap(),
                value: ControlValue::new(ControlRecordClass::State, codec::encode(commit.mutation()).unwrap(), JournalLimits::BASELINE).unwrap(),
            },
            1 => ControlWrite {
                key: key(RESTRICTION_PREFIX, namespace).unwrap(),
                value: ControlValue::new(ControlRecordClass::Operation, b"broken".to_vec(), JournalLimits::BASELINE).unwrap(),
            },
            _ => {
                let mut policy = state().policy;
                if mode == 2 { policy.live_deny_generation += 1; }
                else { policy.namespace_id = SourceNamespaceId::from_bytes([99; 16]); }
                ControlWrite { key: key(POLICY_PREFIX, namespace).unwrap(), value: policy_value(&policy).unwrap() }
            }
        };
        raw_write(&mut journal, vec![write], 92);
        let before = journal.committed_writes();
        assert_eq!(journal.read_security_restriction(namespace, &context(&Cancel::default())),
            Err(AccessPolicyJournalError::Record(ControlError::StoreCorrupt)));
        assert_eq!(journal.committed_writes(), before);
    }
}

#[test]
fn capacity_cancellation_and_foreign_identity_preserve_the_original_state() {
    let scratch = Scratch::new();
    let mut journal = scratch.create(JournalLimits::BASELINE);
    let first = initial(&mut journal);
    let read = journal.read_security_restriction(state().policy.namespace_id, &context(&Cancel::default())).unwrap();
    let mut enormous = state();
    enormous.denied_memberships = BoundedSet::from_items((0_u128..4096).map(|id| SourceMembershipId::from_bytes(id.to_be_bytes()))).unwrap();
    assert_eq!(read.prepare_restriction(&first, op("huge"), digest(), enormous, owners()), Err(ControlError::BudgetExceeded));
    let mut next = state(); next.policy.live_deny_generation += 1;
    let command = read.prepare_restriction(&first, op("cancel"), digest(), next, owners()).unwrap();
    let before = journal.read_snapshot().unwrap();
    let cancel = Cancel::default(); cancel.0.store(true, Ordering::SeqCst);
    assert!(journal.commit_security_restriction(&command, &context(&cancel)).is_err());
    assert_eq!(journal.read_snapshot().unwrap(), before);
    let mut foreign = command.clone(); foreign.identity.owner_epoch = OwnerEpoch::new(2).unwrap();
    assert_eq!(journal.commit_security_restriction(&foreign, &context(&Cancel::default())),
        Err(AccessPolicyJournalError::Record(ControlError::IdentityMismatch)));
    assert_eq!(journal.read_snapshot().unwrap(), before);

    let small = Scratch::new();
    let mut limited = small.create(JournalLimits { max_value_bytes: 128, ..JournalLimits::BASELINE });
    let read = limited.read_security_restriction(state().policy.namespace_id, &context(&Cancel::default())).unwrap();
    let command = read.initialize(op("too-big-for-config"), digest(), state(), owners()).unwrap();
    let before = limited.read_snapshot().unwrap();
    assert!(limited.commit_security_restriction(&command, &context(&Cancel::default())).is_err());
    assert_eq!(limited.read_snapshot().unwrap(), before);
}

#[test]
fn debug_excludes_memberships_domains_operations_and_policy_payloads() {
    let scratch = Scratch::new();
    let mut journal = scratch.create(JournalLimits::BASELINE);
    let commit = initial(&mut journal);
    let read = journal.read_security_restriction(state().policy.namespace_id, &context(&Cancel::default())).unwrap();
    let debug = format!("{:?} {:?} {:?} {:?}", state(), commit.mutation(), read, commit);
    for private in ["domain:fixture", "operation:one", "denied_memberships", "purged_memberships", "policy_digest"] {
        assert!(!debug.contains(private));
    }
}

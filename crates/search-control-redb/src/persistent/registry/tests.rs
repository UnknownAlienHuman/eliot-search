//! Real disk transaction/reopen proofs with injected technical owner metadata.
//! These do not qualify source admission, native custody or power-loss behavior.
use super::*;
use crate::persistent::operation::{Check, Point, Unscoped};
use crate::persistent::{Boundary, RECORDS, encode_value};
use crate::{
    CommitRecoveryDecision, ControlMutation, ControlPortCommand, ControlSnapshotPublisher,
    ControlWrite, MutationId, PersistentControlJournal, PublicationVisibilityState,
    control_mutation_identity,
};
use search_contracts::{
    CollectionGenerationId, DataRootId, Epoch, InstallationIncarnationId, OpaqueRef, OwnerEpoch,
    PublicationGuards, RequestId,
};
use search_ports::{
    CancellationProbe, ControlJournalPort, ControlSnapshotPort, OperationContext, PackageOpaque,
};
use std::cell::Cell;
use std::fs::{self, File, OpenOptions};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

const LIMITS: JournalLimits = JournalLimits::BASELINE;
static NEXT: AtomicU64 = AtomicU64::new(0);
#[path = "tests/original_context.rs"]
mod original_context;
#[derive(Clone, Debug)]
struct Cancel;
impl PackageOpaque for Cancel {
    fn owner_package(&self) -> &'static str {
        "search-control-redb"
    }
}
impl CancellationProbe for Cancel {
    fn is_cancelled(&self) -> bool {
        false
    }
}
fn context() -> OperationContext<Cancel> {
    OperationContext::new(
        RequestId::from_bytes([1; 16]),
        60_000,
        Cancel,
        OpaqueRef::new("budget:registry-disk-fixture").unwrap(),
    )
    .unwrap()
}
fn identity(version: u32) -> JournalIdentity {
    JournalIdentity {
        installation_incarnation_id: InstallationIncarnationId::from_bytes([1; 16]),
        data_root_id: DataRootId::from_bytes([2; 16]),
        owner_epoch: OwnerEpoch::new(1).unwrap(),
        path_identity_digest: Blake3Digest32::from_bytes([3; 32]),
        schema_family_digest: Blake3Digest32::from_bytes([4; 32]),
        schema_version: version,
    }
}
fn visibility() -> PublicationVisibilityState {
    PublicationVisibilityState {
        collection_generation_id: CollectionGenerationId::from_bytes([5; 16]),
        schema_identity_digest: Blake3Digest32::from_bytes([6; 32]),
        visible_epoch: Epoch::new(0).unwrap(),
        guards: PublicationGuards {
            owner_epoch: OwnerEpoch::new(1).unwrap(),
            source_catalog_generation: 0,
            membership_generation: 0,
            access_generation: 0,
            shadow_generation: 0,
            purge_generation: 0,
            profile_digest: Blake3Digest32::from_bytes([7; 32]),
        },
        last_receipt: None,
    }
}
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "eliot-registry-disk-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        Self(root)
    }
    fn file(&self) -> File {
        OpenOptions::new()
            .read(true)
            .write(true)
            .open(self.0.join("control.redb"))
            .unwrap()
    }
    fn create(&self, version: u32) -> PersistentControlJournal {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(self.0.join("control.redb"))
            .unwrap();
        let mut journal =
            PersistentControlJournal::create(file, identity(version), LIMITS).unwrap();
        if version == 5 {
            journal.initialize_operating_profile(
                crate::JournalProfileInitialization::Indexed(visibility()),
                MutationId([9; 32]),
                Blake3Digest32::from_bytes([8; 32]),
                &context(),
            ).unwrap();
        } else {
            journal.initialize_publication_visibility(visibility(), MutationId([9; 32]),
                Blake3Digest32::from_bytes([8; 32]), &context()).unwrap();
        }
        journal
    }
    fn reopen(&self) -> PersistentControlJournal {
        PersistentControlJournal::open(
            self.file(),
            identity(crate::REGISTRY_JOURNAL_SCHEMA_VERSION),
            LIMITS,
        )
        .unwrap()
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn state_key() -> RegistryStateKey {
    RegistryStateKey::new(
        RegistryStateFamily::Source,
        b"owner-canonical-key-fixture".to_vec(),
        LIMITS,
    )
    .unwrap()
}
fn request(
    id: &str,
    generation: u64,
    revision: u64,
    before: Option<&[u8]>,
    after: &[u8],
) -> RegistryMutationRequest {
    RegistryMutationRequest::new(
        OpaqueId::new(id).unwrap(),
        generation,
        revision,
        Blake3Digest32::from_bytes([11; 32]),
        b"owner-canonical-operation-fixture".to_vec(),
        vec![
            RegistryStateChange::new(
                state_key(),
                before.map(<[u8]>::to_vec),
                after.to_vec(),
                LIMITS,
            )
            .unwrap(),
        ],
        LIMITS,
    )
    .unwrap()
}

#[test]
fn registry_disk_restart_retains_operation_generation_and_distinct_revision() {
    let scratch = Scratch::new();
    let first = request("operation:first", 1, 0, None, b"source-A");
    {
        let mut journal = scratch.create(5);
        let result = journal.commit_registry_batch(&first, &context()).unwrap();
        assert_eq!(result.readback().journal_generation(), 2);
        assert_eq!(result.readback().registry_revision(), 1);
    }
    let mut reopened = scratch.reopen();
    let snapshot = reopened.read_registry_snapshot(&context()).unwrap();
    assert_eq!(
        snapshot
            .operation(first.envelope().operation_id())
            .unwrap()
            .journal_generation_before(),
        1
    );
    assert_eq!(snapshot.states()[0].canonical_bytes(), b"source-A");
    let replay = reopened.commit_registry_batch(&first, &context()).unwrap();
    assert!(replay.receipt().replayed);
    assert_eq!(reopened.committed_writes(), 0);
}

#[test]
fn registry_historical_replay_keeps_current_values_and_original_generation() {
    let scratch = Scratch::new();
    let mut journal = scratch.create(5);
    let first = request("operation:first", 1, 0, None, b"source-A");
    journal.commit_registry_batch(&first, &context()).unwrap();
    journal
        .commit_registry_batch(
            &request("operation:second", 2, 1, Some(b"source-A"), b"source-B"),
            &context(),
        )
        .unwrap();
    drop(journal);
    let mut reopened = scratch.reopen();
    let replay = reopened.commit_registry_batch(&first, &context()).unwrap();
    assert!(replay.receipt().replayed);
    assert_eq!(replay.receipt().before_generation, 1);
    assert_eq!(replay.readback().registry_revision(), 2);
    assert_eq!(replay.readback().states()[0].canonical_bytes(), b"source-B");
    let changed_generation = request("operation:first", 3, 0, None, b"source-A");
    assert_eq!(
        reopened
            .commit_registry_batch(&changed_generation, &context())
            .unwrap_err()
            .control_error(),
        ControlError::OperationConflict
    );
    assert_eq!(reopened.committed_writes(), 0);
}

#[test]
fn registry_false_before_or_revision_has_no_partial_commit_or_operation() {
    let scratch = Scratch::new();
    let mut journal = scratch.create(5);
    journal
        .commit_registry_batch(&request("first", 1, 0, None, b"A"), &context())
        .unwrap();
    let before = journal.verify().unwrap();
    let writes = journal.committed_writes();
    for bad in [
        request("bad-before", 2, 1, Some(b"wrong"), b"B"),
        request("bad-revision", 2, 7, Some(b"A"), b"B"),
    ] {
        assert_eq!(
            journal
                .commit_registry_batch(&bad, &context())
                .unwrap_err()
                .control_error(),
            ControlError::GenerationMismatch
        );
        assert_eq!(journal.verify().unwrap(), before);
    }
    assert_eq!(journal.committed_writes(), writes);
}

#[test]
fn registry_lost_acknowledgement_reopens_and_recovers_without_rewrite() {
    let scratch = Scratch::new();
    let mut journal = scratch.create(5);
    let first = request("unknown", 1, 0, None, b"A");
    assert_eq!(
        journal.commit_registry_checked(&first, Boundary::LostAcknowledgement, &Unscoped),
        Err(ControlError::CommitOutcomeUnknown)
    );
    assert!(journal.requires_recovery());
    drop(journal);
    let mut reopened = scratch.reopen();
    assert!(matches!(
        reopened.recover_registry_batch(&first, &context()).unwrap(),
        CommitRecoveryDecision::Committed(_)
    ));
    assert_eq!(
        reopened
            .read_registry_snapshot(&context())
            .unwrap()
            .registry_revision(),
        1
    );
    assert_eq!(reopened.committed_writes(), 0);
}

#[test]
fn registry_final_readback_refusal_retains_the_original_pending_fence() {
    struct Refuse(Cell<bool>);
    impl Check for Refuse {
        fn check(&self, point: Point) -> Result<(), ControlError> {
            if point == Point::MutationComplete {
                self.0.set(true);
                return Ok(());
            }
            if self.0.get() {
                return Err(ControlError::ReadCancelled);
            }
            Ok(())
        }
    }
    let scratch = Scratch::new();
    let mut journal = scratch.create(5);
    let first = request("late-readback", 1, 0, None, b"A");
    assert_eq!(
        journal.commit_registry_checked(&first, Boundary::Normal, &Refuse(Cell::new(false))),
        Err(ControlError::CommitOutcomeUnknown)
    );
    assert!(journal.requires_recovery());
    assert!(matches!(
        journal.recover_registry_batch(&first, &context()).unwrap(),
        CommitRecoveryDecision::Committed(_)
    ));
    assert!(!journal.requires_recovery());
}

#[test]
fn registry_generic_writes_and_deletes_cannot_replace_reserved_records() {
    let scratch = Scratch::new();
    let mut journal = scratch.create(5);
    let first = request("first", 1, 0, None, b"A");
    journal.commit_registry_batch(&first, &context()).unwrap();
    let before = journal.verify().unwrap();
    let key = codec::operation_key(first.envelope().operation_id(), LIMITS).unwrap();
    for (writes, deletes) in [
        (
            vec![ControlWrite {
                key: key.clone(),
                value: codec::operation_value(first.envelope(), LIMITS).unwrap(),
            }],
            vec![],
        ),
        (vec![], vec![key]),
    ] {
        assert_eq!(
            journal.transact(&ControlMutation::new(
                MutationId([13; 32]),
                Blake3Digest32::from_bytes([14; 32]),
                2,
                writes,
                deletes
            )),
            Err(ControlError::InvalidKey)
        );
        assert_eq!(journal.verify().unwrap(), before);
    }
}

#[test]
fn registry_full_raw_operation_id_and_envelope_conflicts_are_checked() {
    let scratch = Scratch::new();
    let mut journal = scratch.create(5);
    let common = "identical-first-thirty-two-characters-";
    let first = request(&format!("{common}A"), 1, 0, None, b"A");
    journal.commit_registry_batch(&first, &context()).unwrap();
    let second = request(&format!("{common}B"), 2, 1, Some(b"A"), b"B");
    assert_ne!(first.control_operation_id(), second.control_operation_id());
    journal.commit_registry_batch(&second, &context()).unwrap();
    let key = codec::operation_key(first.envelope().operation_id(), LIMITS).unwrap();
    assert!(matches!(
        codec::decode_record(
            &key,
            &codec::operation_value(second.envelope(), LIMITS).unwrap(),
            LIMITS
        ),
        Err(ControlError::OperationConflict)
    ));
    let mut different = first.clone();
    different.envelope.owner_bytes.push(1);
    assert_eq!(
        journal
            .commit_registry_batch(&different, &context())
            .unwrap_err()
            .control_error(),
        ControlError::OperationConflict
    );
}

#[test]
fn registry_active_item_and_wrapped_64k_limits_refuse_before_dispatch() {
    let oversized = vec![0; 256 * 1024];
    assert_eq!(
        RegistryStateChange::new(state_key(), None, oversized.clone(), LIMITS).unwrap_err(),
        ControlError::BudgetExceeded
    );
    let change = RegistryStateChange::new(state_key(), None, b"A".to_vec(), LIMITS).unwrap();
    assert_eq!(
        RegistryMutationRequest::new(
            OpaqueId::new("oversize").unwrap(),
            1,
            0,
            Blake3Digest32::from_bytes([1; 32]),
            oversized,
            vec![change.clone()],
            LIMITS
        )
        .unwrap_err(),
        ControlError::BudgetExceeded
    );
    assert_eq!(
        RegistryMutationRequest::new(
            OpaqueId::new("item-bound").unwrap(),
            1,
            0,
            Blake3Digest32::from_bytes([1; 32]),
            b"metadata".to_vec(),
            vec![change],
            JournalLimits {
                max_mutation_items: 5,
                ..LIMITS
            }
        )
        .unwrap_err(),
        ControlError::BudgetExceeded
    );
}

#[test]
fn registry_old_schema_collision_is_refused_on_actual_open() {
    let scratch = Scratch::new();
    let mut journal = scratch.create(4);
    let ordinary = ControlKey::new(b"historical-generic".to_vec(), LIMITS).unwrap();
    let value = ControlValue::new(
        ControlRecordClass::State,
        b"opaque-old-bytes".to_vec(),
        LIMITS,
    )
    .unwrap();
    journal
        .transact(&ControlMutation::new(
            MutationId([21; 32]),
            Blake3Digest32::from_bytes([22; 32]),
            1,
            vec![ControlWrite {
                key: ordinary.clone(),
                value: value.clone(),
            }],
            vec![],
        ))
        .unwrap();
    // Model bytes produced by an older binary before this prefix was reserved.
    let write = journal.database.begin_write().unwrap();
    {
        let mut table = write.open_table(RECORDS).unwrap();
        table.remove(ordinary.as_bytes()).unwrap();
        table
            .insert(
                b"eliot.control.registry.v1/old-generic".as_slice(),
                encode_value(&value).as_slice(),
            )
            .unwrap();
    }
    write.commit().unwrap();
    drop(journal);
    assert!(matches!(
        PersistentControlJournal::open(scratch.file(), identity(4), LIMITS),
        Err(ControlError::SchemaMismatch)
    ));
}

#[test]
fn registry_shared_port_suspends_snapshot_until_verified_publication() {
    let scratch = Scratch::new();
    let mut journal = scratch.create(5);
    let mut publisher = ControlSnapshotPublisher::new();
    let first = request("shared-port", 1, 0, None, b"A");
    let mut port = journal.bind_control_port::<Cancel>(&mut publisher).unwrap();
    port.recover_snapshot(&context()).unwrap();
    let command = ControlPortCommand::Registry(first.clone());
    let receipt = port
        .transact(
            &command,
            &context(),
            &control_mutation_identity(first.control_operation_id()).unwrap(),
        )
        .unwrap();
    assert!(port.current_snapshot().is_err());
    port.publish_committed_snapshot(&receipt, &context())
        .unwrap();
    assert_eq!(port.current_snapshot().unwrap().generation, 2);
}

#[test]
fn registry_normal_readback_work_does_not_scan_prior_operation_history() {
    struct Count(Cell<usize>);
    impl Check for Count {
        fn check(&self, point: Point) -> Result<(), ControlError> {
            if matches!(
                point,
                Point::ReadRecord | Point::ReadOperation | Point::Readback
            ) {
                self.0.set(self.0.get() + 1);
            }
            Ok(())
        }
    }
    let scratch = Scratch::new();
    let mut journal = scratch.create(5);
    let first = request("count-first", 1, 0, None, b"A");
    let count = Count(Cell::new(0));
    journal
        .commit_registry_checked(&first, Boundary::Normal, &count)
        .unwrap();
    let first_work = count.0.get();
    for revision in 1..8 {
        let next = request(
            &format!("count-{revision}"),
            revision + 1,
            revision,
            Some(b"A"),
            b"A",
        );
        let count = Count(Cell::new(0));
        journal
            .commit_registry_checked(&next, Boundary::Normal, &count)
            .unwrap();
        assert_eq!(count.0.get(), first_work);
    }
}

#[test]
fn registry_root_family_roundtrips_disk_snapshot_and_original_operation_recovery() {
    // Opaque technical fixture payloads; source owner validates actual RootRecord CBOR.
    let root_key = RegistryStateKey::new(
        RegistryStateFamily::Root,
        b"source-owner-root-key-fixture".to_vec(),
        LIMITS,
    )
    .unwrap();
    let first = RegistryMutationRequest::new(
        OpaqueId::new("root:original").unwrap(),
        1,
        0,
        Blake3Digest32::from_bytes([23; 32]),
        b"owner-root-operation-fixture".to_vec(),
        vec![
            RegistryStateChange::new(root_key.clone(), None, b"root-revision-1".to_vec(), LIMITS)
                .unwrap(),
        ],
        LIMITS,
    )
    .unwrap();
    let scratch = Scratch::new();
    let mut journal = scratch.create(5);
    let result = journal.commit_registry_batch(&first, &context()).unwrap();
    assert_eq!(
        result.readback().states()[0].key().family(),
        RegistryStateFamily::Root
    );
    let snapshot = journal.read_snapshot().unwrap();
    let physical_key = codec::state_key(&root_key, LIMITS).unwrap();
    assert_eq!(
        snapshot.get(&physical_key).unwrap().class(),
        ControlRecordClass::Identity
    );
    // Unknown family tags stay closed even when class and value bytes otherwise match.
    let mut unknown = physical_key.as_bytes().to_vec();
    unknown[b"eliot.control.registry.v1/state/".len()] = 99;
    assert!(
        codec::decode_record(
            &ControlKey::new(unknown, LIMITS).unwrap(),
            snapshot.get(&physical_key).unwrap(),
            LIMITS
        )
        .is_err()
    );
    drop(journal);
    let mut reopened = scratch.reopen();
    let restored = reopened.read_registry_snapshot(&context()).unwrap();
    assert_eq!(restored.states()[0].key(), &root_key);
    assert_eq!(restored.states()[0].canonical_bytes(), b"root-revision-1");
    assert_eq!(
        restored.operation(first.envelope().operation_id()).unwrap(),
        first.envelope()
    );
    assert!(matches!(
        reopened.recover_registry_batch(&first, &context()).unwrap(),
        CommitRecoveryDecision::Committed(_)
    ));
    let next = RegistryMutationRequest::new(
        OpaqueId::new("root:next").unwrap(),
        2,
        1,
        Blake3Digest32::from_bytes([24; 32]),
        b"owner-next-root-operation-fixture".to_vec(),
        vec![
            RegistryStateChange::new(
                root_key,
                Some(b"root-revision-1".to_vec()),
                b"root-revision-2".to_vec(),
                LIMITS,
            )
            .unwrap(),
        ],
        LIMITS,
    )
    .unwrap();
    reopened.commit_registry_batch(&next, &context()).unwrap();
    let replay = reopened.commit_registry_batch(&first, &context()).unwrap();
    assert!(replay.receipt().replayed);
    assert_eq!(
        replay.readback().states()[0].canonical_bytes(),
        b"root-revision-2"
    );
    assert_eq!(replay.readback().operation().journal_generation_before(), 1);
}

#[test]
fn registry_targeted_lookup_proves_absence_and_rechecks_immutable_current_targets() {
    let scratch = Scratch::new();
    let mut journal = scratch.create(5);
    let first = request("targeted:first", 1, 0, None, b"A");
    assert_eq!(journal.limits(), LIMITS);
    assert_eq!(
        journal
            .lookup_registry_operation(first.envelope().operation_id(), &context())
            .unwrap(),
        RegistryOperationLookup::Absent {
            identity: identity(5),
            journal_generation: 1,
            registry_revision: 0
        }
    );
    journal.commit_registry_batch(&first, &context()).unwrap();
    let RegistryOperationLookup::Committed(observed) = journal
        .lookup_registry_operation(first.envelope().operation_id(), &context())
        .unwrap()
    else {
        panic!("operation");
    };
    assert_eq!(
        (
            observed.identity(),
            observed.journal_generation(),
            observed.registry_revision()
        ),
        (identity(5), 2, 1)
    );
    assert_eq!(observed.envelope(), first.envelope());
    journal
        .commit_registry_batch(
            &request("targeted:second", 2, 1, Some(b"A"), b"B"),
            &context(),
        )
        .unwrap();
    // The first immutable operation is unchanged, while final coordinates/targets
    // all come from one newer coherent read. No mixed-generation state is returned.
    let read = journal
        .read_registry_operation_targets(observed.envelope(), &[state_key()], &context())
        .unwrap();
    assert_eq!(
        (read.journal_generation(), read.registry_revision()),
        (3, 2)
    );
    assert_eq!(read.operation(), first.envelope());
    assert_eq!(read.states()[0].canonical_bytes(), b"B");
    let mut forged = observed.envelope().clone();
    forged.owner_bytes.push(0);
    assert_eq!(
        journal
            .read_registry_operation_targets(&forged, &[state_key()], &context())
            .unwrap_err()
            .control_error(),
        ControlError::OperationConflict
    );
    let mut publisher = ControlSnapshotPublisher::new();
    let port = journal.bind_control_port::<Cancel>(&mut publisher).unwrap();
    assert_eq!(port.limits(), LIMITS);
    assert!(matches!(
        port.lookup_registry_operation(first.envelope().operation_id(), &context())
            .unwrap(),
        RegistryOperationLookup::Committed(_)
    ));
    assert_eq!(
        port.read_registry_operation_targets(first.envelope(), &[state_key()], &context())
            .unwrap()
            .registry_revision(),
        2
    );
}

#[test]
fn registry_targeted_duplicate_missing_and_active_key_bounds_refuse_whole_read() {
    let scratch = Scratch::new();
    let mut journal = scratch.create(5);
    let first = request("targeted:bounds", 1, 0, None, b"A");
    journal.commit_registry_batch(&first, &context()).unwrap();
    assert_eq!(
        journal
            .read_registry_operation_targets(
                first.envelope(),
                &[state_key(), state_key()],
                &context()
            )
            .unwrap_err()
            .control_error(),
        ControlError::DuplicateMutationKey
    );
    let missing =
        RegistryStateKey::new(RegistryStateFamily::Root, b"missing-root".to_vec(), LIMITS).unwrap();
    assert_eq!(
        journal
            .read_registry_operation_targets(first.envelope(), &[missing], &context())
            .unwrap_err()
            .control_error(),
        ControlError::StoreCorrupt
    );
    let oversized = RegistryStateKey::new(
        RegistryStateFamily::Root,
        vec![1; 5000],
        JournalLimits {
            max_key_bytes: 8192,
            ..LIMITS
        },
    )
    .unwrap();
    assert_eq!(
        journal
            .read_registry_operation_targets(first.envelope(), &[oversized], &context())
            .unwrap_err()
            .control_error(),
        ControlError::BudgetExceeded
    );
    assert_eq!(
        journal
            .read_registry_operation_targets(first.envelope(), &[], &context())
            .unwrap_err()
            .control_error(),
        ControlError::BudgetExceeded
    );
    assert_eq!(
        journal
            .read_registry_operation_targets(
                first.envelope(),
                &vec![state_key(); LIMITS.max_mutation_items + 1],
                &context()
            )
            .unwrap_err()
            .control_error(),
        ControlError::BudgetExceeded
    );
}

#[test]
fn registry_targeted_pending_and_partial_state_never_return_absent() {
    let scratch = Scratch::new();
    let mut journal = scratch.create(5);
    let first = request("targeted:unknown", 1, 0, None, b"A");
    assert_eq!(
        journal.commit_registry_checked(&first, Boundary::LostAcknowledgement, &Unscoped),
        Err(ControlError::CommitOutcomeUnknown)
    );
    assert_eq!(
        journal
            .lookup_registry_operation(first.envelope().operation_id(), &context())
            .unwrap_err()
            .control_error(),
        ControlError::CommitOutcomeUnknown
    );
    assert_eq!(
        journal
            .read_registry_operation_targets(first.envelope(), &[state_key()], &context())
            .unwrap_err()
            .control_error(),
        ControlError::CommitOutcomeUnknown
    );
    journal.recover_registry_batch(&first, &context()).unwrap();
    // Keep record/value counters exact while damaging just the revision locator.
    let write = journal.database.begin_write().unwrap();
    {
        let mut table = write.open_table(RECORDS).unwrap();
        let raw = table.remove(REVISION).unwrap().unwrap().value().to_vec();
        table
            .insert(b"unrelated-counter".as_slice(), raw.as_slice())
            .unwrap();
    }
    write.commit().unwrap();
    assert_eq!(
        journal
            .lookup_registry_operation(&OpaqueId::new("uncommitted-id").unwrap(), &context())
            .unwrap_err()
            .control_error(),
        ControlError::StoreCorrupt
    );
}

#[test]
fn registry_targeted_detached_operation_receipt_is_error_not_absence() {
    let scratch = Scratch::new();
    let mut journal = scratch.create(5);
    let first = request("targeted:detached", 1, 0, None, b"A");
    journal.commit_registry_batch(&first, &context()).unwrap();
    let key = codec::operation_key(first.envelope().operation_id(), LIMITS).unwrap();
    let write = journal.database.begin_write().unwrap();
    {
        let mut table = write.open_table(RECORDS).unwrap();
        let raw = table
            .remove(key.as_bytes())
            .unwrap()
            .unwrap()
            .value()
            .to_vec();
        table
            .insert(b"detached-operation".as_slice(), raw.as_slice())
            .unwrap();
    }
    write.commit().unwrap();
    assert_eq!(
        journal
            .lookup_registry_operation(first.envelope().operation_id(), &context())
            .unwrap_err()
            .control_error(),
        ControlError::StoreCorrupt
    );
}

#[test]
fn registry_targeted_normal_lookup_work_is_independent_of_history_length() {
    struct Count(Cell<usize>);
    impl Check for Count {
        fn check(&self, point: Point) -> Result<(), ControlError> {
            if matches!(
                point,
                Point::ReadRecord | Point::ReadOperation | Point::Readback
            ) {
                self.0.set(self.0.get() + 1);
            }
            Ok(())
        }
    }
    let scratch = Scratch::new();
    let mut journal = scratch.create(5);
    let first = request("targeted:scaling-first", 1, 0, None, b"A");
    journal.commit_registry_batch(&first, &context()).unwrap();
    let count = Count(Cell::new(0));
    journal
        .lookup_registry_checked(first.envelope().operation_id(), &count)
        .unwrap();
    journal
        .read_registry_targets_checked(first.envelope(), &[state_key()], &count)
        .unwrap();
    let expected = count.0.get();
    for revision in 1..8 {
        journal
            .commit_registry_batch(
                &request(
                    &format!("targeted:scaling-{revision}"),
                    revision + 1,
                    revision,
                    Some(b"A"),
                    b"A",
                ),
                &context(),
            )
            .unwrap();
        let count = Count(Cell::new(0));
        journal
            .lookup_registry_checked(first.envelope().operation_id(), &count)
            .unwrap();
        journal
            .read_registry_targets_checked(first.envelope(), &[state_key()], &count)
            .unwrap();
        assert_eq!(count.0.get(), expected);
    }
}

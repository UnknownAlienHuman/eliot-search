//! Actual disk/profile proofs with opaque technical metadata, not source/native qualification.
use super::super::{Unscoped, encode_value};
use super::*;
use crate::{
    ControlPortCommand, ControlSnapshotPublisher, RegistryMutationRequest, RegistryStateChange,
    RegistryStateFamily, RegistryStateKey, control_mutation_identity,
};
use search_contracts::{
    Blake3Digest32, CollectionGenerationId, DataRootId, Epoch, InstallationIncarnationId, OpaqueId,
    OpaqueRef, PublicationGuards, RequestId,
};
use search_ports::{ControlJournalPort, ControlSnapshotPort, PackageOpaque};
use std::fs::{self, OpenOptions};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

const LIMITS: JournalLimits = JournalLimits::BASELINE;
const INIT: MutationId = MutationId([9; 32]);
const DIGEST: Blake3Digest32 = Blake3Digest32::from_bytes([8; 32]);
static NEXT: AtomicU64 = AtomicU64::new(0);
#[derive(Debug)]
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
        OpaqueRef::new("operating-profile-fixture").unwrap(),
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
        let path = std::env::temp_dir().join(format!(
            "eliot-profile-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn create(&self, version: u32) -> PersistentControlJournal {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(self.0.join("control.redb"))
            .unwrap();
        PersistentControlJournal::create(file, identity(version), LIMITS).unwrap()
    }
    fn open(&self) -> Result<PersistentControlJournal, ControlError> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(self.0.join("control.redb"))
            .unwrap();
        PersistentControlJournal::open(file, identity(5), LIMITS)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn request(
    id: &str,
    generation: u64,
    revision: u64,
    previous: Option<&[u8]>,
    after: &[u8],
) -> RegistryMutationRequest {
    let changes = [RegistryStateFamily::Root, RegistryStateFamily::Source]
        .into_iter()
        .map(|family| {
            let key =
                RegistryStateKey::new(family, b"owner-canonical-key".to_vec(), LIMITS).unwrap();
            RegistryStateChange::new(key, previous.map(<[u8]>::to_vec), after.to_vec(), LIMITS)
                .unwrap()
        })
        .collect();
    RegistryMutationRequest::new(
        OpaqueId::new(id).unwrap(),
        generation,
        revision,
        DIGEST,
        b"owner-canonical-operation".to_vec(),
        changes,
        LIMITS,
    )
    .unwrap()
}

#[test]
fn operating_profile_direct_root_source_restart_publishes_without_route_and_replays_history() {
    let scratch = Scratch::new();
    let mut journal = scratch.create(5);
    journal
        .initialize_operating_profile(
            JournalProfileInitialization::Direct,
            INIT,
            DIGEST,
            &context(),
        )
        .unwrap();
    let first = request("direct-first", 1, 0, None, b"owner-state-A");
    let mut publisher = ControlSnapshotPublisher::new();
    journal
        .recover_snapshot_publication_with_context(&mut publisher, &context())
        .unwrap();
    assert_eq!(publisher.current().unwrap().generation, 1);
    {
        let mut bound = journal.bind_control_port::<Cancel>(&mut publisher).unwrap();
        assert!(bound.current_snapshot().is_err());
        let receipt = bound
            .transact(
                &ControlPortCommand::Registry(first.clone()),
                &context(),
                &control_mutation_identity(first.control_operation_id()).unwrap(),
            )
            .unwrap();
        assert!(bound.current_snapshot().is_err());
        bound
            .publish_committed_snapshot(&receipt, &context())
            .unwrap();
        assert_eq!(bound.current_snapshot().unwrap().generation, 2);
    }
    assert_eq!(
        journal
            .read_registry_snapshot(&context())
            .unwrap()
            .states()
            .len(),
        2
    );
    journal
        .commit_registry_batch(
            &request(
                "direct-second",
                2,
                1,
                Some(b"owner-state-A"),
                b"owner-state-B",
            ),
            &context(),
        )
        .unwrap();
    drop(journal);
    let mut reopened = scratch.open().unwrap();
    assert_eq!(
        reopened.operating_profile(&context()).unwrap(),
        JournalOperatingProfile::Direct
    );
    let replay = reopened.commit_registry_batch(&first, &context()).unwrap();
    assert!(replay.receipt().replayed);
    assert_eq!(replay.receipt().after_generation, 2);
    assert_eq!(replay.readback().journal_generation(), 3);
    assert!(
        replay
            .readback()
            .states()
            .iter()
            .all(|state| state.canonical_bytes() == b"owner-state-B")
    );
    assert_eq!(reopened.committed_writes(), 0);
}

#[test]
fn operating_profile_direct_rejects_generic_profile_and_index_claims_and_later_transition() {
    let scratch = Scratch::new();
    let mut journal = scratch.create(5);
    journal
        .initialize_operating_profile(
            JournalProfileInitialization::Direct,
            INIT,
            DIGEST,
            &context(),
        )
        .unwrap();
    let mode_key = ControlKey::new(KEY.to_vec(), LIMITS).unwrap();
    for key in [
        KEY,
        super::super::publication::KEY,
        super::super::publication::visibility::STATE,
        b"publication_receipts/visible/v1/fake".as_slice(),
    ] {
        let mutation = ControlMutation::new(
            MutationId([10; 32]),
            DIGEST,
            1,
            vec![ControlWrite {
                key: ControlKey::new(key.to_vec(), LIMITS).unwrap(),
                value: ControlValue::new(ControlRecordClass::State, b"fake".to_vec(), LIMITS)
                    .unwrap(),
            }],
            vec![],
        );
        assert!(journal.transact(&mutation).is_err());
    }
    assert!(
        journal
            .transact(&ControlMutation::new(
                MutationId([10; 32]),
                DIGEST,
                1,
                vec![],
                vec![mode_key]
            ))
            .is_err()
    );
    assert_eq!(
        journal
            .initialize_publication_visibility(
                visibility(),
                MutationId([11; 32]),
                DIGEST,
                &context()
            )
            .unwrap_err()
            .control_error(),
        ControlError::MigrationUnverified
    );
    assert!(
        journal
            .initialize_operating_profile(
                JournalProfileInitialization::Indexed(visibility()),
                INIT,
                DIGEST,
                &context()
            )
            .is_err()
    );
    assert_eq!(journal.read_snapshot().unwrap().generation, 1);
    assert_eq!(
        journal
            .read_publication_checkpoint(&context())
            .unwrap_err()
            .control_error(),
        ControlError::MigrationUnverified
    );
}

#[test]
fn operating_profile_inert_allows_only_migration_and_refuses_registry_and_publication() {
    let scratch = Scratch::new();
    let mut journal = scratch.create(5);
    journal
        .initialize_operating_profile(
            JournalProfileInitialization::MigrationInert,
            INIT,
            DIGEST,
            &context(),
        )
        .unwrap();
    journal
        .transact(&ControlMutation::new(
            MutationId([12; 32]),
            DIGEST,
            1,
            vec![ControlWrite {
                key: ControlKey::new(b"migration/original".to_vec(), LIMITS).unwrap(),
                value: ControlValue::new(
                    ControlRecordClass::Migration,
                    b"original-input".to_vec(),
                    LIMITS,
                )
                .unwrap(),
            }],
            vec![],
        ))
        .unwrap();
    let mut publisher = ControlSnapshotPublisher::new();
    assert!(journal.bind_control_port::<Cancel>(&mut publisher).is_err());
    assert!(
        journal
            .lookup_registry_operation(&OpaqueId::new("absent").unwrap(), &context())
            .is_err()
    );
    assert!(
        journal
            .commit_registry_batch(&request("inert-forbidden", 2, 0, None, b"A"), &context())
            .is_err()
    );
    assert!(journal.read_registry_snapshot(&context()).is_err());
    assert!(
        journal
            .recover_snapshot_publication_with_context(&mut publisher, &context())
            .is_err()
    );
    assert!(publisher.current().is_none());
    drop(journal);
    let reopened = scratch.open().unwrap();
    assert_eq!(
        reopened.operating_profile(&context()).unwrap(),
        JournalOperatingProfile::MigrationInert
    );
    assert_eq!(reopened.read_snapshot().unwrap().generation, 2);
}

#[test]
fn operating_profile_indexed_atomic_seed_and_missing_state_refusal() {
    let scratch = Scratch::new();
    let mut journal = scratch.create(5);
    let mut invalid = visibility();
    invalid.guards.owner_epoch = OwnerEpoch::new(2).unwrap();
    assert!(
        journal
            .initialize_operating_profile(
                JournalProfileInitialization::Indexed(invalid),
                INIT,
                DIGEST,
                &context()
            )
            .is_err()
    );
    assert_eq!(journal.read_snapshot().unwrap().generation, 0);
    journal
        .initialize_operating_profile(
            JournalProfileInitialization::Indexed(visibility()),
            INIT,
            DIGEST,
            &context(),
        )
        .unwrap();
    assert_eq!(
        journal.read_publication_visibility(&context()).unwrap(),
        Some(visibility())
    );
    let write = journal.database.begin_write().unwrap();
    {
        let mut table = write.open_table(RECORDS).unwrap();
        table
            .remove(super::super::publication::visibility::STATE)
            .unwrap();
    }
    write.commit().unwrap();
    drop(journal);
    assert!(scratch.open().is_err());
}

#[test]
fn operating_profile_initialization_unknown_conflict_exact_recovery_and_historical_replay() {
    let aborted = Scratch::new();
    let mut before = aborted.create(5);
    assert_eq!(
        before
            .initialize_profile_checked(
                JournalProfileInitialization::Direct,
                INIT,
                DIGEST,
                Boundary::BeforeCommit,
                &Unscoped
            )
            .unwrap_err(),
        ControlError::StoreUnavailable
    );
    assert_eq!(
        before
            .recover_operating_profile_initialization(
                JournalProfileInitialization::Direct,
                INIT,
                DIGEST,
                &context()
            )
            .unwrap(),
        CommitRecoveryDecision::NotCommittedRetrySameOperation
    );
    before
        .initialize_operating_profile(
            JournalProfileInitialization::Direct,
            INIT,
            DIGEST,
            &context(),
        )
        .unwrap();
    let scratch = Scratch::new();
    let mut journal = scratch.create(5);
    assert_eq!(
        journal
            .initialize_profile_checked(
                JournalProfileInitialization::Direct,
                INIT,
                DIGEST,
                Boundary::LostAcknowledgement,
                &Unscoped
            )
            .unwrap_err(),
        ControlError::CommitOutcomeUnknown
    );
    assert!(journal.operating_profile(&context()).is_err());
    assert_eq!(
        journal
            .recover_operating_profile_initialization(
                JournalProfileInitialization::MigrationInert,
                INIT,
                DIGEST,
                &context()
            )
            .unwrap(),
        CommitRecoveryDecision::ConflictingInput
    );
    assert!(journal.requires_recovery());
    assert!(matches!(
        journal
            .recover_operating_profile_initialization(
                JournalProfileInitialization::Direct,
                INIT,
                DIGEST,
                &context()
            )
            .unwrap(),
        CommitRecoveryDecision::Committed(_)
    ));
    journal
        .commit_registry_batch(&request("after-recovery", 1, 0, None, b"A"), &context())
        .unwrap();
    let writes = journal.committed_writes();
    let replay = journal
        .initialize_operating_profile(
            JournalProfileInitialization::Direct,
            INIT,
            DIGEST,
            &context(),
        )
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.after_generation, 1);
    assert_eq!(journal.committed_writes(), writes);
}

#[test]
fn operating_profile_codec_original_join_and_old_schema_semantics() {
    let scratch = Scratch::new();
    let mut journal = scratch.create(5);
    journal
        .initialize_operating_profile(
            JournalProfileInitialization::Direct,
            INIT,
            DIGEST,
            &context(),
        )
        .unwrap();
    let forged = Record {
        initial: JournalProfileInitialization::MigrationInert,
        operation: INIT,
        epoch: identity(5).owner_epoch,
    }
    .value(LIMITS)
    .unwrap();
    let write = journal.database.begin_write().unwrap();
    {
        let mut table = write.open_table(RECORDS).unwrap();
        table.insert(KEY, encode_value(&forged).as_slice()).unwrap();
    }
    write.commit().unwrap();
    drop(journal);
    assert!(scratch.open().is_err());
    let legacy = Scratch::new();
    let mut old = legacy.create(4);
    old.initialize_publication_visibility(visibility(), INIT, DIGEST, &context())
        .unwrap();
    old.transact(&ControlMutation::new(
        MutationId([13; 32]),
        DIGEST,
        1,
        vec![ControlWrite {
            key: ControlKey::new(KEY.to_vec(), LIMITS).unwrap(),
            value: ControlValue::new(
                ControlRecordClass::Migration,
                b"legacy-generic".to_vec(),
                LIMITS,
            )
            .unwrap(),
        }],
        vec![],
    ))
    .unwrap();
    assert_eq!(old.verify().unwrap().generation, 2);
}

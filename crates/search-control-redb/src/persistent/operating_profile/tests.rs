//! Actual disk/profile proofs with opaque technical metadata, not source/native qualification.
use super::super::{Unscoped, encode_value};
use super::*;
use crate::{
    ControlPortCommand, ControlSnapshotPublisher, RegistryMutationRequest, RegistryStateChange,
    RegistryStateFamily, RegistryStateKey, MigrationBootstrapWork, control_mutation_identity,
};
use crate::policy_codec::AccessPolicyRecord;
use crate::security_restriction::{
    MAX_RESTRICTION_DEPENDENTS, SecurityPolicyState,
};
use search_contracts::{
    AccessPolicyRevision, Blake3Digest32, BoundedSet, CollectionGenerationId, DataRootId, Epoch,
    InstallationIncarnationId, OpaqueId, OpaqueRef, PublicationGuards, PurgeFenceRevision,
    RequestId, ShadowFenceRevision, SourceNamespaceId, SourceOwnerGeneration,
};
use search_ports::{ControlJournalPort, ControlSnapshotPort, PackageOpaque};
use std::fs::{self, OpenOptions};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

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

fn guard_security_state() -> SecurityPolicyState {
    SecurityPolicyState {
        policy: AccessPolicyRecord {
            namespace_id: SourceNamespaceId::from_bytes([31; 16]),
            owner_generation: SourceOwnerGeneration::from_bytes([32; 32]),
            policy_revision: AccessPolicyRevision::new(1),
            live_deny_generation: 1,
            shadow_fence_revision: ShadowFenceRevision::new(1),
            purge_fence_revision: PurgeFenceRevision::new(1),
            policy_digest: Blake3Digest32::from_bytes([33; 32]),
        },
        security_domain_ref: OpaqueRef::new("domain:guard-image-fixture").unwrap(),
        snapshot_digest: Blake3Digest32::from_bytes([34; 32]),
        denied_memberships: BoundedSet::empty(),
        purged_memberships: BoundedSet::empty(),
        fail_closed: false,
    }
}

fn guard_security_owners() -> BoundedSet<OpaqueId, MAX_RESTRICTION_DEPENDENTS> {
    BoundedSet::from_items([OpaqueId::new("owner:guard-image-fixture").unwrap()]).unwrap()
}

fn publication_guards(source: u64, membership: u64, access: u64) -> PublicationGuards {
    PublicationGuards {
        source_catalog_generation: source,
        membership_generation: membership,
        access_generation: access,
        shadow_generation: 1,
        purge_generation: 1,
        ..visibility().guards
    }
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
fn operating_profile_indexed_genesis_refuses_valid_equal_size_seed_substitution() {
    let scratch = Scratch::new();
    let mut journal = scratch.create(5);
    journal
        .initialize_operating_profile(
            JournalProfileInitialization::Indexed(visibility()),
            INIT,
            DIGEST,
            &context(),
        )
        .unwrap();
    let mut substituted = visibility();
    substituted.collection_generation_id = CollectionGenerationId::from_bytes([99; 16]);
    let expected =
        super::super::publication::visibility::codec::state(&visibility(), LIMITS).unwrap();
    let replacement =
        super::super::publication::visibility::codec::state(&substituted, LIMITS).unwrap();
    assert_eq!(expected.as_bytes().len(), replacement.as_bytes().len());
    let write = journal.database.begin_write().unwrap();
    {
        let mut records = write.open_table(RECORDS).unwrap();
        let encoded = encode_value(&replacement);
        records
            .insert(
                super::super::publication::visibility::STATE,
                encoded.as_slice(),
            )
            .unwrap();
    }
    write.commit().unwrap();
    assert!(journal.read_snapshot().is_err());
    assert!(journal.read_publication_visibility(&context()).is_err());
    drop(journal);
    assert!(scratch.open().is_err());
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

#[test]
fn operating_profile_adoption_preserves_genesis_and_sources_across_restart_and_replay() {
    let scratch = Scratch::new();
    let mut journal = scratch.create(5);
    journal.initialize_operating_profile(JournalProfileInitialization::Direct, INIT, DIGEST, &context()).unwrap();
    journal.commit_registry_batch(&request("before-adoption", 1, 0, None, b"A"), &context()).unwrap();
    let before = journal.read_snapshot().unwrap();
    let profile_key = ControlKey::new(KEY.to_vec(), LIMITS).unwrap();
    let genesis = before.get(&profile_key).unwrap().clone();
    let prepared = journal.prepare_indexed_profile_adoption(visibility(), MutationId([61; 32]), DIGEST, &context()).unwrap();
    assert_eq!(prepared.expected_generation(), 2);
    let receipt = journal.adopt_indexed_profile(&prepared, &context()).unwrap();
    assert_eq!((receipt.before_generation, receipt.after_generation), (2, 3));
    let observed = journal.read_projection_snapshot_checked(&Unscoped).unwrap().unwrap();
    assert_eq!(observed.initial_route_revision().unwrap().get(), 1);
    let mut foreign = *observed.visibility();
    foreign.collection_generation_id = CollectionGenerationId::from_bytes([99; 16]);
    assert_eq!(initial_route_revision(&journal.read_snapshot().unwrap(), &foreign, LIMITS).unwrap(), None);
    assert_eq!(journal.operating_profile(&context()).unwrap(), JournalOperatingProfile::Indexed);
    assert_eq!(journal.read_snapshot().unwrap().get(&profile_key), Some(&genesis));
    assert_eq!(journal.read_registry_snapshot(&context()).unwrap().states().len(), 2);
    let writes = journal.committed_writes();
    assert!(journal.adopt_indexed_profile(&prepared, &context()).unwrap().replayed);
    assert_eq!(journal.committed_writes(), writes);
    journal.commit_registry_batch(&request("after-adoption", 3, 1, Some(b"A"), b"B"), &context()).unwrap();
    drop(journal);
    let mut reopened = scratch.open().unwrap();
    assert_eq!(reopened.operating_profile(&context()).unwrap(), JournalOperatingProfile::Indexed);
    assert_eq!(reopened.read_projection_snapshot_checked(&Unscoped).unwrap().unwrap().initial_route_revision().unwrap().get(), 1);
    assert_eq!(reopened.read_publication_visibility(&context()).unwrap(), Some(visibility()));
    let recovered = reopened.prepare_indexed_profile_adoption(visibility(), MutationId([61; 32]), DIGEST, &context()).unwrap();
    assert_eq!(recovered, prepared);
    assert!(reopened.adopt_indexed_profile(&recovered, &context()).unwrap().replayed);
    assert_eq!(reopened.read_snapshot().unwrap().get(&profile_key), Some(&genesis));
}

#[test]
fn operating_profile_adoption_stale_or_substituted_inputs_and_generic_mutation_refuse() {
    let scratch = Scratch::new();
    let mut journal = scratch.create(5);
    journal.initialize_operating_profile(JournalProfileInitialization::Direct, INIT, DIGEST, &context()).unwrap();
    let prepared = journal.prepare_indexed_profile_adoption(visibility(), MutationId([62; 32]), DIGEST, &context()).unwrap();
    journal.commit_registry_batch(&request("intervening", 1, 0, None, b"A"), &context()).unwrap();
    assert_eq!(journal.adopt_indexed_profile(&prepared, &context()).unwrap_err().control_error(), ControlError::TransactionConflict);
    let prepared = journal.prepare_indexed_profile_adoption(visibility(), MutationId([62; 32]), DIGEST, &context()).unwrap();
    let forbidden = ControlMutation::new(MutationId([63; 32]), DIGEST, 2, vec![ControlWrite {
        key: ControlKey::new(adoption::KEY.to_vec(), LIMITS).unwrap(),
        value: ControlValue::new(ControlRecordClass::State, b"forbidden".to_vec(), LIMITS).unwrap(),
    }], vec![]);
    assert_eq!(journal.transact(&forbidden).unwrap_err(), ControlError::InvalidKey);
    journal.adopt_indexed_profile(&prepared, &context()).unwrap();
    let mut substituted = visibility();
    substituted.collection_generation_id = CollectionGenerationId::from_bytes([64; 16]);
    assert_eq!(journal.prepare_indexed_profile_adoption(substituted, MutationId([62; 32]), DIGEST, &context()).unwrap_err().control_error(), ControlError::OperationConflict);
    assert_eq!(journal.read_publication_visibility(&context()).unwrap(), Some(visibility()));
}

#[test]
fn operating_profile_adoption_lost_ack_requires_original_recovery_and_rejects_tampered_record() {
    let scratch = Scratch::new();
    let mut journal = scratch.create(5);
    journal.initialize_operating_profile(JournalProfileInitialization::Direct, INIT, DIGEST, &context()).unwrap();
    let prepared = journal.prepare_indexed_profile_adoption(visibility(), MutationId([65; 32]), DIGEST, &context()).unwrap();
    assert_eq!(journal.adopt_indexed_checked(&prepared, Boundary::LostAcknowledgement, &Unscoped).unwrap_err(), ControlError::CommitOutcomeUnknown);
    assert!(journal.operating_profile(&context()).is_err());
    assert!(matches!(journal.recover_indexed_profile_adoption(&prepared, &context()).unwrap(), CommitRecoveryDecision::Committed(_)));
    assert_eq!(journal.operating_profile(&context()).unwrap(), JournalOperatingProfile::Indexed);
    let write = journal.database.begin_write().unwrap();
    {
        let mut records = write.open_table(RECORDS).unwrap();
        let key = ControlKey::new(adoption::KEY.to_vec(), LIMITS).unwrap();
        let mut bytes = journal.read_snapshot().unwrap().get(&key).unwrap().as_bytes().to_vec();
        bytes[40] ^= 1;
        let forged = ControlValue::new(ControlRecordClass::State, bytes, LIMITS).unwrap();
        records.insert(adoption::KEY, encode_value(&forged).as_slice()).unwrap();
    }
    write.commit().unwrap();
    assert!(journal.read_snapshot().is_err());
    drop(journal);
    assert!(scratch.open().is_err());
}

#[test]
#[allow(clippy::too_many_lines)] // One end-to-end redb regression covers both source-row preconditions.
fn publication_guard_image_joins_exact_registry_and_security_rows() {
    let scratch = Scratch::new();
    let mut journal = scratch.create(5);
    journal
        .initialize_operating_profile(
            JournalProfileInitialization::Indexed(visibility()),
            INIT,
            DIGEST,
            &context(),
        )
        .unwrap();

    let first_registry = request("guard-registry:first", 1, 0, None, b"owner-state-one");
    journal
        .commit_registry_batch(&first_registry, &context())
        .unwrap();
    let namespace = guard_security_state().policy.namespace_id;
    let initial_security = journal
        .read_security_restriction(namespace, &context())
        .unwrap();
    let initial_security_mutation = initial_security
        .initialize(
            OpaqueId::new("guard-security:first").unwrap(),
            DIGEST,
            guard_security_state(),
            guard_security_owners(),
        )
        .unwrap();
    let initial_security_commit = journal
        .commit_security_restriction(&initial_security_mutation, &context())
        .unwrap();

    let registry = journal.read_registry_snapshot(&context()).unwrap();
    let security = journal
        .read_security_restriction(namespace, &context())
        .unwrap();
    let checkpoint = journal
        .read_publication_checkpoint(&context())
        .unwrap()
        .unwrap();
    assert_eq!(registry.registry_revision(), 1);
    assert_eq!(checkpoint.generation(), 3);

    let work = |_unit: MigrationBootstrapWork| Ok::<(), ()>(());
    let deadline = Instant::now() + Duration::from_secs(60);
    {
        let mut publisher = ControlSnapshotPublisher::new();
        let mut bound = journal
            .bind_control_port_with_original_context::<Cancel, _>(
                &mut publisher,
                deadline,
                &work,
            )
            .unwrap();
        let first_request = bound
            .prepare_publication_guard_image_with_original_context(
                &checkpoint,
                &registry,
                std::slice::from_ref(&security),
                publication_guards(17, 29, 1),
                MutationId([71; 32]),
                DIGEST,
                deadline,
                &work,
            )
            .unwrap();
        let first_identity = control_mutation_identity(first_request.operation_id()).unwrap();
        let first_commit = bound
            .transact_publication_guard_image_with_original_context(
                &first_request,
                &first_identity,
                deadline,
                &work,
            )
            .unwrap();
        assert_eq!(first_commit.commit().before_generation, 3);
        assert_eq!(first_commit.commit().after_generation, 4);
        assert_eq!(first_commit.checkpoint().visibility().visible_epoch, Epoch::new(0).unwrap());
        assert_eq!(first_commit.checkpoint().visibility().last_receipt, None);
        assert_eq!(first_commit.checkpoint().visibility().guards, publication_guards(17, 29, 1));
    }

    let second_registry = request(
        "guard-registry:second",
        4,
        1,
        Some(b"owner-state-one"),
        b"owner-state-two",
    );
    journal
        .commit_registry_batch(&second_registry, &context())
        .unwrap();
    let checkpoint_after_registry = journal
        .read_publication_checkpoint(&context())
        .unwrap()
        .unwrap();
    {
        let mut publisher = ControlSnapshotPublisher::new();
        let mut bound = journal
            .bind_control_port_with_original_context::<Cancel, _>(
                &mut publisher,
                deadline,
                &work,
            )
            .unwrap();
        let stale_registry_request = bound
            .prepare_publication_guard_image_with_original_context(
                &checkpoint_after_registry,
                &registry,
                std::slice::from_ref(&security),
                publication_guards(18, 30, 1),
                MutationId([72; 32]),
                DIGEST,
                deadline,
                &work,
            )
            .unwrap();
        let stale_registry_identity =
            control_mutation_identity(stale_registry_request.operation_id()).unwrap();
        assert_eq!(
            bound
                .transact_publication_guard_image_with_original_context(
                    &stale_registry_request,
                    &stale_registry_identity,
                    deadline,
                    &work,
                )
                .unwrap_err()
                .control,
            ControlError::GenerationMismatch,
        );
    }

    let current_security = journal
        .read_security_restriction(namespace, &context())
        .unwrap();
    let mut replacement_security = guard_security_state();
    replacement_security.policy.policy_revision = AccessPolicyRevision::new(2);
    replacement_security.policy.live_deny_generation = 2;
    replacement_security.snapshot_digest = Blake3Digest32::from_bytes([35; 32]);
    let replacement_mutation = current_security
        .prepare_restriction(
            &initial_security_commit,
            OpaqueId::new("guard-security:second").unwrap(),
            Blake3Digest32::from_bytes([36; 32]),
            replacement_security,
            guard_security_owners(),
        )
        .unwrap();
    let replacement_commit = journal
        .commit_security_restriction(&replacement_mutation, &context())
        .unwrap();
    assert_eq!(replacement_commit.receipt().after_generation, 6);

    let current_registry = journal.read_registry_snapshot(&context()).unwrap();
    let current_security = journal
        .read_security_restriction(namespace, &context())
        .unwrap();
    let checkpoint_after_security = journal
        .read_publication_checkpoint(&context())
        .unwrap()
        .unwrap();
    let final_checkpoint = {
        let mut publisher = ControlSnapshotPublisher::new();
        let mut bound = journal
            .bind_control_port_with_original_context::<Cancel, _>(
                &mut publisher,
                deadline,
                &work,
            )
            .unwrap();
        let stale_security_request = bound
            .prepare_publication_guard_image_with_original_context(
                &checkpoint_after_security,
                &current_registry,
                std::slice::from_ref(&security),
                publication_guards(18, 30, 1),
                MutationId([73; 32]),
                DIGEST,
                deadline,
                &work,
            )
            .unwrap();
        let stale_security_identity =
            control_mutation_identity(stale_security_request.operation_id()).unwrap();
        assert_eq!(
            bound
                .transact_publication_guard_image_with_original_context(
                    &stale_security_request,
                    &stale_security_identity,
                    deadline,
                    &work,
                )
                .unwrap_err()
                .control,
            ControlError::GenerationMismatch,
        );
        let current_security_request = bound
            .prepare_publication_guard_image_with_original_context(
                &checkpoint_after_security,
                &current_registry,
                std::slice::from_ref(&current_security),
                publication_guards(18, 30, 2),
                MutationId([74; 32]),
                DIGEST,
                deadline,
                &work,
            )
            .unwrap();
        let current_security_identity =
            control_mutation_identity(current_security_request.operation_id()).unwrap();
        let current_security_commit = bound
            .transact_publication_guard_image_with_original_context(
                &current_security_request,
                &current_security_identity,
                deadline,
                &work,
            )
            .unwrap();
        assert_eq!(current_security_commit.commit().after_generation, 7);
        bound
            .read_publication_checkpoint_with_original_context(deadline, &work)
            .unwrap()
            .unwrap()
    };
    assert_eq!(final_checkpoint.generation(), 7);
    assert_eq!(final_checkpoint.visibility().guards, publication_guards(18, 30, 2));
}

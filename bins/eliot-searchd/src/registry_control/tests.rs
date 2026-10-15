//! Normal Root production and historical Source metadata through real redb.
//! Journal/native identity inputs are synthetic; this is not NativeCtx qualification.

use super::*;
#[path = "tests/original_inputs.rs"]
mod original_inputs;
use search_contracts::{
    Blake3Digest32, DataRootId, InstallationIncarnationId, NonZeroRevision, OpaqueCanonicalBytes,
    OpaqueRef, OwnerEpoch, PublicationGuards, ReceiptRef, RequestId, RootBindingId, SourceId,
    SourceIdentity, SourceIdentityKind, SourceNamespaceId,
};
use search_control_redb::MutationId;
use search_ports::FakeCancellation;
use search_source_registry::api::{
    DEFAULT_REGISTRY_LIMITS, RegisterRootRequest, SourceRegistry, register_root,
};
use search_source_registry::recovery::{
    RegistryBatch, RegistryChange, RegistryOperation, apply_admission_batch,
};
use std::fs::{self, File, OpenOptions};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "search-registry-adapter-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).expect("fresh scratch");
        Self(path)
    }
    fn file(&self, create: bool) -> File {
        OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(create)
            .open(self.0.join("control.redb"))
            .expect("scratch journal")
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        assert_eq!(self.0.parent(), Some(std::env::temp_dir().as_path()));
        assert!(
            self.0
                .file_name()
                .expect("name")
                .to_string_lossy()
                .starts_with("search-registry-adapter-")
        );
        fs::remove_dir_all(&self.0).expect("closed scratch");
    }
}
fn context() -> OperationContext<FakeCancellation> {
    OperationContext::new(
        RequestId::from_bytes([1; 16]),
        60_000,
        FakeCancellation::new(false),
        OpaqueRef::new("registry-adapter-fixture").expect("budget"),
    )
    .expect("context")
}
fn identity() -> JournalIdentity {
    JournalIdentity {
        installation_incarnation_id: InstallationIncarnationId::from_bytes([2; 16]),
        data_root_id: DataRootId::from_bytes([3; 16]),
        owner_epoch: OwnerEpoch::new(1).expect("epoch"),
        path_identity_digest: Blake3Digest32::from_bytes([4; 32]),
        schema_family_digest: Blake3Digest32::from_bytes([5; 32]),
        schema_version: search_control_redb::CURRENT_JOURNAL_SCHEMA_VERSION,
    }
}
fn journal(
    scratch: &Scratch,
    context: &OperationContext<FakeCancellation>,
) -> PersistentControlJournal {
    let mut journal =
        PersistentControlJournal::create(scratch.file(true), identity(), JournalLimits::BASELINE)
            .expect("actual redb initialization");
    journal
        .initialize_operating_profile(
            search_control_redb::JournalProfileInitialization::Direct,
            MutationId([9; 32]),
            Blake3Digest32::from_bytes([10; 32]),
            context,
        )
        .expect("explicit DIRECT profile without an indexed route");
    journal
}
fn root(id: u8) -> RegisterRootRequest {
    RegisterRootRequest {
        root_binding_id: RootBindingId::from_bytes([id; 16]),
        canonical_root_digest: Blake3Digest32::from_bytes([id; 32]),
        policy_fingerprint: Blake3Digest32::from_bytes([12; 32]),
        policy_revision: NonZeroRevision::new(1).expect("revision"),
        owner_epoch: OwnerEpoch::new(1).expect("owner"),
        owner_fence_receipt: ReceiptRef::new("fixture-owner-fence").expect("reference"),
        policy_receipt: ReceiptRef::new("fixture-policy").expect("reference"),
    }
}

#[test]
fn actual_root_producer_restores_history_and_replays_original_without_rollback() {
    let scratch = Scratch::new();
    let context = context();
    let mut journal = journal(&scratch, &context);
    let mut publisher = ControlSnapshotPublisher::new();
    journal
        .recover_snapshot_publication_with_context(&mut publisher, &context)
        .expect("initial snapshot");
    assert!(publisher.current().is_some());
    let mut source = SourceRegistry::new(DEFAULT_REGISTRY_LIMITS).expect("owner");
    let first_id = OpaqueId::new("first-root").expect("id");
    let first_digest = Blake3Digest32::from_bytes([21; 32]);
    let first_receipt = ReceiptRef::new("first-root-receipt").expect("reference");
    {
        let mut port = RegistryJournal::bind(
            &mut journal,
            &mut publisher,
            identity(),
            DEFAULT_REGISTRY_LIMITS,
        )
        .expect("bind");
        let first = register_root(
            &mut source,
            &root(13),
            &first_id,
            first_digest,
            &first_receipt,
            &mut port,
            &context,
        )
        .expect("real owner transaction");
        assert_eq!(first.registry_revision, 1);
        register_root(
            &mut source,
            &root(14),
            &OpaqueId::new("second-root").expect("id"),
            Blake3Digest32::from_bytes([22; 32]),
            &ReceiptRef::new("second-receipt").expect("reference"),
            &mut port,
            &context,
        )
        .expect("second actual transaction");
    }
    let committed = journal.committed_writes();
    assert!(
        publisher.current().is_none(),
        "the old pointer cannot admit after mutation"
    );
    drop(journal);
    let mut reopened =
        PersistentControlJournal::open(scratch.file(false), identity(), JournalLimits::BASELINE)
            .expect("disk reopen");
    let mut reopened_publisher = ControlSnapshotPublisher::new();
    let mut port = RegistryJournal::bind(
        &mut reopened,
        &mut reopened_publisher,
        identity(),
        DEFAULT_REGISTRY_LIMITS,
    )
    .expect("same owner binding");
    let observed = port
        .restore(&context)
        .expect("actual complete disk state/history");
    let mut restored = observed.into_registry();
    assert_eq!(restored.revision(), 2);
    let replay = register_root(
        &mut restored,
        &root(13),
        &first_id,
        first_digest,
        &first_receipt,
        &mut port,
        &context,
    )
    .expect("original operation replay");
    assert!(replay.replayed);
    assert_eq!(replay.receipt, first_receipt);
    assert_eq!(restored.revision(), 2);
    assert!(restored.root(root(14).root_binding_id).is_ok());
    port.finish_metadata_publication(&context)
        .expect("explicit current disk publication");
    drop(port);
    assert!(reopened_publisher.current().is_some());
    // A read/replay after reopen cannot add any write to this incarnation.
    assert_eq!(reopened.committed_writes(), 0);
    assert!(committed >= 3);
}

#[test]
fn identity_and_cancel_refusals_cannot_be_proven_absence() {
    let scratch = Scratch::new();
    let context = context();
    let mut journal = journal(&scratch, &context);
    let mut foreign = identity();
    foreign.data_root_id = DataRootId::from_bytes([99; 16]);
    let mut publisher = ControlSnapshotPublisher::new();
    assert!(
        RegistryJournal::<FakeCancellation>::bind(
            &mut journal,
            &mut publisher,
            foreign,
            DEFAULT_REGISTRY_LIMITS
        )
        .is_err()
    );
    let port = RegistryJournal::bind(
        &mut journal,
        &mut publisher,
        identity(),
        DEFAULT_REGISTRY_LIMITS,
    )
    .expect("bind");
    let cancelled = OperationContext::new(
        RequestId::from_bytes([30; 16]),
        60_000,
        FakeCancellation::new(true),
        OpaqueRef::new("cancelled-fixture").expect("budget"),
    )
    .expect("context");
    assert!(
        port.load_batch(&OpaqueId::new("absent").expect("id"), &cancelled)
            .is_err()
    );
    assert!(matches!(
        port.load_batch(&OpaqueId::new("absent").expect("id"), &context),
        Ok(RegistryBatchLookup::Absent { revision: 0 })
    ));
}

struct SourceAdmissionFixture {
    policy: search_source_admission::CanonicalAdmissionPolicy,
    original: search_source_admission::original_envelope::OriginalAdmissionEnvelope,
    binding: search_source_identity::SourceBinding,
    assignment: search_source_registry::source::AdmissionBindingProof,
}

fn source_admission_fixture() -> SourceAdmissionFixture {
    use search_source_admission::original_envelope::retain_original_admission;
    use search_source_admission::{
        AdmissionBudget, CancelFlag, DEFAULT_ADMISSION_LIMITS, UnvalidatedObservationInput,
        baseline_policy, evaluate, issue_receipt, validate_observation,
    };
    use search_source_identity::{CanonicalRelativePath, SourceBinding, SourceObservation};
    use search_source_registry::source::AdmissionBindingProof;

    let policy = baseline_policy(1, 1024);
    let observation = validate_observation(
        &UnvalidatedObservationInput {
            locator_class: "normalized-file".to_owned(),
            location_class: "local-fixed".to_owned(),
            source_kind: "file".to_owned(),
            source_class: "regular".to_owned(),
            byte_size: Some(10),
            is_generated: Some(false),
            is_vendor: Some(false),
            is_binary: Some(false),
            is_system: Some(false),
            sensitivity: Some("public".to_owned()),
            detector_id: Some("detector:baseline-v1".to_owned()),
            profile_id: Some(search_source_admission::BASELINE_PROFILE.to_owned()),
            unavailable_fields: vec![],
            unknown_fields: vec![],
        },
        &policy,
        DEFAULT_ADMISSION_LIMITS,
    )
    .expect("actual owner normalization");
    let decision = evaluate(
        &policy,
        &observation,
        AdmissionBudget::default_budget(),
        CancelFlag::live(),
    )
    .expect("actual owner evaluation");
    let admission = issue_receipt(&policy, &observation, &decision).expect("actual owner receipt");
    let original = retain_original_admission(
        &policy,
        &observation,
        &admission,
        AdmissionBudget::default_budget(),
        CancelFlag::live(),
    )
    .expect("actual original inputs");
    let id = SourceIdentity {
        source_namespace_id: SourceNamespaceId::from_bytes([31; 16]),
        source_id: SourceId::from_bytes([32; 16]),
        identity_kind: SourceIdentityKind::NtfsFile,
        stable_identity_components: OpaqueCanonicalBytes::from_validated(
            b"synthetic-native-fixture".to_vec(),
        )
        .expect("bounded synthetic identity"),
    };
    let binding = SourceBinding::new(
        id.clone(),
        SourceObservation {
            root_binding_id: RootBindingId::from_bytes([13; 16]),
            relative_path: CanonicalRelativePath::new(
                "sample.rs".to_owned(),
                search_source_identity::DEFAULT_IDENTITY_LIMITS,
            )
            .expect("relative locator"),
            stable_file_identity_digest: Some(Blake3Digest32::from_bytes([33; 32])),
            content_digest: Blake3Digest32::from_bytes([34; 32]),
            content_bytes: 10,
            observation_receipt: ReceiptRef::new("fixture-observation").expect("reference"),
        },
        NonZeroRevision::new(1).expect("revision"),
        NonZeroRevision::new(1).expect("revision"),
    );
    let assignment = AdmissionBindingProof {
        observation_digest: Blake3Digest32::from_bytes(*admission.observation_digest().as_bytes()),
        source_identity: id,
        assignment_digest: Blake3Digest32::from_bytes([36; 32]),
        assignment_receipt: ReceiptRef::new("synthetic-identity-assignment").expect("reference"),
        readback_verified: true,
    };
    SourceAdmissionFixture {
        policy,
        original,
        binding,
        assignment,
    }
}

#[test]
fn source_owner_original_admission_and_binding_survive_real_disk_restore() {
    use search_source_admission::{CancelFlag, policy_fingerprint};

    let scratch = Scratch::new();
    let context = context();
    let mut journal = journal(&scratch, &context);
    let mut publisher = ControlSnapshotPublisher::new();
    let mut source = SourceRegistry::new(DEFAULT_REGISTRY_LIMITS).expect("owner");
    let SourceAdmissionFixture {
        policy,
        original,
        binding,
        assignment,
    } = source_admission_fixture();
    let id = binding.identity().clone();
    let admission = original.receipt().clone();
    {
        let mut port = RegistryJournal::bind(
            &mut journal,
            &mut publisher,
            identity(),
            DEFAULT_REGISTRY_LIMITS,
        )
        .expect("bind");
        let mut request = root(13);
        request.policy_fingerprint =
            Blake3Digest32::from_bytes(*policy_fingerprint(&policy).as_bytes());
        register_root(
            &mut source,
            &request,
            &OpaqueId::new("source-root").expect("id"),
            Blake3Digest32::from_bytes([35; 32]),
            &ReceiptRef::new("source-root-receipt").expect("reference"),
            &mut port,
            &context,
        )
        .expect("root through the actual port");
        apply_admission_batch(
            &mut source,
            RegistryBatch {
                expected_registry_revision: 1,
                operation: RegistryOperation::new(
                    OpaqueId::new("source-op").expect("id"),
                    Blake3Digest32::from_bytes([37; 32]),
                ),
                changes: vec![RegistryChange::RegisterSource {
                    admission: admission.clone(),
                    original_admission: Some(original),
                    binding,
                    assignment,
                    receipt: ReceiptRef::new("source-receipt").expect("reference"),
                }],
            },
            &ReceiptRef::new("source-batch-receipt").expect("reference"),
            &mut port,
            &context,
        )
        .expect("source through the actual state transaction");
    }
    drop(journal);
    let mut reopened =
        PersistentControlJournal::open(scratch.file(false), identity(), JournalLimits::BASELINE)
            .expect("actual reopen");
    let mut reopened_publisher = ControlSnapshotPublisher::new();
    let port = RegistryJournal::bind(
        &mut reopened,
        &mut reopened_publisher,
        identity(),
        DEFAULT_REGISTRY_LIMITS,
    )
    .expect("bind");
    let observed = port.restore(&context).expect("actual owner restore");
    let record = observed
        .registry()
        .sources()
        .get(&id)
        .expect("restored exact source");
    assert_eq!(record.legacy_admission(), Some(&admission));
    assert!(
        record
            .original_admission()
            .expect("retained original")
            .verify_current_policy(&policy, CancelFlag::live())
            .is_ok()
    );
    assert_eq!(record.binding().relative_path().as_str(), "sample.rs");
    assert_eq!(observed.registry().revision(), 2);
}

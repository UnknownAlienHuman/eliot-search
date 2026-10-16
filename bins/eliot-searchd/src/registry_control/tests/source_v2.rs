//! Focused normal-adapter cases for the permanent-v2 Source semantic spine.
//!
//! Journal/native identifiers and the identity allocator are deliberately
//! synthetic here. The cases qualify the real redb adapter and real v2 owners;
//! they do not claim Native custody or entropy qualification.

#[path = "source_v2_rebind.rs"]
mod rebind;

use super::*;
use search_contracts::{
    Blake3Digest32, CatalogRevision, NonZeroRevision, OpaqueId, OpaqueRef, OwnerEpoch, ReceiptRef,
    RequestId, RootBindingId,
};
use search_control_redb::MigrationBootstrapWork;
use search_ports::{
    DisclosureClass, MutationIdentity, Port, PortErrorKind, PortFailure, PortReceipt,
    PortRetryability,
};
use search_runtime_owner::OwnerError;
use search_source_admission::policy_v2::{
    CheckpointFailureV2, DEFAULT_COMPILE_LIMITS, POLICY_SCHEMA_VERSION, PolicyInputV2, PredicateV2,
    RuleInputV2, compile_policy_v2,
};
use search_source_admission::{
    AcceptedCapabilities, AdmissionOutcome, BASELINE_PROFILE,
    ConfigSectionInput as AdmissionSection, RuleEffect, SectionPlatform, SourceClass,
    UnvalidatedObservationInput, validate_section,
};
use search_source_identity::{
    CaseBehavior, FilesystemIdentityProfile, IdentityObservation, LinkBehavior,
    ObservationConfidence, PathObservation, ReparseBehavior, SourceObservation, StableFieldPolicy,
    StableIdentityEvidence, StableIdentityKey, UnicodeBehavior, derive_canonical_path_key,
    validate_identity_observation,
};
use search_source_provenance::{OriginalAdmissionV2, OriginalInputsLayoutV2, RetainedPolicyV2};
use search_source_registry::api::{
    SourceAdmissionPreparationV2, SourceAdmissionRequestV2, SourceAdmissionV2Error,
    SourceNamespaceSelectionV2, SourceOwnerScopeV2, SourceRegistry, commit_source_admission_v2,
    prepare_source_admission_v2, recover_source_admission_v2,
};
use search_source_registry::error::{RegistryControlPort, RegistryPortError};
use search_source_registry::recovery::{RegistryBatchLookup, RegistryBatchReadback};
use search_source_registry::source_ids::SourceIdentityIdPortV2;
use std::{
    cell::Cell,
    fmt,
    sync::Arc,
    time::{Duration, Instant},
};

#[derive(Clone, Debug, Eq, PartialEq)]
struct CounterError;

impl fmt::Display for CounterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("synthetic source-id counter failure")
    }
}
impl std::error::Error for CounterError {}
impl PortFailure for CounterError {
    fn kind(&self) -> PortErrorKind {
        PortErrorKind::DependencyUnavailable
    }
    fn retryability(&self) -> PortRetryability {
        PortRetryability::Never
    }
    fn disclosure(&self) -> DisclosureClass {
        DisclosureClass::Redacted
    }
    fn operation_id(&self) -> Option<&OpaqueId> {
        None
    }
}

struct CounterIds {
    next: u8,
    allocations: usize,
}
impl CounterIds {
    fn new(next: u8) -> Self {
        Self {
            next,
            allocations: 0,
        }
    }
    fn fresh(&mut self) -> [u8; 16] {
        let mut bytes = [0; 16];
        bytes[0] = self.next;
        bytes[6] = 0x40;
        bytes[8] = 0x80;
        self.next = self.next.wrapping_add(1);
        self.allocations += 1;
        bytes
    }
}
impl Port for CounterIds {
    type Error = CounterError;
    type Cancellation = FakeCancellation;
}
impl SourceIdentityIdPortV2 for CounterIds {
    fn fresh_source_id(
        &mut self,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<search_contracts::SourceId, Self::Error> {
        context.preflight().expect("live source-id context");
        Ok(search_contracts::SourceId::from_bytes(self.fresh()))
    }
    fn fresh_source_namespace_id(
        &mut self,
        context: &OperationContext<Self::Cancellation>,
    ) -> Result<search_contracts::SourceNamespaceId, Self::Error> {
        context.preflight().expect("live namespace-id context");
        Ok(search_contracts::SourceNamespaceId::from_bytes(
            self.fresh(),
        ))
    }
}

struct SourcePreparationPort<'a, 'journal> {
    control: &'a mut super::super::RegistryJournal<'journal, FakeCancellation>,
    ids: &'a mut CounterIds,
}

impl Port for SourcePreparationPort<'_, '_> {
    type Error = RegistryPortError;
    type Cancellation = FakeCancellation;
}

impl RegistryControlPort for SourcePreparationPort<'_, '_> {
    fn commit_batch(
        &mut self,
        batch: &search_source_registry::recovery::PreparedBatch,
        context: &OperationContext<FakeCancellation>,
        mutation: &MutationIdentity,
    ) -> Result<RegistryBatchReadback, RegistryPortError> {
        self.control.commit_batch(batch, context, mutation)
    }

    fn load_batch(
        &self,
        operation_id: &OpaqueId,
        context: &OperationContext<FakeCancellation>,
    ) -> Result<RegistryBatchLookup, RegistryPortError> {
        self.control.load_batch(operation_id, context)
    }

    fn persist_entry(
        &mut self,
        entry: &search_source_registry::error::RegistryJournalEntry,
        context: &OperationContext<FakeCancellation>,
        mutation: &MutationIdentity,
    ) -> Result<PortReceipt, RegistryPortError> {
        self.control.persist_entry(entry, context, mutation)
    }

    fn load_entry(
        &self,
        operation_id: &OpaqueId,
        context: &OperationContext<FakeCancellation>,
    ) -> Result<Option<search_source_registry::error::RegistryJournalEntry>, RegistryPortError>
    {
        self.control.load_entry(operation_id, context)
    }
}

impl SourceIdentityIdPortV2 for SourcePreparationPort<'_, '_> {
    fn fresh_source_id(
        &mut self,
        context: &OperationContext<FakeCancellation>,
    ) -> Result<search_contracts::SourceId, RegistryPortError> {
        self.ids.fresh_source_id(context).map_err(|_| {
            search_source_registry::error::registry_port_error(
                PortErrorKind::DependencyUnavailable,
                PortRetryability::Never,
                search_source_registry::error::RegistryError::DurabilityRejected,
                None,
            )
        })
    }

    fn fresh_source_namespace_id(
        &mut self,
        context: &OperationContext<FakeCancellation>,
    ) -> Result<search_contracts::SourceNamespaceId, RegistryPortError> {
        self.ids.fresh_source_namespace_id(context).map_err(|_| {
            search_source_registry::error::registry_port_error(
                PortErrorKind::DependencyUnavailable,
                PortRetryability::Never,
                search_source_registry::error::RegistryError::DurabilityRejected,
                None,
            )
        })
    }
}

fn platform() -> (SectionPlatform, AcceptedCapabilities) {
    (
        SectionPlatform {
            os: "windows".into(),
            arch: "x64".into(),
        },
        AcceptedCapabilities {
            profiles: vec![BASELINE_PROFILE.into()],
        },
    )
}

fn profile() -> FilesystemIdentityProfile {
    FilesystemIdentityProfile::new(
        NonZeroRevision::new(1).expect("profile revision"),
        Blake3Digest32::from_bytes([61; 32]),
        CaseBehavior::Sensitive,
        UnicodeBehavior::PreserveScalarValues,
        StableFieldPolicy::Required,
        StableFieldPolicy::Required,
        LinkBehavior::StablePhysicalIdentity,
        ReparseBehavior::FinalTargetIdentity,
    )
    .expect("qualified synthetic C04 profile")
}

fn policy(effect: RuleEffect, revision: u64) -> RetainedPolicyV2 {
    let (platform, capabilities) = platform();
    let section = validate_section(
        &AdmissionSection {
            bootstrap_policy_profile: BASELINE_PROFILE.into(),
            max_file_bytes: 1_024,
            allow_generated: false,
            allow_vendor: false,
            allow_binary: false,
            unknown_keys: vec![],
        },
        &platform,
        &capabilities,
    )
    .expect("explicit section");
    let compiled = compile_policy_v2(
        &PolicyInputV2 {
            schema_version: POLICY_SCHEMA_VERSION,
            revision,
            section,
            rules: vec![RuleInputV2 {
                id: format!("semantic-{effect:?}"),
                precedence: 0,
                source_class: SourceClass::Regular,
                predicate: PredicateV2::Always,
                effect,
                unknown_fields: vec![],
            }],
            unknown_fields: vec![],
        },
        DEFAULT_COMPILE_LIMITS,
        &mut || Ok(()),
    )
    .expect("explicit source policy");
    RetainedPolicyV2::capture(&compiled, &platform, &capabilities, &mut || Ok(()))
        .expect("retained policy")
}

fn original(policy: &RetainedPolicyV2, path: &str, digest: u8) -> OriginalAdmissionV2 {
    let raw = UnvalidatedObservationInput {
        locator_class: "normalized-file".into(),
        location_class: "local-fixed".into(),
        source_kind: "file".into(),
        source_class: "regular".into(),
        byte_size: Some(10),
        is_generated: Some(false),
        is_vendor: Some(false),
        is_binary: Some(false),
        is_system: Some(false),
        sensitivity: Some("public".into()),
        detector_id: Some(format!("detector:semantic-spine-{digest}")),
        profile_id: Some(BASELINE_PROFILE.into()),
        unavailable_fields: vec![],
        unknown_fields: vec![],
    };
    let observation = search_source_admission::original_inputs_v2::reconstitute_observation_v2(
        &raw,
        Some(path.as_bytes()),
        policy.compiled(),
        search_source_admission::observation_v2::MAX_OBSERVATION_PREIMAGE_BYTES,
        &mut || Ok(()),
    )
    .expect("actual normalized source observation");
    OriginalAdmissionV2::retain(Arc::new(policy.clone()), &observation, &mut || Ok(()))
        .expect("actual v2 receipt")
}

fn identity_observation(
    root: RootBindingId,
    profile: FilesystemIdentityProfile,
    path: &str,
    digest: u8,
    catalog_revision: u64,
) -> search_source_identity::ValidatedIdentityObservation {
    let path_key = derive_canonical_path_key(
        &PathObservation {
            root_binding_id: root,
            root_relative_lookup_path: path.into(),
            profile_revision: profile.revision(),
            profile_schema_digest: profile.schema_digest(),
            normalization_attested: true,
        },
        profile,
    )
    .expect("C04 path key");
    validate_identity_observation(
        IdentityObservation {
            path_key,
            stable_evidence: StableIdentityEvidence::Exact(StableIdentityKey::Filesystem {
                volume_identity: Blake3Digest32::from_bytes([digest; 32]),
                file_identity: Blake3Digest32::from_bytes([digest.wrapping_add(1); 32]),
                generation: Some(1),
            }),
            content_digest_hint: Some(Blake3Digest32::from_bytes([digest.wrapping_add(2); 32])),
            metadata_generation: Some(1),
            byte_length_hint: Some(10),
            claimed_source_id: None,
            candidate_catalog_revision: CatalogRevision::new(catalog_revision),
            profile_revision: profile.revision(),
            profile_schema_digest: profile.schema_digest(),
            confidence: ObservationConfidence::Exact,
            evidence_digest: Blake3Digest32::from_bytes([digest.wrapping_add(3); 32]),
        },
        profile,
    )
    .expect("C04 validated identity observation")
}

fn request(
    original: OriginalAdmissionV2,
    root: RootBindingId,
    profile: FilesystemIdentityProfile,
    expected_registry_revision: u64,
    namespace: SourceNamespaceSelectionV2,
    path: &str,
    digest: u8,
    operation: &str,
) -> SourceAdmissionRequestV2 {
    SourceAdmissionRequestV2 {
        expected_registry_revision,
        root_binding_id: root,
        expected_root_revision: NonZeroRevision::new(1).expect("root revision"),
        namespace,
        owner: SourceOwnerScopeV2 {
            system_id: OpaqueId::new("semantic-test-owner").expect("owner"),
            installation_incarnation: super::identity().installation_incarnation_id,
            epoch: OwnerEpoch::new(1).expect("owner epoch"),
        },
        identity_profile: profile,
        identity_observation: identity_observation(
            root,
            profile,
            path,
            digest,
            expected_registry_revision,
        ),
        binding_observation: SourceObservation {
            root_binding_id: root,
            relative_path: search_source_identity::CanonicalRelativePath::new(
                path,
                search_source_identity::DEFAULT_IDENTITY_LIMITS,
            )
            .expect("relative path"),
            stable_file_identity_digest: Some(Blake3Digest32::from_bytes(
                [digest.wrapping_add(1); 32],
            )),
            content_digest: Blake3Digest32::from_bytes([digest.wrapping_add(2); 32]),
            content_bytes: 10,
            observation_receipt: ReceiptRef::new(format!("observation:{operation}"))
                .expect("observation receipt"),
        },
        original,
        expected_source_revision: None,
        operation_id: OpaqueId::new(operation).expect("operation"),
        receipt: ReceiptRef::new(format!("receipt:{operation}")).expect("receipt"),
    }
}

fn bind_v4<'a>(
    journal: &'a mut search_control_redb::PersistentControlJournal,
    publisher: &'a mut search_control_redb::ControlSnapshotPublisher,
    work: &'a dyn Fn(MigrationBootstrapWork) -> Result<(), OwnerError>,
    platform: SectionPlatform,
    capabilities: AcceptedCapabilities,
) -> super::RegistryJournal<'a, FakeCancellation> {
    super::RegistryJournal::bind_original_v4(
        journal,
        publisher,
        super::identity(),
        search_source_registry::api::DEFAULT_REGISTRY_LIMITS,
        Instant::now() + Duration::from_secs(60),
        work,
        platform,
        capabilities,
    )
    .expect("normal v4 journal binding")
}

fn register_source_root(
    registry: &mut SourceRegistry,
    port: &mut super::RegistryJournal<'_, FakeCancellation>,
    context: &OperationContext<FakeCancellation>,
    original: &OriginalAdmissionV2,
) {
    let mut root = super::root(13);
    root.policy_revision = original.receipt().decision().policy_revision();
    root.policy_fingerprint = Blake3Digest32::from_bytes(
        *original
            .receipt()
            .decision()
            .policy_fingerprint()
            .as_bytes(),
    );
    search_source_registry::api::register_root(
        registry,
        &root,
        &OpaqueId::new("semantic-root").expect("root operation"),
        Blake3Digest32::from_bytes([70; 32]),
        &ReceiptRef::new("semantic-root-receipt").expect("root receipt"),
        port,
        context,
    )
    .expect("root commit");
}

fn prepare(
    registry: &mut SourceRegistry,
    request: &SourceAdmissionRequestV2,
    ids: &mut CounterIds,
    port: &mut super::RegistryJournal<'_, FakeCancellation>,
    context: &OperationContext<FakeCancellation>,
) -> Result<SourceAdmissionPreparationV2, SourceAdmissionV2Error<RegistryPortError>> {
    let mut combined = SourcePreparationPort { control: port, ids };
    let mut probe = || Ok::<_, CheckpointFailureV2>(());
    prepare_source_admission_v2(registry, request, &mut combined, context, &mut probe)
}

fn prepared(
    registry: &mut SourceRegistry,
    request: &SourceAdmissionRequestV2,
    ids: &mut CounterIds,
    port: &mut super::RegistryJournal<'_, FakeCancellation>,
    context: &OperationContext<FakeCancellation>,
) -> search_source_registry::api::PreparedSourceAdmissionV2 {
    match prepare(registry, request, ids, port, context).expect("source preparation") {
        SourceAdmissionPreparationV2::Prepared(value) => value,
        SourceAdmissionPreparationV2::Replayed(_) => panic!("unexpected replay"),
    }
}

#[test]
fn source_v2_commits_new_namespace_and_restarts_through_real_v4_readback() {
    let scratch = Scratch::new();
    let context = context();
    let (platform, capabilities) = platform();
    let profile = profile();
    let policy = policy(RuleEffect::Allow, 7);
    let original_one = original(&policy, "src/one.rs", 71);
    let original_two = original(&policy, "src/two.rs", 81);
    let foreign_policy = self::policy(RuleEffect::Allow, 8);
    let foreign_original = original(&foreign_policy, "src/foreign.rs", 91);
    let fail = Cell::new(false);
    let work = |unit: MigrationBootstrapWork| {
        if fail.get() && matches!(unit, MigrationBootstrapWork::Write(n) if n > 0) {
            Err(OwnerError::OwnerCancelledBeforeMutation)
        } else {
            Ok(())
        }
    };
    let mut journal = super::journal(&scratch, &context);
    let mut publisher = search_control_redb::ControlSnapshotPublisher::new();
    let mut registry = SourceRegistry::new(search_source_registry::api::DEFAULT_REGISTRY_LIMITS)
        .expect("registry");
    let root = RootBindingId::from_bytes([13; 16]);
    let mut ids = CounterIds::new(1);
    {
        let mut port = bind_v4(
            &mut journal,
            &mut publisher,
            &work,
            platform.clone(),
            capabilities.clone(),
        );
        let policy_reference =
            OriginalInputsLayoutV2::policy_reference(&policy, &mut || Ok(())).unwrap();
        let admission_reference =
            OriginalInputsLayoutV2::admission_reference(&original_one, &mut || Ok(())).unwrap();
        register_source_root(&mut registry, &mut port, &context, &original_one);
        let first_request = request(
            original_one.clone(),
            root,
            profile,
            1,
            SourceNamespaceSelectionV2::CreateNew,
            "src/one.rs",
            71,
            "semantic-source-one",
        );
        let first = prepared(&mut registry, &first_request, &mut ids, &mut port, &context);
        let refs = search_source_registry::recovery::required_original_refs_from_batch_v4(
            first.batch().encoded(),
            &mut || Ok(()),
        )
        .expect("prepared v4 references");
        assert!(
            !refs.is_empty(),
            "v4 source operation must carry original references"
        );
        let mut probe = || Ok::<_, CheckpointFailureV2>(());
        commit_source_admission_v2(&mut registry, &first, &mut port, &context, &mut probe)
            .expect("first source commit");
        port.original_inputs_snapshot(
            &[policy_reference.as_slice(), admission_reference.as_slice()],
            &context,
        )
        .expect("normal Source port retained originals before dispatch acknowledgement");
        let namespace = registry
            .namespaces()
            .values()
            .next()
            .expect("created namespace")
            .clone();
        let namespace_selection = SourceNamespaceSelectionV2::Existing {
            id: namespace.ownership.source_namespace_id,
            revision: namespace.ownership.ownership_record_revision,
            generation: namespace.ownership.source_owner_generation,
        };
        drop(port);
        let writes_before_foreign = journal.committed_writes();
        let mut port = bind_v4(
            &mut journal,
            &mut publisher,
            &work,
            platform.clone(),
            capabilities.clone(),
        );
        let mut foreign_owner = request(
            original_two.clone(),
            root,
            profile,
            2,
            namespace_selection,
            "src/two.rs",
            81,
            "foreign-owner-source",
        );
        foreign_owner.owner.system_id = OpaqueId::new("foreign-owner").unwrap();
        let mut foreign_epoch = foreign_owner.clone();
        foreign_epoch.owner.system_id = namespace.ownership.owner_system_id.clone();
        foreign_epoch.owner.epoch = OwnerEpoch::new(2).unwrap();
        let foreign_policy_request = request(
            foreign_original,
            root,
            profile,
            2,
            namespace_selection,
            "src/foreign.rs",
            91,
            "foreign-policy-source",
        );
        let allocations_before_foreign = ids.allocations;
        assert!(matches!(prepare(&mut registry, &foreign_epoch, &mut ids, &mut port, &context),
            Err(SourceAdmissionV2Error::Registry(search_source_registry::error::RegistryError::NamespaceOwnershipConflict))));
        for invalid in [&foreign_owner, &foreign_policy_request] {
            assert!(prepare(&mut registry, invalid, &mut ids, &mut port, &context,).is_err());
        }
        assert_eq!(ids.allocations, allocations_before_foreign);
        drop(port);
        assert_eq!(journal.committed_writes(), writes_before_foreign);
        let mut port = bind_v4(
            &mut journal,
            &mut publisher,
            &work,
            platform.clone(),
            capabilities.clone(),
        );
        let second_request = request(
            original_two.clone(),
            root,
            profile,
            2,
            namespace_selection,
            "src/two.rs",
            81,
            "semantic-source-two",
        );
        let second = prepared(
            &mut registry,
            &second_request,
            &mut ids,
            &mut port,
            &context,
        );
        let mut probe = || Ok::<_, CheckpointFailureV2>(());
        commit_source_admission_v2(&mut registry, &second, &mut port, &context, &mut probe)
            .expect("second source commit");
        assert_eq!(ids.allocations, 3, "one namespace and two source IDs");
        let first_policy = registry
            .sources()
            .values()
            .next()
            .unwrap()
            .permanent_admission()
            .unwrap();
        let second_policy = registry
            .sources()
            .values()
            .nth(1)
            .unwrap()
            .permanent_admission()
            .unwrap();
        assert_eq!(
            first_policy.policy_reference(),
            second_policy.policy_reference()
        );
        assert_eq!(registry.sources().len(), 2);
        assert_eq!(registry.namespaces().len(), 1);
    }
    drop(journal);
    let mut reopened = search_control_redb::PersistentControlJournal::open(
        scratch.file(false),
        super::identity(),
        search_control_redb::JournalLimits::BASELINE,
    )
    .expect("reopen redb");
    let mut reopened_publisher = search_control_redb::ControlSnapshotPublisher::new();
    let port = bind_v4(
        &mut reopened,
        &mut reopened_publisher,
        &work,
        platform,
        capabilities,
    );
    let restored = port.restore(&context).expect("v4 disk restore");
    assert_eq!(restored.registry().revision(), 3);
    assert_eq!(restored.registry().sources().len(), 2);
    assert_eq!(restored.registry().namespaces().len(), 1);
    assert!(
        restored
            .registry()
            .sources()
            .values()
            .all(|source| source.permanent_admission().is_some())
    );
}

#[test]
fn source_v2_normal_port_retains_restarts_and_replays_with_qualified_entropy() {
    let scratch = Scratch::new();
    let context = context();
    let (platform, capabilities) = platform();
    let profile = profile();
    let policy = policy(RuleEffect::Allow, 7);
    let original = original(&policy, "src/qualified.rs", 113);
    let work = |_unit: MigrationBootstrapWork| Ok::<_, OwnerError>(());
    let root = RootBindingId::from_bytes([13; 16]);
    let mut request = request(
        original.clone(),
        root,
        profile,
        1,
        SourceNamespaceSelectionV2::CreateNew,
        "src/qualified.rs",
        113,
        "qualified-source-replay",
    );
    request.owner.epoch = OwnerEpoch::new(2).unwrap();
    let mut journal = super::journal(&scratch, &context);
    let mut publisher = search_control_redb::ControlSnapshotPublisher::new();
    let mut registry =
        SourceRegistry::new(search_source_registry::api::DEFAULT_REGISTRY_LIMITS).unwrap();
    {
        let mut port = bind_v4(
            &mut journal,
            &mut publisher,
            &work,
            platform.clone(),
            capabilities.clone(),
        );
        register_source_root(&mut registry, &mut port, &context, &original);
        assert_eq!(registry.roots()[&root].owner_epoch.get(), 1);
        let mut probe = || Ok::<_, CheckpointFailureV2>(());
        let prepared = match prepare_source_admission_v2(
            &mut registry,
            &request,
            &mut port,
            &context,
            &mut probe,
        )
        .expect("qualified source preparation")
        {
            SourceAdmissionPreparationV2::Prepared(value) => value,
            SourceAdmissionPreparationV2::Replayed(_) => {
                panic!("fresh qualified operation replayed")
            }
        };
        let mut probe = || Ok::<_, CheckpointFailureV2>(());
        commit_source_admission_v2(&mut registry, &prepared, &mut port, &context, &mut probe)
            .expect("qualified source commit retains originals");
        // Cross the real producer/storage boundary: consuming retention guards
        // must compare exactly the bytes the v4 producer actually committed.
        // This is not Native or retention-consumer qualification.
        assert_eq!(prepared.batch().record().state_encoding_profile(), "eliot.registry-state.v4");
        let mut conditions = Vec::new();
        for delta in prepared.batch().states() {
            let family = match delta.key() {
                search_source_registry::recovery::RegistryStateKey::Source(_) => search_control_redb::RegistryStateFamily::Source,
                search_source_registry::recovery::RegistryStateKey::Namespace(_) => search_control_redb::RegistryStateFamily::Namespace,
                _ => panic!("unexpected fresh source delta"),
            };
            let key = search_control_redb::RegistryStateKey::new(family, delta.key_bytes().to_vec(),
                search_control_redb::JournalLimits::BASELINE).unwrap();
            conditions.push(key.current_condition(delta.after_bytes(), search_control_redb::JournalLimits::BASELINE).unwrap());
        }
        let root_state = search_source_registry::recovery::RegistryStateRecord::Root(registry.roots()[&root].clone());
        let root_key = search_control_redb::RegistryStateKey::new(search_control_redb::RegistryStateFamily::Root,
            root_state.key().encode().unwrap(), search_control_redb::JournalLimits::BASELINE).unwrap();
        conditions.push(root_key.current_condition(&root_state.encode().unwrap(), search_control_redb::JournalLimits::BASELINE).unwrap());
        let keys = conditions.iter().map(|condition| condition.key().clone()).collect::<Vec<_>>();
        let observed = port.journal.read_control_keys_with_original_context(&keys,
            Instant::now() + Duration::from_secs(60), &work).expect("real source/namespace/root point read");
        assert_eq!(conditions.len(), 3);
        for condition in &conditions {
            assert_eq!(observed.get(condition.key()), condition.expected(), "exact wrapped owner bytes");
        }
    }
    drop(journal);

    let mut reopened = search_control_redb::PersistentControlJournal::open(
        scratch.file(false),
        super::identity(),
        search_control_redb::JournalLimits::BASELINE,
    )
    .expect("reopen redb");
    let mut reopened_publisher = search_control_redb::ControlSnapshotPublisher::new();
    let mut port = bind_v4(
        &mut reopened,
        &mut reopened_publisher,
        &work,
        platform,
        capabilities,
    );
    let restored = port.restore(&context).expect("retained originals restore");
    let mut replay_registry = restored.into_registry();
    assert_eq!(replay_registry.revision(), 2);
    assert_eq!(replay_registry.sources().len(), 1);
    assert_eq!(replay_registry.roots()[&root].owner_epoch.get(), 1);
    assert_eq!(replay_registry.namespaces().values().next().unwrap().ownership.owner_epoch.get(), 2);
    let mut probe = || Ok::<_, CheckpointFailureV2>(());
    let replay = prepare_source_admission_v2(
        &mut replay_registry,
        &request,
        &mut port,
        &context,
        &mut probe,
    )
    .expect("exact operation replay after restart");
    assert!(matches!(
        replay,
        SourceAdmissionPreparationV2::Replayed(receipt)
            if receipt.operation_id == request.operation_id
                && receipt.receipt == request.receipt
    ));
    assert_eq!(replay_registry.revision(), 2);
}

#[test]
fn source_v2_replay_after_later_operation_returns_original_before_ids_or_write() {
    let scratch = Scratch::new();
    let context = context();
    let (platform, capabilities) = platform();
    let profile = profile();
    let policy = policy(RuleEffect::Allow, 7);
    let original_one = original(&policy, "src/one.rs", 91);
    let original_two = original(&policy, "src/two.rs", 101);
    let work = |_unit: MigrationBootstrapWork| Ok::<_, OwnerError>(());
    let mut journal = super::journal(&scratch, &context);
    let mut publisher = search_control_redb::ControlSnapshotPublisher::new();
    let mut registry =
        SourceRegistry::new(search_source_registry::api::DEFAULT_REGISTRY_LIMITS).unwrap();
    let root = RootBindingId::from_bytes([13; 16]);
    let first_request = request(
        original_one.clone(),
        root,
        profile,
        1,
        SourceNamespaceSelectionV2::CreateNew,
        "src/one.rs",
        91,
        "replay-source-one",
    );
    let second_request = request(
        original_two.clone(),
        root,
        profile,
        2,
        SourceNamespaceSelectionV2::CreateNew,
        "src/two.rs",
        101,
        "replay-source-two",
    );
    {
        let mut port = bind_v4(
            &mut journal,
            &mut publisher,
            &work,
            platform.clone(),
            capabilities.clone(),
        );
        register_source_root(&mut registry, &mut port, &context, &original_one);
        let mut ids = CounterIds::new(11);
        let first = prepared(&mut registry, &first_request, &mut ids, &mut port, &context);
        let mut probe = || Ok::<_, CheckpointFailureV2>(());
        commit_source_admission_v2(&mut registry, &first, &mut port, &context, &mut probe).unwrap();
        let second = prepared(
            &mut registry,
            &second_request,
            &mut ids,
            &mut port,
            &context,
        );
        let mut probe = || Ok::<_, CheckpointFailureV2>(());
        commit_source_admission_v2(&mut registry, &second, &mut port, &context, &mut probe)
            .unwrap();
    }
    let writes_after_o2 = journal.committed_writes();
    let mut replay_ids = CounterIds::new(201);
    {
        let mut port = bind_v4(&mut journal, &mut publisher, &work, platform, capabilities);
        let replay = prepare(
            &mut registry,
            &first_request,
            &mut replay_ids,
            &mut port,
            &context,
        )
        .expect("same O1 replay");
        let SourceAdmissionPreparationV2::Replayed(receipt) = replay else {
            panic!("O1 was not acknowledged from its original committed record")
        };
        assert_eq!(receipt.operation_id, first_request.operation_id);
        assert_eq!(receipt.receipt, first_request.receipt);
        assert_eq!(registry.revision(), 3);
    }
    assert_eq!(replay_ids.allocations, 0);
    assert_eq!(journal.committed_writes(), writes_after_o2);
}

fn assert_decision_refusal(effect: RuleEffect, expected: AdmissionOutcome) {
    let scratch = Scratch::new();
    let context = context();
    let (platform, capabilities) = platform();
    let profile = profile();
    let policy = policy(effect, 7);
    let original = original(&policy, "src/refused.rs", 111);
    assert_eq!(original.receipt().decision().outcome(), expected);
    let work = |_unit: MigrationBootstrapWork| Ok::<_, OwnerError>(());
    let mut journal = super::journal(&scratch, &context);
    let mut publisher = search_control_redb::ControlSnapshotPublisher::new();
    let mut registry =
        SourceRegistry::new(search_source_registry::api::DEFAULT_REGISTRY_LIMITS).unwrap();
    {
        let mut port = bind_v4(
            &mut journal,
            &mut publisher,
            &work,
            platform.clone(),
            capabilities.clone(),
        );
        register_source_root(&mut registry, &mut port, &context, &original);
    }
    let before = journal.committed_writes();
    let mut port = bind_v4(&mut journal, &mut publisher, &work, platform, capabilities);
    let mut ids = CounterIds::new(31);
    let request = request(
        original,
        RootBindingId::from_bytes([13; 16]),
        profile,
        1,
        SourceNamespaceSelectionV2::CreateNew,
        "src/refused.rs",
        111,
        "refused-source",
    );
    let result = prepare(&mut registry, &request, &mut ids, &mut port, &context);
    assert!(matches!(
        result,
        Err(SourceAdmissionV2Error::Decision(outcome)) if outcome == expected
    ));
    drop(port);
    assert_eq!(ids.allocations, 0);
    assert_eq!(journal.committed_writes(), before);
    assert!(registry.sources().is_empty() && registry.namespaces().is_empty());
}

#[test]
fn source_v2_denies_review_and_unsupported_before_ids_or_writes() {
    assert_decision_refusal(RuleEffect::Deny, AdmissionOutcome::Deny);
    assert_decision_refusal(RuleEffect::Review, AdmissionOutcome::ReviewRequired);
    assert_decision_refusal(RuleEffect::Unsupported, AdmissionOutcome::Unsupported);
}

#[test]
fn source_v2_stale_root_namespace_bytes_and_c04_observation_refuse_before_ids() {
    let scratch = Scratch::new();
    let context = context();
    let (platform, capabilities) = platform();
    let profile = profile();
    let policy = policy(RuleEffect::Allow, 7);
    let original = original(&policy, "src/valid.rs", 121);
    let work = |_unit: MigrationBootstrapWork| Ok::<_, OwnerError>(());
    let mut journal = super::journal(&scratch, &context);
    let mut publisher = search_control_redb::ControlSnapshotPublisher::new();
    let mut registry =
        SourceRegistry::new(search_source_registry::api::DEFAULT_REGISTRY_LIMITS).unwrap();
    {
        let mut port = bind_v4(
            &mut journal,
            &mut publisher,
            &work,
            platform.clone(),
            capabilities.clone(),
        );
        register_source_root(&mut registry, &mut port, &context, &original);
    }
    let before = journal.committed_writes();
    let mut port = bind_v4(&mut journal, &mut publisher, &work, platform, capabilities);
    let valid = request(
        original.clone(),
        RootBindingId::from_bytes([13; 16]),
        profile,
        1,
        SourceNamespaceSelectionV2::CreateNew,
        "src/valid.rs",
        121,
        "valid-source",
    );
    for (name, invalid) in [
        ("stale-root", {
            let mut value = valid.clone();
            value.expected_root_revision = NonZeroRevision::new(2).unwrap();
            value.operation_id = OpaqueId::new("stale-root").unwrap();
            value
        }),
        ("stale-namespace", {
            let mut value = valid.clone();
            value.namespace = SourceNamespaceSelectionV2::Existing {
                id: search_contracts::SourceNamespaceId::from_bytes([201; 16]),
                revision: NonZeroRevision::new(1).unwrap(),
                generation: search_contracts::SourceOwnerGeneration::from_bytes([202; 32]),
            };
            value.operation_id = OpaqueId::new("stale-namespace").unwrap();
            value
        }),
        ("bytes", {
            let mut value = valid.clone();
            value.binding_observation.content_bytes = 11;
            value.operation_id = OpaqueId::new("bad-bytes").unwrap();
            value
        }),
        ("c04-observation", {
            let mut value = valid.clone();
            value.identity_observation =
                identity_observation(value.root_binding_id, profile, "src/other.rs", 122, 1);
            value.operation_id = OpaqueId::new("bad-observation").unwrap();
            value
        }),
    ] {
        let mut ids = CounterIds::new(141);
        assert!(
            prepare(&mut registry, &invalid, &mut ids, &mut port, &context,).is_err(),
            "{name}"
        );
        assert_eq!(ids.allocations, 0, "{name} allocated an ID");
    }
    drop(port);
    assert_eq!(journal.committed_writes(), before);
    let cancelled_context = OperationContext::new(
        RequestId::from_bytes([177; 16]),
        60_000,
        FakeCancellation::new(true),
        OpaqueRef::new("semantic-source-v2-cancelled").unwrap(),
    )
    .unwrap();
    let mut port = bind_v4(
        &mut journal,
        &mut publisher,
        &work,
        SectionPlatform {
            os: "windows".into(),
            arch: "x64".into(),
        },
        AcceptedCapabilities {
            profiles: vec![BASELINE_PROFILE.into()],
        },
    );
    let mut ids = CounterIds::new(199);
    assert!(
        prepare(
            &mut registry,
            &valid,
            &mut ids,
            &mut port,
            &cancelled_context,
        )
        .is_err()
    );
    assert_eq!(ids.allocations, 0, "pre-cancel drew entropy");
    drop(port);
    assert_eq!(journal.committed_writes(), before, "pre-cancel wrote redb");
}

#[test]
fn source_v2_unknown_blocks_other_prepared_dispatch_and_exact_recovery_reuses_ids() {
    let scratch = Scratch::new();
    let context = context();
    let (platform, capabilities) = platform();
    let profile = profile();
    let policy = policy(RuleEffect::Allow, 7);
    let original = original(&policy, "src/unknown.rs", 131);
    let fail = Cell::new(false);
    let work = |unit: MigrationBootstrapWork| {
        if fail.get() && matches!(unit, MigrationBootstrapWork::Write(n) if n > 0) {
            Err(OwnerError::OwnerCancelledBeforeMutation)
        } else {
            Ok(())
        }
    };
    let mut journal = super::journal(&scratch, &context);
    let mut publisher = search_control_redb::ControlSnapshotPublisher::new();
    let mut registry =
        SourceRegistry::new(search_source_registry::api::DEFAULT_REGISTRY_LIMITS).unwrap();
    let mut port = bind_v4(&mut journal, &mut publisher, &work, platform, capabilities);
    register_source_root(&mut registry, &mut port, &context, &original);
    let first_request = request(
        original.clone(),
        RootBindingId::from_bytes([13; 16]),
        profile,
        1,
        SourceNamespaceSelectionV2::CreateNew,
        "src/unknown.rs",
        131,
        "unknown-source",
    );
    let mut ids = CounterIds::new(151);
    let prepared = prepared(&mut registry, &first_request, &mut ids, &mut port, &context);
    let allocated = ids.allocations;
    let mut probe = || Ok::<_, CheckpointFailureV2>(());
    fail.set(true);
    assert!(
        commit_source_admission_v2(&mut registry, &prepared, &mut port, &context, &mut probe)
            .is_err()
    );
    let other = request(
        original,
        RootBindingId::from_bytes([13; 16]),
        profile,
        1,
        SourceNamespaceSelectionV2::CreateNew,
        "src/other-unknown.rs",
        141,
        "other-unknown-source",
    );
    let before_other_ids = ids.allocations;
    assert!(prepare(&mut registry, &other, &mut ids, &mut port, &context,).is_err());
    assert_eq!(ids.allocations, before_other_ids);
    fail.set(false);
    let decision = port
        .recover_pending(&context)
        .expect("exact control recovery");
    assert!(!port.requires_recovery());
    let mut recovery_probe = || Ok::<_, CheckpointFailureV2>(());
    let recovery = recover_source_admission_v2(
        &mut registry,
        &prepared,
        &port,
        &context,
        &mut recovery_probe,
    )
    .expect("exact semantic recovery");
    assert_eq!(ids.allocations, allocated);
    match recovery {
        search_source_registry::api::SourceAdmissionRecoveryV2::Committed(_) => {}
        search_source_registry::api::SourceAdmissionRecoveryV2::RetrySamePrepared => {
            let mut commit_probe = || Ok::<_, CheckpointFailureV2>(());
            commit_source_admission_v2(
                &mut registry,
                &prepared,
                &mut port,
                &context,
                &mut commit_probe,
            )
            .expect("same prepared retry");
        }
    }
    assert!(matches!(
        decision,
        search_control_redb::CommitRecoveryDecision::Committed(_)
            | search_control_redb::CommitRecoveryDecision::NotCommittedRetrySameOperation
    ));
    assert_eq!(ids.allocations, allocated);
    assert_eq!(registry.sources().len(), 1);
}

//! Two compact causal cases for namespace-only C03 preparation.
//!
//! Scope: semantic, process-local evidence over the existing in-memory C03
//! journal model and the real v4 prepared-batch engine. These cases claim no
//! Native custody, on-disk durability, C18 security genesis or Source-body
//! permission. The thin port below wraps that existing model and adds only the
//! v4 operation codec, the fresh-identity counter and the coherent policy
//! reference the v4 namespace codec requires. It is not a second production
//! store and not a second namespace allocator.

use std::collections::BTreeMap;
use std::sync::Arc;

use search_contracts::{
    Blake3Digest32, InstallationIncarnationId, NamespaceOwnershipStatus, NonZeroRevision, OpaqueId,
    OpaqueRef, OwnerEpoch, PolicyRevision, ReceiptRef, RequestId, RootBindingId, SourceNamespaceId,
    SourceNamespaceOwnership, SourceOwnerGeneration,
};
use search_ports::{
    FakeCancellation, MutationIdentity, OperationContext, Port, PortErrorKind, PortReceipt,
    PortRetryability,
};
use search_source_admission::policy_v2::{
    CheckpointFailureV2, DEFAULT_COMPILE_LIMITS, POLICY_SCHEMA_VERSION, PolicyInputV2, PredicateV2,
    RuleInputV2, compile_policy_v2,
};
use search_source_admission::{
    AcceptedCapabilities, AdmissionError, BASELINE_PROFILE, ConfigSectionInput as AdmissionSection,
    RuleEffect, SectionPlatform, SourceClass, validate_section,
};
use search_source_identity::FilesystemIdentityProfile;
use search_source_provenance::{OriginalInputsLayoutV2, RetainedPolicyV2};
use search_source_records::batch::{
    BatchCodecV4Error, DecodedBatch, OriginalInputResolver, RegistryStateDelta, RegistryStateKey,
    RegistryStateRecord,
};
use search_source_records::error::{JournalEntryKind, RegistryJournalEntry};
use search_source_records::namespace::NamespaceCutoverState;

use super::{
    NativeNamespacePreparationV2, NativeNamespaceRequestV2, PreparedNativeNamespaceV2,
    SourceAdmissionV2Error, commit_native_namespace_v2, prepare_native_namespace_v2,
};
use crate::error::{
    DEFAULT_REGISTRY_LIMITS, InMemoryRegistryJournal, RegistryControlPort, RegistryError,
    RegistryPortError,
};
use crate::recovery::{
    MAX_REGISTRY_RESTORE_BYTES, RegistryBatchLookup, RegistryBatchReadback,
    RegistryRestoreOperation, RegistryRestoreState, SourceRegistry, decode_batch_record_v4,
    restore_registry_state_v4,
};
use crate::root::RegisterRootRequest;
use crate::source_ids::SourceIdentityIdPortV2;

const ROOT: RootBindingId = RootBindingId::from_bytes([13; 16]);
const INSTALLATION: InstallationIncarnationId = InstallationIncarnationId::from_bytes([7; 16]);
const ROOT_OPERATION: &str = "operation:namespace-proof-root";
const V4_BATCH: &str = "eliot.registry-batch.v4";

/// Exactly the UUID-v4 namespace identity the production path draws first.
const DRAWN_NAMESPACE: SourceNamespaceId = SourceNamespaceId::from_bytes([
    0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x40, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
]);

fn cancel() -> FakeCancellation {
    FakeCancellation::new(false)
}

fn operation_context() -> OperationContext<FakeCancellation> {
    OperationContext::new(
        RequestId::from_bytes([1; 16]),
        60_000,
        cancel(),
        OpaqueRef::new("budget:namespace-proof").expect("budget"),
    )
    .expect("context")
}

fn opaque(value: &str) -> OpaqueId {
    OpaqueId::new(value).expect("opaque")
}

fn receipt(value: &str) -> ReceiptRef {
    ReceiptRef::new(value).expect("receipt")
}

fn revision(value: u64) -> NonZeroRevision {
    NonZeroRevision::new(value).expect("revision")
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

/// One retained, compiled policy captured through the real owner factories.
fn policy(policy_revision: u64) -> RetainedPolicyV2 {
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
    .expect("explicit admission section");
    let compiled = compile_policy_v2(
        &PolicyInputV2 {
            schema_version: POLICY_SCHEMA_VERSION,
            revision: policy_revision,
            section,
            rules: vec![RuleInputV2 {
                id: "namespace-proof-allow".into(),
                precedence: 0,
                source_class: SourceClass::Regular,
                predicate: PredicateV2::Always,
                effect: RuleEffect::Allow,
                unknown_fields: vec![],
            }],
            unknown_fields: vec![],
        },
        DEFAULT_COMPILE_LIMITS,
        &mut || Ok(()),
    )
    .expect("compiled policy");
    RetainedPolicyV2::capture(&compiled, &platform, &capabilities, &mut || Ok(()))
        .expect("retained policy")
}

fn profile() -> FilesystemIdentityProfile {
    search_source_identity::ntfs_source_profile_v1().expect("qualified NTFS baseline profile")
}

fn policy_reference(policy: &RetainedPolicyV2) -> Vec<u8> {
    let mut probe = || Ok::<_, CheckpointFailureV2>(());
    OriginalInputsLayoutV2::policy_reference(policy, &mut probe).expect("policy reference")
}

/// Resolves the single retained policy reference a namespace-only v4 delta
/// carries, mirroring the normal control adapter's coherent-snapshot contract.
/// It resolves no admission and no Source original input.
struct RetainedPolicyResolver(Arc<RetainedPolicyV2>);

impl OriginalInputResolver for RetainedPolicyResolver {
    fn resolve_policy(
        &mut self,
        policy_reference: &[u8],
        probe: &mut dyn FnMut() -> Result<(), CheckpointFailureV2>,
    ) -> Result<Arc<RetainedPolicyV2>, BatchCodecV4Error> {
        if OriginalInputsLayoutV2::policy_reference(self.0.as_ref(), probe)? != policy_reference {
            return Err(AdmissionError::ReceiptMismatch.into());
        }
        Ok(Arc::clone(&self.0))
    }

    fn resolve_admission(
        &mut self,
        _admission_reference: &[u8],
        _policy: Arc<RetainedPolicyV2>,
        _probe: &mut dyn FnMut() -> Result<(), CheckpointFailureV2>,
    ) -> Result<search_source_provenance::OriginalAdmissionV2, BatchCodecV4Error> {
        Err(AdmissionError::ReceiptMismatch.into())
    }
}

/// Fresh UUID-v4 allocation counter; every draw is observable.
struct FreshIds {
    next: u8,
    allocations: usize,
}

impl FreshIds {
    const fn new(next: u8) -> Self {
        Self {
            next,
            allocations: 0,
        }
    }

    fn fresh(&mut self) -> [u8; 16] {
        let mut bytes = [0_u8; 16];
        bytes[0] = self.next;
        bytes[6] = 0x40;
        bytes[8] = 0x80;
        self.next = self.next.wrapping_add(1);
        self.allocations += 1;
        bytes
    }
}

struct RetainedOperation {
    bytes: Vec<u8>,
    v4: bool,
}

/// Thin test port over the existing C03 in-memory journal.
///
/// The existing model keeps revision/CAS and retained state bytes for every
/// family. This wrapper adds only the v4 operation codec, the fresh-identity
/// counter and the coherent policy reference the v4 namespace codec requires.
struct NamespacePort {
    journal: InMemoryRegistryJournal<FakeCancellation>,
    ids: FreshIds,
    operations: BTreeMap<OpaqueId, RetainedOperation>,
    policy: Arc<RetainedPolicyV2>,
    revision: u64,
    writes: usize,
}

impl NamespacePort {
    fn new(retained: Arc<RetainedPolicyV2>) -> Self {
        Self {
            journal: InMemoryRegistryJournal::new(1_000).expect("journal"),
            ids: FreshIds::new(1),
            operations: BTreeMap::new(),
            policy: retained,
            revision: 0,
            writes: 0,
        }
    }

    fn readback(&self, record: &DecodedBatch) -> Result<RegistryBatchReadback, RegistryError> {
        RegistryBatchReadback::new(
            record.clone(),
            self.revision,
            record
                .deltas()
                .iter()
                .map(|delta| delta.after().clone())
                .collect(),
            DEFAULT_REGISTRY_LIMITS,
        )
    }

    fn quarantine(operation_id: &OpaqueId) -> RegistryPortError {
        crate::error::registry_port_error(
            PortErrorKind::Quarantined,
            PortRetryability::Never,
            RegistryError::SnapshotInvalid,
            Some(operation_id.clone()),
        )
    }
}

impl Port for NamespacePort {
    type Error = RegistryPortError;
    type Cancellation = FakeCancellation;
}

impl RegistryControlPort for NamespacePort {
    fn commit_batch(
        &mut self,
        batch: &crate::recovery::PreparedBatch,
        context: &OperationContext<FakeCancellation>,
        mutation: &MutationIdentity,
    ) -> Result<RegistryBatchReadback, RegistryPortError> {
        let observed = self.journal.commit_batch(batch, context, mutation)?;
        self.operations.insert(
            mutation.operation_id.clone(),
            RetainedOperation {
                bytes: batch.encoded().to_vec(),
                v4: batch.record().encoding_profile() == V4_BATCH,
            },
        );
        self.revision = observed.revision();
        self.writes += 1;
        Ok(observed)
    }

    fn load_batch(
        &self,
        operation_id: &OpaqueId,
        context: &OperationContext<FakeCancellation>,
    ) -> Result<RegistryBatchLookup, RegistryPortError> {
        context.preflight().map_err(|_| {
            crate::error::registry_port_error(
                PortErrorKind::CancelledBeforeSideEffect,
                PortRetryability::SameRequest,
                RegistryError::CancelledBeforeCommit,
                Some(operation_id.clone()),
            )
        })?;
        let Some(retained) = self.operations.get(operation_id) else {
            return self.journal.load_batch(operation_id, context);
        };
        if !retained.v4 {
            // Legacy families keep the existing model's own lookup semantics.
            return self.journal.load_batch(operation_id, context);
        }
        let mut resolver = RetainedPolicyResolver(Arc::clone(&self.policy));
        let record = decode_batch_record_v4(
            &retained.bytes,
            DEFAULT_REGISTRY_LIMITS,
            &mut resolver,
            &mut || Ok(()),
        )
        .map_err(|_| Self::quarantine(operation_id))?;
        self.readback(&record)
            .map(RegistryBatchLookup::Committed)
            .map_err(|_| Self::quarantine(operation_id))
    }

    fn persist_entry(
        &mut self,
        entry: &RegistryJournalEntry,
        context: &OperationContext<FakeCancellation>,
        mutation: &MutationIdentity,
    ) -> Result<PortReceipt, RegistryPortError> {
        self.journal.persist_entry(entry, context, mutation)
    }

    fn load_entry(
        &self,
        operation_id: &OpaqueId,
        context: &OperationContext<FakeCancellation>,
    ) -> Result<Option<RegistryJournalEntry>, RegistryPortError> {
        self.journal.load_entry(operation_id, context)
    }
}

impl SourceIdentityIdPortV2 for NamespacePort {
    fn fresh_source_id(
        &mut self,
        context: &OperationContext<FakeCancellation>,
    ) -> Result<search_contracts::SourceId, RegistryPortError> {
        context.preflight().expect("live source-id context");
        Ok(search_contracts::SourceId::from_bytes(self.ids.fresh()))
    }

    fn fresh_source_namespace_id(
        &mut self,
        context: &OperationContext<FakeCancellation>,
    ) -> Result<SourceNamespaceId, RegistryPortError> {
        context.preflight().expect("live namespace-id context");
        Ok(SourceNamespaceId::from_bytes(self.ids.fresh()))
    }
}

fn registry_with_root(port: &mut NamespacePort) -> SourceRegistry {
    let mut registry = SourceRegistry::new(DEFAULT_REGISTRY_LIMITS).expect("finite registry");
    let retained = Arc::clone(&port.policy);
    let request = RegisterRootRequest {
        root_binding_id: ROOT,
        canonical_root_digest: Blake3Digest32::from_bytes([11; 32]),
        policy_fingerprint: Blake3Digest32::from_bytes(*retained.fingerprint().as_bytes()),
        policy_revision: revision(retained.compiled().revision().get()),
        owner_epoch: OwnerEpoch::new(1).expect("root epoch"),
        owner_fence_receipt: receipt("receipt:root-owner-fence"),
        policy_receipt: receipt("receipt:root-policy"),
    };
    crate::api::register_root(
        &mut registry,
        &request,
        &opaque(ROOT_OPERATION),
        Blake3Digest32::from_bytes([12; 32]),
        &receipt("receipt:namespace-proof-root"),
        port,
        &operation_context(),
    )
    .expect("root registration");
    registry
}

fn namespace_request(
    retained: &Arc<RetainedPolicyV2>,
    expected_registry_revision: u64,
    operation: &str,
) -> NativeNamespaceRequestV2 {
    NativeNamespaceRequestV2 {
        expected_registry_revision,
        root_binding_id: ROOT,
        expected_root_revision: revision(1),
        owner: super::SourceOwnerScopeV2 {
            system_id: opaque("namespace-proof-owner"),
            installation_incarnation: INSTALLATION,
            epoch: OwnerEpoch::new(1).expect("owner epoch"),
        },
        identity_profile: profile(),
        policy: Arc::clone(retained),
        operation_id: opaque(operation),
        receipt: receipt(&format!("receipt:{operation}")),
    }
}

fn namespace_state(
    retained: &Arc<RetainedPolicyV2>,
    registry_revision: u64,
    namespace_id: SourceNamespaceId,
    last_receipt: ReceiptRef,
) -> NamespaceCutoverState {
    let mut ownership = SourceNamespaceOwnership {
        source_namespace_id: namespace_id,
        owner_system_id: opaque("namespace-proof-owner"),
        owner_installation_incarnation_id: INSTALLATION,
        owner_epoch: OwnerEpoch::new(1).expect("owner epoch"),
        ownership_record_revision: revision(1),
        source_owner_generation: SourceOwnerGeneration::from_bytes([0; 32]),
        source_admission_policy_revision: PolicyRevision::new(retained.compiled().revision().get()),
        status: NamespaceOwnershipStatus::Active,
        cutover_receipt_ref: None,
    };
    ownership.source_owner_generation =
        search_domain::source_ownership_v2::source_owner_generation_v2(&ownership)
            .expect("six-axis owner generation");
    let mut probe = || Ok::<_, CheckpointFailureV2>(());
    NamespaceCutoverState::new_v2_metadata(
        ownership,
        ROOT,
        profile(),
        Arc::clone(retained),
        policy_reference(retained.as_ref()),
        registry_revision,
        last_receipt,
        &mut probe,
    )
    .expect("permanent-v2 namespace metadata")
}

/// Builds the namespace-only v4 operation through the real prepared-batch
/// engine: same delta, fingerprint and record as the production preparation.
///
/// Only the original-input resolver differs, because the production call site
/// passes an empty resolver that cannot satisfy the namespace delta's own
/// retained-policy reference.
fn prepared_namespace(
    registry: &SourceRegistry,
    retained: &Arc<RetainedPolicyV2>,
    request: &NativeNamespaceRequestV2,
) -> PreparedNativeNamespaceV2 {
    let mut probe = || Ok::<_, CheckpointFailureV2>(());
    let fingerprint = super::fingerprint::<RegistryPortError>(request, &mut probe)
        .expect("owner-computed request fingerprint");
    let after = registry
        .revision()
        .checked_add(1)
        .expect("bounded revision");
    let namespace = namespace_state(retained, after, DRAWN_NAMESPACE, request.receipt.clone());
    let record = DecodedBatch::from_observed_parts_v4(
        RegistryJournalEntry::new(
            request.operation_id.clone(),
            fingerprint,
            registry.revision(),
            after,
            JournalEntryKind::BatchApply,
            request.receipt.clone(),
        ),
        fingerprint,
        1,
        vec![
            RegistryStateDelta::new(
                RegistryStateKey::Namespace(DRAWN_NAMESPACE),
                None,
                RegistryStateRecord::Namespace(namespace),
            )
            .expect("namespace-only delta"),
        ],
        registry.limits(),
        &mut RetainedPolicyResolver(Arc::clone(retained)),
        &mut probe,
    )
    .expect("namespace-only v4 record");
    PreparedNativeNamespaceV2 {
        batch: crate::recovery::batch::prepare_v4_record(registry, record)
            .expect("prepared v4 namespace batch"),
    }
}

fn decode_namespace_operation(bytes: &[u8], retained: &Arc<RetainedPolicyV2>) -> DecodedBatch {
    let mut resolver = RetainedPolicyResolver(Arc::clone(retained));
    decode_batch_record_v4(
        bytes,
        DEFAULT_REGISTRY_LIMITS,
        &mut resolver,
        &mut || Ok(()),
    )
    .expect("exact v4 namespace operation roundtrip")
}

#[test]
fn namespace_only_prepare_commit_replay_after_restore_keeps_uuid_without_source_body() {
    let context = operation_context();
    let retained = Arc::new(policy(7));
    let mut port = NamespacePort::new(Arc::clone(&retained));
    let mut registry = registry_with_root(&mut port);
    let root_operation = opaque(ROOT_OPERATION);
    let request = namespace_request(&retained, registry.revision(), "namespace-proof-one");

    // DEFECT GUARD. The production preparation refuses its own namespace delta:
    // the v4 engine is handed `MemoryOriginals::new(vec![])` while the namespace
    // delta carries the retained policy reference, so the closed codec demands a
    // policy resolution the empty resolver cannot satisfy. The fresh path can
    // therefore never return `Prepared`, and the refusal follows the namespace
    // UUID draw, so a caller retry would redraw that identity. When the
    // production call site is repaired this guard fails on purpose and the
    // isolation half below becomes the whole case.
    let mut probe = || Ok::<_, CheckpointFailureV2>(());
    let refused =
        prepare_native_namespace_v2(&mut registry, &request, &mut port, &context, &mut probe);
    assert!(
        matches!(
            refused,
            Err(SourceAdmissionV2Error::Admission(
                AdmissionError::ReceiptMismatch
            ))
        ),
        "expected the empty-resolver ReceiptMismatch refusal from the production call site"
    );
    assert_eq!(
        port.ids.allocations, 1,
        "the refusal follows the namespace UUID draw instead of preceding it"
    );
    assert_eq!(port.writes, 1, "only the root registration was dispatched");
    assert_eq!(registry.revision(), 1);
    assert!(registry.namespaces().is_empty() && registry.sources().is_empty());
    assert!(
        !port.operations.contains_key(&request.operation_id),
        "the refused preparation must leave no durable namespace operation"
    );

    // Isolation of the cause: the identical namespace-only delta, produced by
    // the same v4 engine, commits as soon as the retained policy reference is
    // resolvable. No Source, no body and no Source original input is involved.
    let prepared = prepared_namespace(&registry, &retained, &request);
    let mut probe = || Ok::<_, CheckpointFailureV2>(());
    let committed =
        commit_native_namespace_v2(&mut registry, &prepared, &mut port, &context, &mut probe)
            .expect("namespace-only commit");
    assert!(!committed.replayed());
    assert_eq!(committed.operation_id(), &request.operation_id);
    assert_eq!(
        committed.namespace().ownership.source_namespace_id,
        DRAWN_NAMESPACE,
        "the committed namespace must be the identity the draw produced"
    );
    assert_eq!(registry.revision(), 2);
    assert!(registry.sources().is_empty(), "no Source was created");
    assert!(
        registry
            .namespaces()
            .values()
            .all(|namespace| namespace.permanent_metadata().is_some()),
        "namespace metadata must be permanent-v2, not historical"
    );

    let committed_bytes = prepared.batch().encoded().to_vec();
    let committed_record = decode_namespace_operation(&committed_bytes, &retained);
    assert_eq!(committed_record.encoding_profile(), V4_BATCH);
    assert_eq!(
        committed_record.deltas().len(),
        1,
        "a namespace-only operation must carry exactly one delta"
    );
    let RegistryStateRecord::Namespace(decoded_namespace) = committed_record.deltas()[0].after()
    else {
        panic!("namespace-only operation must carry a namespace delta");
    };
    assert_eq!(
        decoded_namespace.ownership.source_namespace_id,
        DRAWN_NAMESPACE
    );
    assert_eq!(
        decoded_namespace
            .permanent_metadata()
            .expect("permanent-v2 metadata")
            .registry_revision(),
        2
    );

    // Exact restore of both retained operations and both current states through
    // the real v4 owner, with a resolver that still resolves no Source input.
    let root_record = match port
        .journal
        .load_batch(&root_operation, &context)
        .expect("root readback")
    {
        RegistryBatchLookup::Committed(readback) => readback,
        RegistryBatchLookup::Absent { .. } => panic!("root operation must be committed"),
    };
    let root_bytes = port
        .operations
        .get(&root_operation)
        .expect("retained root operation bytes")
        .bytes
        .clone();
    let root_state = root_record.states()[0].clone();
    let root_state_key = root_state.key().encode().expect("root state key");
    let root_state_bytes = root_state.encode().expect("root state bytes");
    let namespace_state_key = committed_record.deltas()[0]
        .key()
        .encode()
        .expect("namespace state key");
    let namespace_state_bytes = committed_record.deltas()[0]
        .after()
        .encode_with_profile("eliot.registry-state.v4")
        .expect("namespace state bytes");
    let states = [
        RegistryRestoreState {
            key: &root_state_key,
            value: &root_state_bytes,
        },
        RegistryRestoreState {
            key: &namespace_state_key,
            value: &namespace_state_bytes,
        },
    ];
    let operations = [
        RegistryRestoreOperation {
            operation_id: &root_operation,
            value: &root_bytes,
        },
        RegistryRestoreOperation {
            operation_id: &request.operation_id,
            value: &committed_bytes,
        },
    ];
    let mut resolver = RetainedPolicyResolver(Arc::clone(&retained));
    let mut probe = || Ok::<_, CheckpointFailureV2>(());
    let mut restored = restore_registry_state_v4(
        2,
        &states,
        &operations,
        DEFAULT_REGISTRY_LIMITS,
        MAX_REGISTRY_RESTORE_BYTES,
        &mut resolver,
        &mut probe,
    )
    .expect("exact v4 restore")
    .into_registry();
    assert_eq!(restored.revision(), 2);
    assert!(restored.sources().is_empty(), "restore created no Source");
    let usable = restored
        .namespaces()
        .get(&DRAWN_NAMESPACE)
        .expect("restored namespace")
        .permanent_metadata()
        .expect("usable permanent-v2 namespace metadata");
    assert_eq!(usable.root_binding_id(), ROOT);
    assert_eq!(usable.identity_profile(), profile());
    assert_eq!(usable.policy_reference(), policy_reference(&retained));
    assert_eq!(usable.registry_revision(), 2);

    // An independent later registry change must not disturb exact replay.
    let later_context = operation_context();
    crate::api::update_root_policy(
        &mut restored,
        ROOT,
        revision(1),
        Blake3Digest32::from_bytes([21; 32]),
        revision(2),
        &opaque("operation:namespace-proof-later"),
        Blake3Digest32::from_bytes([22; 32]),
        &receipt("receipt:namespace-proof-later"),
        &mut port,
        &later_context,
    )
    .expect("later independent root policy change");
    assert_eq!(restored.revision(), 3);

    let allocations_before_replay = port.ids.allocations;
    let mut probe = || Ok::<_, CheckpointFailureV2>(());
    let replayed =
        prepare_native_namespace_v2(&mut restored, &request, &mut port, &context, &mut probe)
            .expect("exact namespace operation replay");
    let NativeNamespacePreparationV2::Replayed(replay) = replayed else {
        panic!("committed namespace operation must replay from its original record");
    };
    assert!(replay.replayed());
    assert_eq!(replay.operation_id(), &request.operation_id);
    assert_eq!(
        replay.namespace().ownership.source_namespace_id,
        DRAWN_NAMESPACE,
        "exact replay must keep the original namespace UUID"
    );
    assert_eq!(
        port.ids.allocations, allocations_before_replay,
        "exact replay drew fresh namespace entropy"
    );
    assert_eq!(
        restored.revision(),
        3,
        "replay must not advance the registry"
    );
    assert!(restored.sources().is_empty(), "replay created no Source");
}

#[test]
fn namespace_only_refusals_precede_any_fresh_namespace_id() {
    let context = operation_context();
    let retained = Arc::new(policy(7));
    let mut port = NamespacePort::new(Arc::clone(&retained));
    let mut registry = registry_with_root(&mut port);
    let committed = namespace_request(&retained, registry.revision(), "namespace-proof-one");
    let prepared = prepared_namespace(&registry, &retained, &committed);
    let mut probe = || Ok::<_, CheckpointFailureV2>(());
    commit_native_namespace_v2(&mut registry, &prepared, &mut port, &context, &mut probe)
        .expect("committed namespace for the refusal matrix");
    assert_eq!(registry.revision(), 2);
    assert_eq!(
        port.ids.allocations, 0,
        "the isolated commit replays the prepared identity and draws nothing"
    );

    let foreign_policy = Arc::new(policy(8));
    let cases: [(&str, NativeNamespaceRequestV2, RegistryError); 4] = [
        (
            "stale-root-revision",
            {
                let mut value = namespace_request(&retained, 2, "refusal-root-revision");
                value.expected_root_revision = revision(2);
                value
            },
            RegistryError::RootPolicyGenerationMismatch,
        ),
        (
            "root-policy-generation-mismatch",
            namespace_request(&foreign_policy, 2, "refusal-root-policy"),
            RegistryError::RootPolicyGenerationMismatch,
        ),
        (
            "existing-matching-namespace",
            namespace_request(&retained, 2, "refusal-existing-namespace"),
            RegistryError::NamespaceOwnershipConflict,
        ),
        (
            "reused-operation-changed-body",
            {
                let mut value = namespace_request(&retained, 2, "namespace-proof-one");
                value.receipt = receipt("receipt:namespace-proof-one-changed");
                value
            },
            RegistryError::OperationConflict,
        ),
    ];
    for (name, request, expected) in cases {
        let allocations_before = port.ids.allocations;
        let writes_before = port.writes;
        let mut probe = || Ok::<_, CheckpointFailureV2>(());
        let result =
            prepare_native_namespace_v2(&mut registry, &request, &mut port, &context, &mut probe);
        assert!(
            matches!(
                result,
                Err(SourceAdmissionV2Error::Registry(reason)) if reason == expected
            ),
            "{name} must refuse with {expected:?} before a fresh namespace UUID is drawn"
        );
        assert_eq!(
            port.ids.allocations, allocations_before,
            "{name} drew fresh namespace entropy"
        );
        assert_eq!(port.writes, writes_before, "{name} dispatched a write");
    }
    assert_eq!(registry.revision(), 2);
    assert_eq!(registry.namespaces().len(), 1);
    assert!(registry.sources().is_empty());
    assert!(port.operations.contains_key(&committed.operation_id));
}

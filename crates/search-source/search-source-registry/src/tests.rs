//! Normative qualification evidence for `search-source-registry`.

use std::collections::{BTreeMap, BTreeSet};

use search_contracts::{
    Blake3Digest32, CutoverAuthorization, CutoverId, CutoverValidation, InstallationIncarnationId,
    MembershipRole, NamespaceOwnershipStatus, NewSourceOwnerActivation, NonZeroRevision,
    OldSourceOwnerFence, OpaqueCanonicalBytes, OpaqueId, OpaqueRef, OwnerEpoch, ReceiptRef,
    ReferencePortfolioId, RequestId, RootBindingId, SourceId, SourceIdentity, SourceIdentityKind,
    SourceNamespaceId, SourceNamespaceOwnership, SourceOwnerCutover, SourceOwnerCutoverProtocolV1,
    SourceOwnerCutoverReceipt, SourceOwnerGeneration, SourceViewRef, UtcTimestamp, WorkspaceId,
};
use search_ports::{FakeCancellation, OperationContext};
use search_source_admission::{
    AdmissionBudget, AdmissionReceipt, BASELINE_PROFILE, CancelFlag, DEFAULT_ADMISSION_LIMITS,
    UnvalidatedObservationInput, baseline_policy, evaluate, issue_receipt, policy_fingerprint,
    validate_observation,
};
use search_source_identity::{
    CanonicalRelativePath, RootBindingId as IdentityRootBindingId, SourceBinding, SourceObservation,
};

use crate::api;
use crate::error::{
    DEFAULT_REGISTRY_LIMITS, InMemoryRegistryJournal, RegistryControlPort, RegistryError,
};
use crate::membership::{
    BindMembershipRequest, MembershipCommand, MembershipKey, MembershipPolicies,
};
use crate::portfolio::PublishPortfolioRequest;
use crate::recovery::{
    NamespaceCutover, RegistryBatch, RegistryChange, RegistryOperation, SourceRegistry,
};
use crate::root::RegisterRootRequest;
use crate::source::AdmissionBindingProof;
use crate::view::{ResolveSourceViewRequest, ResolveWorkspaceViewRequest};

fn cancel() -> FakeCancellation {
    FakeCancellation::new(false)
}

fn cancelled() -> FakeCancellation {
    FakeCancellation::new(true)
}

fn context(cancellation: FakeCancellation) -> OperationContext<FakeCancellation> {
    OperationContext::new(
        RequestId::from_bytes([1; 16]),
        1_000,
        cancellation,
        OpaqueRef::new("budget:test").expect("budget"),
    )
    .expect("context")
}

fn journal() -> InMemoryRegistryJournal<FakeCancellation> {
    InMemoryRegistryJournal::new(1_000).expect("journal")
}

fn opaque(value: &str) -> OpaqueId {
    OpaqueId::new(value).expect("opaque")
}

fn receipt(value: &str) -> ReceiptRef {
    ReceiptRef::new(value).expect("receipt")
}

fn digest(byte: u8) -> Blake3Digest32 {
    Blake3Digest32::from_bytes([byte; 32])
}

fn operation(name: &str, byte: u8) -> RegistryOperation {
    RegistryOperation::new(
        opaque(format!("registry-operation:{name}").as_str()),
        digest(byte),
    )
}

fn allow_receipt(policy_revision: u64) -> AdmissionReceipt {
    let policy = baseline_policy(policy_revision, 1_024);
    let input = UnvalidatedObservationInput {
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
        profile_id: Some(BASELINE_PROFILE.to_owned()),
        unavailable_fields: Vec::new(),
        unknown_fields: Vec::new(),
    };
    let observation =
        validate_observation(&input, &policy, DEFAULT_ADMISSION_LIMITS).expect("observation");
    let decision = evaluate(
        &policy,
        &observation,
        AdmissionBudget::default_budget(),
        CancelFlag::live(),
    )
    .expect("decision");
    assert_eq!(
        decision.outcome(),
        search_source_admission::AdmissionOutcome::Allow
    );
    issue_receipt(&policy, &observation, &decision).expect("receipt")
}

fn root_policy_fingerprint() -> Blake3Digest32 {
    let policy = baseline_policy(1, 1_024);
    Blake3Digest32::from_bytes(*policy_fingerprint(&policy).as_bytes())
}

fn source_identity(name: &str) -> SourceIdentity {
    let mut source_id = [0_u8; 16];
    for (index, byte) in name.bytes().enumerate() {
        let slot = index % source_id.len();
        source_id[slot] ^= byte;
    }
    SourceIdentity {
        source_namespace_id: SourceNamespaceId::from_bytes([1; 16]),
        source_id: SourceId::from_bytes(source_id),
        identity_kind: SourceIdentityKind::NtfsFile,
        stable_identity_components: OpaqueCanonicalBytes::from_validated(
            format!("registry-test:{name}").into_bytes(),
        )
        .expect("stable identity components"),
    }
}

fn source_binding(name: &str) -> SourceBinding {
    SourceBinding::new(
        source_identity(name),
        SourceObservation {
            root_binding_id: IdentityRootBindingId::from_bytes([9; 16]),
            relative_path: CanonicalRelativePath::new(
                format!("{name}.rs"),
                search_source_identity::DEFAULT_IDENTITY_LIMITS,
            )
            .expect("path"),
            stable_file_identity_digest: Some(digest(2)),
            content_digest: digest(3),
            content_bytes: 10,
            observation_receipt: receipt(format!("receipt:observation:{name}").as_str()),
        },
        NonZeroRevision::new(1).expect("revision"),
        NonZeroRevision::new(1).expect("revision"),
    )
}

fn assignment(name: &str, admission: &AdmissionReceipt) -> AdmissionBindingProof {
    AdmissionBindingProof {
        observation_digest: Blake3Digest32::from_bytes(*admission.observation_digest().as_bytes()),
        source_identity: source_identity(name),
        assignment_digest: digest(6),
        assignment_receipt: receipt(format!("receipt:assignment:{name}").as_str()),
        readback_verified: true,
    }
}

fn register_change(name: &str) -> RegistryChange {
    let admission = allow_receipt(1);
    let proof = assignment(name, &admission);
    RegistryChange::RegisterSource {
        admission,
        binding: source_binding(name),
        assignment: proof,
        receipt: receipt(format!("receipt:register:{name}").as_str()),
    }
}

fn registry_with_source(name: &str) -> SourceRegistry {
    let mut registry = SourceRegistry::new(DEFAULT_REGISTRY_LIMITS).expect("registry");
    registry
        .apply(RegistryBatch {
            expected_registry_revision: 0,
            operation: operation("register", 1),
            changes: vec![register_change(name)],
        })
        .expect("register");
    registry
}

fn root_request() -> RegisterRootRequest {
    RegisterRootRequest {
        root_binding_id: RootBindingId::from_bytes([9; 16]),
        canonical_root_digest: digest(11),
        policy_fingerprint: root_policy_fingerprint(),
        policy_revision: NonZeroRevision::new(1).expect("revision"),
        owner_epoch: OwnerEpoch::new(1).expect("epoch"),
        owner_fence_receipt: receipt("receipt:owner-fence"),
        policy_receipt: receipt("receipt:policy"),
    }
}

fn bind_request(name: &str, corpus: &str) -> BindMembershipRequest {
    BindMembershipRequest {
        key: MembershipKey {
            corpus_id: opaque(corpus),
            source_identity: source_identity(name),
        },
        generation: NonZeroRevision::new(1).expect("generation"),
        policies: MembershipPolicies {
            role: MembershipRole::Source,
            access_policy: opaque("policy:access"),
            scoring_policy: opaque("policy:scoring"),
            residency_policy: opaque("policy:residency"),
            corpus_policy_revision: NonZeroRevision::new(1).expect("revision"),
        },
    }
}

// ---------------------------------------------------------------------------
// Preserved legacy invariants
// ---------------------------------------------------------------------------

#[test]
fn registration_is_exact_revision_guarded() {
    let mut registry = SourceRegistry::new(DEFAULT_REGISTRY_LIMITS).expect("registry");
    assert_eq!(
        registry.apply(RegistryBatch {
            expected_registry_revision: 1,
            operation: operation("register", 1),
            changes: vec![register_change("one")],
        }),
        Err(RegistryError::RegistryRevisionConflict)
    );
    assert_eq!(registry.revision(), 0);
}

#[test]
fn full_payload_operation_replay_is_exact() {
    let mut registry = SourceRegistry::new(DEFAULT_REGISTRY_LIMITS).expect("registry");
    let batch = RegistryBatch {
        expected_registry_revision: 0,
        operation: operation("register", 1),
        changes: vec![register_change("one")],
    };
    let first = registry.apply(batch.clone()).expect("first");
    let replay = registry.apply(batch).expect("replay");
    assert_eq!(first.after_revision, replay.after_revision);
    assert!(replay.replayed);
    assert_eq!(registry.revision(), 1);
}

#[test]
fn operation_identity_reuse_with_other_payload_is_rejected() {
    let mut registry = SourceRegistry::new(DEFAULT_REGISTRY_LIMITS).expect("registry");
    registry
        .apply(RegistryBatch {
            expected_registry_revision: 0,
            operation: operation("same", 1),
            changes: vec![register_change("one")],
        })
        .expect("first");
    assert_eq!(
        registry.apply(RegistryBatch {
            expected_registry_revision: 1,
            operation: operation("same", 2),
            changes: vec![register_change("two")],
        }),
        Err(RegistryError::OperationConflict)
    );
}

#[test]
fn candidate_assignment_mismatch_is_atomic_failure() {
    let mut registry = SourceRegistry::new(DEFAULT_REGISTRY_LIMITS).expect("registry");
    let admission = allow_receipt(1);
    let mut proof = assignment("one", &admission);
    proof.observation_digest = digest(99);
    assert_eq!(
        registry.apply(RegistryBatch {
            expected_registry_revision: 0,
            operation: operation("register", 1),
            changes: vec![RegistryChange::RegisterSource {
                admission,
                binding: source_binding("one"),
                assignment: proof,
                receipt: receipt("receipt:register"),
            }],
        }),
        Err(RegistryError::AdmissionBindingMismatch)
    );
    assert_eq!(registry.revision(), 0);
}

#[test]
fn duplicate_source_corpus_membership_is_rejected() {
    let mut registry = registry_with_source("one");
    let membership = crate::membership::NewMembership {
        key: MembershipKey {
            corpus_id: opaque("corpus:test"),
            source_identity: source_identity("one"),
        },
        generation: NonZeroRevision::new(1).expect("generation"),
        receipt: receipt("receipt:membership"),
    };
    registry
        .apply(RegistryBatch {
            expected_registry_revision: 1,
            operation: operation("membership-one", 2),
            changes: vec![RegistryChange::AddMembership(membership.clone())],
        })
        .expect("membership");
    assert_eq!(
        registry.apply(RegistryBatch {
            expected_registry_revision: 2,
            operation: operation("membership-two", 3),
            changes: vec![RegistryChange::AddMembership(membership)],
        }),
        Err(RegistryError::MembershipCollision)
    );
}

#[test]
fn retiring_source_removes_it_from_active_portfolio() {
    let mut registry = registry_with_source("one");
    let corpus = opaque("corpus:test");
    registry
        .apply(RegistryBatch {
            expected_registry_revision: 1,
            operation: operation("membership", 2),
            changes: vec![RegistryChange::AddMembership(
                crate::membership::NewMembership {
                    key: MembershipKey {
                        corpus_id: corpus.clone(),
                        source_identity: source_identity("one"),
                    },
                    generation: NonZeroRevision::new(1).expect("generation"),
                    receipt: receipt("receipt:membership"),
                },
            )],
        })
        .expect("membership");
    registry
        .apply(RegistryBatch {
            expected_registry_revision: 2,
            operation: operation("retire", 3),
            changes: vec![RegistryChange::RetireSource {
                identity: source_identity("one"),
                expected_source_revision: NonZeroRevision::new(1).expect("revision"),
                receipt: receipt("receipt:retire"),
            }],
        })
        .expect("retire");
    let portfolio = registry
        .active_portfolio(&corpus, NonZeroRevision::new(1).expect("generation"), 10)
        .expect("portfolio");
    assert!(portfolio.is_empty());
}

#[test]
fn namespace_cutover_is_atomic_and_inventory_bound() {
    let mut registry = SourceRegistry::new(DEFAULT_REGISTRY_LIMITS).expect("registry");
    registry
        .apply(RegistryBatch {
            expected_registry_revision: 0,
            operation: operation("register", 1),
            changes: vec![register_change("one"), register_change("two")],
        })
        .expect("register");
    let corpus = opaque("corpus:test");
    registry
        .apply(RegistryBatch {
            expected_registry_revision: 1,
            operation: operation("cutover", 2),
            changes: vec![RegistryChange::CutoverNamespace(NamespaceCutover {
                corpus_id: corpus.clone(),
                expected_generation: None,
                next_generation: NonZeroRevision::new(1).expect("generation"),
                frozen_inventory: vec![source_identity("one"), source_identity("two")],
                inventory_digest: digest(7),
                authorization_verified: true,
                readback_verified: true,
                receipt: receipt("receipt:cutover"),
            })],
        })
        .expect("cutover");
    let portfolio = registry
        .active_portfolio(&corpus, NonZeroRevision::new(1).expect("generation"), 10)
        .expect("portfolio");
    assert_eq!(portfolio.len(), 2);
}

#[test]
fn failed_multi_change_batch_does_not_partially_register() {
    let mut registry = SourceRegistry::new(DEFAULT_REGISTRY_LIMITS).expect("registry");
    let admission = allow_receipt(1);
    let mut invalid = assignment("two", &admission);
    invalid.readback_verified = false;
    assert_eq!(
        registry.apply(RegistryBatch {
            expected_registry_revision: 0,
            operation: operation("batch", 9),
            changes: vec![
                register_change("one"),
                RegistryChange::RegisterSource {
                    admission,
                    binding: source_binding("two"),
                    assignment: invalid,
                    receipt: receipt("receipt:two"),
                },
            ],
        }),
        Err(RegistryError::AdmissionBindingEvidenceMissing)
    );
    assert_eq!(registry.revision(), 0);
    assert_eq!(
        registry.source(&source_identity("one")),
        Err(RegistryError::SourceNotFound)
    );
}

// ---------------------------------------------------------------------------
// Normative root operations with control-port persistence
// ---------------------------------------------------------------------------

#[test]
fn root_register_replay_conflict_and_policy_obligations() {
    let mut registry = SourceRegistry::new(DEFAULT_REGISTRY_LIMITS).expect("registry");
    let mut port = journal();
    let ctx = context(cancel());
    let request = root_request();
    let first = api::register_root(
        &mut registry,
        &request,
        &opaque("operation:root-one"),
        digest(21),
        &receipt("receipt:root-one"),
        &mut port,
        &ctx,
    )
    .expect("register");
    assert!(!first.replayed);
    assert_eq!(registry.revision(), 1);
    assert_eq!(port.len(), 1);
    let replay = api::register_root(
        &mut registry,
        &request,
        &opaque("operation:root-one"),
        digest(21),
        &receipt("receipt:root-one"),
        &mut port,
        &ctx,
    )
    .expect("replay");
    assert!(replay.replayed);
    assert_eq!(registry.revision(), 1);
    assert_eq!(
        api::register_root(
            &mut registry,
            &request,
            &opaque("operation:root-one"),
            digest(22),
            &receipt("receipt:root-one"),
            &mut port,
            &ctx,
        ),
        Err(RegistryError::OperationConflict)
    );
    let change = api::update_root_policy(
        &mut registry,
        request.root_binding_id,
        first.record_revision,
        digest(13),
        NonZeroRevision::new(2).expect("revision"),
        &opaque("operation:root-policy"),
        digest(23),
        &receipt("receipt:root-policy"),
        &mut port,
        &ctx,
    )
    .expect("policy change");
    assert!(!change.obligations.is_empty());
    assert_eq!(registry.revision(), 2);
    let unbind = api::unbind_root(
        &mut registry,
        request.root_binding_id,
        change.record_revision,
        &opaque("operation:root-unbind"),
        digest(24),
        &receipt("receipt:root-unbind"),
        &mut port,
        &ctx,
    )
    .expect("unbind");
    assert!(!unbind.obligations.is_empty());
    assert_eq!(registry.revision(), 3);
}

#[test]
fn root_canonical_conflict_is_rejected() {
    let mut registry = SourceRegistry::new(DEFAULT_REGISTRY_LIMITS).expect("registry");
    let mut port = journal();
    let ctx = context(cancel());
    let first = root_request();
    api::register_root(
        &mut registry,
        &first,
        &opaque("operation:root-a"),
        digest(31),
        &receipt("receipt:a"),
        &mut port,
        &ctx,
    )
    .expect("first");
    let second = RegisterRootRequest {
        root_binding_id: RootBindingId::from_bytes([10; 16]),
        ..first
    };
    assert_eq!(
        api::register_root(
            &mut registry,
            &second,
            &opaque("operation:root-b"),
            digest(32),
            &receipt("receipt:b"),
            &mut port,
            &ctx,
        ),
        Err(RegistryError::RootIdentityConflict)
    );
}

// ---------------------------------------------------------------------------
// Normative source admission with verified receipt persistence
// ---------------------------------------------------------------------------

fn registry_with_root() -> (SourceRegistry, InMemoryRegistryJournal<FakeCancellation>) {
    let mut registry = SourceRegistry::new(DEFAULT_REGISTRY_LIMITS).expect("registry");
    let mut port = journal();
    let ctx = context(cancel());
    api::register_root(
        &mut registry,
        &root_request(),
        &opaque("operation:root"),
        digest(40),
        &receipt("receipt:root"),
        &mut port,
        &ctx,
    )
    .expect("root");
    (registry, port)
}

#[test]
fn admit_source_requires_current_allow_receipt() {
    let (mut registry, mut port) = registry_with_root();
    let ctx = context(cancel());
    let current = allow_receipt(1);
    let admitted = api::admit_source(
        &mut registry,
        &source_identity("one"),
        &source_binding("one"),
        &current,
        &assignment("one", &current),
        &opaque("operation:admit-one"),
        digest(41),
        &receipt("receipt:admit-one"),
        &mut port,
        &ctx,
    )
    .expect("admit");
    assert!(!admitted.replayed);
    assert_eq!(
        api::admit_source(
            &mut registry,
            &source_identity("one"),
            &source_binding("one"),
            &current,
            &assignment("one", &current),
            &opaque("operation:admit-two"),
            digest(42),
            &receipt("receipt:admit-two"),
            &mut port,
            &ctx,
        ),
        Err(RegistryError::SourceAlreadyAdmittedConflict)
    );
    let stale = allow_receipt(2);
    assert_eq!(
        api::admit_source(
            &mut registry,
            &source_identity("two"),
            &source_binding("two"),
            &stale,
            &assignment("two", &stale),
            &opaque("operation:admit-stale"),
            digest(43),
            &receipt("receipt:stale"),
            &mut port,
            &ctx,
        ),
        Err(RegistryError::AdmissionReceiptStale)
    );
    let mismatched_proof = {
        let mut proof = assignment("three", &current);
        proof.observation_digest = digest(99);
        proof
    };
    assert_eq!(
        api::admit_source(
            &mut registry,
            &source_identity("three"),
            &source_binding("three"),
            &current,
            &mismatched_proof,
            &opaque("operation:admit-mismatch"),
            digest(44),
            &receipt("receipt:mismatch"),
            &mut port,
            &ctx,
        ),
        Err(RegistryError::AdmissionReceiptMismatch)
    );
}

#[test]
fn restrictive_revalidation_fences_downstream_use() {
    let (mut registry, mut port) = registry_with_root();
    let ctx = context(cancel());
    let current = allow_receipt(1);
    api::admit_source(
        &mut registry,
        &source_identity("one"),
        &source_binding("one"),
        &current,
        &assignment("one", &current),
        &opaque("operation:admit"),
        digest(51),
        &receipt("receipt:admit"),
        &mut port,
        &ctx,
    )
    .expect("admit");
    let update = api::revalidate_admitted_source(
        &mut registry,
        &source_identity("one"),
        NonZeroRevision::new(1).expect("revision"),
        &current,
        &opaque("operation:revalidate"),
        digest(52),
        &receipt("receipt:revalidate"),
        &mut port,
        &ctx,
    )
    .expect("revalidate");
    assert!(
        update
            .obligations
            .contains(&crate::source::SourceRevalidationObligation::FenceServing)
    );
}

// ---------------------------------------------------------------------------
// Membership requires the exact current admission receipt
// ---------------------------------------------------------------------------

#[test]
fn membership_requires_matching_admission_receipt() {
    let (mut registry, mut port) = registry_with_root();
    let ctx = context(cancel());
    let current = allow_receipt(1);
    api::admit_source(
        &mut registry,
        &source_identity("one"),
        &source_binding("one"),
        &current,
        &assignment("one", &current),
        &opaque("operation:admit"),
        digest(61),
        &receipt("receipt:admit"),
        &mut port,
        &ctx,
    )
    .expect("admit");
    let bound = api::bind_membership(
        &mut registry,
        &bind_request("one", "corpus:test"),
        &current,
        &opaque("operation:bind"),
        digest(62),
        &receipt("receipt:bind"),
        &mut port,
        &ctx,
    )
    .expect("bind");
    assert!(!bound.replayed);
    assert_eq!(
        api::bind_membership(
            &mut registry,
            &bind_request("one", "corpus:test"),
            &current,
            &opaque("operation:bind-two"),
            digest(63),
            &receipt("receipt:bind-two"),
            &mut port,
            &ctx,
        ),
        Err(RegistryError::MembershipConflict)
    );
}

#[test]
fn registry_cannot_weaken_admission_rules() {
    let (mut registry, mut port) = registry_with_root();
    let ctx = context(cancel());
    let current = allow_receipt(1);
    api::admit_source(
        &mut registry,
        &source_identity("one"),
        &source_binding("one"),
        &current,
        &assignment("one", &current),
        &opaque("operation:admit"),
        digest(71),
        &receipt("receipt:admit"),
        &mut port,
        &ctx,
    )
    .expect("admit");
    let stale = allow_receipt(2);
    assert_eq!(
        api::bind_membership(
            &mut registry,
            &bind_request("one", "corpus:test"),
            &stale,
            &opaque("operation:bind-stale"),
            digest(72),
            &receipt("receipt:bind-stale"),
            &mut port,
            &ctx,
        ),
        Err(RegistryError::AdmissionReceiptStale)
    );
    let other = allow_receipt(1);
    let mut foreign_proof = assignment("one", &other);
    foreign_proof.observation_digest = digest(98);
    let _ = foreign_proof;
    let tampered = {
        let policy = baseline_policy(1, 1_024);
        let input = UnvalidatedObservationInput {
            locator_class: "normalized-file".to_owned(),
            location_class: "local-fixed".to_owned(),
            source_kind: "file".to_owned(),
            source_class: "regular".to_owned(),
            byte_size: Some(11),
            is_generated: Some(false),
            is_vendor: Some(false),
            is_binary: Some(false),
            is_system: Some(false),
            sensitivity: Some("public".to_owned()),
            detector_id: Some("detector:baseline-v1".to_owned()),
            profile_id: Some(BASELINE_PROFILE.to_owned()),
            unavailable_fields: Vec::new(),
            unknown_fields: Vec::new(),
        };
        let observation =
            validate_observation(&input, &policy, DEFAULT_ADMISSION_LIMITS).expect("observation");
        let decision = evaluate(
            &policy,
            &observation,
            AdmissionBudget::default_budget(),
            CancelFlag::live(),
        )
        .expect("decision");
        issue_receipt(&policy, &observation, &decision).expect("receipt")
    };
    assert_eq!(
        api::bind_membership(
            &mut registry,
            &bind_request("one", "corpus:test"),
            &tampered,
            &opaque("operation:bind-foreign"),
            digest(73),
            &receipt("receipt:bind-foreign"),
            &mut port,
            &ctx,
        ),
        Err(RegistryError::AdmissionReceiptMismatch)
    );
}

#[test]
fn reverse_membership_id_is_stable_and_unique_per_source_corpus() {
    let (mut registry, mut port) = registry_with_root();
    let ctx = context(cancel());
    let current = allow_receipt(1);
    for name in ["one", "two"] {
        api::admit_source(
            &mut registry,
            &source_identity(name),
            &source_binding(name),
            &current,
            &assignment(name, &current),
            &opaque(format!("operation:admit-{name}").as_str()),
            digest(80),
            &receipt(format!("receipt:admit-{name}").as_str()),
            &mut port,
            &ctx,
        )
        .expect("admit");
    }
    let first = api::bind_membership(
        &mut registry,
        &bind_request("one", "corpus:test"),
        &current,
        &opaque("operation:bind-one"),
        digest(81),
        &receipt("receipt:bind-one"),
        &mut port,
        &ctx,
    )
    .expect("bind one");
    let second = api::bind_membership(
        &mut registry,
        &bind_request("two", "corpus:test"),
        &current,
        &opaque("operation:bind-two"),
        digest(82),
        &receipt("receipt:bind-two"),
        &mut port,
        &ctx,
    )
    .expect("bind two");
    assert_ne!(first.membership_id, second.membership_id);
    let derived = crate::membership::derive_membership_id(&bind_request("one", "corpus:test").key);
    assert_eq!(derived, first.membership_id);
    assert_eq!(
        registry
            .membership_key_by_id(first.membership_id)
            .expect("reverse"),
        &bind_request("one", "corpus:test").key
    );
    let transitioned = api::transition_membership(
        &mut registry,
        &bind_request("one", "corpus:test").key,
        MembershipCommand::Restrict,
        NonZeroRevision::new(1).expect("revision"),
        &opaque("operation:restrict"),
        digest(83),
        &receipt("receipt:restrict"),
        &mut port,
        &ctx,
    )
    .expect("restrict");
    assert!(!transitioned.obligations.is_empty());
}

#[test]
fn empty_reference_portfolio_reason() {
    let (mut registry, mut port) = registry_with_root();
    let ctx = context(cancel());
    assert_eq!(
        api::publish_reference_portfolio(
            &mut registry,
            &PublishPortfolioRequest {
                portfolio_id: ReferencePortfolioId::from_bytes([1; 16]),
                portfolio_revision: search_contracts::PortfolioRevision::new(1),
                membership_precedence: Vec::new(),
                allow_empty: false,
            },
            &opaque("operation:portfolio-empty"),
            digest(91),
            &receipt("receipt:portfolio-empty"),
            &mut port,
            &ctx,
        ),
        Err(RegistryError::ReferenceScopeEmpty)
    );
    let allowed = api::publish_reference_portfolio(
        &mut registry,
        &PublishPortfolioRequest {
            portfolio_id: ReferencePortfolioId::from_bytes([1; 16]),
            portfolio_revision: search_contracts::PortfolioRevision::new(1),
            membership_precedence: Vec::new(),
            allow_empty: true,
        },
        &opaque("operation:portfolio-allowed"),
        digest(92),
        &receipt("receipt:portfolio-allowed"),
        &mut port,
        &ctx,
    )
    .expect("explicit empty");
    assert!(!allowed.replayed);
}

// ---------------------------------------------------------------------------
// Views: explicit scope, coherence, non-disclosure, workspace revisions
// ---------------------------------------------------------------------------

fn registry_with_view() -> (
    SourceRegistry,
    InMemoryRegistryJournal<FakeCancellation>,
    OpaqueId,
) {
    let (mut registry, mut port) = registry_with_root();
    let ctx = context(cancel());
    let current = allow_receipt(1);
    for name in ["one", "two"] {
        api::admit_source(
            &mut registry,
            &source_identity(name),
            &source_binding(name),
            &current,
            &assignment(name, &current),
            &opaque(format!("operation:admit-{name}").as_str()),
            digest(100),
            &receipt(format!("receipt:admit-{name}").as_str()),
            &mut port,
            &ctx,
        )
        .expect("admit");
        api::bind_membership(
            &mut registry,
            &bind_request(name, "corpus:test"),
            &current,
            &opaque(format!("operation:bind-{name}").as_str()),
            digest(101),
            &receipt(format!("receipt:bind-{name}").as_str()),
            &mut port,
            &ctx,
        )
        .expect("bind");
    }
    (registry, port, opaque("corpus:test"))
}

#[test]
fn source_view_never_implicit() {
    let (registry, _port, corpus) = registry_with_view();
    let mut allowed = BTreeSet::new();
    for record in registry.memberships().values() {
        allowed.insert(record.membership_id());
    }
    assert_eq!(
        api::resolve_source_view(
            &registry,
            &ResolveSourceViewRequest {
                corpus_id: corpus.clone(),
                generation: NonZeroRevision::new(1).expect("generation"),
                portfolio_id: None,
                explicit_memberships: Vec::new(),
                max_items: 10,
            },
            &allowed,
            NonZeroRevision::new(1).expect("generation"),
        ),
        Err(RegistryError::SourceViewAmbiguous)
    );
    let explicit: Vec<search_contracts::SourceMembershipId> = allowed.iter().copied().collect();
    let view = api::resolve_source_view(
        &registry,
        &ResolveSourceViewRequest {
            corpus_id: corpus,
            generation: NonZeroRevision::new(1).expect("generation"),
            portfolio_id: None,
            explicit_memberships: explicit,
            max_items: 10,
        },
        &allowed,
        NonZeroRevision::new(1).expect("generation"),
    )
    .expect("explicit view");
    assert_eq!(view.allowed_memberships.len(), 2);
    assert!(view.missing.is_empty());
    let verified = api::verify_view(&view, &registry, &BTreeMap::new()).expect("verify");
    assert_eq!(verified.view, view);
}

#[test]
fn source_view_stale_generation_fails_closed() {
    let (registry, _port, corpus) = registry_with_view();
    let mut allowed = BTreeSet::new();
    for record in registry.memberships().values() {
        allowed.insert(record.membership_id());
    }
    let explicit: Vec<search_contracts::SourceMembershipId> = allowed.iter().copied().collect();
    assert_eq!(
        api::resolve_source_view(
            &registry,
            &ResolveSourceViewRequest {
                corpus_id: corpus,
                generation: NonZeroRevision::new(2).expect("generation"),
                portfolio_id: None,
                explicit_memberships: explicit,
                max_items: 10,
            },
            &allowed,
            NonZeroRevision::new(2).expect("generation"),
        ),
        Err(RegistryError::SourceViewStale)
    );
}

#[test]
fn foreign_membership_is_never_disclosed() {
    let (mut registry, mut port) = registry_with_root();
    let ctx = context(cancel());
    let current = allow_receipt(1);
    for (name, corpus) in [("one", "corpus:own"), ("two", "corpus:foreign")] {
        api::admit_source(
            &mut registry,
            &source_identity(name),
            &source_binding(name),
            &current,
            &assignment(name, &current),
            &opaque(format!("operation:admit-{name}").as_str()),
            digest(110),
            &receipt(format!("receipt:admit-{name}").as_str()),
            &mut port,
            &ctx,
        )
        .expect("admit");
        api::bind_membership(
            &mut registry,
            &bind_request(name, corpus),
            &current,
            &opaque(format!("operation:bind-{name}").as_str()),
            digest(111),
            &receipt(format!("receipt:bind-{name}").as_str()),
            &mut port,
            &ctx,
        )
        .expect("bind");
    }
    let redacted = api::redacted_view(&registry, &opaque("corpus:own")).expect("redacted");
    assert_eq!(redacted.own_memberships.len(), 1);
    assert_eq!(redacted.own_source_count, 1);
    let debug = format!("{redacted:?}");
    assert!(!debug.contains("corpus:foreign"));
}

#[test]
fn branch_or_index_change_creates_new_workspace_view_revision() {
    let (registry, _port, _corpus) = registry_with_view();
    let first = api::resolve_workspace_view(
        &registry,
        &ResolveWorkspaceViewRequest {
            workspace_id: WorkspaceId::from_bytes([1; 16]),
            root_binding_id: RootBindingId::from_bytes([9; 16]),
            branch_digest: digest(120),
            index_digest: digest(121),
            buffer_revision: 1,
        },
    )
    .expect("first");
    let second = api::resolve_workspace_view(
        &registry,
        &ResolveWorkspaceViewRequest {
            workspace_id: WorkspaceId::from_bytes([1; 16]),
            root_binding_id: RootBindingId::from_bytes([9; 16]),
            branch_digest: digest(122),
            index_digest: digest(121),
            buffer_revision: 1,
        },
    )
    .expect("second");
    assert_ne!(first.view_revision_id, second.view_revision_id);
}

// ---------------------------------------------------------------------------
// Cutover closed automaton
// ---------------------------------------------------------------------------

fn initial_ownership(namespace: SourceNamespaceId) -> SourceNamespaceOwnership {
    SourceNamespaceOwnership {
        source_namespace_id: namespace,
        owner_system_id: opaque("system:old"),
        owner_installation_incarnation_id: InstallationIncarnationId::from_bytes([1; 16]),
        owner_epoch: OwnerEpoch::new(1).expect("epoch"),
        ownership_record_revision: NonZeroRevision::new(1).expect("revision"),
        source_owner_generation: SourceOwnerGeneration::from_bytes([10; 32]),
        source_admission_policy_revision: search_contracts::PolicyRevision::new(1),
        status: NamespaceOwnershipStatus::Active,
        cutover_receipt_ref: None,
    }
}

fn registry_with_namespace() -> (
    SourceRegistry,
    InMemoryRegistryJournal<FakeCancellation>,
    SourceNamespaceId,
) {
    let (mut registry, port) = registry_with_root();
    let namespace = SourceNamespaceId::from_bytes([7; 16]);
    registry.namespaces_mut().insert(
        namespace,
        crate::cutover::NamespaceCutoverState::new(initial_ownership(namespace)),
    );
    (registry, port, namespace)
}

fn timestamp(value: &str) -> UtcTimestamp {
    UtcTimestamp::parse(value).expect("timestamp")
}

fn cutover_receipt_for(
    namespace: SourceNamespaceId,
    old_owner: OpaqueId,
    new_owner: OpaqueId,
    old_generation: SourceOwnerGeneration,
    new_generation: SourceOwnerGeneration,
    activation_revision: NonZeroRevision,
) -> SourceOwnerCutoverReceipt {
    SourceOwnerCutoverReceipt {
        protocol: SourceOwnerCutoverProtocolV1,
        cutover: SourceOwnerCutover {
            cutover_id: CutoverId::from_bytes([3; 16]),
            source_namespace_id: namespace,
            identity_mapping_digest: digest(130),
            prepared_at: timestamp("2026-09-02T10:00:00.000000Z"),
            effective_at: timestamp("2026-09-02T10:00:01.000000Z"),
        },
        old_owner: OldSourceOwnerFence {
            owner_system_id: old_owner,
            source_owner_generation_before_fence: old_generation,
            fence_revision: NonZeroRevision::new(3).expect("revision"),
            final_source_view_ref: SourceViewRef {
                source_view_digest: digest(131),
                workspace_view_revision_ref: None,
            },
            final_revision_set_digest: digest(132),
            terminal_status: NamespaceOwnershipStatus::Fenced,
        },
        new_owner: NewSourceOwnerActivation {
            owner_system_id: new_owner,
            source_owner_generation_after_activation: new_generation,
            activation_revision,
            admitted_revision_set_digest: digest(133),
            status: NamespaceOwnershipStatus::Active,
        },
        validation: CutoverValidation {
            compatibility_receipt_refs: search_contracts::BoundedList::new(vec![receipt(
                "receipt:compat",
            )])
            .expect("compat"),
            integrity_receipt_refs: search_contracts::BoundedList::empty(),
            unresolved_sources_and_reasons: search_contracts::BoundedList::empty(),
        },
        authorization: CutoverAuthorization {
            old_owner_authorization_ref: OpaqueRef::new("auth:old").expect("auth"),
            new_owner_authorization_ref: OpaqueRef::new("auth:new").expect("auth"),
            issued_at: timestamp("2026-09-02T09:59:59.000000Z"),
        },
    }
}

#[test]
fn dual_active_owner_rejected() {
    let (mut registry, mut port, namespace) = registry_with_namespace();
    let ctx = context(cancel());
    let preparation = api::prepare_namespace_cutover(
        &mut registry,
        namespace,
        opaque("system:new"),
        InstallationIncarnationId::from_bytes([2; 16]),
        vec![source_identity("one")],
        Vec::new(),
        digest(140),
        CutoverId::from_bytes([3; 16]),
        receipt("receipt:cutover-ref"),
        &opaque("operation:prepare"),
        digest(141),
        &receipt("receipt:prepare"),
        &mut port,
        &ctx,
    )
    .expect("prepare");
    assert_eq!(
        api::prepare_namespace_cutover(
            &mut registry,
            namespace,
            opaque("system:other"),
            InstallationIncarnationId::from_bytes([3; 16]),
            vec![source_identity("one")],
            Vec::new(),
            digest(140),
            CutoverId::from_bytes([4; 16]),
            receipt("receipt:cutover-other"),
            &opaque("operation:prepare-two"),
            digest(142),
            &receipt("receipt:prepare-two"),
            &mut port,
            &ctx,
        ),
        Err(RegistryError::NamespaceOwnershipConflict)
    );
    let _ = preparation;
}

#[test]
fn cutover_state_machine_and_fence_before_activation() {
    let (mut registry, mut port, namespace) = registry_with_namespace();
    let ctx = context(cancel());
    let old_generation = SourceOwnerGeneration::from_bytes([10; 32]);
    let preparation = api::prepare_namespace_cutover(
        &mut registry,
        namespace,
        opaque("system:new"),
        InstallationIncarnationId::from_bytes([2; 16]),
        vec![source_identity("one")],
        Vec::new(),
        digest(150),
        CutoverId::from_bytes([3; 16]),
        receipt("receipt:cutover-ref"),
        &opaque("operation:prepare"),
        digest(151),
        &receipt("receipt:prepare"),
        &mut port,
        &ctx,
    )
    .expect("prepare");
    let recovered = api::recover_cutover(
        &registry,
        namespace,
        &opaque("operation:prepare"),
        &port,
        &ctx,
    )
    .expect("recover prepared");
    assert_eq!(recovered, crate::cutover::CutoverRecoveryDecision::Prepared);
    let fence = api::fence_old_owner(
        &mut registry,
        &preparation,
        &opaque("operation:fence"),
        digest(152),
        &receipt("receipt:fence"),
        &mut port,
        &ctx,
    )
    .expect("fence");
    assert!(!fence.replayed);
    let recovered = api::recover_cutover(
        &registry,
        namespace,
        &opaque("operation:fence"),
        &port,
        &ctx,
    )
    .expect("recover fenced");
    assert_eq!(
        recovered,
        crate::cutover::CutoverRecoveryDecision::OldOwnerFenced
    );
    let new_generation = SourceOwnerGeneration::from_bytes([20; 32]);
    let activation_revision = NonZeroRevision::new(4).expect("revision");
    let wire = cutover_receipt_for(
        namespace,
        opaque("system:old"),
        opaque("system:new"),
        old_generation,
        new_generation,
        activation_revision,
    );
    let old_state = SourceNamespaceOwnership {
        source_namespace_id: namespace,
        owner_system_id: opaque("system:old"),
        owner_installation_incarnation_id: InstallationIncarnationId::from_bytes([1; 16]),
        owner_epoch: OwnerEpoch::new(1).expect("epoch"),
        ownership_record_revision: NonZeroRevision::new(3).expect("revision"),
        source_owner_generation: fence.fenced_generation,
        source_admission_policy_revision: search_contracts::PolicyRevision::new(1),
        status: NamespaceOwnershipStatus::Fenced,
        cutover_receipt_ref: Some(receipt("receipt:cutover-ref")),
    };
    let new_state = SourceNamespaceOwnership {
        source_namespace_id: namespace,
        owner_system_id: opaque("system:new"),
        owner_installation_incarnation_id: InstallationIncarnationId::from_bytes([2; 16]),
        owner_epoch: OwnerEpoch::new(2).expect("epoch"),
        ownership_record_revision: activation_revision,
        source_owner_generation: new_generation,
        source_admission_policy_revision: search_contracts::PolicyRevision::new(1),
        status: NamespaceOwnershipStatus::Active,
        cutover_receipt_ref: Some(receipt("receipt:cutover-ref")),
    };
    let verified = api::verify_cutover_receipt(&registry, &wire, &old_state, &new_state, 1, 0)
        .expect("verify");
    assert_eq!(verified.new_generation, new_generation);
    let activation = api::activate_new_owner(
        &mut registry,
        &preparation,
        &fence,
        &wire,
        OwnerEpoch::new(2).expect("epoch"),
        &opaque("operation:activate"),
        digest(153),
        &receipt("receipt:activate"),
        &mut port,
        &ctx,
    )
    .expect("activate");
    assert_eq!(activation.new_generation, new_generation);
    assert_ne!(activation.new_generation, old_generation);
}

#[test]
fn ordinary_export_copy_cannot_satisfy_cutover_receipt() {
    let (registry, _port, namespace) = registry_with_namespace();
    let old_state = initial_ownership(namespace);
    let mut new_state = old_state.clone();
    new_state.owner_system_id = opaque("system:new");
    new_state.status = NamespaceOwnershipStatus::Active;
    let mut wire = cutover_receipt_for(
        namespace,
        opaque("system:old"),
        opaque("system:new"),
        SourceOwnerGeneration::from_bytes([10; 32]),
        SourceOwnerGeneration::from_bytes([20; 32]),
        NonZeroRevision::new(2).expect("revision"),
    );
    wire.old_owner.terminal_status = NamespaceOwnershipStatus::Active;
    assert_eq!(
        api::verify_cutover_receipt(&registry, &wire, &old_state, &new_state, 1, 0),
        Err(RegistryError::CutoverReceiptMismatch)
    );
}

#[test]
fn activate_without_fence_is_cutover_required() {
    let (mut registry, mut port, namespace) = registry_with_namespace();
    let ctx = context(cancel());
    let preparation = api::prepare_namespace_cutover(
        &mut registry,
        namespace,
        opaque("system:new"),
        InstallationIncarnationId::from_bytes([2; 16]),
        vec![source_identity("one")],
        Vec::new(),
        digest(160),
        CutoverId::from_bytes([3; 16]),
        receipt("receipt:cutover-ref"),
        &opaque("operation:prepare"),
        digest(161),
        &receipt("receipt:prepare"),
        &mut port,
        &ctx,
    )
    .expect("prepare");
    let fence = crate::cutover::OwnerFenceReceipt {
        cutover_id: preparation.cutover_id,
        namespace_id: namespace,
        fenced_generation: preparation.current_generation,
        fence_revision: NonZeroRevision::new(3).expect("revision"),
        registry_revision: 99,
        operation_id: opaque("operation:fake-fence"),
        receipt: receipt("receipt:fake-fence"),
        replayed: false,
    };
    let wire = cutover_receipt_for(
        namespace,
        opaque("system:old"),
        opaque("system:new"),
        preparation.current_generation,
        SourceOwnerGeneration::from_bytes([20; 32]),
        NonZeroRevision::new(4).expect("revision"),
    );
    assert_eq!(
        api::activate_new_owner(
            &mut registry,
            &preparation,
            &fence,
            &wire,
            OwnerEpoch::new(2).expect("epoch"),
            &opaque("operation:activate"),
            digest(162),
            &receipt("receipt:activate"),
            &mut port,
            &ctx,
        ),
        Err(RegistryError::CutoverRequired)
    );
}

// ---------------------------------------------------------------------------
// Recovery, batches, digests, port persistence
// ---------------------------------------------------------------------------

#[test]
fn crash_unknown_outcome_at_every_transaction() {
    let mut registry = SourceRegistry::new(DEFAULT_REGISTRY_LIMITS).expect("registry");
    let port = journal();
    let ctx = context(cancel());
    let unknown = api::recover_registry_mutation(
        &registry,
        &opaque("operation:missing"),
        digest(170),
        &port,
        &ctx,
    )
    .expect("recover missing");
    assert_eq!(
        unknown,
        crate::recovery::RegistryMutationRecovery::RetrySameOperation
    );
    registry
        .apply(RegistryBatch {
            expected_registry_revision: 0,
            operation: operation("register", 1),
            changes: vec![register_change("one")],
        })
        .expect("apply");
    let recovered = api::recover_registry_mutation(
        &registry,
        &opaque("registry-operation:register"),
        digest(1),
        &port,
        &ctx,
    )
    .expect("recover committed");
    match recovered {
        crate::recovery::RegistryMutationRecovery::Recovered(receipt) => {
            assert_eq!(receipt.after_revision, 1);
        }
        other => panic!("expected recovered, got {other:?}"),
    }
    assert_eq!(
        api::recover_registry_mutation(
            &registry,
            &opaque("registry-operation:register"),
            digest(99),
            &port,
            &ctx,
        )
        .expect("conflict"),
        crate::recovery::RegistryMutationRecovery::Conflict
    );
}

#[test]
fn batch_accounts_one_outcome_per_input_and_cancellation_recovery() {
    let (mut registry, mut port) = registry_with_root();
    let ctx = context(cancel());
    let current = allow_receipt(1);
    for name in ["one", "two"] {
        api::admit_source(
            &mut registry,
            &source_identity(name),
            &source_binding(name),
            &current,
            &assignment(name, &current),
            &opaque(format!("operation:admit-{name}").as_str()),
            digest(180),
            &receipt(format!("receipt:admit-{name}").as_str()),
            &mut port,
            &ctx,
        )
        .expect("admit");
    }
    let snapshot = api::registry_snapshot(&registry);
    let validated = api::validate_live_snapshot(&registry).expect("snapshot should validate");
    assert_eq!(snapshot.revision, validated.revision());
    let batch = RegistryBatch {
        expected_registry_revision: registry.revision(),
        operation: operation("batch", 181),
        changes: vec![register_change("three"), register_change("four")],
    };
    let receipt_out = api::apply_admission_batch(
        &mut registry,
        batch,
        &receipt("receipt:batch"),
        &mut port,
        &ctx,
    )
    .expect("batch");
    assert_eq!(receipt_out.items.len(), 2);
    assert!(
        receipt_out
            .items
            .iter()
            .all(|item| item.status == crate::recovery::BatchItemStatus::Committed)
    );
    assert!(!receipt_out.replayed);
    let cancelled_ctx = context(cancelled());
    let expected_cancelled = registry.revision();
    assert_eq!(
        api::apply_admission_batch(
            &mut registry,
            RegistryBatch {
                expected_registry_revision: expected_cancelled,
                operation: operation("batch-cancelled", 182),
                changes: vec![register_change("five")],
            },
            &receipt("receipt:cancelled"),
            &mut port,
            &cancelled_ctx,
        ),
        Err(RegistryError::CancelledBeforeCommit)
    );
}

#[test]
fn receipt_persistence_flows_through_vendor_neutral_port() {
    let (mut registry, mut port) = registry_with_root();
    let ctx = context(cancel());
    let current = allow_receipt(1);
    let before = port.len();
    api::admit_source(
        &mut registry,
        &source_identity("one"),
        &source_binding("one"),
        &current,
        &assignment("one", &current),
        &opaque("operation:admit-port"),
        digest(190),
        &receipt("receipt:admit-port"),
        &mut port,
        &ctx,
    )
    .expect("admit");
    assert_eq!(port.len(), before + 1);
    let loaded = port
        .load_entry(&opaque("operation:admit-port"), &ctx)
        .expect("load");
    assert!(loaded.is_some());
    assert_eq!(loaded.expect("entry").mutation_digest, digest(190));
}

#[test]
fn snapshot_digest_is_deterministic_and_revision_sensitive() {
    let (registry, _port, _corpus) = registry_with_view();
    let snapshot = api::registry_snapshot(&registry);
    let validated = api::validate_live_snapshot(&registry).expect("validated");
    assert_eq!(
        api::snapshot_digest(&snapshot),
        api::snapshot_digest(validated.snapshot())
    );
    let mut changed = snapshot.clone();
    changed.revision += 1;
    assert_ne!(
        api::snapshot_digest(&snapshot),
        api::snapshot_digest(&changed)
    );
}

#[test]
fn portfolio_admitted_only() {
    let (mut registry, mut port) = registry_with_root();
    let ctx = context(cancel());
    let current = allow_receipt(1);
    api::admit_source(
        &mut registry,
        &source_identity("one"),
        &source_binding("one"),
        &current,
        &assignment("one", &current),
        &opaque("operation:admit"),
        digest(200),
        &receipt("receipt:admit"),
        &mut port,
        &ctx,
    )
    .expect("admit");
    let bound = api::bind_membership(
        &mut registry,
        &bind_request("one", "corpus:test"),
        &current,
        &opaque("operation:bind"),
        digest(201),
        &receipt("receipt:bind"),
        &mut port,
        &ctx,
    )
    .expect("bind");
    api::transition_membership(
        &mut registry,
        &bind_request("one", "corpus:test").key,
        MembershipCommand::Retire,
        bound.membership_revision,
        &opaque("operation:retire-membership"),
        digest(202),
        &receipt("receipt:retire-membership"),
        &mut port,
        &ctx,
    )
    .expect("retire membership");
    assert_eq!(
        api::publish_reference_portfolio(
            &mut registry,
            &PublishPortfolioRequest {
                portfolio_id: ReferencePortfolioId::from_bytes([5; 16]),
                portfolio_revision: search_contracts::PortfolioRevision::new(1),
                membership_precedence: vec![bound.membership_id],
                allow_empty: false,
            },
            &opaque("operation:portfolio"),
            digest(203),
            &receipt("receipt:portfolio"),
            &mut port,
            &ctx,
        ),
        Err(RegistryError::ReferencePortfolioInvalid)
    );
}

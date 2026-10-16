use crate::error::{DEFAULT_REGISTRY_LIMITS, RegistryError};
use crate::namespace::NamespaceCutoverState;
use crate::portfolio_history::PortfolioHistory;
use crate::root::{RootRecord, RootStatus};
use crate::snapshot::{
    RegistrySnapshot, snapshot_digest, snapshot_digest_v5, validate_registry_snapshot,
};
use crate::source::{RegisteredSource, SourceLifecycle};
use search_contracts::{
    Blake3Digest32, InstallationIncarnationId, NamespaceOwnershipStatus, NonZeroRevision,
    OwnerEpoch, PolicyRevision, ReceiptRef, RootBindingId, SourceNamespaceId,
    SourceNamespaceOwnership, SourceOwnerGeneration,
};
use search_source_identity::{
    CaseBehavior, FilesystemIdentityProfile, LinkBehavior, ReparseBehavior, StableFieldPolicy,
    UnicodeBehavior,
};
use search_source_provenance::OriginalInputsLayoutV2;
use std::collections::BTreeMap;
use std::sync::Arc;

fn revision(value: u64) -> NonZeroRevision {
    NonZeroRevision::new(value).expect("revision")
}

fn receipt(value: &str) -> ReceiptRef {
    ReceiptRef::new(value).expect("receipt")
}

fn profile(case_behavior: CaseBehavior) -> FilesystemIdentityProfile {
    FilesystemIdentityProfile::new(
        revision(1),
        Blake3Digest32::from_bytes([6; 32]),
        case_behavior,
        UnicodeBehavior::PreserveScalarValues,
        StableFieldPolicy::Required,
        StableFieldPolicy::Required,
        LinkBehavior::StablePhysicalIdentity,
        ReparseBehavior::FinalTargetIdentity,
    )
    .expect("qualified profile")
}

fn namespace(
    original: &search_source_provenance::OriginalAdmissionV2,
    profile: FilesystemIdentityProfile,
) -> NamespaceCutoverState {
    let mut ownership = SourceNamespaceOwnership {
        source_namespace_id: SourceNamespaceId::from_bytes([1; 16]),
        owner_system_id: search_contracts::OpaqueId::new("search-provider").expect("owner"),
        owner_installation_incarnation_id: InstallationIncarnationId::from_bytes([2; 16]),
        owner_epoch: OwnerEpoch::new(1).expect("epoch"),
        ownership_record_revision: revision(1),
        source_owner_generation: SourceOwnerGeneration::from_bytes([0; 32]),
        source_admission_policy_revision: PolicyRevision::new(
            original.policy().compiled().revision().get(),
        ),
        status: NamespaceOwnershipStatus::Active,
        cutover_receipt_ref: None,
    };
    ownership.source_owner_generation =
        search_domain::source_ownership_v2::source_owner_generation_v2(&ownership)
            .expect("owner generation");
    let policy_reference =
        OriginalInputsLayoutV2::policy_reference(original.policy().as_ref(), &mut || Ok(()))
            .expect("policy reference");
    NamespaceCutoverState::new_v2_metadata(
        ownership,
        RootBindingId::from_bytes([3; 16]),
        profile,
        Arc::clone(original.policy()),
        policy_reference,
        1,
        receipt("namespace:admit"),
        &mut || Ok(()),
    )
    .expect("namespace metadata")
}

fn snapshot(source: RegisteredSource, namespace: NamespaceCutoverState) -> RegistrySnapshot {
    let namespace_id = namespace.ownership.source_namespace_id;
    let root_id = namespace
        .permanent_metadata()
        .expect("permanent namespace")
        .root_binding_id();
    let root = RootRecord {
        root_binding_id: root_id,
        canonical_root_digest: Blake3Digest32::from_bytes([7; 32]),
        policy_fingerprint: Blake3Digest32::from_bytes([8; 32]),
        policy_revision: revision(7),
        owner_epoch: OwnerEpoch::new(1).expect("epoch"),
        record_revision: revision(1),
        registry_revision: 1,
        status: RootStatus::Active,
        last_receipt: receipt("root:admit"),
    };
    let source_id = source.identity().clone();
    RegistrySnapshot {
        revision: 1,
        roots: BTreeMap::from([(root_id, root)]),
        sources: BTreeMap::from([(source_id, source)]),
        memberships: BTreeMap::new(),
        portfolios: BTreeMap::new(),
        portfolio_history: PortfolioHistory::new(),
        active_generations: BTreeMap::new(),
        ownerships: BTreeMap::from([(namespace_id, namespace.ownership.clone())]),
        namespace_states: BTreeMap::from([(namespace_id, namespace)]),
    }
}

#[test]
fn v5_digest_binds_permanent_source_and_qualified_namespace_metadata() {
    let original = super::original();
    let source = super::registered(original.clone());
    let state = namespace(&original, profile(CaseBehavior::Sensitive));
    let snapshot = snapshot(source.clone(), state);
    let baseline = snapshot_digest_v5(&snapshot).expect("v5 digest");

    let replacement = RegisteredSource::new_permanent(
        source.binding().clone(),
        super::permanent(original.clone(), 10),
        SourceLifecycle::Active,
        source.source_revision(),
        source.registry_revision(),
        source.last_receipt().clone(),
    )
    .expect("replacement source");
    let mut changed_source = snapshot.clone();
    changed_source
        .sources
        .insert(source.identity().clone(), replacement);
    assert_ne!(
        snapshot_digest_v5(&changed_source).expect("source digest"),
        baseline
    );

    let changed_namespace = namespace(&original, profile(CaseBehavior::InsensitiveAscii));
    let mut changed_profile = snapshot;
    let namespace_id = changed_namespace.ownership.source_namespace_id;
    changed_profile
        .ownerships
        .insert(namespace_id, changed_namespace.ownership.clone());
    changed_profile
        .namespace_states
        .insert(namespace_id, changed_namespace);
    assert_ne!(
        snapshot_digest_v5(&changed_profile).expect("profile digest"),
        baseline
    );
}

#[test]
fn v5_rejects_flat_namespace_tampering_and_old_v4_refuses_rich_models() {
    let original = super::original();
    let rich = snapshot(
        super::registered(original.clone()),
        namespace(&original, profile(CaseBehavior::Sensitive)),
    );
    assert_eq!(snapshot_digest(&rich), Err(RegistryError::SnapshotInvalid));

    let mut tampered = rich.clone();
    tampered
        .ownerships
        .get_mut(&SourceNamespaceId::from_bytes([1; 16]))
        .expect("ownership")
        .owner_epoch = OwnerEpoch::new(2).expect("epoch");
    assert_eq!(
        snapshot_digest_v5(&tampered),
        Err(RegistryError::SnapshotInvalid)
    );
    assert_eq!(
        validate_registry_snapshot(&tampered, DEFAULT_REGISTRY_LIMITS),
        Err(RegistryError::SnapshotInvalid)
    );
}

#[test]
fn legacy_v4_empty_digest_remains_deterministic_without_v2_projection() {
    let empty = RegistrySnapshot {
        revision: 0,
        roots: BTreeMap::new(),
        sources: BTreeMap::new(),
        memberships: BTreeMap::new(),
        portfolios: BTreeMap::new(),
        portfolio_history: PortfolioHistory::new(),
        active_generations: BTreeMap::new(),
        ownerships: BTreeMap::new(),
        namespace_states: BTreeMap::new(),
    };
    let v4 = snapshot_digest(&empty);
    let v5 = snapshot_digest_v5(&empty);
    let clone = empty.clone();
    assert_eq!(v4, snapshot_digest(&clone));
    assert_eq!(v5, snapshot_digest_v5(&clone));
    assert_eq!(
        v4.expect("v4 digest").0,
        Blake3Digest32::from_bytes([
            187, 244, 160, 14, 212, 42, 248, 24, 96, 237, 124, 225, 41, 38, 45, 170, 100, 113, 170,
            250, 9, 64, 172, 109, 125, 41, 228, 164, 230, 231, 51, 87,
        ])
    );
    assert_eq!(
        v5.expect("v5 digest").0,
        Blake3Digest32::from_bytes([
            176, 187, 96, 88, 120, 207, 147, 155, 130, 182, 161, 26, 2, 243, 28, 164, 162, 238,
            250, 202, 39, 17, 121, 39, 189, 66, 184, 2, 174, 94, 212, 39,
        ])
    );
    assert_ne!(v4.expect("v4 digest").0, v5.expect("v5 digest").0);
}

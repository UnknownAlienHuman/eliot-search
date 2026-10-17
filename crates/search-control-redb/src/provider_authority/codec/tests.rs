use search_contracts::evidence::RecipeIdV2;
use search_contracts::{
    AccessPartitionId, AuthoritativeGrantPolicy, BindingId, BoundedSet, CorpusOrPortfolioId,
    DisclosureCeiling, InstallationId, InstallationIncarnationId, MAX_SET_ITEMS, Modality,
    OpaqueId, OpaqueRef, ProfileId, RecipeIdV1, ScopeDomainId, SensitivityClass,
    SourceMembershipId, StandalonePolicyRecord, StandalonePolicyState, UtcTimestamp,
};

use crate::{ControlError, ControlRecordClass, ControlValue, JournalLimits};

use super::{EVIDENCE_RECIPE_PERMISSION_EXTENSION, decode_policy, encode_policy};

fn policy(evidence_recipes: BoundedSet<RecipeIdV2, MAX_SET_ITEMS>) -> StandalonePolicyRecord {
    StandalonePolicyRecord {
        policy: AuthoritativeGrantPolicy {
            binding_id: BindingId::from_bytes([1; 16]),
            binding_generation: 1,
            policy_generation: 1,
            installation_id: InstallationId::from_bytes([2; 16]),
            installation_incarnation_id: InstallationIncarnationId::from_bytes([3; 16]),
            principal_opaque_id: OpaqueId::new("principal").expect("principal"),
            client_scope_ref: OpaqueRef::new("scope").expect("scope"),
            scope_domain_id: ScopeDomainId::from_bytes([4; 16]),
            allowed_membership_ids: BoundedSet::from_items([SourceMembershipId::from_bytes(
                [5; 16],
            )])
            .expect("membership"),
            allowed_corpus_or_portfolio_ids:
                BoundedSet::<CorpusOrPortfolioId, MAX_SET_ITEMS>::empty(),
            reference_portfolio_revision: None,
            allowed_access_partitions: BoundedSet::from_items([AccessPartitionId::from_bytes(
                [6; 16],
            )])
            .expect("partition"),
            allowed_modalities: BoundedSet::from_items([Modality::Text]).expect("modality"),
            permitted_recipe_families: BoundedSet::from_items([RecipeIdV1::Locate])
                .expect("v1 recipe"),
            permitted_evidence_recipes: evidence_recipes,
            allowed_budget_classes: BoundedSet::from_items([
                ProfileId::new("interactive").expect("budget")
            ])
            .expect("budget set"),
            sensitivity_ceiling: SensitivityClass::Project,
            disclosure_ceiling: DisclosureCeiling::LocalOnly,
            source_read_permission: true,
            exact_scan_permission: false,
            issued_boot_id: OpaqueId::new("boot").expect("boot"),
            revocation_generation: 1,
            maximum_ttl_ms: 10_000,
        },
        state: StandalonePolicyState::Active,
        issued_at: UtcTimestamp::parse("2026-10-16T00:00:00.000000Z").expect("issued"),
        expires_at: None,
    }
}

#[test]
fn legacy_policy_bytes_remain_the_prefix_and_decode_without_v2_permission() {
    let legacy = policy(BoundedSet::empty());
    let bytes = encode_policy(&legacy).expect("legacy encoding");
    assert_eq!(decode_policy(&bytes).expect("legacy decode"), legacy);
    assert!(
        !bytes
            .as_bytes()
            .windows(EVIDENCE_RECIPE_PERMISSION_EXTENSION.len())
            .any(|window| window == EVIDENCE_RECIPE_PERMISSION_EXTENSION)
    );
}

#[test]
fn versioned_evidence_permission_extension_round_trips_and_fails_closed() {
    let v1 = encode_policy(&policy(BoundedSet::empty())).expect("v1 policy");
    let v2_record = policy(
        BoundedSet::from_items([RecipeIdV2::RetrieveEvidence, RecipeIdV2::OrientScope])
            .expect("v2 permissions"),
    );
    let v2 = encode_policy(&v2_record).expect("v2 policy");
    assert!(v2.as_bytes().starts_with(v1.as_bytes()));
    assert_eq!(decode_policy(&v2).expect("v2 decode"), v2_record);

    let mut corrupt = v2.as_bytes().to_vec();
    let marker = v1.as_bytes().len();
    corrupt[marker] ^= 1;
    let corrupt = ControlValue::new(ControlRecordClass::State, corrupt, JournalLimits::BASELINE)
        .expect("bounded corrupt fixture");
    assert_eq!(decode_policy(&corrupt), Err(ControlError::StoreCorrupt));
}

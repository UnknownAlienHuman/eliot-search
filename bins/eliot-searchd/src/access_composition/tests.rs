use super::*;
use search_access::{
    AccessModality, AuthoritativeAccessSnapshot, AuthoritativePolicyState,
    GrantClaims, GrantValidationContext, IndexedRouteFence,
    MembershipAccessBinding, NamespacePolicyFence, RequestedMembershipScope,
};
use search_contracts::{
    AccessPolicyRevision, BindingId, Blake3Digest32, CollectionGenerationId,
    Epoch, GrantId, InstallationId, InstallationIncarnationId, OpaqueId,
    OwnerEpoch, PurgeFenceRevision, RecipeIdV1, ShadowFenceRevision,
    SourceMembershipId, SourceNamespaceId, SourceOwnerGeneration,
};
use std::collections::{BTreeMap, BTreeSet};

struct Bundle {
    claims: GrantClaims,
    context: GrantValidationContext,
    requested: RequestedMembershipScope,
    grant_scope: BTreeSet<SourceMembershipId>,
    authoritative: AuthoritativeAccessSnapshot,
    policy_fence: NamespacePolicyFence,
    authoritative_policy: AuthoritativePolicyState,
    route: IndexedRouteFence,
    live: LiveSecurityState,
}

fn oid(value: &str) -> OpaqueId {
    OpaqueId::new(value).expect("fixture id")
}

fn member(n: u128) -> SourceMembershipId {
    SourceMembershipId::from_bytes(n.to_be_bytes())
}

fn bundle() -> Bundle {
    let installation_id = InstallationId::from_bytes([1; 16]);
    let incarnation = InstallationIncarnationId::from_bytes([2; 16]);
    let binding_id = BindingId::from_bytes([3; 16]);
    let boot = oid("boot-gate-0001");
    let members = [member(1), member(2)];
    let bindings = members
        .into_iter()
        .map(|membership| {
            (
                membership,
                MembershipAccessBinding {
                    membership_id: membership,
                    access_partition_digest: Blake3Digest32::from_bytes([0xA1; 32]),
                    scoring_partition_digest: Blake3Digest32::from_bytes([0xB2; 32]),
                    projection_membership_id: oid("projection-gate"),
                    active: true,
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    let namespace_id = SourceNamespaceId::from_bytes([0x20; 16]);
    let owner_generation = SourceOwnerGeneration::from_bytes([0x21; 32]);
    Bundle {
        claims: GrantClaims {
            grant_id: GrantId::from_bytes([9; 16]),
            binding_id,
            installation_id,
            installation_incarnation_id: incarnation,
            issued_boot_id: boot.clone(),
            issued_at_ms: 1_000,
            expires_at_ms: 2_000,
            nonce: oid("nonce-gate-0001"),
            revocation_generation: 4,
            allowed_recipes: BTreeSet::from([RecipeIdV1::FindText]),
            allowed_modalities: BTreeSet::from([AccessModality::Direct]),
            allowed_budget_classes: BTreeSet::from([oid("budget-gate")]),
            max_source_read_bytes: 1_024,
            max_result_bytes: 1_024,
        },
        context: GrantValidationContext {
            binding_id,
            installation_id,
            installation_incarnation_id: incarnation,
            boot_id: boot,
            now_ms: 1_500,
            current_revocation_generation: 4,
            signature_verified: true,
            pairing_verified: true,
            nonce_accepted: true,
        },
        requested: RequestedMembershipScope {
            memberships: BTreeSet::from(members),
        },
        grant_scope: BTreeSet::from(members),
        authoritative: AuthoritativeAccessSnapshot {
            generation: 7,
            source_catalog_generation: 3,
            membership_generation: 5,
            bindings,
            snapshot_digest: Blake3Digest32::from_bytes([0xCC; 32]),
        },
        policy_fence: NamespacePolicyFence {
            namespace_id,
            owner_generation,
            policy_revision: AccessPolicyRevision::new(11),
        },
        authoritative_policy: AuthoritativePolicyState {
            namespace_id,
            owner_generation,
            policy_revision: AccessPolicyRevision::new(11),
            shadow_revision: ShadowFenceRevision::new(6),
            purge_revision: PurgeFenceRevision::new(6),
        },
        route: IndexedRouteFence {
            collection_generation_id: CollectionGenerationId::from_bytes([0x10; 16]),
            visible_epoch: Epoch::new(12).expect("epoch"),
            route_generation: 2,
            owner_epoch: OwnerEpoch::new(1).expect("owner"),
        },
        live: LiveSecurityState {
            generation: 7,
            denied_memberships: BTreeSet::new(),
            purged_memberships: BTreeSet::new(),
            fail_closed: false,
            snapshot_digest: Blake3Digest32::from_bytes([0xDD; 32]),
        },
    }
}

fn admit(bundle: &Bundle) -> Result<PreRetrievalAdmission, AccessGateDenial> {
    admit_pre_retrieval(PreRetrievalRequest {
        claims: bundle.claims.clone(),
        validation_context: &bundle.context,
        recipe: RecipeIdV1::FindText,
        modality: AccessModality::Direct,
        requested_scope: &bundle.requested,
        grant_scope: &bundle.grant_scope,
        authoritative: &bundle.authoritative,
        policy_fence: &bundle.policy_fence,
        authoritative_policy: &bundle.authoritative_policy,
        route: bundle.route,
        live: &bundle.live,
        overlap_proof: None,
        max_legs: 8,
    })
}

#[test]
fn clean_request_admits_with_predicate_digests() {
    let bundle = bundle();
    let admission = admit(&bundle).expect("clean request admits");
    assert_eq!(admission.leg_count, 2);
    assert_eq!(admission.live_generation, 7);
    assert_eq!(admission.predicate_digests.len(), 2);
}

#[test]
fn revoked_grant_denies_before_provider_dispatch() {
    let mut bundle = bundle();
    bundle.context.current_revocation_generation = 99;
    let denial = admit(&bundle).expect_err("revoked grant denies");
    assert_eq!(denial.reason, ACCESS_PRE_RETRIEVAL_DENIED);
    assert_eq!(denial.access_code, AccessError::GrantRevoked.code());
}

#[test]
fn live_purge_denies_admission() {
    let mut bundle = bundle();
    bundle.live.purged_memberships.insert(member(1));
    let denial = admit(&bundle).expect_err("purged scope denies");
    assert_eq!(denial.access_code, AccessError::LivePurge.code());
}

#[test]
fn emission_recheck_fires_on_new_revocation() {
    let live = bundle().live;
    let memberships = BTreeSet::from([member(1), member(2)]);
    recheck_before_emission(&memberships, 7, &live).expect("clean live passes");
    let mut revoked = live;
    revoked.generation = 8;
    revoked.denied_memberships.insert(member(2));
    let denial = recheck_before_emission(&memberships, 7, &revoked)
        .expect_err("new revocation fires at emission");
    assert_eq!(denial.reason, ACCESS_LIVE_BARRIER_DENIED);
    assert_eq!(denial.access_code, AccessError::LiveRevocation.code());
}

#[test]
fn expansion_reauthorizes_live_state() {
    let live = bundle().live;
    let memberships = BTreeSet::from([member(1)]);
    recheck_before_expansion(&memberships, 7, &live, false).expect("clean handle pass");
    recheck_before_expansion(&memberships, 7, &live, true)
        .expect("clean continuation pass");
    let mut purged = live;
    purged.generation = 8;
    purged.purged_memberships.insert(member(1));
    let denial = recheck_before_expansion(&memberships, 7, &purged, false)
        .expect_err("purge fires at expansion");
    assert_eq!(denial.access_code, AccessError::LivePurge.code());
}

#[test]
fn contaminated_legs_discarded_whole() {
    let bundle = bundle();
    let admission = admit(&bundle).expect("baseline admits");
    assert_eq!(admission.leg_count, 2);
    let plan = compile_pre_retrieval(PreRetrievalRequest {
        claims: bundle.claims.clone(),
        validation_context: &bundle.context,
        recipe: RecipeIdV1::FindText,
        modality: AccessModality::Direct,
        requested_scope: &bundle.requested,
        grant_scope: &bundle.grant_scope,
        authoritative: &bundle.authoritative,
        policy_fence: &bundle.policy_fence,
        authoritative_policy: &bundle.authoritative_policy,
        route: bundle.route,
        live: &bundle.live,
        overlap_proof: None,
        max_legs: 8,
    })
    .expect("baseline plan compiles");
    let execution = plan
        .legs
        .iter()
        .map(|leg| search_access::LegSecurityPopulation {
            leg_id: leg.leg_id,
            memberships: leg.memberships.clone(),
            security_generation: bundle.live.generation,
            idf_population_digest: None,
        })
        .collect::<Vec<_>>();
    let mut moved = bundle.live.clone();
    moved.generation = 8;
    moved.denied_memberships.insert(member(2));
    assert_eq!(
        discard_contaminated_legs(&execution, &bundle.live, &moved),
        BTreeSet::from([1])
    );
}

#[test]
fn local_token_never_admits() {
    let denial = deny_local_token_as_authority();
    assert_eq!(denial.reason, ACCESS_LOCAL_TOKEN_NOT_AUTHORITY);
    assert_eq!(
        denial.access_code,
        AccessError::LocalIdentityNotAuthority.code()
    );
}

#[test]
fn qdrant_parity_deferred() {
    assert_eq!(QDRANT_LIVE_PARITY_DEFERRED_TO, "T24");
    assert_eq!(
        QDRANT_LIVE_PARITY_DEFERRED_TO,
        search_access::QDRANT_LIVE_PARITY_DEFERRED_TO
    );
}

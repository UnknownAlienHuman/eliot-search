//! Synthetic public-API regressions for membership identity integrity.
//! These fixtures do not verify signatures, execute a provider or qualify T20/T24.

use std::collections::{BTreeMap, BTreeSet};

use search_access::{
    AccessError, AccessModality, AuthoritativeAccessSnapshot, AuthoritativePolicyState,
    AuthorizedScope, GrantClaims, GrantValidationContext, IndexedRouteFence, LiveSecurityState,
    MembershipAccessBinding, NamespacePolicyFence, OverlapFreeRouteProof, PreRetrievalRequest,
    RequestedMembershipScope, SafeRetrievalLeg, compile_pre_retrieval, compile_safe_legs,
    intersect_scope,
};
use search_contracts::{
    AccessPolicyRevision, BindingId, Blake3Digest32, CollectionGenerationId, Epoch, GrantId,
    InstallationId, InstallationIncarnationId, OpaqueId, OwnerEpoch, PurgeFenceRevision,
    RecipeIdV1, ShadowFenceRevision, SourceMembershipId, SourceNamespaceId, SourceOwnerGeneration,
};

const fn member(seed: u8) -> SourceMembershipId {
    SourceMembershipId::from_bytes([seed; 16])
}

const fn digest(seed: u8) -> Blake3Digest32 {
    Blake3Digest32::from_bytes([seed; 32])
}

fn snapshot() -> AuthoritativeAccessSnapshot {
    AuthoritativeAccessSnapshot {
        generation: 7,
        source_catalog_generation: 3,
        membership_generation: 5,
        bindings: (1..=3).map(|seed| (member(seed), MembershipAccessBinding {
            membership_id: member(seed),
            access_partition_digest: digest(1),
            scoring_partition_digest: digest(2),
            projection_membership_id: OpaqueId::new(format!("synthetic:projection:{seed}")).unwrap(),
            active: true,
        })).collect(),
        snapshot_digest: digest(3),
    }
}

fn scope_from(snapshot: &AuthoritativeAccessSnapshot, ids: &BTreeSet<SourceMembershipId>) -> AuthorizedScope {
    AuthorizedScope {
        memberships: ids.iter().map(|id| (*id, snapshot.bindings[id].clone())).collect(),
        access_snapshot_generation: snapshot.generation,
        source_catalog_generation: snapshot.source_catalog_generation,
        membership_generation: snapshot.membership_generation,
        snapshot_digest: snapshot.snapshot_digest,
    }
}

fn route() -> IndexedRouteFence {
    IndexedRouteFence {
        collection_generation_id: CollectionGenerationId::from_bytes([4; 16]),
        visible_epoch: Epoch::new(12).unwrap(),
        route_generation: 2,
        owner_epoch: OwnerEpoch::new(1).unwrap(),
    }
}

fn proof(scope: &AuthorizedScope) -> OverlapFreeRouteProof {
    OverlapFreeRouteProof {
        route: route(),
        memberships: scope.memberships.keys().copied().collect(),
        access_snapshot_generation: scope.access_snapshot_generation,
        profile_digest: digest(4),
        proof_digest: digest(5),
    }
}

fn legs(scope: &AuthorizedScope, proof: Option<&OverlapFreeRouteProof>) -> Result<Vec<SafeRetrievalLeg>, AccessError> {
    compile_safe_legs(scope, route(), 7, 6, 6, proof, 3)
}

#[test]
fn selected_keys_cannot_redirect_to_other_memberships_with_or_without_grouping() {
    // Every 3-key -> 3-ID mapping, every nonempty requested subset and both
    // grouping paths: 27 * 7 * 2 = 378 cases at each public boundary.
    for mapping in 0_u8..27 {
        let mut registry = snapshot();
        for (key, value) in [(1, mapping % 3), (2, mapping / 3 % 3), (3, mapping / 9)] {
            registry.bindings.get_mut(&member(key)).unwrap().membership_id = member(value + 1);
        }
        let original = registry.clone();
        for mask in 1_u8..8 {
            let ids = (1_u8..=3).filter(|seed| mask & (1 << (seed - 1)) != 0)
                .map(member).collect::<BTreeSet<_>>();
            let requested = RequestedMembershipScope { memberships: ids.clone() };
            let coherent = ids.iter().all(|id| registry.bindings[id].membership_id == *id);
            for grouped in [false, true] {
                // Public fields permit direct construction, bypassing intersection.
                let supplied = scope_from(&registry, &ids);
                let before = supplied.clone();
                let overlap = proof(&supplied);
                let direct = legs(&supplied, grouped.then_some(&overlap));
                let intersected = intersect_scope(&requested, &ids, &registry);
                if coherent {
                    assert_eq!(intersected.unwrap(), supplied);
                    let plans = direct.unwrap();
                    assert_eq!(plans.len(), if grouped { 1 } else { ids.len() });
                    let emitted_ids = plans.iter().flat_map(|leg| leg.memberships.iter().copied())
                        .collect::<BTreeSet<_>>();
                    assert_eq!(emitted_ids, ids);
                    for leg in &plans {
                        let predicate_ids = leg.eligibility_plans.iter().map(|plan| plan.membership_id)
                            .collect::<BTreeSet<_>>();
                        assert_eq!(predicate_ids, leg.memberships);
                        for plan in &leg.eligibility_plans {
                            assert!(plan.predicates().is_consistent());
                            let binding = &registry.bindings[&plan.membership_id];
                            assert_eq!(plan.projection_membership_id, binding.projection_membership_id);
                            assert_eq!(plan.access_partition_digest, binding.access_partition_digest);
                            assert_eq!(plan.scoring_partition_digest, binding.scoring_partition_digest);
                        }
                    }
                } else {
                    assert_eq!(intersected, Err(AccessError::SnapshotStale));
                    assert_eq!(direct, Err(AccessError::SnapshotStale));
                }
                assert_eq!(supplied, before);
            }
        }
        assert_eq!(registry, original, "validation must not rewrite inconsistent input");
    }
}

#[test]
fn empty_scope_is_not_a_successful_empty_or_grouped_leg() {
    let scope = scope_from(&snapshot(), &BTreeSet::new());
    let overlap = proof(&scope);
    for grouped in [false, true] {
        assert_eq!(legs(&scope, grouped.then_some(&overlap)), Err(AccessError::AuthorizedScopeEmpty));
    }
    assert_eq!(compile_safe_legs(&scope, route(), 7, 6, 6, None, 0),
        Err(AccessError::RetrievalLegBudgetExceeded));
}

#[test]
fn unknown_foreign_inactive_and_empty_scope_refusals_are_preserved() {
    let mut registry = snapshot();
    registry.bindings.get_mut(&member(1)).unwrap().membership_id = member(2);
    let requested = RequestedMembershipScope { memberships: BTreeSet::from([member(1)]) };
    assert_eq!(intersect_scope(&requested, &BTreeSet::new(), &registry), Err(AccessError::ScopeUnauthorized));
    let unknown = RequestedMembershipScope { memberships: BTreeSet::from([member(9)]) };
    assert_eq!(intersect_scope(&unknown, &unknown.memberships, &registry), Err(AccessError::ScopeUnknown));
    let empty = RequestedMembershipScope { memberships: BTreeSet::new() };
    assert_eq!(intersect_scope(&empty, &empty.memberships, &registry), Err(AccessError::AuthorizedScopeEmpty));
    registry.bindings.get_mut(&member(1)).unwrap().active = false;
    assert_eq!(intersect_scope(&requested, &requested.memberships, &registry), Err(AccessError::ScopeUnauthorized));
    let direct = scope_from(&registry, &requested.memberships);
    let overlap = proof(&direct);
    for grouped in [false, true] {
        assert_eq!(legs(&direct, grouped.then_some(&overlap)), Err(AccessError::ScopeUnauthorized));
    }
}

#[test]
fn unrelated_bad_bindings_do_not_widen_or_poison_the_selected_scope() {
    let mut registry = snapshot();
    registry.bindings.get_mut(&member(3)).unwrap().membership_id = member(2);
    let requested = RequestedMembershipScope { memberships: BTreeSet::from([member(1)]) };
    let scope = intersect_scope(&requested, &BTreeSet::from([member(1), member(2)]), &registry).unwrap();
    assert_eq!(scope.memberships, BTreeMap::from([(member(1), registry.bindings[&member(1)].clone())]));
    assert_eq!(legs(&scope, None).unwrap()[0].memberships, requested.memberships);
}

#[test]
fn stale_grouping_proof_keeps_coherent_memberships_in_separate_legs() {
    let registry = snapshot();
    let scope = scope_from(&registry, &BTreeSet::from([member(1), member(2)]));
    let mut stale = proof(&scope);
    stale.access_snapshot_generation -= 1;
    assert_eq!(legs(&scope, Some(&stale)), legs(&scope, None));
}

struct Admission {
    claims: GrantClaims,
    context: GrantValidationContext,
    requested: RequestedMembershipScope,
    registry: AuthoritativeAccessSnapshot,
    policy: AuthoritativePolicyState,
    fence: NamespacePolicyFence,
    live: LiveSecurityState,
}

impl Admission {
    fn new() -> Self {
        let binding_id = BindingId::from_bytes([1; 16]);
        let installation_id = InstallationId::from_bytes([2; 16]);
        let installation_incarnation_id = InstallationIncarnationId::from_bytes([3; 16]);
        let boot = OpaqueId::new("synthetic:boot").unwrap();
        let fence = NamespacePolicyFence {
            namespace_id: SourceNamespaceId::from_bytes([4; 16]),
            owner_generation: SourceOwnerGeneration::from_bytes([5; 32]),
            policy_revision: AccessPolicyRevision::new(6),
        };
        Self {
            claims: GrantClaims {
                grant_id: GrantId::from_bytes([6; 16]), binding_id, installation_id,
                installation_incarnation_id, issued_boot_id: boot.clone(),
                issued_at_ms: 1_000, expires_at_ms: 2_000,
                nonce: OpaqueId::new("synthetic:nonce").unwrap(), revocation_generation: 4,
                allowed_recipes: BTreeSet::from([RecipeIdV1::FindText]),
                allowed_modalities: BTreeSet::from([AccessModality::Direct, AccessModality::Lexical]),
                allowed_budget_classes: BTreeSet::from([OpaqueId::new("synthetic:budget").unwrap()]),
                max_source_read_bytes: 1_024, max_result_bytes: 1_024,
            },
            context: GrantValidationContext {
                binding_id, installation_id, installation_incarnation_id, boot_id: boot,
                now_ms: 1_500, current_revocation_generation: 4,
                signature_verified: true, pairing_verified: true, nonce_accepted: true,
            },
            requested: RequestedMembershipScope { memberships: BTreeSet::from([member(1)]) },
            registry: snapshot(),
            policy: AuthoritativePolicyState {
                namespace_id: fence.namespace_id, owner_generation: fence.owner_generation,
                policy_revision: fence.policy_revision,
                shadow_revision: ShadowFenceRevision::new(6), purge_revision: PurgeFenceRevision::new(6),
            },
            fence,
            live: LiveSecurityState {
                generation: 7, denied_memberships: BTreeSet::new(), purged_memberships: BTreeSet::new(),
                fail_closed: false, snapshot_digest: digest(7),
            },
        }
    }

    fn request<'a>(&'a self, modality: AccessModality, proof: Option<&'a OverlapFreeRouteProof>) -> PreRetrievalRequest<'a> {
        PreRetrievalRequest {
            claims: self.claims.clone(), validation_context: &self.context,
            recipe: RecipeIdV1::FindText, modality, requested_scope: &self.requested,
            grant_scope: &self.requested.memberships, authoritative: &self.registry,
            policy_fence: &self.fence, authoritative_policy: &self.policy, route: route(),
            live: &self.live, overlap_proof: proof, max_legs: 3,
        }
    }
}

#[test]
fn admission_never_checks_one_membership_while_planning_another() {
    for modality in [AccessModality::Direct, AccessModality::Lexical] {
        for purged in [false, true] {
            let mut fixture = Admission::new();
            if purged {
                fixture.live.purged_memberships.insert(member(2));
            } else {
                fixture.live.denied_memberships.insert(member(2));
            }
            fixture.registry.bindings.get_mut(&member(1)).unwrap().membership_id = member(2);
            let original = fixture.registry.clone();
            let overlap = proof(&scope_from(&fixture.registry, &fixture.requested.memberships));
            for grouped in [false, true] {
                let result = compile_pre_retrieval(fixture.request(modality, grouped.then_some(&overlap)));
                assert_eq!(result.unwrap_err(), AccessError::SnapshotStale);
                assert_eq!(fixture.registry, original);
            }
            // Correcting the producer snapshot restores the exact requested scope,
            // not the denied neighbour. Runtime validation itself makes no repair.
            fixture.registry.bindings.get_mut(&member(1)).unwrap().membership_id = member(1);
            let admitted = compile_pre_retrieval(fixture.request(modality, None)).unwrap();
            assert_eq!(admitted.legs[0].memberships, fixture.requested.memberships);
            assert_eq!(admitted.legs[0].eligibility_plans[0].membership_id, member(1));
        }
    }
}

#[test]
fn grant_and_policy_validation_still_precede_snapshot_validation() {
    let mut fixture = Admission::new();
    fixture.registry.bindings.get_mut(&member(1)).unwrap().membership_id = member(2);
    fixture.context.signature_verified = false;
    assert_eq!(compile_pre_retrieval(fixture.request(AccessModality::Direct, None)).unwrap_err(),
        AccessError::GrantSignatureInvalid);
    fixture.context.signature_verified = true;
    fixture.fence.policy_revision = AccessPolicyRevision::new(99);
    assert_eq!(compile_pre_retrieval(fixture.request(AccessModality::Direct, None)).unwrap_err(),
        AccessError::PolicyRevisionStale);
    fixture.fence.policy_revision = fixture.policy.policy_revision;
    assert_eq!(compile_pre_retrieval(fixture.request(AccessModality::Direct, None)).unwrap_err(),
        AccessError::SnapshotStale);
}

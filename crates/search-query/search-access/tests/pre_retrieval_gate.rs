//! T20 discriminating fixtures: mandatory pre-retrieval access compiler and
//! live-barrier path over DIRECT legs and vendor-neutral predicates.
//!
//! Every test proves denial *before* any source byte or provider call: the
//! compiler is pure, so a returned `Err` means no retrieval, scoring, count,
//! facet or trace could have been influenced.
//!
//! Live Qdrant scoring/IDF/count parity is an explicit T24 obligation, not a
//! T20 claim ([`search_access::QDRANT_LIVE_PARITY_DEFERRED_TO`]).

use std::collections::{BTreeMap, BTreeSet};

use search_access::{
    AccessCheckpoint, AccessError, AccessModality, ActiveRequestDecision,
    AuthoritativeAccessSnapshot, AuthoritativePolicyState, IndexedRouteFence,
    LegSecurityPopulation, LiveSecurityState, MembershipAccessBinding, NamespacePolicyFence,
    OverlapFreeRouteProof, PreRetrievalRequest, RequestedMembershipScope,
    classify_active_request_contamination, classify_contaminated_legs, compile_pre_retrieval,
    deny_handle_as_grant, deny_local_identity_as_grant, idf_predicate_digest,
    retain_eligible_plans, retrieval_predicate_digest,
};
use search_contracts::{
    AccessPolicyRevision, BindingId, Blake3Digest32, CollectionGenerationId, Epoch, GrantId,
    InstallationId, InstallationIncarnationId, OpaqueId, OwnerEpoch, PurgeFenceRevision,
    RecipeIdV1, ShadowFenceRevision, SourceMembershipId, SourceNamespaceId, SourceOwnerGeneration,
};

fn oid(value: &str) -> OpaqueId {
    OpaqueId::new(value).expect("fixture opaque id must be valid")
}

const fn membership(n: u128) -> SourceMembershipId {
    SourceMembershipId::from_bytes(n.to_be_bytes())
}

const fn digest(byte: u8) -> Blake3Digest32 {
    Blake3Digest32::from_bytes([byte; 32])
}

fn binding(member: SourceMembershipId) -> MembershipAccessBinding {
    MembershipAccessBinding {
        membership_id: member,
        access_partition_digest: digest(0xA1),
        scoring_partition_digest: digest(0xB2),
        projection_membership_id: oid("projection-fixture"),
        active: true,
    }
}

fn authoritative(members: &[SourceMembershipId]) -> AuthoritativeAccessSnapshot {
    AuthoritativeAccessSnapshot {
        generation: 7,
        source_catalog_generation: 3,
        membership_generation: 5,
        bindings: members
            .iter()
            .map(|m| (*m, binding(*m)))
            .collect::<BTreeMap<_, _>>(),
        snapshot_digest: digest(0xCC),
    }
}

fn grant_fixture() -> (
    search_access::GrantClaims,
    search_access::GrantValidationContext,
) {
    let installation_id = InstallationId::from_bytes([1; 16]);
    let incarnation = InstallationIncarnationId::from_bytes([2; 16]);
    let binding_id = BindingId::from_bytes([3; 16]);
    let boot = oid("boot-fixture-0001");
    let nonce = oid("nonce-fixture-0001");
    let claims = search_access::GrantClaims {
        grant_id: GrantId::from_bytes([9; 16]),
        binding_id,
        installation_id,
        installation_incarnation_id: incarnation,
        issued_boot_id: boot.clone(),
        issued_at_ms: 1_000,
        expires_at_ms: 2_000,
        nonce,
        revocation_generation: 4,
        allowed_recipes: BTreeSet::from([RecipeIdV1::FindText]),
        allowed_modalities: BTreeSet::from([AccessModality::Direct]),
        allowed_budget_classes: BTreeSet::from([oid("budget-interactive")]),
        max_source_read_bytes: 1_024,
        max_result_bytes: 1_024,
    };
    let context = search_access::GrantValidationContext {
        binding_id,
        installation_id,
        installation_incarnation_id: incarnation,
        boot_id: boot,
        now_ms: 1_500,
        current_revocation_generation: 4,
        signature_verified: true,
        pairing_verified: true,
        nonce_accepted: true,
    };
    (claims, context)
}

fn route_fixture() -> IndexedRouteFence {
    IndexedRouteFence {
        collection_generation_id: CollectionGenerationId::from_bytes([0x10; 16]),
        visible_epoch: Epoch::new(12).expect("fixture epoch must be valid"),
        route_generation: 2,
        owner_epoch: OwnerEpoch::new(1).expect("fixture owner epoch must be valid"),
    }
}

const fn live_fixture() -> LiveSecurityState {
    LiveSecurityState {
        generation: 7,
        denied_memberships: BTreeSet::new(),
        purged_memberships: BTreeSet::new(),
        fail_closed: false,
        snapshot_digest: digest(0xDD),
    }
}

const fn policy_pair() -> (NamespacePolicyFence, AuthoritativePolicyState) {
    let namespace_id = SourceNamespaceId::from_bytes([0x20; 16]);
    let owner_generation = SourceOwnerGeneration::from_bytes([0x21; 32]);
    let fence = NamespacePolicyFence {
        namespace_id,
        owner_generation,
        policy_revision: AccessPolicyRevision::new(11),
    };
    let authoritative = AuthoritativePolicyState {
        namespace_id,
        owner_generation,
        policy_revision: AccessPolicyRevision::new(11),
        shadow_revision: ShadowFenceRevision::new(6),
        purge_revision: PurgeFenceRevision::new(6),
    };
    (fence, authoritative)
}

struct Fixture {
    claims: search_access::GrantClaims,
    context: search_access::GrantValidationContext,
    requested: RequestedMembershipScope,
    grant_scope: BTreeSet<SourceMembershipId>,
    authoritative: AuthoritativeAccessSnapshot,
    policy_fence: NamespacePolicyFence,
    authoritative_policy: AuthoritativePolicyState,
    route: IndexedRouteFence,
    live: LiveSecurityState,
}

impl Fixture {
    fn two_member() -> Self {
        let members = [membership(1), membership(2)];
        let (claims, context) = grant_fixture();
        let (policy_fence, authoritative_policy) = policy_pair();
        Self {
            claims,
            context,
            requested: RequestedMembershipScope {
                memberships: BTreeSet::from(members),
            },
            grant_scope: BTreeSet::from(members),
            authoritative: authoritative(members.as_slice()),
            policy_fence,
            authoritative_policy,
            route: route_fixture(),
            live: live_fixture(),
        }
    }

    fn request<'a>(
        &'a self,
        recipe: RecipeIdV1,
        modality: AccessModality,
        max_legs: usize,
        proof: Option<&'a OverlapFreeRouteProof>,
    ) -> PreRetrievalRequest<'a> {
        PreRetrievalRequest {
            claims: self.claims.clone(),
            validation_context: &self.context,
            recipe,
            modality,
            requested_scope: &self.requested,
            grant_scope: &self.grant_scope,
            authoritative: &self.authoritative,
            policy_fence: &self.policy_fence,
            authoritative_policy: &self.authoritative_policy,
            route: self.route,
            live: &self.live,
            overlap_proof: proof,
            max_legs,
        }
    }

    fn current_proof(&self) -> OverlapFreeRouteProof {
        OverlapFreeRouteProof {
            route: self.route,
            memberships: self.requested.memberships.clone(),
            access_snapshot_generation: self.authoritative.generation,
            profile_digest: digest(0xE0),
            proof_digest: digest(0xE1),
        }
    }
}

#[test]
fn two_disjoint_scopes_never_widen() {
    let mut fixture = Fixture::two_member();
    // Requesting a foreign membership the grant never contained denies.
    fixture.requested = RequestedMembershipScope {
        memberships: BTreeSet::from([membership(1), membership(3)]),
    };
    let error = compile_pre_retrieval(fixture.request(
        RecipeIdV1::FindText,
        AccessModality::Direct,
        8,
        None,
    ))
    .expect_err("foreign scope must deny before any provider call");
    assert_eq!(error, AccessError::ScopeUnauthorized);

    // A narrow request inside the grant compiles to exactly the requested leg.
    fixture.requested = RequestedMembershipScope {
        memberships: BTreeSet::from([membership(1)]),
    };
    let plan = compile_pre_retrieval(fixture.request(
        RecipeIdV1::FindText,
        AccessModality::Direct,
        8,
        None,
    ))
    .expect("narrow scope must compile");
    assert_eq!(plan.legs.len(), 1);
    assert_eq!(plan.legs[0].memberships, BTreeSet::from([membership(1)]));
}

#[test]
fn overlapping_memberships_require_current_proof() {
    let fixture = Fixture::two_member();
    // Without a proof each membership stays in its own scoring population.
    let split = compile_pre_retrieval(fixture.request(
        RecipeIdV1::FindText,
        AccessModality::Direct,
        8,
        None,
    ))
    .expect("split legs must compile");
    assert_eq!(split.legs.len(), 2);

    // A stale proof never fuses legs into one shared IDF population.
    let stale = OverlapFreeRouteProof {
        access_snapshot_generation: fixture.authoritative.generation.saturating_add(99),
        ..fixture.current_proof()
    };
    let still_split = compile_pre_retrieval(fixture.request(
        RecipeIdV1::FindText,
        AccessModality::Direct,
        8,
        Some(&stale),
    ))
    .expect("stale proof must split, not fail open");
    assert_eq!(still_split.legs.len(), 2);

    // A current proof fuses exactly one coherent leg.
    let proof = fixture.current_proof();
    let fused = compile_pre_retrieval(fixture.request(
        RecipeIdV1::FindText,
        AccessModality::Direct,
        8,
        Some(&proof),
    ))
    .expect("current proof must fuse");
    assert_eq!(fused.legs.len(), 1);
    assert_eq!(fused.legs[0].memberships.len(), 2);
}

#[test]
fn revoked_access_during_scan_discards_whole_leg() {
    let fixture = Fixture::two_member();
    let plan = compile_pre_retrieval(fixture.request(
        RecipeIdV1::FindText,
        AccessModality::Direct,
        8,
        None,
    ))
    .expect("baseline legs must compile");
    assert_eq!(plan.legs.len(), 2);

    let previous = live_fixture();
    let mut current = live_fixture();
    current.generation = 8;
    current.denied_memberships.insert(membership(2));
    let execution = plan
        .legs
        .iter()
        .map(|leg| LegSecurityPopulation {
            leg_id: leg.leg_id,
            memberships: leg.memberships.clone(),
            security_generation: previous.generation,
            idf_population_digest: None,
        })
        .collect::<Vec<_>>();
    // Whole-leg discard: candidate-only cleanup cannot preserve ordering.
    assert_eq!(
        classify_contaminated_legs(&execution, &previous, &current),
        search_access::ContaminationDecision::DiscardLegs(BTreeSet::from([1]))
    );
    // The active-request barrier agrees: replan without the denied member.
    assert_eq!(
        classify_active_request_contamination(
            &BTreeSet::from([membership(1), membership(2)]),
            &previous,
            &current,
            true
        ),
        ActiveRequestDecision::DiscardAndReplan
    );
}

#[test]
fn purge_and_revocation_barriers_precede_scoring() {
    let mut fixture = Fixture::two_member();
    fixture.live.purged_memberships.insert(membership(1));
    let error = compile_pre_retrieval(fixture.request(
        RecipeIdV1::FindText,
        AccessModality::Direct,
        8,
        None,
    ))
    .expect_err("purged scope must deny before scoring");
    assert_eq!(error, AccessError::LivePurge);

    fixture.live.purged_memberships.clear();
    fixture.live.denied_memberships.insert(membership(2));
    let error = compile_pre_retrieval(fixture.request(
        RecipeIdV1::FindText,
        AccessModality::Direct,
        8,
        None,
    ))
    .expect_err("revoked scope must deny before scoring");
    assert_eq!(error, AccessError::LiveRevocation);
}

#[test]
fn stale_grant_replay_denied() {
    let fixture = Fixture::two_member();
    // Expired grant: replayed after expiry.
    let mut expired = Fixture::two_member();
    expired.context.now_ms = 2_000;
    let error = compile_pre_retrieval(expired.request(
        RecipeIdV1::FindText,
        AccessModality::Direct,
        8,
        None,
    ))
    .expect_err("expired grant must deny");
    assert_eq!(error, AccessError::GrantExpired);

    // Revoked generation: replayed stale revocation counter.
    let mut revoked = Fixture::two_member();
    revoked.context.current_revocation_generation = 5;
    let error = compile_pre_retrieval(revoked.request(
        RecipeIdV1::FindText,
        AccessModality::Direct,
        8,
        None,
    ))
    .expect_err("revoked grant must deny");
    assert_eq!(error, AccessError::GrantRevoked);

    // Rebound token: same grant presented on a foreign binding.
    let mut rebound = Fixture::two_member();
    rebound.context.binding_id = search_contracts::BindingId::from_bytes([0xFF; 16]);
    let error = compile_pre_retrieval(rebound.request(
        RecipeIdV1::FindText,
        AccessModality::Direct,
        8,
        None,
    ))
    .expect_err("rebound grant must deny");
    assert_eq!(error, AccessError::GrantBindingMismatch);

    let _ = fixture;
}

#[test]
fn retrieval_and_idf_predicates_share_one_contract() {
    let fixture = Fixture::two_member();
    let proof = fixture.current_proof();
    let plan = compile_pre_retrieval(fixture.request(
        RecipeIdV1::FindText,
        AccessModality::Direct,
        8,
        Some(&proof),
    ))
    .expect("fused plan must compile");
    assert!(!plan.predicates.is_empty());
    for predicates in &plan.predicates {
        assert!(predicates.is_consistent());
        assert_eq!(predicates.retrieval_digest, predicates.idf_digest);
    }
    // Per-leg digest accessors expose the same single contract digest.
    for leg in &plan.legs {
        for eligibility in &leg.eligibility_plans {
            assert_eq!(
                retrieval_predicate_digest(eligibility),
                idf_predicate_digest(eligibility)
            );
        }
    }
    // Admission permit is bound to the request-admission checkpoint only.
    assert_eq!(plan.permit.checkpoint, AccessCheckpoint::RequestAdmission);
}

#[test]
fn denied_source_excluded_before_counts() {
    let fixture = Fixture::two_member();
    let proof = fixture.current_proof();
    let plan = compile_pre_retrieval(fixture.request(
        RecipeIdV1::FindText,
        AccessModality::Direct,
        8,
        Some(&proof),
    ))
    .expect("baseline plan must compile");
    let plans = plan.legs[0].eligibility_plans.as_slice();
    let retained = retain_eligible_plans(plans, &fixture.live).expect("clean live keeps plans");
    assert_eq!(retained.len(), plans.len());

    let mut denied_live = fixture.live.clone();
    denied_live.generation = 8;
    denied_live.denied_memberships.insert(membership(1));
    let error =
        retain_eligible_plans(plans, &denied_live).expect_err("denied plan must not reach counts");
    assert_eq!(error, AccessError::LiveRevocation);
}

#[test]
fn invalid_namespace_owner_policy_deny_before_provider() {
    let fixture = Fixture::two_member();

    let mut foreign_namespace = Fixture::two_member();
    foreign_namespace.policy_fence.namespace_id = SourceNamespaceId::from_bytes([0x99; 16]);
    let error = compile_pre_retrieval(foreign_namespace.request(
        RecipeIdV1::FindText,
        AccessModality::Direct,
        8,
        None,
    ))
    .expect_err("foreign namespace must deny");
    assert_eq!(error, AccessError::NamespaceUnknown);

    let mut foreign_owner = Fixture::two_member();
    foreign_owner.policy_fence.owner_generation = SourceOwnerGeneration::from_bytes([0x99; 32]);
    let error = compile_pre_retrieval(foreign_owner.request(
        RecipeIdV1::FindText,
        AccessModality::Direct,
        8,
        None,
    ))
    .expect_err("foreign owner must deny");
    assert_eq!(error, AccessError::OwnerMismatch);

    let mut stale_policy = Fixture::two_member();
    stale_policy.policy_fence.policy_revision = AccessPolicyRevision::new(10);
    let error = compile_pre_retrieval(stale_policy.request(
        RecipeIdV1::FindText,
        AccessModality::Direct,
        8,
        None,
    ))
    .expect_err("stale policy revision must deny");
    assert_eq!(error, AccessError::PolicyRevisionStale);

    let _ = fixture;
}

#[test]
fn handle_and_local_identity_never_authorize() {
    assert_eq!(
        deny_handle_as_grant(),
        AccessError::HandlePossessionNotAuthority
    );
    assert_eq!(deny_handle_as_grant().code(), "ACCESS_HANDLE_NOT_AUTHORITY");
    assert_eq!(
        deny_local_identity_as_grant(),
        AccessError::LocalIdentityNotAuthority
    );
    assert_eq!(
        deny_local_identity_as_grant().code(),
        "ACCESS_LOCAL_IDENTITY_NOT_AUTHORITY"
    );
}

#[test]
fn direct_modality_compiles_through_same_gate() {
    let fixture = Fixture::two_member();
    let plan = compile_pre_retrieval(fixture.request(
        RecipeIdV1::FindText,
        AccessModality::Direct,
        8,
        None,
    ))
    .expect("DIRECT must compile through the mandatory gate");
    assert_eq!(plan.legs.len(), 2);
    assert_eq!(plan.permit.live_generation, fixture.live.generation);

    // An ungranted modality is not smuggled through the same gate.
    let error = compile_pre_retrieval(fixture.request(
        RecipeIdV1::FindText,
        AccessModality::Lexical,
        8,
        None,
    ))
    .expect_err("ungranted modality must deny");
    assert_eq!(error, AccessError::ModalityDenied);

    // An ungranted recipe family is rejected even with a valid grant.
    let error =
        compile_pre_retrieval(fixture.request(RecipeIdV1::Locate, AccessModality::Direct, 8, None))
            .expect_err("ungranted recipe must deny");
    assert_eq!(error, AccessError::RecipeDenied);
}

#[test]
fn qdrant_live_parity_is_t24_not_t20() {
    assert_eq!(search_access::QDRANT_LIVE_PARITY_DEFERRED_TO, "T24");
}

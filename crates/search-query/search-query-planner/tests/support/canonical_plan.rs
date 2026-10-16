use std::collections::BTreeMap;

use search_access::{AuthorizedScope, MembershipAccessBinding, SafeRetrievalLeg};
use search_contracts::{
    AccessPartitionId, AccessPolicyRevision, Blake3Digest32, Blake3ExactInputLimit, BoundedList,
    BoundedName, BoundedSet, CatalogRevision, ClientScopeFence, CollectionRouteRevision,
    DisclosureCeiling, EntityKind, ExactnessRequirements, FusionProfileId, GrantFence, GrantId,
    InstallationId, InstallationIncarnationId, MembershipRevision, ObservationCursorRevision,
    ObservationFreshness, ObservationFreshnessState, OpaqueId, OpaqueRef, PlanFingerprint, PlanId,
    PriorityClass, ProfileFence, ProfileId, ProjectionProfileSetId, PurgeFenceRevision,
    QueryExecutionBudget, QuerySnapshotFence, QuerySnapshotFingerprint, RecipeBodyV1, RecipeIdV1,
    RequestId, RequestedScope, RequiredDenominator, SearchReadGrantClaims, SearchRecipeRequest,
    SearchTaskPlan, ShadowFenceRevision, SourceRevisionId, SourceView, UtcTimestamp,
};
use search_query_planner::{
    AdvancedCapabilities, CapabilitySet, CompiledSearchPlan, RetrievalCapabilities,
    compile_canonical_plan, normalize_recipe,
};

pub(crate) fn lexical_budget(
    deadline_ms: u64,
    max_candidates: u32,
    max_source_read_bytes: u64,
    max_cpu_ms: u64,
    max_memory_bytes: u64,
) -> QueryExecutionBudget {
    QueryExecutionBudget {
        priority_class: PriorityClass::Interactive,
        deadline_ms,
        max_scoring_legs: 1,
        max_prefetch_candidates_per_leg: max_candidates,
        max_validated_candidates: max_candidates,
        max_source_read_bytes,
        max_exact_scan_items: 0,
        max_exact_scan_bytes: 0,
        max_materialized_result_bytes: max_source_read_bytes,
        max_cpu_ms,
        max_memory_bytes,
    }
}

pub(crate) fn compile_lexical_plan(
    request_id: RequestId,
    safe_leg: SafeRetrievalLeg,
    budget: QueryExecutionBudget,
) -> CompiledSearchPlan {
    let memberships = safe_leg.memberships.iter().copied().collect::<Vec<_>>();
    assert!(
        !memberships.is_empty(),
        "canonical fixture membership scope"
    );
    assert_eq!(safe_leg.leg_id, 0, "canonical fixture first leg");

    let bindings = safe_leg
        .eligibility_plans
        .iter()
        .map(|eligibility| {
            (
                eligibility.membership_id,
                MembershipAccessBinding {
                    membership_id: eligibility.membership_id,
                    access_partition_digest: eligibility.access_partition_digest,
                    scoring_partition_digest: eligibility.scoring_partition_digest,
                    projection_membership_id: eligibility.projection_membership_id.clone(),
                    active: true,
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(
        bindings.len(),
        memberships.len(),
        "canonical fixture eligibility membership set"
    );
    let scope = AuthorizedScope {
        memberships: bindings,
        access_snapshot_generation: 11,
        source_catalog_generation: 1,
        membership_generation: 1,
        snapshot_digest: Blake3Digest32::from_bytes([9; 32]),
    };

    let source_view = SourceView::RetainedRevision(SourceRevisionId::from_bytes([2; 16]));
    let route = safe_leg.route.clone();
    let first_eligibility = safe_leg
        .eligibility_plans
        .first()
        .expect("canonical fixture eligibility plan");
    let mut snapshot = QuerySnapshotFence {
        installation_incarnation_id: InstallationIncarnationId::from_bytes([1; 16]),
        collection_generation_id: Some(route.collection_generation_id),
        visible_epoch: Some(route.visible_epoch),
        collection_route_revision: CollectionRouteRevision::new(route.route_generation),
        catalog_revision: CatalogRevision::new(1),
        membership_revision: MembershipRevision::new(1),
        reference_portfolio_revision: None,
        access_policy_revision: AccessPolicyRevision::new(1),
        shadow_fence_revision: ShadowFenceRevision::new(first_eligibility.shadow_generation),
        purge_fence_revision: PurgeFenceRevision::new(first_eligibility.purge_generation),
        overlay_revision: search_contracts::OverlayRevision::new(1),
        observation_cursor_revision: ObservationCursorRevision::new(1),
        observation_freshness: ObservationFreshness {
            state: ObservationFreshnessState::CurrentConfirmed,
            observation_cursor_revision: ObservationCursorRevision::new(1),
            observed_age_ms: None,
        },
        source_view: source_view.clone(),
        workspace_view_revision_ref: None,
        lexical_profile_ids: BoundedList::new(vec![profile("lexical-v1")])
            .expect("canonical fixture lexical profile"),
        snapshot_fingerprint: QuerySnapshotFingerprint::from_bytes([0; 32]),
    };
    seal_snapshot(&mut snapshot);

    let request = SearchRecipeRequest::new(
        request_id,
        RecipeIdV1::Locate,
        source_view,
        RequestedScope::ExplicitMemberships(
            BoundedList::new(memberships.clone()).expect("canonical fixture requested scope"),
        ),
        profile("interactive"),
        RecipeBodyV1::Locate(search_contracts::LocateRecipe {
            subject: search_contracts::SubjectSelector::NormalizedName(
                search_contracts::NormalizedNameSelector {
                    name: BoundedName::new("parse_manifest").expect("canonical fixture name"),
                    entity_kinds: BoundedSet::<EntityKind, 4096>::empty(),
                },
            ),
            evidence_roles: BoundedSet::empty(),
        }),
    )
    .expect("canonical fixture recipe request");
    let recipe = normalize_recipe(request.clone()).expect("canonical fixture normalized recipe");

    let client_scope_ref = OpaqueRef::new("scope:canonical-fixture").expect("fixture scope");
    let scope_domain_id = search_contracts::ScopeDomainId::from_bytes([3; 16]);
    let grant_id = GrantId::from_bytes([5; 16]);
    let grant = SearchReadGrantClaims {
        grant_id,
        installation_id: InstallationId::from_bytes([14; 16]),
        installation_incarnation_id: InstallationIncarnationId::from_bytes([1; 16]),
        binding_id: search_contracts::BindingId::from_bytes([15; 16]),
        principal_opaque_id: OpaqueId::new("principal").expect("fixture principal"),
        client_scope_ref: client_scope_ref.clone(),
        scope_domain_id,
        allowed_membership_ids: BoundedSet::from_items(memberships.iter().copied())
            .expect("fixture grant memberships"),
        allowed_corpus_or_portfolio_ids: BoundedSet::empty(),
        reference_portfolio_revision: None,
        allowed_access_partitions: BoundedSet::from_items([AccessPartitionId::from_bytes([7; 16])])
            .expect("fixture access partition"),
        allowed_modalities: BoundedSet::from_items([search_contracts::Modality::Text])
            .expect("fixture modality"),
        permitted_recipe_families: BoundedSet::from_items([RecipeIdV1::Locate])
            .expect("fixture recipe grant"),
        maximum_budget_class: profile("interactive"),
        sensitivity_ceiling: search_contracts::SensitivityClass::Public,
        disclosure_ceiling: DisclosureCeiling::LocalOnly,
        source_read_permission: true,
        exact_scan_permission: false,
        issued_boot_id: OpaqueId::new("boot").expect("fixture boot"),
        issued_at: UtcTimestamp::parse("2026-10-16T00:00:00.000000Z").expect("fixture issue time"),
        expires_at: UtcTimestamp::parse("2026-10-16T00:05:00.000000Z").expect("fixture expiry"),
        nonce: OpaqueId::new("nonce").expect("fixture nonce"),
        revocation_generation: 4,
    };

    let mut task_plan = SearchTaskPlan {
        plan_id: PlanId::from_bytes([16; 16]),
        provider_protocol_version: search_contracts::ProtocolVersion { major: 1, minor: 0 },
        request_id,
        recipe_request_digest: request
            .canonical_request_digest()
            .expect("canonical fixture request digest"),
        grant_fence: GrantFence {
            grant_id,
            revocation_generation: 4,
        },
        client_scope_fence: ClientScopeFence {
            client_scope_ref,
            scope_domain_id,
        },
        query_snapshot_fence: snapshot,
        source_owner_fences: BoundedList::empty(),
        selected_membership_ids: BoundedList::new(memberships)
            .expect("canonical fixture selected memberships"),
        profile_fence: ProfileFence {
            fusion_profile_id: FusionProfileId::new("fusion-v1").expect("fixture fusion profile"),
            projection_profile_set_ids: BoundedList::new(vec![
                ProjectionProfileSetId::new("lexical-v1").expect("fixture projection profile"),
            ])
            .expect("fixture projection profile set"),
            optional_provider_profile_ids: BoundedList::empty(),
        },
        overlay_snapshot_refs: BoundedList::empty(),
        query_execution_budget: budget,
        exactness_requirements: ExactnessRequirements {
            required_denominator: RequiredDenominator::CandidateScope,
            require_current_observation: true,
            allow_truthful_partial: true,
        },
        additional_state_dependencies: BoundedList::empty(),
        plan_fingerprint: PlanFingerprint::from_bytes([0; 32]),
        created_at: UtcTimestamp::parse("2026-10-16T00:00:00.000000Z")
            .expect("fixture creation time"),
        expires_at: UtcTimestamp::parse("2026-10-16T00:01:00.000000Z")
            .expect("fixture expiration time"),
    };

    let capabilities = CapabilitySet {
        retrieval: RetrievalCapabilities {
            direct: false,
            lexical: true,
            exact: false,
        },
        advanced: AdvancedCapabilities {
            structural: false,
            semantic: false,
            rerank: false,
        },
    };
    let safe_legs = [safe_leg];
    task_plan.additional_state_dependencies = BoundedList::new(vec![
        search_query_planner::canonical_execution_dependency(capabilities, &scope, &safe_legs)
            .expect("canonical fixture execution dependency"),
    ])
    .expect("canonical fixture dependencies");
    seal_task_plan(&mut task_plan);

    compile_canonical_plan(&task_plan, &grant, &recipe, scope, capabilities, &safe_legs)
        .expect("canonical fixture compiled plan")
}

fn profile(value: &str) -> ProfileId {
    ProfileId::new(value).expect("canonical fixture profile")
}

fn digest(bytes: &[u8]) -> [u8; 32] {
    let limit = Blake3ExactInputLimit::new(bytes.len()).expect("bounded fixture bytes");
    *search_contracts::blake3_exact_bytes(bytes, limit)
        .expect("canonical fixture BLAKE3")
        .as_bytes()
}

fn seal_snapshot(snapshot: &mut QuerySnapshotFence) {
    let input = snapshot
        .canonical_fingerprint_input()
        .expect("canonical fixture snapshot");
    snapshot.snapshot_fingerprint = QuerySnapshotFingerprint::from_bytes(digest(input.as_slice()));
}

fn seal_task_plan(plan: &mut SearchTaskPlan) {
    let input = plan
        .canonical_fingerprint_input()
        .expect("canonical fixture task plan");
    plan.plan_fingerprint = PlanFingerprint::from_bytes(digest(input.as_slice()));
}

//! One-owner C18-to-C22 lexical planning contract case; no Root runtime or backend client.

use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, Instant};

use search_access::{
    AccessError, AuthoritativeAccessSnapshot, AuthoritativePolicyState,
    CanonicalEligibilitySecuritySnapshot, CanonicalEvidencePopulationRequest,
    CanonicalEvidencePopulationVerificationPort, CanonicalPopulationVerificationPort,
    CanonicalProjectionMembership, CompiledEligibilityPopulation, EvidencePopulationRecipe,
    LiveSecurityState, MembershipAccessBinding, NamespacePolicyFence, RequestSecurityFence,
    RequestedMembershipScope, compile_canonical_evidence_population,
};
use search_contracts::evidence::{
    EvidenceHints, EvidenceIntent, PublicScopeSelector, RecipeIdV2, RetrieveEvidenceRequestV2,
};
use search_contracts::indexed::{
    CollectionSchema, NamedVectorRequirement, VectorName, VectorRequirement,
};
use search_contracts::{
    AccessPartitionId, AccessPolicyRevision, ArtifactDigest, AuthoritativeGrantPolicy, BindingId,
    Blake3Digest32, Blake3ExactInputLimit, BoundedList, BoundedSet, CatalogRevision,
    CollectionGenerationId, CollectionRoute, CollectionRouteRevision, CorpusId, DisclosureCeiling,
    Epoch, GrantId, InstallationId, InstallationIncarnationId, MAX_SET_ITEMS, MembershipRevision,
    Modality, ObservationCursorRevision, ObservationFreshness, ObservationFreshnessState, OpaqueId,
    OpaqueRef, OverlayRevision, PriorityClass, ProfileId, ProjectionMembershipId,
    ProjectionProfileSetId, PurgeFenceRevision, QueryExecutionBudget, QuerySnapshotFence,
    QuerySnapshotFingerprint, ReceiptRef, ScopeDomainId, ScoringPartitionId, SearchReadGrantClaims,
    SensitivityClass, ShadowFenceRevision, SourceMembershipId, SourceNamespaceId,
    SourceOwnerGeneration, SourceRevisionId, SourceView, UtcTimestamp, blake3_exact_bytes,
};
use search_lexical::{
    AcceptedCodeIdentifiersProfile, CODE_IDENTIFIERS_PROFILE_ID, CodeIdentifierLimits,
    CodeIdentifiersProfile, SparseError, SparseLimits, qualify_code_identifiers_profile,
};
use search_query_planner::{EvidenceOmittedCapabilityV2, PlanError, compile_retrieve_evidence_v2};

struct Issued {
    claims: SearchReadGrantClaims,
    recipes: BTreeSet<RecipeIdV2>,
}

struct Owner {
    claims: SearchReadGrantClaims,
    recipes: BTreeSet<RecipeIdV2>,
    selector: PublicScopeSelector,
    scope: RequestedMembershipScope,
    fence: QuerySnapshotFence,
    projection: CanonicalProjectionMembership,
}

impl CanonicalPopulationVerificationPort for Owner {
    type IssuedGrant = Issued;

    fn verified_issued_claims(
        &mut self,
        issued: &Self::IssuedGrant,
    ) -> Result<SearchReadGrantClaims, AccessError> {
        Ok(issued.claims.clone())
    }

    fn current_projection_binding(
        &mut self,
        issued: &Self::IssuedGrant,
        membership_id: SourceMembershipId,
        fence: &QuerySnapshotFence,
    ) -> Result<CanonicalProjectionMembership, AccessError> {
        if issued.claims != self.claims
            || membership_id != self.projection.source_membership_id
            || fence != &self.fence
        {
            return Err(AccessError::SnapshotStale);
        }
        Ok(self.projection.clone())
    }
}

impl CanonicalEvidencePopulationVerificationPort for Owner {
    fn verified_evidence_recipes(
        &mut self,
        issued: &Self::IssuedGrant,
    ) -> Result<BTreeSet<RecipeIdV2>, AccessError> {
        if issued.claims != self.claims || issued.recipes != self.recipes {
            return Err(AccessError::GrantBindingMismatch);
        }
        Ok(self.recipes.clone())
    }

    fn resolve_evidence_scope(
        &mut self,
        issued: &Self::IssuedGrant,
        selector: &PublicScopeSelector,
        fence: &QuerySnapshotFence,
    ) -> Result<RequestedMembershipScope, AccessError> {
        if issued.claims != self.claims || selector != &self.selector || fence != &self.fence {
            return Err(AccessError::ScopeUnknown);
        }
        Ok(self.scope.clone())
    }
}

struct Fixture {
    claims: SearchReadGrantClaims,
    policy: AuthoritativeGrantPolicy,
    scope: RequestedMembershipScope,
    access: AuthoritativeAccessSnapshot,
    namespace_fence: NamespacePolicyFence,
    policy_state: AuthoritativePolicyState,
    snapshot: QuerySnapshotFence,
    schema: CollectionSchema,
    route: CollectionRoute,
    projection: CanonicalProjectionMembership,
    manifests: BTreeMap<ProjectionMembershipId, ReceiptRef>,
    live: LiveSecurityState,
    request: RetrieveEvidenceRequestV2,
    permissions: BoundedSet<RecipeIdV2, MAX_SET_ITEMS>,
}

impl Fixture {
    #[allow(clippy::too_many_lines)]
    fn new() -> Self {
        let member = SourceMembershipId::from_bytes([1; 16]);
        let incarnation = InstallationIncarnationId::from_bytes([2; 16]);
        let generation = CollectionGenerationId::from_bytes([3; 16]);
        let projection_id = ProjectionMembershipId::from_bytes([4; 16]);
        let access_partition = AccessPartitionId::from_bytes([5; 16]);
        let scoring_partition = ScoringPartitionId::from_bytes([6; 16]);
        let profile_set = ProjectionProfileSetId::new("lexical-v1").expect("profile set");
        let schema = CollectionSchema::initial(
            incarnation,
            generation,
            profile_set.clone(),
            &[NamedVectorRequirement {
                name: VectorName::new("lex_code_v1").expect("vector name"),
                requirement: VectorRequirement::Sparse {
                    index_ceiling: 1024,
                    idf_enabled: true,
                },
            }],
        )
        .expect("indexed schema");
        let projection = CanonicalProjectionMembership {
            source_membership_id: member,
            installation_incarnation_id: incarnation,
            collection_generation_id: generation,
            projection_membership_id: projection_id,
            access_partition_id: access_partition,
            scoring_partition_id: scoring_partition,
            projection_profile_set_id: profile_set,
        };
        let corpus_id = CorpusId::from_bytes([7; 16]);
        let permissions =
            BoundedSet::from_items([RecipeIdV2::RetrieveEvidence]).expect("evidence permission");
        let claims = SearchReadGrantClaims {
            grant_id: GrantId::from_bytes([8; 16]),
            installation_id: InstallationId::from_bytes([9; 16]),
            installation_incarnation_id: incarnation,
            binding_id: BindingId::from_bytes([10; 16]),
            principal_opaque_id: OpaqueId::new("principal").expect("principal"),
            client_scope_ref: OpaqueRef::new("scope").expect("scope"),
            scope_domain_id: ScopeDomainId::from_bytes([11; 16]),
            allowed_membership_ids: BoundedSet::from_items([member]).expect("membership"),
            allowed_corpus_or_portfolio_ids: BoundedSet::from_items([
                search_contracts::CorpusOrPortfolioId::Corpus(corpus_id),
            ])
            .expect("corpus"),
            reference_portfolio_revision: None,
            allowed_access_partitions: BoundedSet::from_items([access_partition])
                .expect("partition"),
            allowed_modalities: BoundedSet::from_items([Modality::Code]).expect("modality"),
            permitted_recipe_families: BoundedSet::empty(),
            maximum_budget_class: ProfileId::new("interactive").expect("budget class"),
            sensitivity_ceiling: SensitivityClass::Confidential,
            disclosure_ceiling: DisclosureCeiling::LocalOnly,
            source_read_permission: true,
            exact_scan_permission: false,
            issued_boot_id: OpaqueId::new("boot").expect("boot"),
            issued_at: UtcTimestamp::parse("2026-10-16T00:00:00.000000Z").expect("issued"),
            expires_at: UtcTimestamp::parse("2026-10-16T00:01:00.000000Z").expect("expiry"),
            nonce: OpaqueId::new("issuer-nonce").expect("nonce"),
            revocation_generation: 1,
        };
        let policy_state = AuthoritativePolicyState {
            namespace_id: SourceNamespaceId::from_bytes([12; 16]),
            owner_generation: SourceOwnerGeneration::from_bytes([13; 32]),
            policy_revision: AccessPolicyRevision::new(1),
            shadow_revision: ShadowFenceRevision::new(1),
            purge_revision: PurgeFenceRevision::new(1),
        };
        let policy = AuthoritativeGrantPolicy {
            binding_id: claims.binding_id,
            binding_generation: 1,
            policy_generation: 1,
            installation_id: claims.installation_id,
            installation_incarnation_id: incarnation,
            principal_opaque_id: claims.principal_opaque_id.clone(),
            client_scope_ref: claims.client_scope_ref.clone(),
            scope_domain_id: claims.scope_domain_id,
            allowed_membership_ids: claims.allowed_membership_ids.clone(),
            allowed_corpus_or_portfolio_ids: claims.allowed_corpus_or_portfolio_ids.clone(),
            reference_portfolio_revision: None,
            allowed_access_partitions: claims.allowed_access_partitions.clone(),
            allowed_modalities: claims.allowed_modalities.clone(),
            permitted_recipe_families: BoundedSet::empty(),
            permitted_evidence_recipes: permissions.clone(),
            allowed_budget_classes: BoundedSet::from_items([claims.maximum_budget_class.clone()])
                .expect("allowed budget"),
            sensitivity_ceiling: claims.sensitivity_ceiling,
            disclosure_ceiling: claims.disclosure_ceiling,
            source_read_permission: true,
            exact_scan_permission: false,
            issued_boot_id: claims.issued_boot_id.clone(),
            revocation_generation: 1,
            maximum_ttl_ms: 60_000,
        };
        let mut snapshot = QuerySnapshotFence {
            installation_incarnation_id: incarnation,
            collection_generation_id: Some(generation),
            visible_epoch: Some(Epoch::new(1).expect("epoch")),
            collection_route_revision: CollectionRouteRevision::new(1),
            catalog_revision: CatalogRevision::new(1),
            membership_revision: MembershipRevision::new(1),
            reference_portfolio_revision: None,
            access_policy_revision: policy_state.policy_revision,
            shadow_fence_revision: policy_state.shadow_revision,
            purge_fence_revision: policy_state.purge_revision,
            overlay_revision: OverlayRevision::new(1),
            observation_cursor_revision: ObservationCursorRevision::new(1),
            observation_freshness: ObservationFreshness {
                state: ObservationFreshnessState::CurrentConfirmed,
                observation_cursor_revision: ObservationCursorRevision::new(1),
                observed_age_ms: None,
            },
            source_view: SourceView::RetainedRevision(SourceRevisionId::from_bytes([14; 16])),
            workspace_view_revision_ref: None,
            lexical_profile_ids: BoundedList::new(vec![
                ProfileId::new(CODE_IDENTIFIERS_PROFILE_ID).expect("lexical profile id"),
            ])
            .expect("lexical profile"),
            snapshot_fingerprint: QuerySnapshotFingerprint::from_bytes([0; 32]),
        };
        seal_snapshot(&mut snapshot);
        let route = CollectionRoute {
            collection_generation_id: generation,
            physical_collection_name: OpaqueRef::new("fixture-collection").expect("collection"),
            schema_identity_digest: *schema.identity().expect("schema identity").as_digest(),
            qualified_qdrant_build: ArtifactDigest::from_bytes([15; 32]),
            committed_visible_epoch: snapshot.visible_epoch.expect("visible epoch"),
            collection_route_revision: snapshot.collection_route_revision,
        };
        let request = RetrieveEvidenceRequestV2 {
            scope: PublicScopeSelector::CorpusRevision {
                corpus_id,
                membership_revision: snapshot.membership_revision,
            },
            source_view: snapshot.source_view.clone(),
            question: search_contracts::BoundedText::new("find the lexical owner")
                .expect("question"),
            intent: EvidenceIntent::LocateImplementation,
            requested_roles: BoundedSet::empty(),
            optional_hints: EvidenceHints::default(),
            profile_id: ProfileId::new("lexical_baseline_v1").expect("request profile"),
            budgets: QueryExecutionBudget {
                priority_class: PriorityClass::Interactive,
                deadline_ms: 1_000,
                max_scoring_legs: 4,
                max_prefetch_candidates_per_leg: 64,
                max_validated_candidates: 32,
                max_source_read_bytes: 1 << 20,
                max_exact_scan_items: 1_000,
                max_exact_scan_bytes: 1 << 20,
                max_materialized_result_bytes: 1 << 20,
                max_cpu_ms: 1_000,
                max_memory_bytes: 1 << 20,
            },
            progressive: true,
        };
        Self {
            claims,
            policy,
            scope: RequestedMembershipScope {
                memberships: BTreeSet::from([member]),
            },
            access: AuthoritativeAccessSnapshot {
                generation: 91,
                source_catalog_generation: 1,
                membership_generation: 1,
                bindings: BTreeMap::from([(
                    member,
                    MembershipAccessBinding {
                        membership_id: member,
                        access_partition_digest: Blake3Digest32::from_bytes([16; 32]),
                        scoring_partition_digest: Blake3Digest32::from_bytes([17; 32]),
                        projection_membership_id: OpaqueId::new("legacy-display-only")
                            .expect("legacy display ID"),
                        active: true,
                    },
                )]),
                snapshot_digest: Blake3Digest32::from_bytes([18; 32]),
            },
            namespace_fence: NamespacePolicyFence {
                namespace_id: policy_state.namespace_id,
                owner_generation: policy_state.owner_generation,
                policy_revision: policy_state.policy_revision,
            },
            policy_state,
            snapshot,
            schema,
            route,
            projection,
            manifests: BTreeMap::from([(
                projection_id,
                ReceiptRef::new("manifest-1").expect("manifest"),
            )]),
            live: LiveSecurityState {
                generation: 217,
                denied_memberships: BTreeSet::new(),
                purged_memberships: BTreeSet::new(),
                fail_closed: false,
                snapshot_digest: Blake3Digest32::from_bytes([19; 32]),
            },
            request,
            permissions,
        }
    }

    fn owner(&self) -> Owner {
        Owner {
            claims: self.claims.clone(),
            recipes: self.permissions.iter().copied().collect(),
            selector: self.request.scope.clone(),
            scope: self.scope.clone(),
            fence: self.snapshot.clone(),
            projection: self.projection.clone(),
        }
    }

    fn compile_population(&self, owner: &mut Owner) -> CompiledEligibilityPopulation {
        let issued = Issued {
            claims: self.claims.clone(),
            recipes: self.permissions.iter().copied().collect(),
        };
        let projection_rows = BTreeMap::from([(
            self.projection.projection_membership_id,
            self.projection.clone(),
        )]);
        let manifests = self.manifests.clone();
        let shadows = BTreeMap::<ProjectionMembershipId, u8>::new();
        compile_canonical_evidence_population(
            owner,
            &issued,
            &CanonicalEvidencePopulationRequest {
                grant: &self.claims,
                grant_policy: &self.policy,
                recipe: EvidencePopulationRecipe {
                    request: &self.request,
                    permitted_recipes: &self.permissions,
                },
                modality: Modality::Code,
                requested_scope: &self.scope,
                authoritative_access: &self.access,
                namespace_policy_fence: &self.namespace_fence,
                authoritative_policy: &self.policy_state,
                snapshot_fence: &self.snapshot,
                collection_schema: &self.schema,
                collection_route: &self.route,
                publication_manifests: &manifests,
                publication_shadows: &shadows,
                projection_memberships: &projection_rows,
                security: CanonicalEligibilitySecuritySnapshot {
                    live: &self.live,
                    namespace_id: self.policy_state.namespace_id,
                    access_policy_revision: self.policy_state.policy_revision,
                    shadow_fence_revision: self.policy_state.shadow_revision,
                    purge_fence_revision: self.policy_state.purge_revision,
                    overlay_revision: self.snapshot.overlay_revision,
                },
            },
        )
        .expect("actual C18 owner-port compiler output")
    }
}

fn seal_snapshot(snapshot: &mut QuerySnapshotFence) {
    let input = snapshot
        .canonical_fingerprint_input()
        .expect("canonical snapshot input");
    let limit = Blake3ExactInputLimit::new(input.len()).expect("snapshot digest limit");
    let digest = blake3_exact_bytes(input.as_slice(), limit).expect("snapshot fingerprint");
    snapshot.snapshot_fingerprint = QuerySnapshotFingerprint::from_bytes(*digest.as_bytes());
}

fn accepted_profile() -> AcceptedCodeIdentifiersProfile {
    let profile = CodeIdentifiersProfile::new().expect("C11 owner profile");
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut checkpoint = || Ok::<(), SparseError>(());
    qualify_code_identifiers_profile(
        profile,
        CodeIdentifierLimits::HARD,
        SparseLimits::BASELINE,
        deadline,
        &mut checkpoint,
    )
    .expect("C11 frozen fixture qualification")
}

#[test]
fn v2_plan_uses_real_c18_output_and_refuses_scope_fence_remap() {
    let fixture = Fixture::new();
    let mut owner = fixture.owner();
    let compiled = fixture.compile_population(&mut owner);
    let request_fence = RequestSecurityFence {
        planned_generation: fixture.access.generation,
        memberships: fixture.scope.memberships.clone(),
    };
    let profile = accepted_profile();
    let plan_id = search_contracts::PlanId::from_bytes([21; 16]);
    let request_id = search_contracts::RequestId::from_bytes([20; 16]);
    let plan = compile_retrieve_evidence_v2(
        plan_id,
        request_id,
        &fixture.request,
        &compiled,
        &fixture.scope,
        &request_fence,
        &profile,
    )
    .expect("C22 lexical plan");

    assert_eq!(plan.plan_id(), plan_id);
    assert_eq!(plan.request_id(), request_id);
    assert_eq!(plan.budget(), fixture.request.budgets);
    assert_eq!(plan.memberships(), &fixture.scope.memberships);
    assert_eq!(
        plan.compiled_plan_digest().as_bytes(),
        plan.identity().digest().as_bytes()
    );
    assert!(std::ptr::eq(plan.population(), compiled.population()));
    assert_eq!(
        plan.population().allowed_projection_memberships(),
        vec![fixture.projection.projection_membership_id]
    );
    assert_eq!(
        plan.lexical_leg().budget().max_candidates(),
        fixture.request.budgets.max_prefetch_candidates_per_leg
    );
    assert_eq!(plan.lexical_leg().leg_id(), 0);
    assert_eq!(
        plan.lexical_leg().leg_kind(),
        search_contracts::LegKind::Lexical
    );
    assert_eq!(
        plan.omitted_capabilities(),
        &[
            EvidenceOmittedCapabilityV2::Structural,
            EvidenceOmittedCapabilityV2::Semantic,
        ]
    );
    assert_eq!(
        plan.lexical_leg().query_vector().indices.len(),
        plan.lexical_leg().query_vector().values.len()
    );
    assert_ne!(*plan.lexical_leg().vector_fingerprint(), [0; 32]);
    let replayed = compile_retrieve_evidence_v2(
        plan_id,
        request_id,
        &fixture.request,
        &compiled,
        &fixture.scope,
        &request_fence,
        &profile,
    )
    .expect("same request and owner data compile again");
    assert_eq!(plan.identity(), replayed.identity());

    let changed_fence = RequestSecurityFence {
        planned_generation: request_fence.planned_generation,
        memberships: BTreeSet::new(),
    };
    assert_eq!(
        compile_retrieve_evidence_v2(
            plan_id,
            request_id,
            &fixture.request,
            &compiled,
            &fixture.scope,
            &changed_fence,
            &profile,
        )
        .err(),
        Some(PlanError::ScopeMismatch)
    );
}

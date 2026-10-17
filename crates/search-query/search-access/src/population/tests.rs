use super::{
    CanonicalEligibilityPopulationRequest, CanonicalEligibilitySecuritySnapshot,
    CanonicalPopulationVerificationPort, CanonicalProjectionMembership,
    CompiledEligibilityPopulation, compile_canonical_eligibility_population,
};
use crate::{
    AccessError, AuthoritativeAccessSnapshot, AuthoritativePolicyState, LiveSecurityState,
    MembershipAccessBinding, NamespacePolicyFence, RequestedMembershipScope,
};
use search_contracts::indexed::{
    CollectionSchema, NamedVectorRequirement, VectorName, VectorRequirement,
};
use search_contracts::{
    AccessPartitionId, AccessPolicyRevision, ArtifactDigest, AuthoritativeGrantPolicy, BindingId,
    Blake3Digest32, Blake3ExactInputLimit, BoundedList, BoundedSet, CatalogRevision,
    CollectionGenerationId, CollectionRoute, CollectionRouteRevision, CorpusOrPortfolioId,
    DisclosureCeiling, Epoch, GrantId, InstallationId, InstallationIncarnationId,
    MembershipRevision, Modality, ObservationCursorRevision, ObservationFreshness,
    ObservationFreshnessState, OpaqueId, OpaqueRef, OverlayRevision, PortfolioRevision, ProfileId,
    ProjectionMembershipId, ProjectionProfileSetId, PurgeFenceRevision, QuerySnapshotFence,
    QuerySnapshotFingerprint, ReceiptRef, RecipeIdV1, ReferencePortfolioId, ScopeDomainId,
    ScoringPartitionId, SearchReadGrantClaims, SensitivityClass, ShadowFenceRevision,
    SourceMembershipId, SourceNamespaceId, SourceOwnerGeneration, SourceRevisionId, SourceView,
    UtcTimestamp, blake3_exact_bytes,
};
use std::collections::{BTreeMap, BTreeSet};

#[path = "tests/evidence.rs"]
mod evidence;

// Private test-only issuer token. Production must adapt the existing Root
// issuer token; there is intentionally no production fixture/receipt issuer.
struct IssuedFixture {
    retained: SearchReadGrantClaims,
}

struct OwnerFixture {
    bindings: BTreeMap<SourceMembershipId, CanonicalProjectionMembership>,
    fence: QuerySnapshotFence,
    binding_reads: usize,
    grant_reads: usize,
}

impl CanonicalPopulationVerificationPort for OwnerFixture {
    type IssuedGrant = IssuedFixture;

    fn verified_issued_claims(
        &mut self,
        issued: &Self::IssuedGrant,
    ) -> Result<SearchReadGrantClaims, AccessError> {
        self.grant_reads += 1;
        Ok(issued.retained.clone())
    }

    fn current_projection_binding(
        &mut self,
        _issued: &Self::IssuedGrant,
        membership_id: SourceMembershipId,
        fence: &QuerySnapshotFence,
    ) -> Result<CanonicalProjectionMembership, AccessError> {
        self.binding_reads += 1;
        if fence != &self.fence {
            return Err(AccessError::SnapshotStale);
        }
        self.bindings
            .get(&membership_id)
            .cloned()
            .ok_or(AccessError::ScopeUnknown)
    }
}

struct Fixture {
    grant: SearchReadGrantClaims,
    policy: AuthoritativeGrantPolicy,
    scope: RequestedMembershipScope,
    access: AuthoritativeAccessSnapshot,
    policy_fence: NamespacePolicyFence,
    policy_state: AuthoritativePolicyState,
    fence: QuerySnapshotFence,
    schema: CollectionSchema,
    route: CollectionRoute,
    rows: BTreeMap<ProjectionMembershipId, CanonicalProjectionMembership>,
    manifests: BTreeMap<ProjectionMembershipId, ReceiptRef>,
    shadows: BTreeMap<ProjectionMembershipId, ()>,
    live: LiveSecurityState,
}

impl Fixture {
    fn new() -> Self {
        let member = SourceMembershipId::from_bytes([1; 16]);
        let incarnation = InstallationIncarnationId::from_bytes([2; 16]);
        let generation = CollectionGenerationId::from_bytes([3; 16]);
        let projection = ProjectionMembershipId::from_bytes([4; 16]);
        let partition = AccessPartitionId::from_bytes([5; 16]);
        let profile = ProjectionProfileSetId::new("lexical-v1").unwrap();
        let schema = CollectionSchema::initial(
            incarnation,
            generation,
            profile.clone(),
            &[NamedVectorRequirement {
                name: VectorName::new("lex_code_v1").unwrap(),
                requirement: VectorRequirement::Sparse {
                    index_ceiling: 1024,
                    idf_enabled: true,
                },
            }],
        )
        .unwrap();
        let row = CanonicalProjectionMembership {
            source_membership_id: member,
            installation_incarnation_id: incarnation,
            collection_generation_id: generation,
            projection_membership_id: projection,
            access_partition_id: partition,
            scoring_partition_id: ScoringPartitionId::from_bytes([6; 16]),
            projection_profile_set_id: profile,
        };
        let grant = SearchReadGrantClaims {
            grant_id: GrantId::from_bytes([7; 16]),
            installation_id: InstallationId::from_bytes([8; 16]),
            installation_incarnation_id: incarnation,
            binding_id: BindingId::from_bytes([9; 16]),
            principal_opaque_id: OpaqueId::new("principal").unwrap(),
            client_scope_ref: OpaqueRef::new("scope").unwrap(),
            scope_domain_id: ScopeDomainId::from_bytes([10; 16]),
            allowed_membership_ids: BoundedSet::from_items([member]).unwrap(),
            allowed_corpus_or_portfolio_ids: BoundedSet::empty(),
            reference_portfolio_revision: None,
            allowed_access_partitions: BoundedSet::from_items([
                partition,
                AccessPartitionId::from_bytes([11; 16]),
            ])
            .unwrap(),
            allowed_modalities: BoundedSet::from_items([Modality::Code]).unwrap(),
            permitted_recipe_families: BoundedSet::from_items([RecipeIdV1::Locate]).unwrap(),
            maximum_budget_class: ProfileId::new("interactive").unwrap(),
            sensitivity_ceiling: SensitivityClass::Confidential,
            disclosure_ceiling: DisclosureCeiling::LocalOnly,
            source_read_permission: true,
            exact_scan_permission: false,
            issued_boot_id: OpaqueId::new("boot").unwrap(),
            issued_at: UtcTimestamp::parse("2026-10-16T00:00:00.000000Z").unwrap(),
            expires_at: UtcTimestamp::parse("2026-10-16T00:01:00.000000Z").unwrap(),
            nonce: OpaqueId::new("retained-nonce-A").unwrap(),
            revocation_generation: 1,
        };
        let policy = AuthoritativeGrantPolicy {
            binding_id: grant.binding_id,
            binding_generation: 1,
            policy_generation: 1,
            installation_id: grant.installation_id,
            installation_incarnation_id: incarnation,
            principal_opaque_id: grant.principal_opaque_id.clone(),
            client_scope_ref: grant.client_scope_ref.clone(),
            scope_domain_id: grant.scope_domain_id,
            allowed_membership_ids: BoundedSet::from_items([
                member,
                SourceMembershipId::from_bytes([12; 16]),
            ])
            .unwrap(),
            allowed_corpus_or_portfolio_ids: grant.allowed_corpus_or_portfolio_ids.clone(),
            reference_portfolio_revision: None,
            allowed_access_partitions: grant.allowed_access_partitions.clone(),
            allowed_modalities: grant.allowed_modalities.clone(),
            permitted_recipe_families: grant.permitted_recipe_families.clone(),
            permitted_evidence_recipes: BoundedSet::empty(),
            allowed_budget_classes: BoundedSet::from_items([grant.maximum_budget_class.clone()])
                .unwrap(),
            sensitivity_ceiling: grant.sensitivity_ceiling,
            disclosure_ceiling: grant.disclosure_ceiling,
            source_read_permission: true,
            exact_scan_permission: false,
            issued_boot_id: grant.issued_boot_id.clone(),
            revocation_generation: 1,
            maximum_ttl_ms: 60_000,
        };
        let policy_state = AuthoritativePolicyState {
            namespace_id: SourceNamespaceId::from_bytes([13; 16]),
            owner_generation: SourceOwnerGeneration::from_bytes([1; 32]),
            policy_revision: AccessPolicyRevision::new(1),
            shadow_revision: ShadowFenceRevision::new(1),
            purge_revision: PurgeFenceRevision::new(1),
        };
        let mut fence = QuerySnapshotFence {
            installation_incarnation_id: incarnation,
            collection_generation_id: Some(generation),
            visible_epoch: Some(Epoch::new(1).unwrap()),
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
            lexical_profile_ids: BoundedList::empty(),
            snapshot_fingerprint: QuerySnapshotFingerprint::from_bytes([0; 32]),
        };
        seal_snapshot(&mut fence);
        let route = CollectionRoute {
            collection_generation_id: generation,
            physical_collection_name: OpaqueRef::new("fixture-collection").unwrap(),
            schema_identity_digest: *schema.identity().unwrap().as_digest(),
            qualified_qdrant_build: ArtifactDigest::from_bytes([1; 32]),
            committed_visible_epoch: fence.visible_epoch.unwrap(),
            collection_route_revision: fence.collection_route_revision,
        };
        Self {
            grant,
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
                        access_partition_digest: Blake3Digest32::from_bytes([7; 32]),
                        scoring_partition_digest: Blake3Digest32::from_bytes([8; 32]),
                        projection_membership_id: OpaqueId::new("legacy-is-not-a-typed-ID")
                            .unwrap(),
                        active: true,
                    },
                )]),
                snapshot_digest: Blake3Digest32::from_bytes([9; 32]),
            },
            policy_fence: NamespacePolicyFence {
                namespace_id: policy_state.namespace_id,
                owner_generation: policy_state.owner_generation,
                policy_revision: policy_state.policy_revision,
            },
            policy_state,
            fence,
            schema,
            route,
            rows: BTreeMap::from([(projection, row)]),
            manifests: BTreeMap::from([(projection, ReceiptRef::new("manifest-X").unwrap())]),
            shadows: BTreeMap::new(),
            live: LiveSecurityState {
                generation: 217,
                denied_memberships: BTreeSet::new(),
                purged_memberships: BTreeSet::new(),
                fail_closed: false,
                snapshot_digest: Blake3Digest32::from_bytes([10; 32]),
            },
        }
    }

    fn issuer(&self) -> IssuedFixture {
        IssuedFixture {
            retained: self.grant.clone(),
        }
    }

    fn owner(&self) -> OwnerFixture {
        OwnerFixture {
            bindings: self
                .rows
                .values()
                .map(|row| (row.source_membership_id, row.clone()))
                .collect(),
            fence: self.fence.clone(),
            binding_reads: 0,
            grant_reads: 0,
        }
    }

    fn compile(
        &self,
        owner: &mut OwnerFixture,
        issued: &IssuedFixture,
    ) -> Result<CompiledEligibilityPopulation, AccessError> {
        compile_canonical_eligibility_population(
            owner,
            issued,
            &CanonicalEligibilityPopulationRequest {
                grant: &self.grant,
                grant_policy: &self.policy,
                recipe: RecipeIdV1::Locate,
                modality: Modality::Code,
                requested_scope: &self.scope,
                authoritative_access: &self.access,
                namespace_policy_fence: &self.policy_fence,
                authoritative_policy: &self.policy_state,
                snapshot_fence: &self.fence,
                collection_schema: &self.schema,
                collection_route: &self.route,
                publication_manifests: &self.manifests,
                publication_shadows: &self.shadows,
                projection_memberships: &self.rows,
                security: CanonicalEligibilitySecuritySnapshot {
                    live: &self.live,
                    namespace_id: self.policy_state.namespace_id,
                    access_policy_revision: self.policy_state.policy_revision,
                    shadow_fence_revision: self.policy_state.shadow_revision,
                    purge_fence_revision: self.policy_state.purge_revision,
                    overlay_revision: self.fence.overlay_revision,
                },
            },
        )
    }
}

fn seal_snapshot(snapshot: &mut QuerySnapshotFence) {
    let input = snapshot.canonical_fingerprint_input().unwrap();
    let digest = blake3_exact_bytes(
        input.as_slice(),
        Blake3ExactInputLimit::new(input.len()).unwrap(),
    )
    .unwrap();
    snapshot.snapshot_fingerprint = QuerySnapshotFingerprint::from_bytes(*digest.as_bytes());
}

#[test]
fn issued_a_cannot_authorize_b_scope_grant_id_or_nonce() {
    for mutation in 0..3 {
        let mut fixture = Fixture::new();
        let issued_a = fixture.issuer();
        let mut owner = fixture.owner();
        match mutation {
            0 => {
                fixture.grant.allowed_membership_ids = fixture.policy.allowed_membership_ids.clone()
            }
            1 => fixture.grant.grant_id = GrantId::from_bytes([99; 16]),
            _ => fixture.grant.nonce = OpaqueId::new("different-nonce-B").unwrap(),
        }
        assert!(matches!(
            fixture.compile(&mut owner, &issued_a),
            Err(AccessError::GrantBindingMismatch)
        ));
        assert_eq!(owner.binding_reads, 0);
    }
}

#[test]
fn authorized_x_cannot_substitute_y_and_another_allowed_partition() {
    let mut fixture = Fixture::new();
    let issued = fixture.issuer();
    let mut owner = fixture.owner();
    let compiled = fixture
        .compile(&mut owner, &issued)
        .expect("valid owner inputs");
    assert_eq!(compiled.snapshot_fence(), &fixture.fence);
    assert_eq!(compiled.grant_fence().grant_id, fixture.grant.grant_id);
    let original_rows = fixture.rows.values().map(|row|
        (row.source_membership_id, row.clone())).collect::<BTreeMap<_, _>>();
    assert_eq!(compiled.projection_memberships(), &original_rows);
    assert!(std::ptr::eq(
        compiled.population(),
        compiled.population().predicate()
    ));
    let mut substituted = fixture.rows.values().next().unwrap().clone();
    substituted.projection_membership_id = ProjectionMembershipId::from_bytes([99; 16]);
    substituted.access_partition_id = AccessPartitionId::from_bytes([11; 16]);
    fixture.manifests.insert(
        substituted.projection_membership_id,
        ReceiptRef::new("manifest-Y").unwrap(),
    );
    fixture.rows = BTreeMap::from([(substituted.projection_membership_id, substituted)]);
    // Caller mutation after compilation cannot substitute the retained owner links.
    assert_eq!(compiled.projection_memberships(), &original_rows);
    assert!(matches!(
        fixture.compile(&mut owner, &issued),
        Err(AccessError::SnapshotStale)
    ));
}

#[test]
fn portfolio_revision_requires_presence_in_policy_grant_and_snapshot() {
    let mut fixture = Fixture::new();
    let revision = PortfolioRevision::new(7);
    fixture.policy.reference_portfolio_revision = Some(revision);
    fixture.policy.allowed_corpus_or_portfolio_ids =
        BoundedSet::from_items([CorpusOrPortfolioId::Portfolio(
            ReferencePortfolioId::from_bytes([20; 16]),
        )])
        .unwrap();
    assert!(matches!(
        fixture.compile(&mut fixture.owner(), &fixture.issuer()),
        Err(AccessError::ScopeUnauthorized)
    ));
    fixture.grant.reference_portfolio_revision = Some(revision);
    assert!(matches!(
        fixture.compile(&mut fixture.owner(), &fixture.issuer()),
        Err(AccessError::ScopeUnauthorized)
    ));
    fixture.fence.reference_portfolio_revision = Some(revision);
    seal_snapshot(&mut fixture.fence);
    fixture
        .compile(&mut fixture.owner(), &fixture.issuer())
        .expect("all portfolio fences present");
}

#[test]
fn retained_revision_keeps_unknown_freshness_but_current_workspace_requires_observation() {
    let mut fixture = Fixture::new();
    fixture.fence.observation_freshness.state = ObservationFreshnessState::Unknown;
    seal_snapshot(&mut fixture.fence);
    let issued = fixture.issuer();
    let mut owner = fixture.owner();
    let compiled = fixture.compile(&mut owner, &issued).expect("exact frozen owner view");
    assert_eq!(compiled.snapshot_fence().source_view, fixture.fence.source_view);
    assert_eq!(compiled.snapshot_fence().observation_freshness.state,
        ObservationFreshnessState::Unknown);
    assert!(owner.binding_reads > 0);

    // A self-consistent caller fence naming another retained revision is still
    // refused by the original owner; Unknown is never a source identity proof.
    fixture.fence.source_view = SourceView::RetainedRevision(SourceRevisionId::from_bytes([99; 16]));
    seal_snapshot(&mut fixture.fence);
    assert!(matches!(fixture.compile(&mut owner, &issued), Err(AccessError::SnapshotStale)));

    let view = search_contracts::WorkspaceViewRevisionId::from_bytes([22; 16]);
    fixture.fence.source_view = SourceView::WorkingTreeCurrent(search_contracts::WorkspaceViewSource {
        workspace_instance_id: search_contracts::WorkspaceId::from_bytes([23; 16]),
        workspace_view_revision_ref: view,
    });
    fixture.fence.workspace_view_revision_ref = Some(view);
    seal_snapshot(&mut fixture.fence);
    let mut current_owner = fixture.owner();
    assert!(matches!(fixture.compile(&mut current_owner, &issued), Err(AccessError::SnapshotStale)));
    assert_eq!(current_owner.binding_reads, 0);
}

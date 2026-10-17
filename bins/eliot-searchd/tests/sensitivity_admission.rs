//! Pure tests of the actual canonical admission and grant template owners.
//! No native root, credential, transport, database or provider is started.

#[allow(dead_code, unused_imports)]
#[path = "../src/access_composition"]
mod access_composition {
    pub mod grant;
    pub mod grant_authority {
        pub mod admission;
    }
}

mod sensitivity_admission {
    use std::cell::Cell;
    use std::collections::{BTreeMap, BTreeSet};

    use search_access::{
        AccessError, AuthoritativeAccessSnapshot, AuthoritativePolicyState, IndexedRouteFence,
        LiveSecurityState, MembershipAccessBinding, NamespacePolicyFence, RequestedMembershipScope,
    };
    use search_contracts::{
        AccessPartitionId, AccessPolicyRevision, BindingId, Blake3Digest32, BoundedList,
        BoundedSet, CollectionGenerationId, CorpusFacetDimension, CorpusProfileRecipe,
        DisclosureCeiling, Epoch, GrantId, InstallationId, InstallationIncarnationId,
        MAX_SET_ITEMS, Modality, OpaqueId, OpaqueRef, OwnerEpoch, ProfileId, PurgeFenceRevision,
        RecipeBodyV1, RecipeIdV1, RequestBody, RequestId, RequestedScope, ScopeDomainId,
        SearchReadGrantClaims, SearchRecipeRequest, SensitivityClass, ShadowFenceRevision,
        SourceIdentityKind, SourceMembershipId, SourceNamespaceId, SourceOwnerGeneration,
        SourceRevisionId, SourceView, UtcTimestamp,
    };
    use search_source_admission::SensitivityLevel;

    use super::access_composition::grant::StandaloneGrantTemplate;
    use super::access_composition::grant_authority::admission::{
        StandaloneAdmissionRequest, StandaloneMembershipSecurity,
        StandaloneMembershipSecuritySnapshot, StandalonePreRetrievalAdmission,
        StandaloneScopeResolver, compile_standalone_pre_retrieval_for_profile,
    };

    fn set<T: Ord>(values: impl IntoIterator<Item = T>) -> BoundedSet<T, MAX_SET_ITEMS> {
        BoundedSet::from_items(values).expect("bounded fixture")
    }

    fn oid(value: &str) -> OpaqueId {
        OpaqueId::new(value).expect("fixture identity")
    }

    struct Resolver(SourceMembershipId);

    impl StandaloneScopeResolver for Resolver {
        fn resolve(&mut self, _: &RequestedScope) -> Result<RequestedMembershipScope, AccessError> {
            Ok(RequestedMembershipScope {
                memberships: BTreeSet::from([self.0]),
            })
        }
    }

    struct Fixture {
        member: SourceMembershipId,
        body: RequestBody,
        template: StandaloneGrantTemplate,
        access: AuthoritativeAccessSnapshot,
        security: StandaloneMembershipSecuritySnapshot,
        planned: NamespacePolicyFence,
        policy: AuthoritativePolicyState,
        route: IndexedRouteFence,
        live: LiveSecurityState,
    }

    impl Fixture {
        fn new() -> Self {
            let member = SourceMembershipId::from_bytes([1; 16]);
            let partition = AccessPartitionId::from_bytes([2; 16]);
            let digest = Blake3Digest32::from_bytes([3; 32]);
            let namespace = SourceNamespaceId::from_bytes([4; 16]);
            let owner = SourceOwnerGeneration::from_bytes([5; 32]);
            let grant = SearchReadGrantClaims {
                grant_id: GrantId::from_bytes([6; 16]),
                installation_id: InstallationId::from_bytes([7; 16]),
                installation_incarnation_id: InstallationIncarnationId::from_bytes([8; 16]),
                binding_id: BindingId::from_bytes([9; 16]),
                principal_opaque_id: oid("principal"),
                client_scope_ref: OpaqueRef::new("client-scope").expect("scope"),
                scope_domain_id: ScopeDomainId::from_bytes([10; 16]),
                allowed_membership_ids: set([member]),
                allowed_corpus_or_portfolio_ids: set([]),
                reference_portfolio_revision: None,
                allowed_access_partitions: set([partition]),
                allowed_modalities: set([Modality::Code]),
                permitted_recipe_families: set([RecipeIdV1::CorpusProfile]),
                maximum_budget_class: ProfileId::new("interactive").expect("budget"),
                sensitivity_ceiling: SensitivityClass::Confidential,
                disclosure_ceiling: DisclosureCeiling::LocalOnly,
                source_read_permission: true,
                exact_scan_permission: false,
                issued_boot_id: oid("boot"),
                issued_at: UtcTimestamp::parse("2026-10-16T00:00:00.000000Z").expect("issued"),
                expires_at: UtcTimestamp::parse("2026-10-16T00:01:00.000000Z").expect("expiry"),
                nonce: oid("nonce"),
                revocation_generation: 1,
            };
            let template = StandaloneGrantTemplate {
                operation_id: oid("grant-operation"),
                binding_id: grant.binding_id,
                binding_generation: 1,
                policy_generation: 1,
                installation_id: grant.installation_id,
                installation_incarnation_id: grant.installation_incarnation_id,
                principal_opaque_id: grant.principal_opaque_id.clone(),
                client_scope_ref: grant.client_scope_ref.clone(),
                scope_domain_id: grant.scope_domain_id,
                allowed_membership_ids: grant.allowed_membership_ids.clone(),
                allowed_corpus_or_portfolio_ids: grant.allowed_corpus_or_portfolio_ids.clone(),
                reference_portfolio_revision: grant.reference_portfolio_revision,
                allowed_access_partitions: grant.allowed_access_partitions.clone(),
                allowed_modalities: grant.allowed_modalities.clone(),
                permitted_recipe_families: grant.permitted_recipe_families.clone(),
                permitted_evidence_recipes: BoundedSet::empty(),
                maximum_budget_class: grant.maximum_budget_class.clone(),
                sensitivity_ceiling: grant.sensitivity_ceiling,
                disclosure_ceiling: grant.disclosure_ceiling,
                source_read_permission: grant.source_read_permission,
                exact_scan_permission: grant.exact_scan_permission,
                issued_boot_id: grant.issued_boot_id.clone(),
                revocation_generation: grant.revocation_generation,
                requested_ttl_ms: 60_000,
            };
            let body = RequestBody {
                recipe_request: SearchRecipeRequest::new(
                    RequestId::from_bytes([11; 16]),
                    RecipeIdV1::CorpusProfile,
                    SourceView::RetainedRevision(SourceRevisionId::from_bytes([12; 16])),
                    RequestedScope::ExplicitMemberships(
                        BoundedList::new(vec![member]).expect("scope"),
                    ),
                    grant.maximum_budget_class.clone(),
                    RecipeBodyV1::CorpusProfile(CorpusProfileRecipe {
                        facets: set([CorpusFacetDimension::Role]),
                    }),
                )
                .expect("recipe"),
                grant,
            };
            Self {
                member,
                body,
                template,
                access: AuthoritativeAccessSnapshot {
                    generation: 7,
                    source_catalog_generation: 3,
                    membership_generation: 5,
                    snapshot_digest: digest,
                    bindings: BTreeMap::from([(
                        member,
                        MembershipAccessBinding {
                            membership_id: member,
                            access_partition_digest: digest,
                            scoring_partition_digest: digest,
                            projection_membership_id: oid("projection-member"),
                            active: true,
                        },
                    )]),
                },
                security: StandaloneMembershipSecuritySnapshot {
                    access_snapshot_digest: digest,
                    memberships: BTreeMap::from([(
                        member,
                        StandaloneMembershipSecurity {
                            access_partition_id: partition,
                            access_partition_digest: digest,
                            modality: Modality::Code,
                            sensitivity: SensitivityClass::Confidential,
                        },
                    )]),
                },
                planned: NamespacePolicyFence {
                    namespace_id: namespace,
                    owner_generation: owner,
                    policy_revision: AccessPolicyRevision::new(1),
                },
                policy: AuthoritativePolicyState {
                    namespace_id: namespace,
                    owner_generation: owner,
                    policy_revision: AccessPolicyRevision::new(1),
                    shadow_revision: ShadowFenceRevision::new(1),
                    purge_revision: PurgeFenceRevision::new(1),
                },
                route: IndexedRouteFence {
                    collection_generation_id: CollectionGenerationId::from_bytes([13; 16]),
                    visible_epoch: Epoch::new(1).expect("epoch"),
                    route_generation: 1,
                    owner_epoch: OwnerEpoch::new(1).expect("owner"),
                },
                live: LiveSecurityState {
                    generation: 7,
                    denied_memberships: BTreeSet::new(),
                    purged_memberships: BTreeSet::new(),
                    fail_closed: false,
                    snapshot_digest: digest,
                },
            }
        }

        fn ceiling(&mut self, ceiling: SensitivityClass) {
            self.template.sensitivity_ceiling = ceiling;
            self.body.grant.sensitivity_ceiling = ceiling;
        }

        fn admit(&self, max_legs: usize) -> Result<StandalonePreRetrievalAdmission, AccessError> {
            compile_standalone_pre_retrieval_for_profile(
                &mut Resolver(self.member),
                StandaloneAdmissionRequest {
                    body: &self.body,
                    template: &self.template,
                    authoritative_access: &self.access,
                    membership_security: &self.security,
                    requested_policy: &self.planned,
                    authoritative_policy: &self.policy,
                    route: self.route,
                    live: &self.live,
                    overlap_proof: None,
                    max_legs,
                },
                Blake3Digest32::from_bytes([14; 32]),
            )
        }
    }

    #[test]
    fn public_grant_refuses_confidential_source_before_planning_and_dispatch() {
        let mut fixture = Fixture::new();
        fixture.ceiling(SensitivityClass::Public);
        let dispatched = Cell::new(false);
        // A zero leg budget would fail later in planning. The sensitivity
        // refusal must win before that stage or any scoring/count dispatch.
        let result = fixture.admit(0).inspect(|_| dispatched.set(true));
        assert_eq!(
            result.expect_err("sensitivity refusal"),
            AccessError::ScopeUnauthorized
        );
        assert!(!dispatched.get());
    }

    #[test]
    fn matching_ceiling_admits_the_same_member_partition_and_modality() {
        let fixture = Fixture::new();
        let admission = fixture.admit(1).expect("matching Confidential ceiling");
        assert_eq!(admission.scope.memberships.len(), 1);
        assert_eq!(admission.legs.len(), 1);
        assert!(admission.scope.memberships.contains_key(&fixture.member));
    }

    #[test]
    fn unknown_classification_cannot_default_to_public() {
        let mut fixture = Fixture::new();
        fixture.ceiling(SensitivityClass::Public);
        fixture.security.memberships.clear();
        assert_eq!(
            fixture.admit(1).expect_err("classification missing"),
            AccessError::SnapshotStale
        );
    }

    #[test]
    fn c03_internal_maps_to_project_and_uses_the_grant_order() {
        let mut fixture = Fixture::new();
        let metadata = fixture
            .security
            .memberships
            .get_mut(&fixture.member)
            .expect("member");
        metadata.sensitivity = SensitivityClass::Project;
        metadata
            .require_source_sensitivity(
                SourceIdentityKind::GitBlobLineage,
                SensitivityLevel::Internal,
            )
            .expect("Internal is project-internal");
        fixture.ceiling(SensitivityClass::Public);
        assert_eq!(
            fixture.admit(1).expect_err("Public excludes Internal"),
            AccessError::ScopeUnauthorized
        );
        fixture.ceiling(SensitivityClass::Project);
        assert!(fixture.admit(1).is_ok());
    }

    #[test]
    fn ntfs_retained_copy_uses_confidential_minimum_even_for_public_decision() {
        let fixture = Fixture::new();
        let metadata = fixture.security.memberships[&fixture.member];
        for decision in [
            SensitivityLevel::Public,
            SensitivityLevel::Internal,
            SensitivityLevel::Confidential,
        ] {
            metadata
                .require_source_sensitivity(SourceIdentityKind::NtfsFile, decision)
                .expect("#598 retained copies are Confidential");
        }
        let lowered = StandaloneMembershipSecurity {
            sensitivity: SensitivityClass::Public,
            ..metadata
        };
        assert_eq!(
            lowered
                .require_source_sensitivity(SourceIdentityKind::NtfsFile, SensitivityLevel::Public),
            Err(AccessError::SnapshotStale)
        );
    }

    #[test]
    fn secret_candidate_is_never_lowered_and_unconditional_denials_stay_denied() {
        let mut fixture = Fixture::new();
        let metadata = fixture
            .security
            .memberships
            .get_mut(&fixture.member)
            .expect("member");
        assert_eq!(
            metadata.require_source_sensitivity(
                SourceIdentityKind::NtfsFile,
                SensitivityLevel::SecretCandidate
            ),
            Err(AccessError::SnapshotStale)
        );
        metadata.sensitivity = SensitivityClass::SecretCandidate;
        metadata
            .require_source_sensitivity(
                SourceIdentityKind::NtfsFile,
                SensitivityLevel::SecretCandidate,
            )
            .expect("preserve SecretCandidate");
        for decision in [SensitivityLevel::Credential, SensitivityLevel::PrivateKey] {
            assert_eq!(
                metadata.require_source_sensitivity(SourceIdentityKind::NtfsFile, decision),
                Err(AccessError::ScopeUnauthorized)
            );
        }
        assert_eq!(
            fixture
                .admit(1)
                .expect_err("Confidential excludes SecretCandidate"),
            AccessError::ScopeUnauthorized
        );
    }
}

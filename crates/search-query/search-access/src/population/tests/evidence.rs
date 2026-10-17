//! Focused private-issuer V2 cases; no Native, source bytes or vendor client.

use super::*;
use crate::{
    CanonicalEvidencePopulationVerificationPort, EvidencePopulationRecipe,
    compile_canonical_evidence_population,
};
use search_contracts::evidence::{
    EvidenceHints, EvidenceIntent, PublicScopeSelector, RecipeIdV2, RetrieveEvidenceRequestV2,
};
use search_contracts::{BoundedText, CorpusId, MAX_SET_ITEMS, PriorityClass, QueryExecutionBudget};

struct Issued {
    common: IssuedFixture,
    recipes: BTreeSet<RecipeIdV2>,
}
struct Owner {
    common: OwnerFixture,
    selector: PublicScopeSelector,
    scope: RequestedMembershipScope,
    recipe_reads: usize,
    scope_reads: usize,
    drift_recipes: bool,
    drift_scope: bool,
}
impl CanonicalPopulationVerificationPort for Owner {
    type IssuedGrant = Issued;
    fn verified_issued_claims(
        &mut self,
        issued: &Issued,
    ) -> Result<SearchReadGrantClaims, AccessError> {
        self.common.verified_issued_claims(&issued.common)
    }
    fn current_projection_binding(
        &mut self,
        issued: &Issued,
        member: SourceMembershipId,
        fence: &QuerySnapshotFence,
    ) -> Result<CanonicalProjectionMembership, AccessError> {
        self.common
            .current_projection_binding(&issued.common, member, fence)
    }
}
impl CanonicalEvidencePopulationVerificationPort for Owner {
    fn verified_evidence_recipes(
        &mut self,
        issued: &Issued,
    ) -> Result<BTreeSet<RecipeIdV2>, AccessError> {
        self.recipe_reads += 1;
        if self.drift_recipes && self.recipe_reads > 1 {
            return Ok(BTreeSet::new());
        }
        Ok(issued.recipes.clone())
    }
    fn resolve_evidence_scope(
        &mut self,
        _: &Issued,
        selector: &PublicScopeSelector,
        fence: &QuerySnapshotFence,
    ) -> Result<RequestedMembershipScope, AccessError> {
        self.scope_reads += 1;
        if selector != &self.selector || fence != &self.common.fence {
            return Err(AccessError::ScopeUnknown);
        }
        if self.drift_scope && self.scope_reads > 1 {
            return Ok(RequestedMembershipScope {
                memberships: BTreeSet::new(),
            });
        }
        Ok(self.scope.clone())
    }
}

fn setup() -> (
    Fixture,
    RetrieveEvidenceRequestV2,
    BoundedSet<RecipeIdV2, MAX_SET_ITEMS>,
    Issued,
    Owner,
) {
    let mut fixture = Fixture::new();
    let corpus = CorpusId::from_bytes([77; 16]);
    fixture.grant.permitted_recipe_families = BoundedSet::empty();
    fixture.policy.permitted_recipe_families = BoundedSet::empty();
    fixture.grant.allowed_corpus_or_portfolio_ids =
        BoundedSet::from_items([CorpusOrPortfolioId::Corpus(corpus)]).unwrap();
    fixture.policy.allowed_corpus_or_portfolio_ids =
        fixture.grant.allowed_corpus_or_portfolio_ids.clone();
    let permissions = BoundedSet::from_items([RecipeIdV2::RetrieveEvidence]).unwrap();
    fixture.policy.permitted_evidence_recipes = permissions.clone();
    let request = RetrieveEvidenceRequestV2 {
        scope: PublicScopeSelector::CorpusRevision {
            corpus_id: corpus,
            membership_revision: fixture.fence.membership_revision,
        },
        source_view: fixture.fence.source_view.clone(),
        question: BoundedText::new("where is the index owner").unwrap(),
        intent: EvidenceIntent::LocateImplementation,
        requested_roles: BoundedSet::empty(),
        optional_hints: EvidenceHints::default(),
        profile_id: ProfileId::new("lexical_baseline_v1").unwrap(),
        budgets: QueryExecutionBudget {
            priority_class: PriorityClass::Interactive,
            deadline_ms: 1000,
            max_scoring_legs: 4,
            max_prefetch_candidates_per_leg: 64,
            max_validated_candidates: 32,
            max_source_read_bytes: 1 << 20,
            max_exact_scan_items: 1000,
            max_exact_scan_bytes: 1 << 20,
            max_materialized_result_bytes: 1 << 20,
            max_cpu_ms: 1000,
            max_memory_bytes: 1 << 20,
        },
        progressive: false,
    };
    let issued = Issued {
        common: fixture.issuer(),
        recipes: permissions.iter().copied().collect(),
    };
    let owner = Owner {
        common: fixture.owner(),
        selector: request.scope.clone(),
        scope: fixture.scope.clone(),
        recipe_reads: 0,
        scope_reads: 0,
        drift_recipes: false,
        drift_scope: false,
    };
    (fixture, request, permissions, issued, owner)
}

fn compile(
    f: &Fixture,
    body: &RetrieveEvidenceRequestV2,
    recipes: &BoundedSet<RecipeIdV2, MAX_SET_ITEMS>,
    issued: &Issued,
    owner: &mut Owner,
) -> Result<CompiledEligibilityPopulation, AccessError> {
    compile_canonical_evidence_population(
        owner,
        issued,
        &CanonicalEligibilityPopulationRequest {
            grant: &f.grant,
            grant_policy: &f.policy,
            recipe: EvidencePopulationRecipe {
                request: body,
                permitted_recipes: recipes,
            },
            modality: Modality::Code,
            requested_scope: &f.scope,
            authoritative_access: &f.access,
            namespace_policy_fence: &f.policy_fence,
            authoritative_policy: &f.policy_state,
            snapshot_fence: &f.fence,
            collection_schema: &f.schema,
            collection_route: &f.route,
            publication_manifests: &f.manifests,
            publication_shadows: &f.shadows,
            projection_memberships: &f.rows,
            security: CanonicalEligibilitySecuritySnapshot {
                live: &f.live,
                namespace_id: f.policy_state.namespace_id,
                access_policy_revision: f.policy_state.policy_revision,
                shadow_fence_revision: f.policy_state.shadow_revision,
                purge_fence_revision: f.policy_state.purge_revision,
                overlay_revision: f.fence.overlay_revision,
            },
        },
    )
}

#[test]
fn evidence_v2_without_v1_permission_keeps_the_same_canonical_population() {
    let baseline = Fixture::new();
    let expected = baseline
        .compile(&mut baseline.owner(), &baseline.issuer())
        .unwrap();
    let (fixture, body, recipes, issued, mut owner) = setup();
    let actual = compile(&fixture, &body, &recipes, &issued, &mut owner).unwrap();
    assert_eq!(actual.population(), expected.population());
    assert_eq!(
        actual.projection_memberships(),
        expected.projection_memberships()
    );
    assert_eq!(actual.snapshot_fence(), expected.snapshot_fence());
    assert_eq!(owner.recipe_reads, 2);
    assert_eq!(owner.scope_reads, 2);
    assert_eq!(
        fixture
            .compile(&mut fixture.owner(), &fixture.issuer())
            .err(),
        Some(AccessError::ScopeUnauthorized)
    );
}

#[test]
fn evidence_v2_rejects_unissued_permissions_scope_revisions_and_live_restrictions() {
    for case in 0..7 {
        let (mut f, mut body, mut recipes, issued, mut owner) = setup();
        let expected = match case {
            0 => {
                recipes = BoundedSet::from_items([RecipeIdV2::OrientScope]).unwrap();
                AccessError::GrantBindingMismatch
            }
            1 => {
                f.policy.permitted_evidence_recipes = BoundedSet::empty();
                AccessError::ScopeUnauthorized
            }
            2 => {
                body.scope = PublicScopeSelector::CorpusRevision {
                    corpus_id: CorpusId::from_bytes([99; 16]),
                    membership_revision: f.fence.membership_revision,
                };
                AccessError::ScopeUnauthorized
            }
            3 => {
                body.scope = PublicScopeSelector::CorpusRevision {
                    corpus_id: CorpusId::from_bytes([77; 16]),
                    membership_revision: MembershipRevision::new(99),
                };
                AccessError::SnapshotStale
            }
            4 => {
                owner.scope.memberships.clear();
                AccessError::ScopeUnauthorized
            }
            5 => {
                f.live.denied_memberships = f.scope.memberships.clone();
                AccessError::LiveRevocation
            }
            _ => {
                f.shadows.insert(*f.rows.keys().next().unwrap(), ());
                AccessError::LiveShadowed
            }
        };
        assert_eq!(
            compile(&f, &body, &recipes, &issued, &mut owner).err(),
            Some(expected),
            "case {case}"
        );
        assert_eq!(owner.common.binding_reads, 0, "case {case}");
    }
}

#[test]
fn evidence_v2_owner_drift_during_compilation_returns_no_private_proof() {
    for scope in [false, true] {
        let (fixture, body, recipes, issued, mut owner) = setup();
        owner.drift_recipes = !scope;
        owner.drift_scope = scope;
        let expected = if scope {
            AccessError::ScopeUnauthorized
        } else {
            AccessError::GrantBindingMismatch
        };
        assert_eq!(
            compile(&fixture, &body, &recipes, &issued, &mut owner).err(),
            Some(expected)
        );
        assert_eq!(owner.common.binding_reads, 1);
    }
}

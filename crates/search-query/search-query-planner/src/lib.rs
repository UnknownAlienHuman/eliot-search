//! Deterministic server-owned query planning for the eleven v1 recipes.
//!
//! Clients select a versioned recipe and bounded preferences. They cannot
//! provide Qdrant collections, filters, point IDs, execution graphs, or access
//! predicates. Planning after captured inputs is pure and retry-safe.

#![forbid(unsafe_code)]
#![allow(
    clippy::large_enum_variant,
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::too_many_arguments,
    clippy::too_many_lines
)]

use core::fmt;
use std::collections::BTreeSet;

use search_access::{AccessModality, AuthorizedScope, SafeRetrievalLeg, ValidatedGrant};
use search_contracts::{
    Blake3ExactInputLimit, LegKind, QueryExecutionBudget, QuerySnapshotFence, RecipeBodyV1,
    RecipeIdV1, RequestId, SearchReadGrantClaims, SearchRecipeRequest, SearchTaskPlan,
    SourceMembershipId, blake3_exact_bytes,
};

/// Closed query-planning failure.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PlanError {
    RecipeBodyMismatch,
    RecipeDenied,
    BudgetClassDenied,
    InvalidSnapshot,
    SnapshotExpired,
    CapabilityUnavailable,
    NoExecutableLegs,
    TooManyLegs,
    BudgetExceeded,
    InvalidDependencyGraph,
    StrictCurrentnessUnavailable,
    IdentityEncoding,
    CanonicalPlanInvalid,
    RequestIdentityMismatch,
    RequestDigestMismatch,
    GrantFenceMismatch,
    ScopeFenceMismatch,
    ScopeMismatch,
    PlanFingerprintMismatch,
}

impl PlanError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::RecipeBodyMismatch => "PLAN_RECIPE_BODY_MISMATCH",
            Self::RecipeDenied => "PLAN_RECIPE_DENIED",
            Self::BudgetClassDenied => "PLAN_BUDGET_CLASS_DENIED",
            Self::InvalidSnapshot => "PLAN_SNAPSHOT_INVALID",
            Self::SnapshotExpired => "PLAN_SNAPSHOT_EXPIRED",
            Self::CapabilityUnavailable => "PLAN_CAPABILITY_UNAVAILABLE",
            Self::NoExecutableLegs => "PLAN_NO_EXECUTABLE_LEGS",
            Self::TooManyLegs => "PLAN_TOO_MANY_LEGS",
            Self::BudgetExceeded => "PLAN_BUDGET_EXCEEDED",
            Self::InvalidDependencyGraph => "PLAN_DEPENDENCY_GRAPH_INVALID",
            Self::StrictCurrentnessUnavailable => "PLAN_CURRENTNESS_UNAVAILABLE",
            Self::IdentityEncoding => "PLAN_IDENTITY_ENCODING_FAILED",
            Self::CanonicalPlanInvalid => "PLAN_CANONICAL_PLAN_INVALID",
            Self::RequestIdentityMismatch => "PLAN_REQUEST_IDENTITY_MISMATCH",
            Self::RequestDigestMismatch => "PLAN_REQUEST_DIGEST_MISMATCH",
            Self::GrantFenceMismatch => "PLAN_GRANT_FENCE_MISMATCH",
            Self::ScopeFenceMismatch => "PLAN_SCOPE_FENCE_MISMATCH",
            Self::ScopeMismatch => "PLAN_SCOPE_MISMATCH",
            Self::PlanFingerprintMismatch => "PLAN_FINGERPRINT_MISMATCH",
        }
    }
}

impl fmt::Display for PlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for PlanError {}

/// Recipe request after exact family/body validation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NormalizedRecipeRequest(SearchRecipeRequest);

impl NormalizedRecipeRequest {
    #[must_use]
    pub const fn request(&self) -> &SearchRecipeRequest {
        &self.0
    }
}

/// Normalizes exactly one closed v1 recipe request.
pub fn normalize_recipe(
    request: SearchRecipeRequest,
) -> Result<NormalizedRecipeRequest, PlanError> {
    if request.body.recipe_id() != request.recipe {
        return Err(PlanError::RecipeBodyMismatch);
    }
    validate_recipe_specific_bounds(&request.body)?;
    Ok(NormalizedRecipeRequest(request))
}

/// Accepted runtime capability set used only by the server planner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetrievalCapabilities {
    pub direct: bool,
    pub lexical: bool,
    pub exact: bool,
}

/// Advanced runtime capabilities used only by the server planner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdvancedCapabilities {
    pub structural: bool,
    pub semantic: bool,
    pub rerank: bool,
}

/// Accepted runtime capability set used only by the server planner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapabilitySet {
    pub retrieval: RetrievalCapabilities,
    pub advanced: AdvancedCapabilities,
}

/// One finite plan-leg budget.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LegBudget {
    pub deadline_ms: u64,
    pub max_candidates: u32,
    pub max_source_read_bytes: u64,
    pub max_cpu_ms: u64,
    pub max_memory_bytes: u64,
}

/// One typed executable plan leg.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlannedLeg {
    pub leg_id: usize,
    pub leg_kind: LegKind,
    pub depends_on: Vec<usize>,
    pub memberships: BTreeSet<SourceMembershipId>,
    pub safe_index_leg: Option<SafeRetrievalLeg>,
    pub budget: LegBudget,
    pub cancellation_boundary: CancellationBoundary,
}

/// Where executor cancellation must be observed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CancellationBoundary {
    BeforeDispatch,
    BetweenPages,
    BetweenSourceReads,
    BeforeEmission,
}

/// Truthful capability omitted from the plan.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum OmittedCapability {
    Lexical,
    Exact,
    Structural,
    Semantic,
    Rerank,
}

/// Frozen package-local deterministic plan digest.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct CompiledPlanDigest(pub [u8; 32]);

/// Finite deterministic executable task plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompiledSearchPlan {
    pub request_id: RequestId,
    pub recipe: RecipeIdV1,
    pub snapshot: QuerySnapshotFence,
    pub authorized_scope: AuthorizedScope,
    pub global_budget: QueryExecutionBudget,
    pub legs: Vec<PlannedLeg>,
    pub omitted_capabilities: BTreeSet<OmittedCapability>,
    pub digest: CompiledPlanDigest,
}

/// Compiles a finite typed leg DAG.
pub fn compile_plan(
    recipe: &NormalizedRecipeRequest,
    grant: &ValidatedGrant,
    authorized_scope: AuthorizedScope,
    snapshot: QuerySnapshotFence,
    capabilities: CapabilitySet,
    global_budget: QueryExecutionBudget,
    safe_index_legs: &[SafeRetrievalLeg],
) -> Result<CompiledSearchPlan, PlanError> {
    let request = recipe.request();
    if !grant.permits_recipe(request.recipe) {
        return Err(PlanError::RecipeDenied);
    }
    if !grant
        .claims()
        .allowed_budget_classes
        .iter()
        .any(|candidate| candidate.as_str() == request.requested_budget_class.as_str())
    {
        return Err(PlanError::BudgetClassDenied);
    }
    compile_plan_inner(
        recipe,
        authorized_scope,
        snapshot,
        capabilities,
        global_budget,
        safe_index_legs,
        |modality| grant.permits_modality(modality),
    )
}

/// Compiles the executor graph against an already admitted canonical task plan.
///
/// The native serving owner must authenticate and check grant lifetime and source authorization
/// before calling this pure planner. This function verifies the request, recipe/budget grant
/// fields, exact plan/grant/scope fences, canonical fingerprints, and safe-leg closure. The
/// contract's `Modality` describes source content classes, so it is deliberately not translated
/// into the legacy retrieval `AccessModality` values here. The returned legacy graph digest is
/// retained for existing consumers; callers use `task_plan.plan_fingerprint` as plan identity.
pub fn compile_canonical_plan(
    task_plan: &SearchTaskPlan,
    grant: &SearchReadGrantClaims,
    recipe: &NormalizedRecipeRequest,
    authorized_scope: AuthorizedScope,
    capabilities: CapabilitySet,
    safe_index_legs: &[SafeRetrievalLeg],
) -> Result<CompiledSearchPlan, PlanError> {
    let request = recipe.request();
    task_plan
        .validate()
        .map_err(|_| PlanError::CanonicalPlanInvalid)?;
    if task_plan.request_id != request.request_id {
        return Err(PlanError::RequestIdentityMismatch);
    }
    if canonical_request_digest(request)? != task_plan.recipe_request_digest {
        return Err(PlanError::RequestDigestMismatch);
    }
    if !grant.permitted_recipe_families.contains(&request.recipe) {
        return Err(PlanError::RecipeDenied);
    }
    if grant.maximum_budget_class != request.requested_budget_class {
        return Err(PlanError::BudgetClassDenied);
    }
    if task_plan.grant_fence.grant_id != grant.grant_id
        || task_plan.grant_fence.revocation_generation != grant.revocation_generation
    {
        return Err(PlanError::GrantFenceMismatch);
    }
    if task_plan.client_scope_fence.client_scope_ref != grant.client_scope_ref
        || task_plan.client_scope_fence.scope_domain_id != grant.scope_domain_id
    {
        return Err(PlanError::ScopeFenceMismatch);
    }
    validate_canonical_scope(
        task_plan,
        grant,
        request,
        &authorized_scope,
        safe_index_legs,
    )?;
    verify_canonical_snapshot(&task_plan.query_snapshot_fence)?;
    verify_canonical_plan(task_plan)?;

    compile_plan_inner(
        recipe,
        authorized_scope,
        task_plan.query_snapshot_fence.clone(),
        capabilities,
        task_plan.query_execution_budget,
        safe_index_legs,
        |_| true,
    )
}

fn compile_plan_inner<F>(
    recipe: &NormalizedRecipeRequest,
    authorized_scope: AuthorizedScope,
    snapshot: QuerySnapshotFence,
    capabilities: CapabilitySet,
    global_budget: QueryExecutionBudget,
    safe_index_legs: &[SafeRetrievalLeg],
    modality_allowed: F,
) -> Result<CompiledSearchPlan, PlanError>
where
    F: Fn(AccessModality) -> bool,
{
    let request = recipe.request();
    snapshot
        .validate()
        .map_err(|_| PlanError::InvalidSnapshot)?;
    validate_global_budget(global_budget)?;

    let modalities = recipe_modalities(request.recipe);
    let mut drafts = Vec::new();
    let mut omitted = BTreeSet::new();
    let all_memberships = authorized_scope
        .memberships
        .keys()
        .copied()
        .collect::<BTreeSet<_>>();

    if modalities.retrieval.direct
        && capabilities.retrieval.direct
        && modality_allowed(AccessModality::Direct)
    {
        drafts.push(LegDraft {
            kind: LegKind::Direct,
            memberships: all_memberships.clone(),
            safe_index_leg: None,
            dependency_role: DependencyRole::Root,
        });
    }

    if modalities.retrieval.lexical {
        if capabilities.retrieval.lexical && modality_allowed(AccessModality::Lexical) {
            if safe_index_legs.is_empty() {
                omitted.insert(OmittedCapability::Lexical);
            } else {
                for safe_leg in safe_index_legs {
                    drafts.push(LegDraft {
                        kind: LegKind::Lexical,
                        memberships: safe_leg.memberships.clone(),
                        safe_index_leg: Some(safe_leg.clone()),
                        dependency_role: DependencyRole::Root,
                    });
                }
            }
        } else {
            omitted.insert(OmittedCapability::Lexical);
        }
    }

    if modalities.retrieval.exact {
        if capabilities.retrieval.exact && modality_allowed(AccessModality::Exact) {
            drafts.push(LegDraft {
                kind: LegKind::Exact,
                memberships: all_memberships.clone(),
                safe_index_leg: None,
                dependency_role: DependencyRole::Root,
            });
        } else {
            omitted.insert(OmittedCapability::Exact);
        }
    }

    if modalities.advanced.structural {
        if capabilities.advanced.structural && modality_allowed(AccessModality::Code) {
            drafts.push(LegDraft {
                kind: LegKind::Structural,
                memberships: all_memberships.clone(),
                safe_index_leg: None,
                dependency_role: DependencyRole::Root,
            });
        } else {
            omitted.insert(OmittedCapability::Structural);
        }
    }

    if modalities.advanced.semantic {
        if capabilities.advanced.semantic && modality_allowed(AccessModality::Semantic) {
            drafts.push(LegDraft {
                kind: LegKind::Semantic,
                memberships: all_memberships.clone(),
                safe_index_leg: None,
                dependency_role: DependencyRole::Root,
            });
        } else {
            omitted.insert(OmittedCapability::Semantic);
        }
    }

    if modalities.advanced.rerank {
        if capabilities.advanced.rerank {
            drafts.push(LegDraft {
                kind: LegKind::Rerank,
                memberships: all_memberships,
                safe_index_leg: None,
                dependency_role: DependencyRole::AllPrevious,
            });
        } else {
            omitted.insert(OmittedCapability::Rerank);
        }
    }

    if drafts.is_empty() {
        return Err(PlanError::NoExecutableLegs);
    }
    if drafts.len()
        > usize::try_from(global_budget.max_scoring_legs).map_err(|_| PlanError::TooManyLegs)?
    {
        return Err(PlanError::TooManyLegs);
    }

    let budgets = allocate_leg_budgets(global_budget, drafts.len())?;
    let mut legs = Vec::with_capacity(drafts.len());
    for (leg_id, (draft, budget)) in drafts.into_iter().zip(budgets).enumerate() {
        let depends_on = match draft.dependency_role {
            DependencyRole::Root => Vec::new(),
            DependencyRole::AllPrevious => (0..leg_id).collect(),
        };
        legs.push(PlannedLeg {
            leg_id,
            leg_kind: draft.kind,
            depends_on,
            memberships: draft.memberships,
            safe_index_leg: draft.safe_index_leg,
            budget,
            cancellation_boundary: cancellation_boundary(draft.kind),
        });
    }
    validate_dag(&legs)?;
    let digest = fingerprint_plan(
        request.request_id,
        request.recipe,
        &snapshot,
        &authorized_scope,
        global_budget,
        &legs,
    );
    Ok(CompiledSearchPlan {
        request_id: request.request_id,
        recipe: request.recipe,
        snapshot,
        authorized_scope,
        global_budget,
        legs,
        omitted_capabilities: omitted,
        digest,
    })
}

fn canonical_request_digest(
    request: &SearchRecipeRequest,
) -> Result<search_contracts::Blake3Digest32, PlanError> {
    request
        .canonical_request_digest()
        .map_err(|_| PlanError::IdentityEncoding)
}

fn verify_canonical_snapshot(snapshot: &QuerySnapshotFence) -> Result<(), PlanError> {
    let input = snapshot
        .canonical_fingerprint_input()
        .map_err(|_| PlanError::InvalidSnapshot)?;
    let digest = canonical_preimage_digest(input.as_slice())?;
    if digest.as_bytes() != snapshot.snapshot_fingerprint.as_bytes() {
        return Err(PlanError::InvalidSnapshot);
    }
    Ok(())
}

fn verify_canonical_plan(plan: &SearchTaskPlan) -> Result<(), PlanError> {
    let input = plan
        .canonical_fingerprint_input()
        .map_err(|_| PlanError::CanonicalPlanInvalid)?;
    let digest = canonical_preimage_digest(input.as_slice())?;
    if digest.as_bytes() != plan.plan_fingerprint.as_bytes() {
        return Err(PlanError::PlanFingerprintMismatch);
    }
    Ok(())
}

fn canonical_preimage_digest(input: &[u8]) -> Result<search_contracts::Blake3Digest32, PlanError> {
    let limit = Blake3ExactInputLimit::new(input.len()).map_err(|_| PlanError::IdentityEncoding)?;
    blake3_exact_bytes(input, limit).map_err(|_| PlanError::IdentityEncoding)
}

fn validate_canonical_scope(
    task_plan: &SearchTaskPlan,
    grant: &SearchReadGrantClaims,
    request: &SearchRecipeRequest,
    scope: &AuthorizedScope,
    safe_legs: &[SafeRetrievalLeg],
) -> Result<(), PlanError> {
    let snapshot = &task_plan.query_snapshot_fence;
    if snapshot.installation_incarnation_id != grant.installation_incarnation_id
        || snapshot.source_view != request.source_view
        || snapshot.reference_portfolio_revision != grant.reference_portfolio_revision
        || scope.source_catalog_generation != snapshot.catalog_revision.get()
        || scope.membership_generation != snapshot.membership_revision.get()
    {
        return Err(PlanError::InvalidSnapshot);
    }
    for revision in [
        requested_portfolio_revision(request),
        comparison_portfolio_revision(request),
    ]
    .into_iter()
    .flatten()
    {
        if snapshot.reference_portfolio_revision != Some(revision) {
            return Err(PlanError::InvalidSnapshot);
        }
    }

    let authorized_memberships = scope.memberships.keys().copied().collect::<BTreeSet<_>>();
    let selected_memberships = task_plan
        .selected_membership_ids
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if authorized_memberships.is_empty()
        || selected_memberships.len() != task_plan.selected_membership_ids.len()
        || selected_memberships != authorized_memberships
        || scope
            .memberships
            .iter()
            .any(|(id, binding)| *id != binding.membership_id || !binding.active)
    {
        return Err(PlanError::ScopeMismatch);
    }
    if let search_contracts::RequestedScope::ExplicitMemberships(requested) =
        &request.requested_scope
    {
        let requested_memberships = requested.iter().copied().collect::<BTreeSet<_>>();
        if requested_memberships.len() != requested.len()
            || requested_memberships != authorized_memberships
        {
            return Err(PlanError::ScopeMismatch);
        }
    }

    let mut common_route = None;
    let mut common_live_security_generation = None;
    let mut safe_memberships = BTreeSet::new();
    for (index, leg) in safe_legs.iter().enumerate() {
        let expected_leg_id = index;
        if leg.memberships.is_empty() || !leg.memberships.is_subset(&authorized_memberships) {
            return Err(PlanError::ScopeMismatch);
        }
        if leg.leg_id != expected_leg_id || !safe_memberships.is_disjoint(&leg.memberships) {
            return Err(PlanError::ScopeMismatch);
        }
        safe_memberships.extend(leg.memberships.iter().copied());
        if snapshot.collection_generation_id != Some(leg.route.collection_generation_id)
            || snapshot.visible_epoch != Some(leg.route.visible_epoch)
            || snapshot.collection_route_revision.get() != leg.route.route_generation
            || common_route.is_some_and(|route| route != leg.route)
        {
            return Err(PlanError::InvalidSnapshot);
        }
        common_route = Some(leg.route);

        let planned_memberships = leg
            .eligibility_plans
            .iter()
            .map(|plan| plan.membership_id)
            .collect::<BTreeSet<_>>();
        let planned_membership_order = leg
            .eligibility_plans
            .iter()
            .map(|plan| plan.membership_id)
            .collect::<Vec<_>>();
        let expected_membership_order = leg.memberships.iter().copied().collect::<Vec<_>>();
        if planned_memberships.len() != leg.eligibility_plans.len()
            || planned_memberships != leg.memberships
            || planned_membership_order != expected_membership_order
        {
            return Err(PlanError::ScopeMismatch);
        }
        for eligibility in &leg.eligibility_plans {
            let Some(binding) = scope.memberships.get(&eligibility.membership_id) else {
                return Err(PlanError::ScopeMismatch);
            };
            if eligibility.projection_membership_id != binding.projection_membership_id
                || eligibility.access_partition_digest != binding.access_partition_digest
                || eligibility.scoring_partition_digest != binding.scoring_partition_digest
            {
                return Err(PlanError::ScopeMismatch);
            }
            if eligibility.collection_generation_id != leg.route.collection_generation_id
                || eligibility.visible_epoch != leg.route.visible_epoch
                || eligibility.shadow_generation != snapshot.shadow_fence_revision.get()
                || eligibility.purge_generation != snapshot.purge_fence_revision.get()
                || common_live_security_generation
                    .is_some_and(|generation| generation != eligibility.live_security_generation)
            {
                return Err(PlanError::InvalidSnapshot);
            }
            common_live_security_generation = Some(eligibility.live_security_generation);
        }
    }
    Ok(())
}

const fn requested_portfolio_revision(
    request: &SearchRecipeRequest,
) -> Option<search_contracts::PortfolioRevision> {
    match &request.requested_scope {
        search_contracts::RequestedScope::ReferencePortfolio(scope) => {
            Some(scope.portfolio_revision)
        }
        _ => None,
    }
}

const fn comparison_portfolio_revision(
    request: &SearchRecipeRequest,
) -> Option<search_contracts::PortfolioRevision> {
    match &request.body {
        RecipeBodyV1::CompareImplementations(recipe) => Some(recipe.references.portfolio_revision),
        _ => None,
    }
}

/// Allocates child ceilings whose sum cannot exceed the admitted global budget.
pub fn allocate_leg_budgets(
    global: QueryExecutionBudget,
    leg_count: usize,
) -> Result<Vec<LegBudget>, PlanError> {
    validate_global_budget(global)?;
    if leg_count == 0
        || leg_count
            > usize::try_from(global.max_scoring_legs).map_err(|_| PlanError::TooManyLegs)?
    {
        return Err(PlanError::TooManyLegs);
    }
    let count_u64 = u64::try_from(leg_count).map_err(|_| PlanError::BudgetExceeded)?;
    let count_u32 = u32::try_from(leg_count).map_err(|_| PlanError::BudgetExceeded)?;
    let candidates = global.max_prefetch_candidates_per_leg;
    if candidates == 0 {
        return Err(PlanError::BudgetExceeded);
    }
    let per_deadline = global.deadline_ms / count_u64;
    let per_read = global.max_source_read_bytes / count_u64;
    let per_cpu = global.max_cpu_ms / count_u64;
    let per_memory = global.max_memory_bytes / count_u64;
    if per_deadline == 0 || per_read == 0 || per_cpu == 0 || per_memory == 0 || count_u32 == 0 {
        return Err(PlanError::BudgetExceeded);
    }
    Ok((0..leg_count)
        .map(|_| LegBudget {
            deadline_ms: per_deadline,
            max_candidates: candidates,
            max_source_read_bytes: per_read,
            max_cpu_ms: per_cpu,
            max_memory_bytes: per_memory,
        })
        .collect())
}

/// Named-axis drift observation after planning.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DriftObservation {
    pub catalog_revision: u64,
    pub membership_revision: u64,
    pub route_revision: u64,
    pub access_revision: u64,
    pub shadow_revision: u64,
    pub purge_revision: u64,
    pub overlay_revision: u64,
    pub observation_cursor_revision: u64,
    pub snapshot_expired: bool,
}

/// Planner drift decision.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlanDriftDecision {
    Valid,
    RevalidateSecurity,
    Replan,
    ExplicitIncomplete,
    SnapshotExpired,
}

/// Classifies drift without delaying restrictive security changes.
#[must_use]
pub const fn validate_drift(
    plan: &CompiledSearchPlan,
    observation: DriftObservation,
) -> PlanDriftDecision {
    if observation.snapshot_expired {
        return PlanDriftDecision::SnapshotExpired;
    }
    let snapshot = &plan.snapshot;
    if observation.access_revision > snapshot.access_policy_revision.get()
        || observation.shadow_revision > snapshot.shadow_fence_revision.get()
        || observation.purge_revision > snapshot.purge_fence_revision.get()
    {
        return PlanDriftDecision::RevalidateSecurity;
    }
    if observation.catalog_revision != snapshot.catalog_revision.get()
        || observation.membership_revision != snapshot.membership_revision.get()
        || observation.route_revision != snapshot.collection_route_revision.get()
        || observation.overlay_revision != snapshot.overlay_revision.get()
    {
        return PlanDriftDecision::Replan;
    }
    if observation.observation_cursor_revision != snapshot.observation_cursor_revision.get() {
        return PlanDriftDecision::ExplicitIncomplete;
    }
    PlanDriftDecision::Valid
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RetrievalModalities {
    direct: bool,
    lexical: bool,
    exact: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AdvancedModalities {
    structural: bool,
    semantic: bool,
    rerank: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RecipeModalities {
    retrieval: RetrievalModalities,
    advanced: AdvancedModalities,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LegDraft {
    kind: LegKind,
    memberships: BTreeSet<SourceMembershipId>,
    safe_index_leg: Option<SafeRetrievalLeg>,
    dependency_role: DependencyRole,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DependencyRole {
    Root,
    AllPrevious,
}

const fn recipe_modalities(recipe: RecipeIdV1) -> RecipeModalities {
    match recipe {
        RecipeIdV1::Locate | RecipeIdV1::InspectEntity | RecipeIdV1::ExploreEntity => {
            RecipeModalities {
                retrieval: RetrievalModalities {
                    direct: true,
                    lexical: true,
                    exact: false,
                },
                advanced: AdvancedModalities {
                    structural: true,
                    semantic: true,
                    rerank: true,
                },
            }
        }
        RecipeIdV1::FindText => RecipeModalities {
            retrieval: RetrievalModalities {
                direct: true,
                lexical: true,
                exact: true,
            },
            advanced: AdvancedModalities {
                structural: false,
                semantic: false,
                rerank: false,
            },
        },
        RecipeIdV1::CompareImplementations => RecipeModalities {
            retrieval: RetrievalModalities {
                direct: true,
                lexical: true,
                exact: false,
            },
            advanced: AdvancedModalities {
                structural: true,
                semantic: true,
                rerank: true,
            },
        },
        RecipeIdV1::CompileExactScan | RecipeIdV1::ExecuteExactScan => RecipeModalities {
            retrieval: RetrievalModalities {
                direct: false,
                lexical: false,
                exact: true,
            },
            advanced: AdvancedModalities {
                structural: false,
                semantic: false,
                rerank: false,
            },
        },
        RecipeIdV1::CorpusProfile
        | RecipeIdV1::CorpusDelta
        | RecipeIdV1::Provenance
        | RecipeIdV1::ExpandHandle => RecipeModalities {
            retrieval: RetrievalModalities {
                direct: true,
                lexical: false,
                exact: false,
            },
            advanced: AdvancedModalities {
                structural: false,
                semantic: false,
                rerank: false,
            },
        },
    }
}

/// DIRECT-spine leg support for one closed v1 recipe.
///
/// Derived from the same closed modality registry that drives leg planning
/// ([`recipe_modalities`]), so the advertised coverage table cannot drift
/// from the planner. Indexed/Qdrant legs are out of scope for the DIRECT
/// spine: recipes that need them for full fidelity run with those
/// capabilities explicitly omitted, never silently downgraded.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectLegSupport {
    /// A `Direct` leg is plannable from the authorized memberships.
    pub direct: bool,
    /// An `Exact` leg is plannable; exact-only recipes are T34-proven.
    pub exact: bool,
    /// Full fidelity wants structural/semantic/rerank legs, which the DIRECT
    /// spine explicitly reports as omitted until their waves accept them.
    pub advanced: bool,
}

/// Classifies one v1 recipe for DIRECT-spine execution.
#[must_use]
pub const fn direct_leg_support(recipe: RecipeIdV1) -> DirectLegSupport {
    match recipe {
        RecipeIdV1::Locate
        | RecipeIdV1::InspectEntity
        | RecipeIdV1::ExploreEntity
        | RecipeIdV1::CompareImplementations => DirectLegSupport {
            direct: true,
            exact: false,
            advanced: true,
        },
        RecipeIdV1::FindText => DirectLegSupport {
            direct: true,
            exact: true,
            advanced: false,
        },
        RecipeIdV1::CorpusProfile
        | RecipeIdV1::CorpusDelta
        | RecipeIdV1::Provenance
        | RecipeIdV1::ExpandHandle => DirectLegSupport {
            direct: true,
            exact: false,
            advanced: false,
        },
        RecipeIdV1::CompileExactScan | RecipeIdV1::ExecuteExactScan => DirectLegSupport {
            direct: false,
            exact: true,
            advanced: false,
        },
    }
}

const fn cancellation_boundary(kind: LegKind) -> CancellationBoundary {
    match kind {
        LegKind::Direct | LegKind::Exact => CancellationBoundary::BetweenSourceReads,
        LegKind::Lexical | LegKind::Structural | LegKind::Semantic => {
            CancellationBoundary::BetweenPages
        }
        LegKind::Rerank => CancellationBoundary::BeforeEmission,
    }
}

const fn validate_recipe_specific_bounds(body: &RecipeBodyV1) -> Result<(), PlanError> {
    match body {
        RecipeBodyV1::ExploreEntity(recipe) if recipe.max_depth == 0 => {
            Err(PlanError::RecipeBodyMismatch)
        }
        RecipeBodyV1::Provenance(recipe) if recipe.max_lineage_depth == 0 => {
            Err(PlanError::RecipeBodyMismatch)
        }
        RecipeBodyV1::ExpandHandle(recipe) if recipe.max_bytes == 0 => {
            Err(PlanError::RecipeBodyMismatch)
        }
        _ => Ok(()),
    }
}

const fn validate_global_budget(budget: QueryExecutionBudget) -> Result<(), PlanError> {
    if budget.deadline_ms == 0
        || budget.max_scoring_legs == 0
        || budget.max_prefetch_candidates_per_leg == 0
        || budget.max_validated_candidates == 0
        || budget.max_source_read_bytes == 0
        || budget.max_materialized_result_bytes == 0
        || budget.max_cpu_ms == 0
        || budget.max_memory_bytes == 0
    {
        Err(PlanError::BudgetExceeded)
    } else {
        Ok(())
    }
}

fn validate_dag(legs: &[PlannedLeg]) -> Result<(), PlanError> {
    for leg in legs {
        if leg
            .depends_on
            .iter()
            .any(|dependency| *dependency >= leg.leg_id)
        {
            return Err(PlanError::InvalidDependencyGraph);
        }
    }
    Ok(())
}

fn fingerprint_plan(
    request_id: RequestId,
    recipe: RecipeIdV1,
    snapshot: &QuerySnapshotFence,
    scope: &AuthorizedScope,
    budget: QueryExecutionBudget,
    legs: &[PlannedLeg],
) -> CompiledPlanDigest {
    let mut lanes = [
        0xcbf2_9ce4_8422_2325_u64,
        0x8422_2325_cbf2_9ce4,
        0x9e37_79b9_7f4a_7c15,
        0xc2b2_ae3d_27d4_eb4f,
    ];
    mix(&mut lanes, request_id.as_bytes());
    mix(&mut lanes, recipe.as_str().as_bytes());
    mix(&mut lanes, snapshot.snapshot_fingerprint.as_bytes());
    mix(&mut lanes, scope.snapshot_digest.as_bytes());
    mix(&mut lanes, &budget.deadline_ms.to_be_bytes());
    mix(&mut lanes, &budget.max_source_read_bytes.to_be_bytes());
    for leg in legs {
        mix(
            &mut lanes,
            &u64::try_from(leg.leg_id).unwrap_or(u64::MAX).to_be_bytes(),
        );
        mix(&mut lanes, &[leg_kind_tag(leg.leg_kind)]);
        for membership in &leg.memberships {
            mix(&mut lanes, membership.as_bytes());
        }
        mix(&mut lanes, &leg.budget.deadline_ms.to_be_bytes());
        mix(&mut lanes, &leg.budget.max_source_read_bytes.to_be_bytes());
    }
    let mut digest = [0_u8; 32];
    for (index, lane) in lanes.into_iter().enumerate() {
        digest[index * 8..index * 8 + 8].copy_from_slice(&lane.to_be_bytes());
    }
    CompiledPlanDigest(digest)
}

const fn leg_kind_tag(kind: LegKind) -> u8 {
    match kind {
        LegKind::Direct => 1,
        LegKind::Exact => 2,
        LegKind::Structural => 3,
        LegKind::Lexical => 4,
        LegKind::Semantic => 5,
        LegKind::Rerank => 6,
    }
}

fn mix(lanes: &mut [u64; 4], bytes: &[u8]) {
    for (index, byte) in bytes.iter().copied().enumerate() {
        let lane = index % lanes.len();
        lanes[lane] ^= u64::from(byte);
        lanes[lane] = lanes[lane]
            .wrapping_mul(0x0000_0100_0000_01b3)
            .rotate_left(u32::try_from(17 + lane * 3).unwrap_or(17));
    }
}

#[cfg(test)]
mod direct_spine_tests {
    use search_contracts::{
        BoundedCanonicalBytes, BoundedList, BoundedName, BoundedSet, CasePolicy,
        CompareImplementationsRecipe, ComparisonAxis, CompileExactScanRecipe, CorpusDeltaDimension,
        CorpusDeltaRecipe, CorpusFacetDimension, CorpusProfileRecipe, EntityKind,
        ExactCompletenessRequirements, ExactInputDomain, ExactPredicate, ExactPredicateKind,
        ExactScanPlanRef, ExecuteExactScanRecipe, ExpandHandleRecipe, ExpandHandleTarget,
        ExploreEntityRecipe, FindTextRecipe, HandleClass, HandleExpansionKind, HandleId,
        InspectEntityRecipe, LocateRecipe, NonZeroRevision, NormalizedNameSelector,
        OpaqueHandleToken, PlanFingerprint, PlanId, PortfolioRevision, PriorityClass, ProfileId,
        ProvenanceRecipe, QueryExecutionBudget, RecipeBodyV1, RecipeIdV1, ReferencePortfolioId,
        ReferencePortfolioScope, RelationKind, RequestId, RequestedScope, SearchRecipeRequest,
        SearchSourceHandle, SourceRevisionId, SourceView, SubjectSelector,
    };

    use super::{PlanError, allocate_leg_budgets, direct_leg_support, normalize_recipe};

    /// Crate dictionary shared with the resolver/comparator baseline: every
    /// subject name below is a real workspace package, never invented text.
    const CRATE_DICTIONARY: [&str; 6] = [
        "search-contracts",
        "search-access",
        "search-exact",
        "search-subject-resolver",
        "search-comparator",
        "search-query-planner",
    ];

    fn profile(name: &str) -> ProfileId {
        ProfileId::new(name).expect("fixture profile")
    }

    fn source_view() -> SourceView {
        SourceView::RetainedRevision(SourceRevisionId::from_bytes([0x51; 16]))
    }

    fn scope() -> RequestedScope {
        RequestedScope::ExplicitMemberships(BoundedList::new(Vec::new()).expect("fixture scope"))
    }

    fn name_selector(word: &str) -> SubjectSelector {
        SubjectSelector::NormalizedName(NormalizedNameSelector {
            name: BoundedName::new(word).expect("fixture name"),
            entity_kinds: BoundedSet::empty(),
        })
    }

    fn kinds(kind: EntityKind) -> BoundedSet<EntityKind, 4096> {
        BoundedSet::from_items([kind]).expect("fixture kinds")
    }

    fn handle(seed: u8) -> SearchSourceHandle {
        SearchSourceHandle {
            handle_id: HandleId::from_bytes([seed; 16]),
            handle_revision: NonZeroRevision::new(1).expect("fixture revision"),
            handle_class: HandleClass::DurableSource,
            expires_at: None,
            opaque_token: OpaqueHandleToken::new(&[0xA5; 32]).expect("fixture token"),
        }
    }

    fn predicate() -> ExactPredicate {
        ExactPredicate {
            kind: ExactPredicateKind::Literal,
            engine_and_version: profile("literal-v1"),
            serialized_form: BoundedCanonicalBytes::from_validated(b"needle".to_vec())
                .expect("fixture predicate bytes"),
            input_domain: ExactInputDomain::DecodedText,
            worst_case_complexity_class: profile("linear-scan"),
        }
    }

    fn request(recipe: RecipeIdV1, body: RecipeBodyV1) -> SearchRecipeRequest {
        SearchRecipeRequest::new(
            RequestId::from_bytes([0x11; 16]),
            recipe,
            source_view(),
            scope(),
            profile("interactive"),
            body,
        )
        .expect("fixture request")
    }

    fn completeness() -> ExactCompletenessRequirements {
        ExactCompletenessRequirements {
            require_every_denominator_item: true,
            require_stable_or_retained_revision: true,
            require_current_observation: true,
            include_authenticated_unsaved_buffers: false,
            fail_on_timeout: true,
            fail_on_cancellation: true,
            fail_on_scope_drift: true,
        }
    }

    fn references() -> ReferencePortfolioScope {
        ReferencePortfolioScope {
            portfolio_id: ReferencePortfolioId::from_bytes([0x09; 16]),
            portfolio_revision: PortfolioRevision::new(3),
        }
    }

    #[test]
    fn registry_is_exactly_eleven_versioned_recipes() {
        assert_eq!(RecipeIdV1::ALL.len(), 11);
        for recipe in RecipeIdV1::ALL {
            assert_eq!(RecipeIdV1::parse(recipe.as_str()), Ok(recipe));
            assert_eq!(RecipeIdV1::parse_versioned(recipe.as_str()), Ok(recipe));
        }
        for rejected in [
            "",
            "locate",
            "find_text",
            "locate@2",
            "LOCATE@1",
            "compare@1",
        ] {
            assert!(RecipeIdV1::parse(rejected).is_err(), "{rejected}");
            assert!(RecipeIdV1::parse_versioned(rejected).is_err(), "{rejected}");
        }
    }

    #[test]
    fn normalize_accepts_all_eleven_bodies() {
        let subject = name_selector(CRATE_DICTIONARY[4]);
        let bodies = [
            (
                RecipeIdV1::Locate,
                RecipeBodyV1::Locate(LocateRecipe {
                    subject: subject.clone(),
                    evidence_roles: BoundedSet::empty(),
                }),
            ),
            (
                RecipeIdV1::FindText,
                RecipeBodyV1::FindText(FindTextRecipe {
                    predicate: predicate(),
                    case_policy: CasePolicy::Exact,
                    context_bytes_before: 32,
                    context_bytes_after: 32,
                }),
            ),
            (
                RecipeIdV1::InspectEntity,
                RecipeBodyV1::InspectEntity(InspectEntityRecipe {
                    subject: subject.clone(),
                    evidence_roles: BoundedSet::empty(),
                    include_relations: BoundedSet::from_items([RelationKind::Definition])
                        .expect("fixture relations"),
                }),
            ),
            (
                RecipeIdV1::CompareImplementations,
                RecipeBodyV1::CompareImplementations(CompareImplementationsRecipe {
                    subject: subject.clone(),
                    references: references(),
                    comparison_axes: BoundedSet::from_items([
                        ComparisonAxis::Interface,
                        ComparisonAxis::Tests,
                    ])
                    .expect("fixture axes"),
                }),
            ),
            (
                RecipeIdV1::ExploreEntity,
                RecipeBodyV1::ExploreEntity(ExploreEntityRecipe {
                    subject,
                    relation_kinds: BoundedSet::from_items([RelationKind::Caller])
                        .expect("fixture relations"),
                    max_depth: 3,
                }),
            ),
            (
                RecipeIdV1::CorpusProfile,
                RecipeBodyV1::CorpusProfile(CorpusProfileRecipe {
                    facets: BoundedSet::from_items([
                        CorpusFacetDimension::Role,
                        CorpusFacetDimension::EntityKind,
                    ])
                    .expect("fixture facets"),
                }),
            ),
            (
                RecipeIdV1::CorpusDelta,
                RecipeBodyV1::CorpusDelta(CorpusDeltaRecipe {
                    from_view: SourceView::RetainedRevision(SourceRevisionId::from_bytes([1; 16])),
                    to_view: SourceView::RetainedRevision(SourceRevisionId::from_bytes([2; 16])),
                    dimensions: BoundedSet::from_items([CorpusDeltaDimension::Source])
                        .expect("fixture dimensions"),
                }),
            ),
            (
                RecipeIdV1::Provenance,
                RecipeBodyV1::Provenance(ProvenanceRecipe {
                    source_handle: handle(4),
                    max_lineage_depth: 4,
                }),
            ),
            (
                RecipeIdV1::CompileExactScan,
                RecipeBodyV1::CompileExactScan {
                    predicate: predicate(),
                    body: CompileExactScanRecipe {
                        completeness_requirements: completeness(),
                    },
                },
            ),
            (
                RecipeIdV1::ExecuteExactScan,
                RecipeBodyV1::ExecuteExactScan(ExecuteExactScanRecipe {
                    plan_ref: ExactScanPlanRef {
                        plan_id: PlanId::from_bytes([0x07; 16]),
                        plan_fingerprint: PlanFingerprint::from_bytes([0x08; 32]),
                    },
                }),
            ),
            (
                RecipeIdV1::ExpandHandle,
                RecipeBodyV1::ExpandHandle(ExpandHandleRecipe {
                    handle: ExpandHandleTarget::Source(handle(5)),
                    expansion: HandleExpansionKind::Excerpt,
                    max_bytes: 1024,
                }),
            ),
        ];
        assert_eq!(bodies.len(), 11);
        for (recipe, body) in bodies {
            assert_eq!(body.recipe_id(), recipe);
            normalize_recipe(request(recipe, body)).expect("eleven v1 normalize");
        }
        assert_eq!(kinds(EntityKind::Function).len(), 1);
    }

    #[test]
    fn normalize_rejects_mismatch_and_zero_bounds() {
        let mismatched = SearchRecipeRequest {
            request_id: RequestId::from_bytes([0x11; 16]),
            recipe: RecipeIdV1::Locate,
            source_view: source_view(),
            requested_scope: scope(),
            requested_budget_class: profile("interactive"),
            body: RecipeBodyV1::FindText(FindTextRecipe {
                predicate: predicate(),
                case_policy: CasePolicy::Exact,
                context_bytes_before: 0,
                context_bytes_after: 0,
            }),
        };
        assert_eq!(
            normalize_recipe(mismatched),
            Err(PlanError::RecipeBodyMismatch)
        );
        let zero_depth = request(
            RecipeIdV1::ExploreEntity,
            RecipeBodyV1::ExploreEntity(ExploreEntityRecipe {
                subject: name_selector(CRATE_DICTIONARY[0]),
                relation_kinds: BoundedSet::empty(),
                max_depth: 0,
            }),
        );
        assert_eq!(
            normalize_recipe(zero_depth),
            Err(PlanError::RecipeBodyMismatch)
        );
        let zero_lineage = request(
            RecipeIdV1::Provenance,
            RecipeBodyV1::Provenance(ProvenanceRecipe {
                source_handle: handle(6),
                max_lineage_depth: 0,
            }),
        );
        assert_eq!(
            normalize_recipe(zero_lineage),
            Err(PlanError::RecipeBodyMismatch)
        );
        let zero_bytes = request(
            RecipeIdV1::ExpandHandle,
            RecipeBodyV1::ExpandHandle(ExpandHandleRecipe {
                handle: ExpandHandleTarget::Source(handle(7)),
                expansion: HandleExpansionKind::Provenance,
                max_bytes: 0,
            }),
        );
        assert_eq!(
            normalize_recipe(zero_bytes),
            Err(PlanError::RecipeBodyMismatch)
        );
    }

    #[test]
    fn direct_leg_support_covers_all_eleven_without_drift() {
        let mut direct_count = 0_usize;
        let mut exact_only_count = 0_usize;
        for recipe in RecipeIdV1::ALL {
            let support = direct_leg_support(recipe);
            if support.direct {
                direct_count += 1;
            } else {
                assert!(support.exact, "{recipe:?}");
                assert!(!support.advanced, "{recipe:?}");
                exact_only_count += 1;
            }
        }
        assert_eq!(direct_count, 9);
        assert_eq!(exact_only_count, 2);
        assert!(direct_leg_support(RecipeIdV1::FindText).exact);
        assert!(!direct_leg_support(RecipeIdV1::CorpusProfile).exact);
        assert!(!direct_leg_support(RecipeIdV1::CorpusProfile).advanced);
        for recipe in [
            RecipeIdV1::Locate,
            RecipeIdV1::InspectEntity,
            RecipeIdV1::ExploreEntity,
            RecipeIdV1::CompareImplementations,
        ] {
            assert!(direct_leg_support(recipe).advanced, "{recipe:?}");
        }
        for recipe in [
            RecipeIdV1::FindText,
            RecipeIdV1::CompileExactScan,
            RecipeIdV1::ExecuteExactScan,
            RecipeIdV1::CorpusProfile,
            RecipeIdV1::CorpusDelta,
            RecipeIdV1::Provenance,
            RecipeIdV1::ExpandHandle,
        ] {
            assert!(!direct_leg_support(recipe).advanced, "{recipe:?}");
        }
    }

    #[test]
    fn leg_budgets_stay_finite_and_bounded() {
        let global = QueryExecutionBudget {
            priority_class: PriorityClass::Interactive,
            deadline_ms: 3000,
            max_scoring_legs: 8,
            max_prefetch_candidates_per_leg: 64,
            max_validated_candidates: 128,
            max_source_read_bytes: 1_048_576,
            max_exact_scan_items: 4096,
            max_exact_scan_bytes: 1_048_576,
            max_materialized_result_bytes: 262_144,
            max_cpu_ms: 2000,
            max_memory_bytes: 67_108_864,
        };
        let budgets = allocate_leg_budgets(global, 3).expect("finite budgets");
        assert_eq!(budgets.len(), 3);
        let mut read_sum = 0_u64;
        for budget in &budgets {
            assert!(budget.deadline_ms > 0);
            assert!(budget.max_source_read_bytes > 0);
            read_sum += budget.max_source_read_bytes;
        }
        assert!(read_sum <= global.max_source_read_bytes);
        assert!(allocate_leg_budgets(global, 0).is_err());
        let zero = QueryExecutionBudget {
            deadline_ms: 0,
            ..global
        };
        assert!(allocate_leg_budgets(zero, 1).is_err());
    }
}

#[cfg(test)]
mod canonical_plan_tests {
    use std::collections::BTreeMap;

    use search_access::{
        AuthorizedScope, BaseEligibilityPlan, IndexedRouteFence, MembershipAccessBinding,
        SafeRetrievalLeg,
    };
    use search_contracts::{
        AccessPartitionId, AccessPolicyRevision, Blake3Digest32, Blake3ExactInputLimit,
        BoundedList, BoundedName, BoundedSet, CatalogRevision, ClientScopeFence,
        CollectionGenerationId, CollectionRouteRevision, DisclosureCeiling, EntityKind, Epoch,
        ExactnessRequirements, FusionProfileId, GrantFence, GrantId, InstallationId,
        InstallationIncarnationId, LegKind, MembershipRevision, Modality,
        ObservationCursorRevision, ObservationFreshness, ObservationFreshnessState, OpaqueId,
        OpaqueRef, OwnerEpoch, PlanFingerprint, PlanId, PriorityClass, ProfileFence, ProfileId,
        ProjectionProfileSetId, PurgeFenceRevision, QueryExecutionBudget, QuerySnapshotFence,
        QuerySnapshotFingerprint, RecipeBodyV1, RecipeIdV1, RequestId, RequestedScope,
        RequiredDenominator, SearchReadGrantClaims, SearchRecipeRequest, SearchTaskPlan,
        ShadowFenceRevision, SourceMembershipId, SourceRevisionId, SourceView, UtcTimestamp,
    };

    use super::{
        AdvancedCapabilities as PlannerAdvancedCapabilities, CapabilitySet as PlannerCapabilities,
        PlanError, RetrievalCapabilities as PlannerRetrievalCapabilities, compile_canonical_plan,
        normalize_recipe,
    };

    fn profile(value: &str) -> ProfileId {
        ProfileId::new(value).expect("fixture profile")
    }

    fn source_view() -> SourceView {
        SourceView::RetainedRevision(SourceRevisionId::from_bytes([2; 16]))
    }

    fn request(name: &str) -> SearchRecipeRequest {
        let membership = SourceMembershipId::from_bytes([6; 16]);
        SearchRecipeRequest::new(
            RequestId::from_bytes([4; 16]),
            RecipeIdV1::Locate,
            source_view(),
            RequestedScope::ExplicitMemberships(
                BoundedList::new(vec![membership]).expect("request membership"),
            ),
            profile("interactive"),
            RecipeBodyV1::Locate(search_contracts::LocateRecipe {
                subject: search_contracts::SubjectSelector::NormalizedName(
                    search_contracts::NormalizedNameSelector {
                        name: BoundedName::new(name).expect("fixture name"),
                        entity_kinds: BoundedSet::<EntityKind, 4096>::empty(),
                    },
                ),
                evidence_roles: BoundedSet::empty(),
            }),
        )
        .expect("fixture request")
    }

    fn digest(bytes: &[u8]) -> [u8; 32] {
        let limit = Blake3ExactInputLimit::new(bytes.len()).expect("bounded fixture bytes");
        *search_contracts::blake3_exact_bytes(bytes, limit)
            .expect("fixture BLAKE3")
            .as_bytes()
    }

    fn seal_snapshot(snapshot: &mut QuerySnapshotFence) {
        let input = snapshot
            .canonical_fingerprint_input()
            .expect("canonical snapshot");
        snapshot.snapshot_fingerprint =
            QuerySnapshotFingerprint::from_bytes(digest(input.as_slice()));
    }

    fn seal_plan(plan: &mut SearchTaskPlan) {
        let input = plan
            .canonical_fingerprint_input()
            .expect("canonical task plan");
        plan.plan_fingerprint = PlanFingerprint::from_bytes(digest(input.as_slice()));
    }

    struct Fixture {
        task_plan: SearchTaskPlan,
        grant: SearchReadGrantClaims,
        recipe: super::NormalizedRecipeRequest,
        scope: AuthorizedScope,
        capabilities: PlannerCapabilities,
        safe_legs: Vec<SafeRetrievalLeg>,
    }

    fn fixture() -> Fixture {
        let member = SourceMembershipId::from_bytes([6; 16]);
        let source_view = source_view();
        let generation = CollectionGenerationId::from_bytes([12; 16]);
        let epoch = Epoch::new(3).expect("epoch");
        let binding = MembershipAccessBinding {
            membership_id: member,
            access_partition_digest: Blake3Digest32::from_bytes([7; 32]),
            scoring_partition_digest: Blake3Digest32::from_bytes([8; 32]),
            projection_membership_id: OpaqueId::new("projection-member")
                .expect("projection membership"),
            active: true,
        };
        let scope = AuthorizedScope {
            memberships: BTreeMap::from([(member, binding.clone())]),
            access_snapshot_generation: 11,
            source_catalog_generation: 1,
            membership_generation: 1,
            snapshot_digest: Blake3Digest32::from_bytes([9; 32]),
        };
        let route = IndexedRouteFence {
            collection_generation_id: generation,
            visible_epoch: epoch,
            route_generation: 1,
            owner_epoch: OwnerEpoch::new(1).expect("owner epoch"),
        };
        let safe_legs = vec![SafeRetrievalLeg {
            leg_id: 0,
            memberships: [member].into_iter().collect(),
            eligibility_plans: vec![BaseEligibilityPlan {
                membership_id: member,
                projection_membership_id: binding.projection_membership_id.clone(),
                access_partition_digest: binding.access_partition_digest,
                scoring_partition_digest: binding.scoring_partition_digest,
                collection_generation_id: generation,
                visible_epoch: epoch,
                live_security_generation: 12,
                shadow_generation: 1,
                purge_generation: 1,
                predicate_digest: search_access::EligibilityPlanDigest([13; 32]),
            }],
            route,
            overlap_proof_digest: None,
        }];
        let mut snapshot = QuerySnapshotFence {
            installation_incarnation_id: InstallationIncarnationId::from_bytes([1; 16]),
            collection_generation_id: Some(generation),
            visible_epoch: Some(epoch),
            collection_route_revision: CollectionRouteRevision::new(1),
            catalog_revision: CatalogRevision::new(1),
            membership_revision: MembershipRevision::new(1),
            reference_portfolio_revision: None,
            access_policy_revision: AccessPolicyRevision::new(1),
            shadow_fence_revision: ShadowFenceRevision::new(1),
            purge_fence_revision: PurgeFenceRevision::new(1),
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
                .expect("lexical profile"),
            snapshot_fingerprint: QuerySnapshotFingerprint::from_bytes([0; 32]),
        };
        seal_snapshot(&mut snapshot);

        let request = request("parse_manifest");
        let recipe = normalize_recipe(request.clone()).expect("normalized recipe");
        let client_scope_ref = OpaqueRef::new("scope:fixture").expect("client scope");
        let scope_domain_id = search_contracts::ScopeDomainId::from_bytes([3; 16]);
        let grant_id = GrantId::from_bytes([5; 16]);
        let grant = SearchReadGrantClaims {
            grant_id,
            installation_id: InstallationId::from_bytes([14; 16]),
            installation_incarnation_id: InstallationIncarnationId::from_bytes([1; 16]),
            binding_id: search_contracts::BindingId::from_bytes([15; 16]),
            principal_opaque_id: OpaqueId::new("principal").expect("principal"),
            client_scope_ref: client_scope_ref.clone(),
            scope_domain_id,
            allowed_membership_ids: BoundedSet::from_items([member]).expect("grant members"),
            allowed_corpus_or_portfolio_ids: BoundedSet::empty(),
            reference_portfolio_revision: None,
            allowed_access_partitions: BoundedSet::from_items([AccessPartitionId::from_bytes(
                [7; 16],
            )])
            .expect("grant access partition"),
            allowed_modalities: BoundedSet::from_items([Modality::Text]).expect("grant modality"),
            permitted_recipe_families: BoundedSet::from_items([RecipeIdV1::Locate])
                .expect("grant recipe"),
            maximum_budget_class: profile("interactive"),
            sensitivity_ceiling: search_contracts::SensitivityClass::Public,
            disclosure_ceiling: DisclosureCeiling::LocalOnly,
            source_read_permission: true,
            exact_scan_permission: false,
            issued_boot_id: OpaqueId::new("boot").expect("boot id"),
            issued_at: UtcTimestamp::parse("2026-10-16T00:00:00.000000Z").expect("issued at"),
            expires_at: UtcTimestamp::parse("2026-10-16T00:05:00.000000Z").expect("expires at"),
            nonce: OpaqueId::new("nonce").expect("nonce"),
            revocation_generation: 4,
        };
        let budget = QueryExecutionBudget {
            priority_class: PriorityClass::Interactive,
            deadline_ms: 1000,
            max_scoring_legs: 2,
            max_prefetch_candidates_per_leg: 16,
            max_validated_candidates: 16,
            max_source_read_bytes: 10_000,
            max_exact_scan_items: 0,
            max_exact_scan_bytes: 0,
            max_materialized_result_bytes: 10_000,
            max_cpu_ms: 1000,
            max_memory_bytes: 100_000,
        };
        let request_digest = request
            .canonical_request_digest()
            .expect("canonical request digest");
        let mut task_plan = SearchTaskPlan {
            plan_id: PlanId::from_bytes([16; 16]),
            provider_protocol_version: search_contracts::ProtocolVersion { major: 1, minor: 0 },
            request_id: request.request_id,
            recipe_request_digest: request_digest,
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
            selected_membership_ids: BoundedList::new(vec![member]).expect("selected member"),
            profile_fence: ProfileFence {
                fusion_profile_id: FusionProfileId::new("fusion-v1").expect("fusion profile"),
                projection_profile_set_ids: BoundedList::new(vec![
                    ProjectionProfileSetId::new("lexical-v1").expect("projection profile"),
                ])
                .expect("projection profile set"),
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
            created_at: UtcTimestamp::parse("2026-10-16T00:00:00.000000Z").expect("created at"),
            expires_at: UtcTimestamp::parse("2026-10-16T00:01:00.000000Z").expect("expires at"),
        };
        seal_plan(&mut task_plan);

        Fixture {
            task_plan,
            grant,
            recipe,
            scope,
            capabilities: PlannerCapabilities {
                retrieval: PlannerRetrievalCapabilities {
                    direct: true,
                    lexical: true,
                    exact: false,
                },
                advanced: PlannerAdvancedCapabilities {
                    structural: false,
                    semantic: false,
                    rerank: false,
                },
            },
            safe_legs,
        }
    }

    fn compile(fixture: &Fixture) -> Result<super::CompiledSearchPlan, PlanError> {
        compile_canonical_plan(
            &fixture.task_plan,
            &fixture.grant,
            &fixture.recipe,
            fixture.scope.clone(),
            fixture.capabilities,
            &fixture.safe_legs,
        )
    }

    #[test]
    fn canonical_plan_reuses_graph_builder_and_safe_leg_closure() {
        let fixture = fixture();
        let compiled = compile(&fixture).expect("canonical plan compiles");
        assert_eq!(compiled.request_id, fixture.task_plan.request_id);
        assert_eq!(compiled.legs.len(), 2);
        assert_eq!(compiled.legs[0].leg_kind, LegKind::Direct);
        assert_eq!(compiled.legs[1].leg_kind, LegKind::Lexical);
        assert_eq!(
            compiled.global_budget,
            fixture.task_plan.query_execution_budget
        );
    }

    #[test]
    fn canonical_plan_rejects_recipe_budget_and_grant_fence_drift() {
        let mut denied_recipe = fixture();
        denied_recipe.grant.permitted_recipe_families = BoundedSet::empty();
        assert_eq!(compile(&denied_recipe), Err(PlanError::RecipeDenied));

        let mut denied_budget = fixture();
        denied_budget.grant.maximum_budget_class = profile("verification");
        assert_eq!(compile(&denied_budget), Err(PlanError::BudgetClassDenied));

        let mut changed_fence = fixture();
        changed_fence.task_plan.grant_fence.revocation_generation += 1;
        assert_eq!(compile(&changed_fence), Err(PlanError::GrantFenceMismatch));
    }

    #[test]
    fn canonical_plan_rejects_request_scope_snapshot_and_fingerprint_drift() {
        let mut changed_request = fixture();
        changed_request.recipe =
            normalize_recipe(request("different_symbol")).expect("normalized changed request");
        assert_eq!(
            compile(&changed_request),
            Err(PlanError::RequestDigestMismatch)
        );

        let mut changed_scope = fixture();
        changed_scope.task_plan.selected_membership_ids = BoundedList::empty();
        assert_eq!(compile(&changed_scope), Err(PlanError::ScopeMismatch));

        let mut changed_route = fixture();
        changed_route.safe_legs[0].route.route_generation += 1;
        assert_eq!(compile(&changed_route), Err(PlanError::InvalidSnapshot));

        let mut changed_safe_scope = fixture();
        changed_safe_scope.safe_legs[0]
            .memberships
            .insert(SourceMembershipId::from_bytes([99; 16]));
        assert_eq!(compile(&changed_safe_scope), Err(PlanError::ScopeMismatch));

        let mut changed_snapshot_hash = fixture();
        changed_snapshot_hash
            .task_plan
            .query_snapshot_fence
            .snapshot_fingerprint = QuerySnapshotFingerprint::from_bytes([99; 32]);
        assert_eq!(
            compile(&changed_snapshot_hash),
            Err(PlanError::InvalidSnapshot)
        );

        let mut changed_plan = fixture();
        changed_plan.task_plan.profile_fence.fusion_profile_id =
            FusionProfileId::new("fusion-v2").expect("changed profile");
        assert_eq!(
            compile(&changed_plan),
            Err(PlanError::PlanFingerprintMismatch)
        );
    }

    #[test]
    fn canonical_plan_rejects_excess_scoring_legs_under_bound_budget() {
        let mut fixture = fixture();
        fixture.task_plan.query_execution_budget.max_scoring_legs = 1;
        seal_plan(&mut fixture.task_plan);
        assert_eq!(compile(&fixture), Err(PlanError::TooManyLegs));
    }
}

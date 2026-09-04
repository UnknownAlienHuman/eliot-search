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
use std::collections::{BTreeMap, BTreeSet};

use search_access::{
    AccessModality, AuthorizedScope, SafeRetrievalLeg, ValidatedGrant,
};
use search_contracts::{
    LegKind, QueryExecutionBudget, QuerySnapshotFence, RecipeBodyV1, RecipeIdV1,
    RequestId, SearchRecipeRequest, SourceMembershipId,
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
pub struct CapabilitySet {
    pub direct: bool,
    pub lexical: bool,
    pub exact: bool,
    pub structural: bool,
    pub semantic: bool,
    pub rerank: bool,
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
    snapshot.validate().map_err(|_| PlanError::InvalidSnapshot)?;
    validate_global_budget(global_budget)?;

    let modalities = recipe_modalities(request.recipe);
    let mut drafts = Vec::new();
    let mut omitted = BTreeSet::new();
    let all_memberships = authorized_scope
        .memberships
        .keys()
        .copied()
        .collect::<BTreeSet<_>>();

    if modalities.direct && capabilities.direct && grant.permits_modality(AccessModality::Direct) {
        drafts.push(LegDraft {
            kind: LegKind::Direct,
            memberships: all_memberships.clone(),
            safe_index_leg: None,
            dependency_role: DependencyRole::Root,
        });
    }

    if modalities.lexical {
        if capabilities.lexical && grant.permits_modality(AccessModality::Lexical) {
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

    if modalities.exact {
        if capabilities.exact && grant.permits_modality(AccessModality::Exact) {
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

    if modalities.structural {
        if capabilities.structural && grant.permits_modality(AccessModality::Code) {
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

    if modalities.semantic {
        if capabilities.semantic && grant.permits_modality(AccessModality::Semantic) {
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

    if modalities.rerank {
        if capabilities.rerank {
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
        > usize::try_from(global_budget.max_scoring_legs)
            .map_err(|_| PlanError::TooManyLegs)?
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

/// Allocates child ceilings whose sum cannot exceed the admitted global budget.
pub fn allocate_leg_budgets(
    global: QueryExecutionBudget,
    leg_count: usize,
) -> Result<Vec<LegBudget>, PlanError> {
    validate_global_budget(global)?;
    if leg_count == 0
        || leg_count
            > usize::try_from(global.max_scoring_legs)
                .map_err(|_| PlanError::TooManyLegs)?
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
pub fn validate_drift(
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
struct RecipeModalities {
    direct: bool,
    lexical: bool,
    exact: bool,
    structural: bool,
    semantic: bool,
    rerank: bool,
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

fn recipe_modalities(recipe: RecipeIdV1) -> RecipeModalities {
    match recipe {
        RecipeIdV1::Locate | RecipeIdV1::InspectEntity | RecipeIdV1::ExploreEntity => {
            RecipeModalities {
                direct: true,
                lexical: true,
                exact: false,
                structural: true,
                semantic: true,
                rerank: true,
            }
        }
        RecipeIdV1::FindText => RecipeModalities {
            direct: true,
            lexical: true,
            exact: true,
            structural: false,
            semantic: false,
            rerank: false,
        },
        RecipeIdV1::CompareImplementations => RecipeModalities {
            direct: true,
            lexical: true,
            exact: false,
            structural: true,
            semantic: true,
            rerank: true,
        },
        RecipeIdV1::CompileExactScan | RecipeIdV1::ExecuteExactScan => RecipeModalities {
            direct: false,
            lexical: false,
            exact: true,
            structural: false,
            semantic: false,
            rerank: false,
        },
        RecipeIdV1::CorpusProfile
        | RecipeIdV1::CorpusDelta
        | RecipeIdV1::Provenance
        | RecipeIdV1::ExpandHandle => RecipeModalities {
            direct: true,
            lexical: false,
            exact: false,
            structural: false,
            semantic: false,
            rerank: false,
        },
    }
}

fn cancellation_boundary(kind: LegKind) -> CancellationBoundary {
    match kind {
        LegKind::Direct | LegKind::Exact => CancellationBoundary::BetweenSourceReads,
        LegKind::Lexical | LegKind::Structural | LegKind::Semantic => {
            CancellationBoundary::BetweenPages
        }
        LegKind::Rerank => CancellationBoundary::BeforeEmission,
    }
}

fn validate_recipe_specific_bounds(body: &RecipeBodyV1) -> Result<(), PlanError> {
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

fn validate_global_budget(budget: QueryExecutionBudget) -> Result<(), PlanError> {
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
        if leg.depends_on.iter().any(|dependency| *dependency >= leg.leg_id) {
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
        mix(&mut lanes, &u64::try_from(leg.leg_id).unwrap_or(u64::MAX).to_be_bytes());
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

fn leg_kind_tag(kind: LegKind) -> u8 {
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

//! Bounded plan admission, scheduling, execution, rank fusion, and cancellation.
//!
//! Raw backend outputs are nominations only. This package never turns them into
//! source-backed evidence and owns no concrete Qdrant, filesystem, process, or
//! control-store adapter.

#![forbid(unsafe_code)]
#![allow(
    clippy::large_enum_variant,
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::too_many_arguments,
    clippy::too_many_lines
)]

use core::cmp::Ordering;
use core::fmt;
use std::collections::{BTreeMap, BTreeSet};

use search_access::{
    AccessCheckpoint, AccessError, AccessPermit, ContaminationDecision,
    LiveSecurityState, RequestSecurityFence, recheck_live_access,
};
use search_contracts::{
    Blake3Digest32, CollectionRouteRevision, LegKind, OpaqueId, RequestId,
    SourceMembershipId,
};
use search_epoch_pins::{EpochPinGuard, EpochPinPurpose, PinError, PinRegistry, RouteIdentity};
use search_query_planner::{
    CompiledPlanDigest, CompiledSearchPlan, LegBudget, PlannedLeg,
};

/// Closed execution failure.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ExecuteError {
    PlanExpired,
    PlanDigestMismatch,
    QueueFull,
    BindingQuotaExceeded,
    DependencyIncomplete,
    AccessDenied,
    BudgetExceeded,
    Cancelled,
    DeadlineExceeded,
    PinAcquisitionFailed,
    BackendUnavailable,
    BackendFailure,
    InvalidNomination,
    PopulationMismatch,
    FusionBudgetExceeded,
    ContaminatedLeg,
    InvalidExecutionState,
}

impl ExecuteError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::PlanExpired => "EXECUTE_PLAN_EXPIRED",
            Self::PlanDigestMismatch => "EXECUTE_PLAN_DIGEST_MISMATCH",
            Self::QueueFull => "EXECUTE_QUEUE_FULL",
            Self::BindingQuotaExceeded => "EXECUTE_BINDING_QUOTA_EXCEEDED",
            Self::DependencyIncomplete => "EXECUTE_DEPENDENCY_INCOMPLETE",
            Self::AccessDenied => "EXECUTE_ACCESS_DENIED",
            Self::BudgetExceeded => "EXECUTE_BUDGET_EXCEEDED",
            Self::Cancelled => "EXECUTE_CANCELLED",
            Self::DeadlineExceeded => "EXECUTE_DEADLINE_EXCEEDED",
            Self::PinAcquisitionFailed => "EXECUTE_PIN_ACQUISITION_FAILED",
            Self::BackendUnavailable => "EXECUTE_BACKEND_UNAVAILABLE",
            Self::BackendFailure => "EXECUTE_BACKEND_FAILURE",
            Self::InvalidNomination => "EXECUTE_NOMINATION_INVALID",
            Self::PopulationMismatch => "EXECUTE_POPULATION_MISMATCH",
            Self::FusionBudgetExceeded => "EXECUTE_FUSION_BUDGET_EXCEEDED",
            Self::ContaminatedLeg => "EXECUTE_CONTAMINATED_LEG",
            Self::InvalidExecutionState => "EXECUTE_STATE_INVALID",
        }
    }
}

impl fmt::Display for ExecuteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for ExecuteError {}

impl From<AccessError> for ExecuteError {
    fn from(_: AccessError) -> Self {
        Self::AccessDenied
    }
}

impl From<PinError> for ExecuteError {
    fn from(_: PinError) -> Self {
        Self::PinAcquisitionFailed
    }
}

/// Scheduler lane.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SchedulerLane {
    Interactive,
    Verification,
    Background,
    Cleanup,
}

/// Finite queue snapshot used at admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SchedulerSnapshot {
    pub global_queued: usize,
    pub global_capacity: usize,
    pub binding_queued: usize,
    pub binding_capacity: usize,
}

/// Request-local execution authority.
pub struct ExecutionGuard {
    request_id: RequestId,
    plan_digest: CompiledPlanDigest,
    owner_id: OpaqueId,
    lane: SchedulerLane,
    deadline_tick: u64,
    max_candidates: usize,
    cancelled: bool,
}

impl ExecutionGuard {
    #[must_use]
    pub const fn request_id(&self) -> RequestId {
        self.request_id
    }

    #[must_use]
    pub const fn plan_digest(&self) -> CompiledPlanDigest {
        self.plan_digest
    }

    #[must_use]
    pub const fn owner_id(&self) -> &OpaqueId {
        &self.owner_id
    }

    #[must_use]
    pub const fn lane(&self) -> SchedulerLane {
        self.lane
    }

    #[must_use]
    pub const fn is_cancelled(&self) -> bool {
        self.cancelled
    }
}

impl fmt::Debug for ExecutionGuard {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExecutionGuard")
            .field("request_id", &self.request_id)
            .field("plan_digest", &self.plan_digest)
            .field("owner_id", &"<opaque>")
            .field("lane", &self.lane)
            .field("deadline_tick", &self.deadline_tick)
            .field("max_candidates", &self.max_candidates)
            .field("cancelled", &self.cancelled)
            .finish()
    }
}

/// Admits one immutable plan into finite process-local scheduler state.
pub fn admit(
    plan: &CompiledSearchPlan,
    owner_id: OpaqueId,
    lane: SchedulerLane,
    now_tick: u64,
    scheduler: SchedulerSnapshot,
) -> Result<ExecutionGuard, ExecuteError> {
    if scheduler.global_capacity == 0
        || scheduler.binding_capacity == 0
        || scheduler.global_queued >= scheduler.global_capacity
    {
        return Err(ExecuteError::QueueFull);
    }
    if scheduler.binding_queued >= scheduler.binding_capacity {
        return Err(ExecuteError::BindingQuotaExceeded);
    }
    let deadline_tick = now_tick
        .checked_add(plan.global_budget.deadline_ms)
        .ok_or(ExecuteError::DeadlineExceeded)?;
    let max_candidates = usize::try_from(plan.global_budget.max_validated_candidates)
        .map_err(|_| ExecuteError::BudgetExceeded)?;
    if max_candidates == 0 {
        return Err(ExecuteError::BudgetExceeded);
    }
    Ok(ExecutionGuard {
        request_id: plan.request_id,
        plan_digest: plan.digest,
        owner_id,
        lane,
        deadline_tick,
        max_candidates,
        cancelled: false,
    })
}

/// Dependency state supplied before scheduling a leg.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DependencyState {
    pub completed_leg_ids: BTreeSet<usize>,
    pub failed_leg_ids: BTreeSet<usize>,
}

/// Scheduled immutable leg ticket.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegTicket {
    pub request_id: RequestId,
    pub plan_digest: CompiledPlanDigest,
    pub owner_id: OpaqueId,
    pub leg: PlannedLeg,
    pub access_permit: AccessPermit,
    pub issued_at_tick: u64,
}

/// Checks dependencies, deadline, cancellation, live access, and budget before queueing.
pub fn schedule_leg(
    guard: &ExecutionGuard,
    leg: &PlannedLeg,
    dependencies: &DependencyState,
    request_security: &RequestSecurityFence,
    live_security: &LiveSecurityState,
    now_tick: u64,
) -> Result<LegTicket, ExecuteError> {
    if guard.cancelled {
        return Err(ExecuteError::Cancelled);
    }
    if now_tick >= guard.deadline_tick {
        return Err(ExecuteError::DeadlineExceeded);
    }
    if leg
        .depends_on
        .iter()
        .any(|dependency| !dependencies.completed_leg_ids.contains(dependency))
    {
        return Err(ExecuteError::DependencyIncomplete);
    }
    if leg
        .depends_on
        .iter()
        .any(|dependency| dependencies.failed_leg_ids.contains(dependency))
    {
        return Err(ExecuteError::DependencyIncomplete);
    }
    validate_leg_budget(leg.budget)?;
    let access_permit = recheck_live_access(
        request_security,
        live_security,
        AccessCheckpoint::BeforeLegDispatch,
    )?;
    Ok(LegTicket {
        request_id: guard.request_id,
        plan_digest: guard.plan_digest,
        owner_id: guard.owner_id.clone(),
        leg: leg.clone(),
        access_permit,
        issued_at_tick: now_tick,
    })
}

/// Process-local route/epoch pins for one indexed leg.
pub struct LegPinSet {
    epoch_pins: Vec<EpochPinGuard>,
}

impl LegPinSet {
    #[must_use]
    pub fn len(&self) -> usize {
        self.epoch_pins.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.epoch_pins.is_empty()
    }
}

/// Acquires only pins required by the scheduled indexed leg.
pub fn acquire_leg_pins(
    ticket: &LegTicket,
    registry: &PinRegistry,
    now_ms: u64,
) -> Result<LegPinSet, ExecuteError> {
    let Some(safe_leg) = &ticket.leg.safe_index_leg else {
        return Ok(LegPinSet {
            epoch_pins: Vec::new(),
        });
    };
    let route = RouteIdentity {
        collection_generation_id: safe_leg.route.collection_generation_id,
        route_revision: CollectionRouteRevision::new(safe_leg.route.route_generation),
    };
    let guard = registry.acquire_epoch_pin(
        route,
        safe_leg.route.visible_epoch,
        ticket.owner_id.clone(),
        EpochPinPurpose::Query,
        now_ms,
    )?;
    Ok(LegPinSet {
        epoch_pins: vec![guard],
    })
}

/// Raw untrusted candidate nomination.
#[derive(Clone, Debug, PartialEq)]
pub struct RawNomination {
    pub candidate_id: OpaqueId,
    pub point_id: [u8; 16],
    pub source_membership_id: SourceMembershipId,
    pub identity_digest: Blake3Digest32,
    pub payload_digest: Blake3Digest32,
    pub raw_score: f32,
    pub scoring_population_digest: Blake3Digest32,
}

/// Closed leg completion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LegCompletion {
    CompleteCandidateScope,
    PartialCandidateScope,
    Failed,
    Cancelled,
    TimedOut,
    Unavailable,
}

/// Bounded backend output; nominations are not evidence.
#[derive(Clone, Debug, PartialEq)]
pub struct LegOutput {
    pub leg_id: usize,
    pub leg_kind: LegKind,
    pub memberships: BTreeSet<SourceMembershipId>,
    pub security_generation: u64,
    pub scoring_population_digest: Option<Blake3Digest32>,
    pub nominations: Vec<RawNomination>,
    pub completion: LegCompletion,
}

/// Vendor-neutral dispatch seam.
pub trait LegBackend {
    type Error;

    fn dispatch(&mut self, ticket: &LegTicket) -> Result<LegOutput, Self::Error>;
}

/// Dispatches one leg and validates bounded untrusted output shape.
pub fn dispatch_leg<B: LegBackend>(
    ticket: &LegTicket,
    backend: &mut B,
) -> Result<LegOutput, ExecuteError> {
    let output = backend
        .dispatch(ticket)
        .map_err(|_| ExecuteError::BackendFailure)?;
    if output.leg_id != ticket.leg.leg_id
        || output.leg_kind != ticket.leg.leg_kind
        || output.memberships != ticket.leg.memberships
        || output.nominations.len()
            > usize::try_from(ticket.leg.budget.max_candidates)
                .map_err(|_| ExecuteError::BudgetExceeded)?
    {
        return Err(ExecuteError::InvalidNomination);
    }
    let mut ids = BTreeSet::new();
    for nomination in &output.nominations {
        if !nomination.raw_score.is_finite()
            || !output
                .memberships
                .contains(&nomination.source_membership_id)
            || !ids.insert(nomination.candidate_id.clone())
        {
            return Err(ExecuteError::InvalidNomination);
        }
        if let Some(population) = output.scoring_population_digest {
            if nomination.scoring_population_digest != population {
                return Err(ExecuteError::PopulationMismatch);
            }
        }
    }
    Ok(output)
}

/// Rank-normalized nomination inside one exact population.
#[derive(Clone, Debug, PartialEq)]
pub struct RankedNomination {
    pub nomination: RawNomination,
    pub rank: usize,
}

/// Output ranked only inside one scoring population.
#[derive(Clone, Debug, PartialEq)]
pub struct RankedLegOutput {
    pub leg_id: usize,
    pub population_digest: Blake3Digest32,
    pub nominations: Vec<RankedNomination>,
    pub completion: LegCompletion,
}

/// Normalizes raw scores only within one exact scoring population.
pub fn normalize_within_population(
    output: LegOutput,
) -> Result<RankedLegOutput, ExecuteError> {
    let population_digest = output
        .scoring_population_digest
        .ok_or(ExecuteError::PopulationMismatch)?;
    if output
        .nominations
        .iter()
        .any(|nomination| nomination.scoring_population_digest != population_digest)
    {
        return Err(ExecuteError::PopulationMismatch);
    }
    let mut nominations = output.nominations;
    nominations.sort_by(|left, right| {
        right
            .raw_score
            .partial_cmp(&left.raw_score)
            .unwrap_or(Ordering::Equal)
            .then_with(|| left.candidate_id.cmp(&right.candidate_id))
    });
    Ok(RankedLegOutput {
        leg_id: output.leg_id,
        population_digest,
        nominations: nominations
            .into_iter()
            .enumerate()
            .map(|(index, nomination)| RankedNomination {
                nomination,
                rank: index.saturating_add(1),
            })
            .collect(),
        completion: output.completion,
    })
}

/// Weighted reciprocal-rank fusion profile.
#[derive(Clone, Debug, PartialEq)]
pub struct FusionProfile {
    pub rank_constant: f64,
    pub leg_weights: BTreeMap<usize, f64>,
    pub max_output_candidates: usize,
}

/// Fused nomination with deterministic candidate identity tie-break.
#[derive(Clone, Debug, PartialEq)]
pub struct FusedNomination {
    pub nomination: RawNomination,
    pub fused_score: f64,
    pub contributing_legs: BTreeSet<usize>,
}

/// Cross-population fusion using ranks only; raw scores never cross populations.
pub fn fuse_safe_legs(
    outputs: &[RankedLegOutput],
    profile: &FusionProfile,
) -> Result<Vec<FusedNomination>, ExecuteError> {
    if !profile.rank_constant.is_finite()
        || profile.rank_constant <= 0.0
        || profile.max_output_candidates == 0
    {
        return Err(ExecuteError::FusionBudgetExceeded);
    }
    let mut fused: BTreeMap<OpaqueId, FusedNomination> = BTreeMap::new();
    for output in outputs {
        let weight = profile
            .leg_weights
            .get(&output.leg_id)
            .copied()
            .unwrap_or(1.0);
        if !weight.is_finite() || weight <= 0.0 {
            return Err(ExecuteError::FusionBudgetExceeded);
        }
        for ranked in &output.nominations {
            let contribution = weight
                / (profile.rank_constant
                    + f64::from(
                        u32::try_from(ranked.rank)
                            .map_err(|_| ExecuteError::FusionBudgetExceeded)?,
                    ));
            let entry = fused
                .entry(ranked.nomination.candidate_id.clone())
                .or_insert_with(|| FusedNomination {
                    nomination: ranked.nomination.clone(),
                    fused_score: 0.0,
                    contributing_legs: BTreeSet::new(),
                });
            if entry.nomination.identity_digest != ranked.nomination.identity_digest {
                return Err(ExecuteError::InvalidNomination);
            }
            entry.fused_score += contribution;
            entry.contributing_legs.insert(output.leg_id);
        }
    }
    let mut values = fused.into_values().collect::<Vec<_>>();
    values.sort_by(|left, right| {
        right
            .fused_score
            .partial_cmp(&left.fused_score)
            .unwrap_or(Ordering::Equal)
            .then_with(|| {
                left.nomination
                    .candidate_id
                    .cmp(&right.nomination.candidate_id)
            })
    });
    values.truncate(profile.max_output_candidates);
    Ok(values)
}

/// Mutable request-local execution state.
#[derive(Clone, Debug, Default)]
pub struct ExecutionState {
    pub outputs: BTreeMap<usize, RankedLegOutput>,
    pub failed_legs: BTreeMap<usize, ExecuteError>,
    pub discarded_legs: BTreeSet<usize>,
    pub omitted_legs: BTreeSet<usize>,
    pub cancelled: bool,
}

/// Drops every influenced leg as a whole.
pub fn discard_contaminated(
    decision: &ContaminationDecision,
    execution: &mut ExecutionState,
) -> ContaminationReceipt {
    let mut discarded = BTreeSet::new();
    if let ContaminationDecision::DiscardLegs(legs) = decision {
        for leg in legs {
            if execution.outputs.remove(leg).is_some() {
                discarded.insert(*leg);
            }
            execution.discarded_legs.insert(*leg);
        }
    }
    ContaminationReceipt {
        discarded_leg_ids: discarded,
    }
}

/// Content-free whole-leg discard receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContaminationReceipt {
    pub discarded_leg_ids: BTreeSet<usize>,
}

/// Truthful execution coverage over the planned leg denominator.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutionCoverage {
    pub planned_legs: usize,
    pub executed_legs: BTreeSet<usize>,
    pub failed_legs: BTreeSet<usize>,
    pub discarded_legs: BTreeSet<usize>,
    pub omitted_legs: BTreeSet<usize>,
    pub complete_candidate_scope: bool,
    pub cancelled: bool,
}

/// Classifies execution coverage without upgrading top-k saturation to complete scope.
#[must_use]
pub fn classify_completion(
    execution: &ExecutionState,
    plan: &CompiledSearchPlan,
) -> ExecutionCoverage {
    let executed_legs = execution.outputs.keys().copied().collect::<BTreeSet<_>>();
    let failed_legs = execution.failed_legs.keys().copied().collect::<BTreeSet<_>>();
    let all_complete = execution.outputs.values().all(|output| {
        output.completion == LegCompletion::CompleteCandidateScope
    });
    let complete_candidate_scope = all_complete
        && executed_legs.len() == plan.legs.len()
        && failed_legs.is_empty()
        && execution.discarded_legs.is_empty()
        && execution.omitted_legs.is_empty()
        && !execution.cancelled;
    ExecutionCoverage {
        planned_legs: plan.legs.len(),
        executed_legs,
        failed_legs,
        discarded_legs: execution.discarded_legs.clone(),
        omitted_legs: execution.omitted_legs.clone(),
        complete_candidate_scope,
        cancelled: execution.cancelled,
    }
}

/// Idempotently cancels request-local execution and releases owned pins by drop.
pub fn cancel(
    guard: &mut ExecutionGuard,
    execution: &mut ExecutionState,
) -> CancelOutcome {
    let already_cancelled = guard.cancelled;
    guard.cancelled = true;
    execution.cancelled = true;
    execution.outputs.clear();
    CancelOutcome {
        request_id: guard.request_id,
        already_cancelled,
    }
}

/// Content-free cancellation result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CancelOutcome {
    pub request_id: RequestId,
    pub already_cancelled: bool,
}

fn validate_leg_budget(budget: LegBudget) -> Result<(), ExecuteError> {
    if budget.deadline_ms == 0
        || budget.max_candidates == 0
        || budget.max_source_read_bytes == 0
        || budget.max_cpu_ms == 0
        || budget.max_memory_bytes == 0
    {
        Err(ExecuteError::BudgetExceeded)
    } else {
        Ok(())
    }
}

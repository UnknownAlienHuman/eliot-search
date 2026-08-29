# `search-query-planner` implementation packet

**Path:** `crates/search-query/search-query-planner`  
**Capability:** C22  
**Delivery:** W4 / P08  
**Gate:** BLOCKED until access compiler, route/publication and contract receipts are accepted  
**Trace:** S19.2-S19.3, S20-S21, S30.3, H15, P08  
**Direct public handoffs:** `search-contracts`, `search-domain`, `search-access`

Apply `../ASSIGNMENT_PROTOCOL.md`. Logical names below express required semantics, not mandatory Rust
spelling.

## Mission

Compile a versioned recipe and validated scope into one bounded, deterministic, server-owned SearchTaskPlan.

## Owns

- recipe normalization and recipe-specific plan compilation
- bounded retrieval leg graph
- state dependency capture and PlanFingerprint input
- budget allocation, priority lanes and exactness/coverage requirements

## Must not own

- accepting client-authored vendor plans or filters
- raw natural-language authority
- executing legs
- silent fallback to online/unregistered sources or optional providers

## Logical primitives

- NormalizedRecipeRequest, PlanContext, PlannedLeg, LegDependency, BudgetAllocation, SearchTaskPlanDraft, PlanDependencySet, PlannerDecision

## Logical operations

1. `normalize_recipe(request) -> Result<NormalizedRecipeRequest, PlanError>`
2. `compile_plan(request, grant, snapshot, capabilities) -> Result<SearchTaskPlan, PlanError>`
3. `allocate_leg_budgets(recipe, global_budget, legs) -> BudgetAllocation`
4. `capture_state_dependencies(context) -> PlanDependencySet`
5. `validate_plan_drift(plan, current) -> ReplanDecision`

## Required invariants

- plan is server-authored and contains no raw collection/point/vendor filter
- one plan binds one coherent source/workspace view and exact owner/security/route/profile generations
- equal load-bearing inputs yield same fingerprint and leg graph
- legs and budgets are bounded
- optional later legs are additive, explicit and capability-gated

## Typed failure surface

- `PLAN_COMPILATION_FAILED`
- `PLAN_FINGERPRINT_MISMATCH`
- `PLAN_STATE_DRIFT`
- `RESOURCE_EXHAUSTED`
- `REFERENCE_SCOPE_EMPTY`
- `RECIPE_NOT_SUPPORTED`

## Exit tests / evidence

- `equal_inputs_equal_plan_and_fingerprint`
- `client_vendor_plan_rejected`
- `bounded_leg_count_and_budget`
- `portfolio_partition_leg_compilation`
- `optional_provider_absence_truthful`
- `state_drift_requires_replan`

## Suggested internal modules

```text
search-query-planner/src/
  recipe.rs
  normalize.rs
  context.rs
  leg.rs
  budget.rs
  dependency.rs
  fingerprint.rs
  drift.rs
  error.rs
```

This is an internal file plan, not a request for more crates.

## Size / split

- Initial `src/` target: **≤ 7,500 hand-written lines**.
- Split review: **before 8,500 total hand-written Rust lines**.
- Hard stop: **10,000 including package-local tests**.
- Recipe-specific compilers may become internal modules; a new crate requires independent dependencies or lifecycle, not recipe count alone.

The handoff must let a downstream agent consume the public contract without reading implementation
internals or the architecture master.

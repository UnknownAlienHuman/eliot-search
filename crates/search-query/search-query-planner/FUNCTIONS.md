# Function contract — `search-query-planner`

**Status:** W4/P08 logical contract; no plan implementation exists yet.

The planner is server-owned. Clients supply one versioned recipe request and bounded preferences, never
Qdrant collections, filters, point IDs, raw execution graphs or authority predicates.

## Operations

### `normalize_recipe(request, contracts) -> Result<NormalizedRecipeRequest, PlanError>`

Accepts exactly one of the eleven v1 recipes. Normalization is recipe-specific, bounded, Unicode/profile
explicit and deterministic. Unknown fields or unsupported recipe/profile variants fail closed.

### `capture_query_snapshot(control_snapshot, source_view, overlay_state, capability_state) -> Result<QuerySnapshotFence, PlanError>`

Captures every Architecture S14 axis explicitly: catalog, membership, reference portfolio, access/live
deny, shadow, purge, overlay, observation cursor/freshness, source/workspace view, collection route,
visible epoch and lexical/profile generations. Direct-only plans use the contract's tagged route/epoch
absence form; generic dependency maps cannot replace named axes.

### `compile_plan(recipe, grant, authorized_scope, snapshot, capabilities, budget) -> Result<SearchTaskPlan, PlanError>`

Builds a finite typed leg DAG using accepted direct, lexical, exact and optional capability descriptors.
Each leg binds its safe eligibility plan, profile, route/epoch requirements, input/output limits,
dependencies and cancellation semantics. Optional legs are additive and explicitly unavailable when
not accepted.

### `allocate_leg_budgets(recipe, global_budget, leg_drafts) -> Result<BudgetAllocation, PlanError>`

Allocates finite deadline, candidate, source-read, CPU, memory and result budgets. Zero means disabled,
never unlimited. The sum of child ceilings cannot exceed the admitted server budget.

### `fingerprint_snapshot(snapshot) -> QuerySnapshotFingerprint`

### `fingerprint_plan(plan_without_fingerprint) -> PlanFingerprint`

Use deterministic canonical serialization and domain separation. Equal load-bearing inputs yield equal
bytes/fingerprints; any named-axis/profile/budget/leg change yields a different fingerprint.

### `validate_drift(plan, current_nonsecurity, current_security) -> PlanDriftDecision`

Classifies `VALID`, `REVALIDATE_SECURITY`, `REPLAN`, `EXPLICIT_INCOMPLETE` or `SNAPSHOT_EXPIRED`.
Restrictive security never waits for replan. Observation gaps cannot satisfy strict-current recipes.

## Semantics

Planning after captured inputs is pure, deterministic and retry-safe. Cancellation/budget exhaustion
returns no executable partial plan. No I/O, mutable registry, pin, scheduler or vendor client is owned.
Configuration implements `config/sections/query.md`; changed budget/profile settings affect only newly
compiled plans unless a stronger accepted reconfiguration action is required.

## Required fixtures

Exact eleven-recipe normalization; snapshot field completeness/no-hidden-axis; direct/index tagged
validity; equal-input golden fingerprints; one-axis drift changes fingerprint; strict currentness rejects
observation gaps; client vendor graph rejection; finite leg/budget property; portfolio partitioning;
optional provider absence truthful; deterministic plan bytes independent of map iteration.

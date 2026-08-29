# Agent contract — search-query-planner

You own only `crates/search-query/search-query-planner/`. Do not edit another package, the root workspace, or shared contracts.
When a missing contract blocks correct work, open a contract-change issue with the exact field,
invariant, producer, consumer and compatibility impact; do not patch around it.

The Architecture 8.4 master does not need to be loaded for ordinary work. This file contains the
package slice. Traceability only: S19.2-S19.3, S20, S21, S30.3, H15, P08.

## Mission

Compile a normalized recipe, coherent view, validated grant and budgets into an immutable vendor-neutral SearchTaskPlan.

## Ownership

- recipe normalization
- load-bearing dependency capture
- bounded leg graph
- PlanFingerprint
- deterministic ordering and replan triggers
- priority and budget assignment

## Forbidden ownership

- accepting raw client Qdrant plans
- database clients
- mixing source/workspace view revisions
- unbounded legs or queues
- embedding client admission authority
- hard dependency on subject-resolution or optional-provider implementations

## Allowed dependencies

`search-contracts`, `search-domain`, `search-access`. Additional internal or external dependencies require an explicit boundary review. Public
APIs may expose only `search-contracts` or package-owned opaque types; vendor types stay private.

## Required logical surface

These are behavior contracts, not mandated Rust syntax. Preserve the semantics even if the concrete API
is improved:

- `compile_plan(request, grant, snapshot, capabilities) -> Result<SearchTaskPlan, PlanError>`
- `normalize_recipe(request) -> NormalizedRecipe`
- `compile_leg_graph(plan_inputs) -> Result<LegGraph, PlanError>`
- `fingerprint_plan(load_bearing_fields) -> PlanFingerprint`
- `revalidate_or_replan(plan, latest_state) -> PlanDisposition`

## Failure surface

Use typed errors/reason codes. Relevant public reasons: `PLAN_STALE`, `REFERENCE_SCOPE_EMPTY`, `RESOURCE_EXHAUSTED`, `INCOMPLETE_COVERAGE`. Never turn a degraded or partial
state into an apparent success.

## Test seams and exit evidence

- `equal inputs produce identical plan fingerprint and leg graph`
- `plan contains no raw collection/filter/point IDs`
- `one coherent source/workspace view binds every leg`
- `budget caps scoring legs, prefetch and source reads`
- `load-bearing drift forces revalidation/replan/incomplete`
- `later subject/overlay/exact legs enter as typed capability inputs without changing the planner boundary`

Property/fault tests belong beside the owning behavior. Shared control-corpus fixtures may be requested,
but the writer does not edit another package opportunistically.

## Size and split guard

- Delivery wave: **W4 / P08**
- Soft `src/` target: **9,000 lines**
- Hard review threshold: **10,000 total hand-written Rust lines**
- Split on a real security, runtime, replacement, test or dependency boundary; never create a forwarding
  wrapper or crate-per-type shell.

## Definition of done

The package has a vendor-neutral public contract, deterministic tests for its invariants, explicit
degradation behavior, no forbidden dependency, and a handoff reporting commands and raw outcomes.
Compilation alone is insufficient.

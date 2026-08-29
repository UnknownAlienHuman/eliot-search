# `search-query-planner` implementation packet

**Path:** `crates/search-query/search-query-planner`  
**Capability:** C22  
**Delivery:** W4 / P08  
**Gate:** BLOCKED until access, snapshot/publication and contract/port receipts are accepted  
**Trace:** S14.1, S19.2-S19.3, S20-S21, S30.3, H15, P08

## Mission

Compile one normalized recipe and authorized scope into a bounded deterministic server-owned plan with
an explicit Architecture S14 `QuerySnapshotFence`.

## Owns

Recipe normalization, exact snapshot capture, bounded leg graph, source-owner/profile/additional
dependency capture, budget allocation, exactness requirements and plan fingerprint input.

## Must not own

Client-authored vendor plans/filters, raw natural-language authority, leg execution, implicit online or
unregistered fallback, optional-provider auto-admission, or generic hashes replacing catalog,
membership, access, shadow, purge, overlay, observation, view, route, epoch or lexical fields.

## Logical operations

1. `normalize_recipe(request) -> Result<NormalizedRecipeRequest, PlanError>`
2. `capture_query_snapshot(context) -> Result<QuerySnapshotFence, PlanError>`
3. `compile_plan(request, grant, snapshot, capabilities) -> Result<SearchTaskPlan, PlanError>`
4. `allocate_leg_budgets(recipe, global_budget, legs) -> BudgetAllocation`
5. `capture_additional_dependencies(context) -> BoundedList<StateDependency>`
6. `validate_plan_drift(plan, current) -> ReplanDecision`

## Invariants

- server-authored, no vendor collection/point/filter;
- one coherent source/workspace view and exact S14 axes;
- direct-only plan is the sole route/epoch-absent form;
- indexed leg requires generation/epoch and qualified lexical profiles;
- strict current workspace rejects observation gap;
- equal load-bearing inputs yield same snapshot/plan fingerprints and leg graph;
- generic dependencies cannot hide required axes;
- legs/budgets bounded; optional legs explicit and capability-gated.

## Exit evidence

Exact snapshot-field fixture; direct-only/indexed tagged validity; observation-gap currentness denial;
equal-input fingerprints; client vendor-plan rejection; bounded leg/budget; portfolio leg compilation;
optional absence truthful; every load-bearing drift classified as replan, security revalidation or
explicit incomplete result.

Target `src/` ≤7,500 lines; split review before 8,500 total; hard stop 10,000 including tests.

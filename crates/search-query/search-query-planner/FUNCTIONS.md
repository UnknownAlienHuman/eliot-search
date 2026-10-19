# search-query-planner — source audit and deletion map

The [query cutover](../README.md) owns the connected operation. This file maps the current 104,042-byte
single-file planner to the small amount of selection logic that may survive.

## Current API and caller reality

| Current source/API | Actual behavior |
|---|---|
| public capability/budget/leg/plan structures | Ordinary callers can construct the values which later functions treat as accepted execution authority. |
| `normalize_recipe(request)` | Validates and rebuilds one of eleven frozen V1 recipe/body variants. It does not select a real handler. |
| `direct_leg_support(recipe)` | Static recipe-to-leg support table. It can advertise a leg with no constructed service. |
| `canonical_execution_dependency(capabilities, scope, safe_index_legs)` | Builds another proof record from supplied capability/scope/leg inputs. |
| `compile_canonical_plan(task_plan, grant, recipe, scope, capabilities, safe_index_legs)` | Consumes a caller-created `SearchTaskPlan`, grant, capability set and safe legs; checks their relationships and returns a public compiled plan. It does not create a Qdrant/source operation. |
| plan fingerprinting | Uses package-local canonicalization/mixing over a subset of executable behavior. Strengthening it would still leave a disconnected protocol. |
| `allocate_leg_budgets(global, leg_count)` | Divides relative duration/work fields among legs, encouraging nested timeout reset and confusing parallel/dependent work. |
| `validate_drift(plan, observation)` | Compares caller-provided observations/fingerprints; it reads no current owner. |
| package callers | `compile_canonical_plan` was found in package tests/support, not normal daemon serving. |

The daemon separately contains a recipe registry, capability projection, grant issuer and authority owner,
with no production constructor found. Do not choose one framework and wrap the other; remove both.

## Replacement selection

The query owner receives:

```text
TypedQueryRequest
PublishedViewGuard
ResolvedScope
RequestContext
actual handler references
```

Baseline selection:

```text
Search   → direct Qdrant BM25 handler
Exact    → direct bounded retained-source scan
Continue → QueryWindow page
Expand   → QueryWindow selected-item neighborhood
```

Optional operations are included only when their concrete service exists. Unsupported modes fail before
backend/source work.

For actual multiple routes/profiles, a private helper may return an ordered finite list:

```text
select_operations(request, view, scope, handlers, limits)
    -> SmallVec<QueryOperation>
```

Rules:

1. `scope` is already non-widening and belongs to `view`; planning never accepts a second grant/snapshot.
2. compatible selected sources share one Qdrant population and one IDF corpus;
3. split only incompatible route/profile generations;
4. every branch receives a nomination ceiling inside the request's counters;
5. all branches share the same absolute deadline/cancellation;
6. actual Qdrant-compatible branches use native prefetch/fusion/grouping;
7. deterministic order is internal execution order, not a signed plan identity.

## Delete after caller cutover

- V1 recipe normalizer and static support matrix;
- public `CapabilitySet`, `SearchTaskPlan`, `CompiledSearchPlan`, `PlannedLeg`, `LegBudget` and related
  constructor/sealing/fingerprint types;
- canonical execution dependency and drift-observation protocols;
- grant, access checkpoint, overlap proof and safe-index-leg planner inputs;
- relative deadline division and per-leg freshness identity;
- compile/seal/golden-plan fixtures and source-shape tests;
- matching plan types from `search-contracts`, provider codecs and daemon authority composition.

Move the few useful route/profile grouping and finite-limit helpers to the direct query owner. Delete this
crate if no substantial independent algorithm remains.

## Verification

Verify that the current typed request reaches only constructed handlers, explicit/empty/unknown scope never
widens, compatible sources share the intended base population, incompatible profiles split deterministically,
all work uses one absolute context and unsupported optional modes perform no backend/source I/O.

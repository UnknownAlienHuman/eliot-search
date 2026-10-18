# Functions — search-query-planner

The planner is implemented in [src/lib.rs](src/lib.rs). This describes current source and the changes
needed by [query integration](../../../docs/implementation/README.md#4-simplify-and-connect-the-query);
the existing API is not a compatibility obligation.

## Implemented API

| Function | Current behavior |
|---|---|
| `normalize_recipe(request)` | Accepts `SearchRecipeRequest`, checks recipe/body agreement and recipe-specific bounds, returns `NormalizedRecipeRequest`. It currently uses the V1 vocabulary. |
| `canonical_execution_dependency(capabilities, authorized_scope, safe_index_legs)` | Builds the canonical dependency record later required by compilation. Retire this proof input with its consumers when simplifying local planning. |
| `compile_canonical_plan(task_plan, grant, recipe, authorized_scope, capabilities, safe_index_legs)` | Verifies supplied scope/grant/dependency/fingerprint data, then builds `CompiledSearchPlan` with bounded legs and omitted capabilities. It consumes `SearchTaskPlan`; it does not create one. |
| `allocate_leg_budgets(global, leg_count)` | Returns `Vec<LegBudget>`. Divides deadline/read/CPU/memory ceilings between legs; the candidate ceiling is already per leg. It allocates limits, not actual resource enforcement. |
| `validate_drift(plan, observation)` | Classifies an expired snapshot, restrictive changes, replanning or observation gaps using `DriftObservation`. |
| `direct_leg_support(recipe)` | Describes the current V1 recipe's DIRECT/exact/advanced support. This table does not prove a live handler exists. |

There is no implemented public `capture_query_snapshot`, `compile_plan`, `fingerprint_snapshot` or
`fingerprint_plan` matching the old draft contract. Compilation callers in the inspected tree are
tests/support code. The real query composition remains to be connected.

## Concrete handoff

The daemon resolves the user's scope and pins one published view. This owner derives the necessary
operations from that view, actual profile, query and finite limits. It must not obtain authorization
from a client-supplied graph, collection, filter or fingerprint.

Simplify the current compilation API and the daemon/executor consumers together. Keep useful pure leg
construction; remove local grant transcripts and externally manufactured plan/dependency hashes.
Restricted-client admission enforces its real delegated scope before this handoff.

Reuse `search-lexical`'s `encode_query` / `encode_code_query` under the published profile. Query and
IDF use the same resolved eligibility population. Ordinary retrieval returns bounded non-exhaustive
candidates; exact scanning is an explicit operation. Do not require nonzero exact-scan budgets for a
query that does not request it.

The runtime supplies one absolute deadline and cancellation; downstream operations enforce remaining
time and concrete byte/candidate/output limits. Optional modes need actual handlers before entering
a plan. A changed scope/view/profile must not be disguised by retaining an old fingerprint.

## Behavior to check

Check selected operation ordering, coherent scope/profile/view, finite limits, absent optional modes
and meaningful drift handling through the connected query. No frozen eleven-recipe inventory, golden
plan hash or source-shape assertion substitutes for retrieving and validating real source evidence.

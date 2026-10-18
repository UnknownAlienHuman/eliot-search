# search-query-planner — current API and removal boundary

## Current source

| API | Actual behavior |
|---|---|
| `normalize_recipe(request)` | Validates the frozen V1 recipe/body family and returns another recipe wrapper. |
| `canonical_execution_dependency(capabilities, scope, safe_index_legs)` | Builds a proof-shaped dependency record from supplied inputs. |
| `compile_canonical_plan(task_plan, grant, recipe, scope, capabilities, safe_index_legs)` | Validates supplied grants/fingerprints/dependencies and returns a public `CompiledSearchPlan`. It consumes, rather than creates, a caller `SearchTaskPlan`. |
| `allocate_leg_budgets(global, leg_count)` | Divides selected limits between supplied legs; it does not enforce backend work. |
| `validate_drift(plan, observation)` | Compares supplied observations/fingerprints and classifies drift. |
| `direct_leg_support(recipe)` | Static support table for eleven V1 recipe IDs; it does not prove a handler exists. |

Search of public main found `compile_canonical_plan` calls in package fixtures/support, not the serving
daemon. `CapabilitySet`, `PlannedLeg`, `CompiledSearchPlan` and `SearchTaskPlan` are publicly constructible;
hardening their digests would still leave a disconnected planning protocol.

## Replacement

The query owner already has the resolved scope, pinned `PublishedView`, typed request, real configured
handlers and finite limits. Select the operations directly from those values.

Baseline:

```text
Search → one BM25 query through the real bridge
Exact → one bounded retained-inventory scan
Continue / Expand → one QueryWindow operation
```

Optional `Orient`, `Compare`, dense retrieval or reranking enter only after their real services are
constructed. For a true multi-branch query, retain a private small operation list and useful budget/order
logic. No public task plan, capability record, dependency digest, plan fingerprint or persistent plan is
needed.

Change executor/daemon/request consumers in the same slice, then delete obsolete recipe normalization,
proof validation and static support-table fixtures. Restricted-client admission remains at its actual
connection/scope boundary and does not travel as a planner grant transcript.

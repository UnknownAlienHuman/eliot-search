# search-query-planner — remove the proof protocol

The product does not need a public planning authority. The query owner already has the pinned view,
resolved scope, typed request, actual constructed services and finite request context. Select the operations
directly from those values.

Read the [architecture](../../../docs/architecture/README.md),
[implementation guide](../../../docs/implementation/README.md), [query cutover](../README.md) and
[source map](FUNCTIONS.md).

## Verified current source

`src/lib.rs` is one 104,042-byte production file. It defines public capability, leg, budget and compiled
plan structures; validates caller-created grants, dependencies and fingerprints; normalizes eleven frozen
V1 recipes; divides relative deadline budgets between legs; and classifies drift from supplied
observations.

Repository search finds `compile_canonical_plan` in this package's own tests/support, not the serving daemon.
The daemon instead contains another recipe registry/authority host and one process-test indexed helper.
Hardening the plan digest or making the fields private would preserve two disconnected planning systems.

## Target

Baseline lexical search is direct:

```text
SearchRequest
+ PublishedViewGuard
+ ResolvedScope
+ RequestContext
+ real Qdrant runtime
→ one native BM25 query
```

No plan object is created merely to call one service.

A private operation list is justified only when multiple real operations exist:

```text
struct QueryOperation {
    route/profile
    concrete handler reference or closed internal kind
    bounded nomination/output ceiling
}
```

Useful pure selection is limited to:

- reject unsupported request modes before work;
- keep one pinned view and non-widening resolved scope;
- group compatible sources into one route/profile population;
- split only incompatible physical routes/profiles;
- choose implemented branches requested by the current request;
- assign finite branch ceilings inside one absolute request deadline;
- preserve deterministic operation and fusion order.

Compatible indexed branches should normally be one native Qdrant query with prefetch/RRF/grouping rather
than several public legs. A real non-Qdrant branch may add one private operation and a small rank merge.

## Remove with consumers

Delete:

- `CapabilitySet`, retrieval/advanced capability booleans and static recipe-support tables;
- caller-created `SearchTaskPlan`, `PlannedLeg`, `CompiledSearchPlan`, plan IDs and canonical fingerprints;
- grant/dependency/scope proof inputs and overlap-proof planning;
- `normalize_recipe` and the eleven-recipe V1 family after the current typed request replaces it;
- `canonical_execution_dependency`, `validate_drift` and supplied observation proofs;
- relative per-leg deadline allocation and persistent plan identity;
- fixtures whose only purpose is creating/sealing/revalidating those records.

Do not replace them with a V3 plan, capability descriptor, internal grant, synthetic dependency digest or
another registry. Availability is the actual constructed handler table. A restricted-client advertisement,
if enabled, is derived information and not planner authority.

## Keep or move

Keep only nontrivial pure helpers that remain after direct integration:

- deterministic incompatible-route/profile grouping;
- checked branch-limit allocation under the request's existing absolute deadline;
- deterministic operation order;
- explicit unsupported-mode errors.

Place these in query composition unless their independent implementation remains substantial. Do not retain
a crate around a one-leg plan.

## Completion

The local `Search` handler reaches the real Qdrant runtime without constructing a capability, grant, recipe,
dependency or compiled plan. Optional operations are unavailable until their real handlers exist.

Run minimal Rust 1.98 compiler/Clippy for changed query/daemon consumers after code. Planner fixture hashes
and proof-shape tests are removed with the APIs; they are not product verification.

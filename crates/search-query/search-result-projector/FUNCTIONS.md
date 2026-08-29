# Function contract — `search-result-projector`

**Status:** W4/P08 bounded result-projection contract; no implementation exists yet.

Only `ValidatedSearchCandidate` values may enter evidence-bearing result fields. Validation gaps,
failed/omitted legs and stale/security outcomes remain explicit non-evidence coverage records.

## Operations

### `project_candidate_set(validated, gaps, execution, plan, budget, handle_port, context) -> Result<SearchCandidateSet, ProjectError>`

Verifies result/plan/snapshot binding, computes truthful coverage, selects bounded handle subjects and
assembles compact candidate cards. Inputs are already source-backed; the projector performs no vendor
readback and cannot accept a raw nomination type.

### `project_recipe_result(recipe, product, context) -> Result<RecipeResultV1, ProjectError>`

Constructs exactly the tagged output variant for the normalized v1 recipe. Ambiguous subject results
cannot coexist with resolved evidence. Exact negative/completeness fields require an accepted exact
execution report and cannot be inferred from indexed top-k.

### `select_handle_subjects(validated, quotas) -> BoundedList<HandleSubject>`

Deterministically recommends normally 2–4 exact source/anchor subjects within grant and result budget.
It requests opaque handles through `HandleFactoryPort`; it never stores records or expands tokens.

### `project_coverage(execution, validation, plan) -> Coverage`

Reports candidate scope, freshness, executed/failed/omitted legs, validation gaps and unknowns.
`complete_scope` is structurally unavailable without exact-plane proof over the authoritative
denominator.

### `enforce_result_budget(draft, budget) -> Result<ProjectedResult, ProjectError>`

Applies deterministic ordering/truncation by source, candidate, variant, metadata and byte ceilings.
Every omission is reflected in coverage/reason metadata; no truncation is silent.

### `bind_result(result, plan, emission_permit) -> Result<BoundResult, ProjectError>`

Binds request/plan/snapshot, source-owner, access/live-deny, view, route/epoch and profile identities and
requires the final emission permit. Restrictive drift rejects or explicitly degrades rather than
emitting a stale binding.

## Failure and cancellation

Projection is bounded pure work except handle mint requests. Cancellation before final binding emits no
partial response advertised as complete. Handle mint failure may omit that handle with an explicit
reason if the recipe permits; it never converts an unvalidated candidate into evidence. Equal inputs
and quotas produce byte-identical ordering and truncation.

## Required fixtures

Golden compact cards and all eleven tagged results; raw nomination type cannot compile/pass; validation
gap has no excerpt/handle; full-file/unbounded chunk rejection; deterministic truncation with omission
receipt; default 2–4 exact handles; handle-factory failure; ambiguity excludes evidence; complete-scope
overclaim rejection; final security/result fence; no mutable handle state or vendor metadata.

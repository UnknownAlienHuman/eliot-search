# Agent contract — search-exact

You own only `crates/search-query/search-exact/`. Do not edit another package, the root workspace,
shared contracts or architecture. Missing fields use the contract-change process.

The Architecture 8.4 master is not required for ordinary work. This is the package slice.
Traceability only: S10.3, S25, H13, P12.

## Mission

Compile and execute bounded exact scans against a frozen authoritative denominator and produce truthful
proof reports through inventory/readback ports.

## Ownership

- `ExactScanPlan` compilation
- literal, safe-regex, qualified-symbol and structural predicate profiles
- frozen SourceRevision denominator
- execution completeness accounting
- negative-proof report semantics

## Forbidden ownership

- using indexed top-k as denominator
- semantic absence claims
- unbounded backtracking regex
- silently changing scope/revision during execution
- depending on concrete source registry, safe reader, revision store, redb or Qdrant packages

## Allowed dependencies

`search-contracts`, `search-domain`, `search-access`. Source inventory and revision byte access are
injected through `SourceInventoryPort`, `SourceRevisionStorePort` and bounded cancellation/budget ports.

## Required logical surface

- `compile_exact_scan(request, scope, inventory) -> Result<ExactScanPlan, ExactError>`
- `execute_exact_scan(plan, budget, cancel, readback) -> Result<ExactExecutionReport, ExactError>`
- `evaluate_completeness(report) -> ExactCoverage`
- `validate_predicate_profile(predicate) -> Result<(), ExactError>`

## Failure surface

Relevant reasons include `SCOPE_CHANGED_OR_REVISION_UNAVAILABLE`, `INCOMPLETE_COVERAGE`,
`EXACT_PREDICATE_UNSUPPORTED`, `EXACT_BUDGET_EXHAUSTED` and `CANCELLED`.

## Test seams and exit evidence

- `complete literal negative requires every denominator item`
- `unreadable/changed item prevents NoMatchInCompleteScope`
- `safe non-backtracking regex is size/time bounded`
- `raw-bytes and decoded-text semantics are distinct`
- `cancellation and scope drift yield incomplete report`
- `semantic-overclaim fixture is rejected`
- `fake inventory/readback ports prove no concrete store dependency`

## Size and split guard

- Delivery wave: **W6 / P12**
- Soft `src/` target: **8,500 lines**
- Hard review threshold: **10,000 hand-written Rust lines**
- Keep one proof owner. Split only if a replaceable predicate engine or measured line growth creates a
  real boundary.

## Definition of done

Every proof binds a frozen authoritative denominator, all omissions are reported and concrete storage
or index adapters are absent from the package graph.

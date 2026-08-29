# Agent contract — search-exact

You own only `crates/search-query/search-exact/`. Do not edit another package, the root workspace, or shared contracts.
When a missing contract blocks correct work, open a contract-change issue with the exact field,
invariant, producer, consumer and compatibility impact; do not patch around it.

The Architecture 8.4 master does not need to be loaded for ordinary work. This file contains the
package slice. Traceability only: S10.3, S25, H13, P12.

## Mission

Compile and execute bounded exact scans against a frozen authoritative denominator and produce truthful proof reports.

## Ownership

- ExactScanPlan compilation
- literal, safe regex, qualified-symbol and structural predicates
- frozen SourceRevision denominator
- execution completeness accounting
- negative-proof report semantics

## Forbidden ownership

- using indexed top-k as denominator
- semantic absence claims
- unbounded backtracking regex
- silently changing scope or revision during execution

## Allowed dependencies

`search-contracts`, `search-domain`, `search-source-registry`, `search-revision-store`, `search-safe-reader`, `search-access`. Additional internal or external dependencies require an explicit boundary review. Public
APIs may expose only `search-contracts` or package-owned opaque types; vendor types stay private.

## Required logical surface

These are behavior contracts, not mandated Rust syntax. Preserve the semantics even if the concrete API
is improved:

- `compile_exact_scan(request, scope, inventory) -> Result<ExactScanPlan, ExactError>`
- `execute_exact_scan(plan, budget, cancel) -> Result<ExactExecutionReport, ExactError>`
- `evaluate_completeness(report) -> ExactCoverage`
- `validate_predicate_profile(predicate) -> Result<(), ExactError>`

## Failure surface

Use typed errors/reason codes. Relevant public reasons: `SCOPE_CHANGED_OR_REVISION_UNAVAILABLE`, `INCOMPLETE_COVERAGE`, `EXACT_PREDICATE_UNSUPPORTED`. Never turn a degraded or partial
state into an apparent success.

## Test seams and exit evidence

- `complete literal negative requires every denominator item`
- `unreadable/changed item prevents NoMatchInCompleteScope`
- `safe non-backtracking regex is size/time bounded`
- `raw-bytes and decoded-text semantics are distinct`
- `cancellation and scope drift yield incomplete report`
- `semantic-overclaim fixture is rejected`

Property/fault tests belong beside the owning behavior. Shared control-corpus fixtures may be requested,
but the writer does not edit another package opportunistically.

## Size and split guard

- Delivery wave: **W6 / P12**
- Soft `src/` target: **8,500 lines**
- Hard review threshold: **10,000 total hand-written Rust lines**
- Split on a real security, runtime, replacement, test or dependency boundary; never create a forwarding
  wrapper or crate-per-type shell.

## Definition of done

The package has a vendor-neutral public contract, deterministic tests for its invariants, explicit
degradation behavior, no forbidden dependency, and a handoff reporting commands and raw outcomes.
Compilation alone is insufficient.

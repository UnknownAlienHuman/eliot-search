# Agent contract — search-result-projector

You own only `crates/search-query/search-result-projector/`. Do not edit another package, the root workspace, or shared contracts.
When a missing contract blocks correct work, open a contract-change issue with the exact field,
invariant, producer, consumer and compatibility impact; do not patch around it.

The Architecture 8.4 master does not need to be loaded for ordinary work. This file contains the
package slice. Traceability only: S23.2-S23.3, S26, H15.4, P08.

## Mission

Project validated candidates, comparison and exact reports into bounded evidence-oriented responses and handles.

## Ownership

- SearchCandidateSet assembly
- coverage/freshness/gap semantics
- default 2-4 recommended handles
- bounded non-content ranking trace
- result byte and disclosure budgets

## Forbidden ownership

- raw full files or unbounded chunk arrays
- Qdrant collections, filters, offsets or payload exposure
- belief/admission/finish dispositions
- calling top-k coverage complete_scope

## Allowed dependencies

`search-contracts`, `search-domain`, `search-candidate-validator`. Additional internal or external dependencies require an explicit boundary review. Public
APIs may expose only `search-contracts` or package-owned opaque types; vendor types stay private.

## Required logical surface

These are behavior contracts, not mandated Rust syntax. Preserve the semantics even if the concrete API
is improved:

- `project_candidate_set(validated, plan, budget) -> Result<SearchCandidateSet, ProjectError>`
- `project_comparison(matrix, plan, budget) -> Result<ComparisonResult, ProjectError>`
- `project_exact_report(report, plan, budget) -> Result<ExactResult, ProjectError>`
- `enforce_disclosure_and_size(response, grant) -> Result<(), ProjectError>`

## Failure surface

Use typed errors/reason codes. Relevant public reasons: `RESULT_BUDGET_EXCEEDED`, `INCOMPLETE_COVERAGE`, `DISCLOSURE_CEILING_EXCEEDED`. Never turn a degraded or partial
state into an apparent success.

## Test seams and exit evidence

- `default result contains bounded recommended handles`
- `raw files and unbounded chunks are impossible`
- `candidate_scope is not relabeled complete_scope`
- `response binds plan/view/owner/security generations`
- `authorized display metadata is resolved only after retrieval`
- `comparison and exact reports enter as contract products without hard package coupling`

Property/fault tests belong beside the owning behavior. Shared control-corpus fixtures may be requested,
but the writer does not edit another package opportunistically.

## Size and split guard

- Delivery wave: **W4 / P08**
- Soft `src/` target: **7,000 lines**
- Hard review threshold: **10,000 total hand-written Rust lines**
- Split on a real security, runtime, replacement, test or dependency boundary; never create a forwarding
  wrapper or crate-per-type shell.

## Definition of done

The package has a vendor-neutral public contract, deterministic tests for its invariants, explicit
degradation behavior, no forbidden dependency, and a handoff reporting commands and raw outcomes.
Compilation alone is insufficient.

# Agent contract — search-result-projector

You own only `crates/search-query/search-result-projector/`. Do not edit another package, the root
workspace, shared contracts or architecture. Missing fields use the contract-change process.

The Architecture 8.4 master is not required for ordinary work. This is the package slice.
Traceability only: S23.2-S23.3, S26, H15.4, P08.

## Mission

Project validated candidates, comparison and exact reports into bounded evidence responses while
delegating handle state to `search-handles`.

## Ownership

- `SearchCandidateSet` assembly
- coverage/freshness/gap semantics
- selection of the default 2–4 recommended handle subjects
- bounded non-content ranking trace
- result byte and disclosure budgets
- requesting opaque handles from `HandleFactoryPort`

## Forbidden ownership

- raw full files or unbounded chunk arrays
- Qdrant collections, filters, offsets or payload exposure
- belief/admission/finish dispositions
- calling top-k coverage `complete_scope`
- storing, expanding or authorizing handles

## Allowed dependencies

`search-contracts`, `search-domain`, `search-candidate-validator`, `search-handles`.
The dependency permits handle minting through the public contract; handle mutable state remains owned
by `search-handles`.

## Required logical surface

- `project_candidate_set(validated, plan, budget, handles) -> Result<SearchCandidateSet, ProjectError>`
- `project_comparison(matrix, plan, budget) -> Result<ComparisonResult, ProjectError>`
- `project_exact_report(report, plan, budget) -> Result<ExactResult, ProjectError>`
- `select_handle_subjects(validated, limits) -> HandleSubjectSet`
- `enforce_disclosure_and_size(response, grant) -> Result<(), ProjectError>`

## Failure surface

Relevant reasons include `RESULT_BUDGET_EXCEEDED`, `INCOMPLETE_COVERAGE`,
`DISCLOSURE_CEILING_EXCEEDED` and `HANDLE_CREATION_FAILED`.

## Test seams and exit evidence

- `default result contains bounded recommended handles`
- `raw files and unbounded chunks are impossible`
- `candidate_scope is not relabeled complete_scope`
- `response binds plan/view/owner/security generations`
- `authorized display metadata is resolved only after retrieval`
- `projector owns no handle table and cannot expand a handle`

## Size and split guard

- Delivery wave: **W4 / P08**
- Soft `src/` target: **7,000 lines**
- Hard review threshold: **10,000 hand-written Rust lines**

## Definition of done

Responses are bounded and truthful, handle subjects are selected deterministically, and all handle
state/authorization remains in `search-handles`.

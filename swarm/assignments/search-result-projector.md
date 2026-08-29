# `search-result-projector` implementation packet

**Path:** `crates/search-query/search-result-projector`  
**Capability:** C26  
**Delivery:** W4 / P08  
**Gate:** BLOCKED until candidate-validator and handle-factory handoffs are accepted  
**Trace:** S23.2-S23.3, S26, H15.4, P08  
**Direct public handoffs:** `search-contracts`, `search-domain`, `search-candidate-validator`, `search-handles`

Apply `../ASSIGNMENT_PROTOCOL.md`. Logical names below express required semantics, not mandatory Rust spelling.

## Mission

Project validated candidates into compact bounded response cards while delegating all mutable handle state and expansion authorization to `search-handles`.

## Owns

- compact candidate/result-card shaping
- response byte/source/variant budgets
- coverage/freshness/gap projection
- deterministic selection of handle subjects
- handle creation requests through `HandleFactoryPort`

## Must not own

- raw full-file or unbounded chunk dumps
- unvalidated Qdrant points or client admission/belief dispositions
- hiding omitted/failed legs or freshness gaps
- handle tables, durable eligibility, expansion authorization or revocation

## Logical primitives

- `ProjectionRequest`, `ResultBudget`, `CandidateCard`, `NavigationSummary`, `VariantCard`, `CoverageCard`, `HandleSubject`, `HandleCreationRequest`, `ProjectedResult`

## Logical operations

1. `project_candidate_set(validated, execution, budget, handles) -> Result<SearchCandidateSet, ProjectError>`
2. `select_handle_subjects(candidates, quotas) -> Vec<HandleSubject>`
3. `project_coverage(execution, validation) -> Coverage`
4. `enforce_result_budget(result, budget) -> Result<ProjectedResult, ProjectError>`

## Required invariants

- default card contains bounded 2-4 recommended exact handles
- `complete_scope` appears only with exact-plane proof
- every card binds plan/view/owner/security generations
- omissions and failures remain explicit
- no raw vendor metadata or reusable authorization decision escapes
- projector owns no mutable handle record and cannot expand a handle

## Typed failure surface

- `RESULT_BUDGET_EXHAUSTED`
- `INCOMPLETE_COVERAGE`
- `HANDLE_CREATION_FAILED`
- `RESULT_BINDING_MISMATCH`
- `NO_VALIDATED_CANDIDATES`

## Exit tests / evidence

- `golden_compact_card`
- `full_file_dump_rejected`
- `coverage_binding_fixture`
- `complete_scope_requires_exact_report`
- `security_generation_in_every_result`
- `deterministic_budget_truncation`
- `projector_has_no_handle_table_or_expansion_path`

## Suggested internal modules

```text
search-result-projector/src/
  budget.rs
  summary.rs
  candidate.rs
  variant.rs
  coverage.rs
  handle_request.rs
  binding.rs
  error.rs
```

## Size / split

- Initial `src/` target: **≤ 7,000 hand-written lines**.
- Split review: **before 8,500 total hand-written Rust lines**.
- Hard stop: **10,000 including package-local tests**.

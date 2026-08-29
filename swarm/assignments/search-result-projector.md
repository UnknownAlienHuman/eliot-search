# `search-result-projector` implementation packet

**Path:** `crates/search-query/search-result-projector`  
**Capability:** C26  
**Delivery:** W4 / P08  
**Gate:** BLOCKED until candidate validator and contracts are accepted  
**Trace:** S23.2-S23.3, S26, H15.4, P08  
**Direct public handoffs:** `search-contracts`, `search-domain`, `search-candidate-validator`

Apply `../ASSIGNMENT_PROTOCOL.md`. Logical names below express required semantics, not mandatory Rust
spelling.

## Mission

Project validated candidates into compact, bounded response cards, provider-local handles and truthful coverage.

## Owns

- compact candidate/result card shaping
- response byte/source/variant budgets
- coverage/freshness/gap projection
- ephemeral handle creation requests and validation receipt binding

## Must not own

- raw full-file or unbounded chunk dumps
- emitting unvalidated Qdrant points
- client admission/belief dispositions
- hiding omitted/failed legs or freshness gaps

## Logical primitives

- ProjectionRequest, ResultBudget, CandidateCard, NavigationSummary, VariantCard, CoverageCard, HandleCreationRequest, ProjectedResult

## Logical operations

1. `project_candidate_set(validated, execution, budget) -> Result<SearchCandidateSet, ProjectError>`
2. `select_recommended_handles(candidates, quotas) -> Vec<SearchSourceHandle>`
3. `project_coverage(execution, validation) -> Coverage`
4. `enforce_result_budget(result, budget) -> Result<ProjectedResult, ProjectError>`

## Required invariants

- default card contains bounded 2-4 recommended exact handles
- complete_scope appears only with exact-plane proof
- every card binds plan fingerprint/view/owner/security generations
- omissions and failures remain explicit
- no raw Qdrant metadata or reusable authorization decision escapes

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

## Suggested internal modules

```text
search-result-projector/src/
  budget.rs
  summary.rs
  candidate.rs
  variant.rs
  coverage.rs
  handle.rs
  binding.rs
  error.rs
```

This is an internal file plan, not a request for more crates.

## Size / split

- Initial `src/` target: **≤ 7,000 hand-written lines**.
- Split review: **before 8,500 total hand-written Rust lines**.
- Hard stop: **10,000 including package-local tests**.
- Comparison-specific output may remain its own contract in comparator; generic projection stays compact and capability-neutral.

The handoff must let a downstream agent consume the public contract without reading implementation
internals or the architecture master.

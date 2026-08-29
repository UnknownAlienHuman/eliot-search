# `search-subject-resolver` implementation packet

**Path:** `crates/search-query/search-subject-resolver`  
**Capability:** C21  
**Delivery:** W6 / P11  
**Gate:** BLOCKED until exact, structural and lexical candidate contracts are accepted  
**Trace:** S21.1, S22, P11  
**Direct public handoffs:** `search-contracts`, `search-domain`

Apply `../ASSIGNMENT_PROTOCOL.md`. Logical names below express required semantics, not mandatory Rust
spelling.

## Mission

Resolve a requested entity under an explicit context through a deterministic ladder and return bounded ambiguity instead of guessing.

## Owns

- subject query normalization
- resolution ladder and match-basis classification
- bounded ambiguity set
- subject resolution receipt tied to source view and plan

## Must not own

- normative selection among materially different definitions
- query execution/storage
- assuming same name means same subject
- implicit workspace/reference scope

## Logical primitives

- SubjectRequest, SubjectContext, SubjectCandidate, SubjectMatchBasis, SubjectResolution, SubjectAmbiguitySet, ResolutionReceipt

## Logical operations

1. `normalize_subject_request(request) -> NormalizedSubjectRequest`
2. `resolve_subject(request, candidate_sources, budget) -> Result<SubjectResolution, SubjectError>`
3. `rank_resolution_basis(candidate) -> SubjectMatchBasis`
4. `build_ambiguity_set(candidates, limit) -> SubjectAmbiguitySet`

## Required invariants

- ladder order: explicit handle/cursor, qualified key, exact normalized name, signature/kind, structural/lexical
- material ambiguity is returned, not silently collapsed
- resolution binds one coherent SourceView/WorkspaceViewRevision
- same-name false positives remain distinguishable
- result set is bounded and deterministically ordered

## Typed failure surface

- `AMBIGUOUS_SUBJECT`
- `SUBJECT_NOT_FOUND`
- `SUBJECT_CONTEXT_STALE`
- `SUBJECT_BUDGET_EXHAUSTED`
- `SUBJECT_SCOPE_EMPTY`

## Exit tests / evidence

- `explicit_handle_wins`
- `qualified_key_before_name`
- `same_name_false_positive`
- `renamed_true_analogue_basis`
- `bounded_ambiguity_set`
- `view_drift_invalidates_resolution`

## Suggested internal modules

```text
search-subject-resolver/src/
  request.rs
  normalize.rs
  ladder.rs
  match_basis.rs
  ambiguity.rs
  receipt.rs
  error.rs
```

This is an internal file plan, not a request for more crates.

## Size / split

- Initial `src/` target: **≤ 6,000 hand-written lines**.
- Split review: **before 8,500 total hand-written Rust lines**.
- Hard stop: **10,000 including package-local tests**.
- Keep ladder and ambiguity together; retrieval of candidates belongs to executor.

The handoff must let a downstream agent consume the public contract without reading implementation
internals or the architecture master.

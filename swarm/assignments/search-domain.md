# `search-domain` implementation packet

**Path:** `crates/search-domain`  
**Capability:** shared pure invariant kernel  
**Delivery:** W0 / P00  
**Gate:** AUTHORIZED after the accepted search-contracts schema handoff  
**Trace:** S3-S5, S8.3, S10, S13.3, S14, S19-S23, S36, H4, P00  
**Direct public handoffs:** `search-contracts`

Apply `../ASSIGNMENT_PROTOCOL.md`. Logical names below express required semantics, not mandatory Rust
spelling.

## Mission

Implement pure state transitions and deterministic decision rules over search-contracts without owning an external capability.

## Owns

- pure ownership/publication/security transition functions
- eligibility/filter abstract syntax and equivalence rules
- canonical plan-fingerprint, stable ordering and coverage classification rules
- reusable invariant predicates that have no I/O or capability owner

## Must not own

- I/O, clocks, random generation, process handles or vendor clients
- source acquisition, query execution, publication transport or access state ownership
- becoming a dumping ground for capability-specific algorithms

## Logical primitives

- SourceOwnershipState and SourceOwnershipCommand
- PublicationState and PublicationEvent
- EligibilityAst and SecurityFenceIdentity
- StableCandidateOrderKey, CoverageEvidence and CoverageClass
- InvariantViolation and DomainDecision

## Logical operations

1. `transition_source_ownership(state, command) -> Result<state, DomainError>`
2. `transition_publication(state, event) -> Result<state, DomainError>`
3. `build_base_eligibility_predicate(fence) -> EligibilityAst`
4. `prove_retrieval_idf_filter_equivalence(retrieval, idf) -> Result<(), DomainError>`
5. `compute_plan_fingerprint(load_bearing_inputs) -> PlanFingerprint`
6. `stable_candidate_order(left, right) -> Ordering`
7. `classify_coverage(execution_evidence) -> Coverage`

## Required invariants

- state machines reject skipped, reverse and dual-owner transitions
- retrieval and IDF base predicates are semantically identical
- equal fingerprint inputs yield equal leg graph identity and total result ordering
- coverage can be complete_scope only with an accepted exact execution proof
- pure functions do not observe wall-clock time or mutable external state

## Typed failure surface

- `INVALID_STATE_TRANSITION`
- `INVARIANT_VIOLATION`
- `PLAN_FINGERPRINT_MISMATCH`
- `COVERAGE_OVERCLAIM`

## Exit tests / evidence

- `ownership_state_machine_property_suite`
- `publication_transition_property_suite`
- `eligibility_ast_equivalence_property`
- `candidate_order_is_total_stable_and_transitive`
- `coverage_never_upgrades_without_exact_proof`
- `dependency_guard_has_only_search_contracts`

## Suggested internal modules

```text
search-domain/src/
  ownership.rs
  publication.rs
  eligibility.rs
  fingerprint.rs
  ordering.rs
  coverage.rs
  error.rs
```

This is an internal file plan, not a request for more crates.

## Size / split

- Initial `src/` target: **≤ 7,000 hand-written lines**.
- Split review: **before 8,500 total hand-written Rust lines**.
- Hard stop: **10,000 including package-local tests**.
- Move a rule out when it begins to own capability state, I/O, a vendor dependency or an independently replaceable policy.

The handoff must let a downstream agent consume the public contract without reading implementation
internals or the architecture master.

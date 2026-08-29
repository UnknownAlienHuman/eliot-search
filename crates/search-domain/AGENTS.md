# Agent contract — search-domain

You own only `crates/search-domain/`. Do not edit another package, the root workspace, or shared contracts.
When a missing contract blocks correct work, open a contract-change issue with the exact field,
invariant, producer, consumer and compatibility impact; do not patch around it.

The Architecture 8.4 master does not need to be loaded for ordinary work. This file contains the
package slice. Traceability only: S3-S5, S8.3, S10, S13.3, S14, S21.3, S23, S36, H1.2, H4, P00.

## Mission

Implement pure state transitions and deterministic decision rules over search-contracts types without owning any external capability.

## Ownership

- pure validation and transition functions
- canonical ordering and plan-fingerprint rules
- eligibility/filter AST semantics
- coverage classification and invariant proofs

## Forbidden ownership

- I/O, clocks, process handles or vendor clients
- becoming a dumping ground for capability-specific logic
- owning source, query, publication or access state

## Allowed dependencies

`search-contracts`. Additional internal or external dependencies require an explicit boundary review. Public
APIs may expose only `search-contracts` or package-owned opaque types; vendor types stay private.

## Required logical surface

These are behavior contracts, not mandated Rust syntax. Preserve the semantics even if the concrete API
is improved:

- `validate_epoch_transition(current, next) -> Result<(), DomainError>`
- `transition_source_ownership(state, command) -> Result<State, DomainError>`
- `transition_publication(state, event) -> Result<State, DomainError>`
- `build_base_eligibility_predicate(fence) -> EligibilityAst`
- `compute_plan_fingerprint(inputs) -> PlanFingerprint`
- `stable_candidate_order(a, b) -> Ordering`
- `classify_coverage(execution) -> Coverage`

## Failure surface

Use typed errors/reason codes. Relevant public reasons: `INVALID_STATE_TRANSITION`, `INVARIANT_VIOLATION`, `PLAN_FINGERPRINT_MISMATCH`. Never turn a degraded or partial
state into an apparent success.

## Test seams and exit evidence

- `state machines reject skipped or reverse transitions`
- `retrieval and IDF base-filter ASTs are equivalent`
- `equal plan inputs yield equal fingerprints and leg ordering`
- `candidate tie-break is total and stable`
- `pure kernel has no forbidden dependencies`

Property/fault tests belong beside the owning behavior. Shared control-corpus fixtures may be requested,
but the writer does not edit another package opportunistically.

## Size and split guard

- Delivery wave: **W0 / P00**
- Soft `src/` target: **7,000 lines**
- Hard review threshold: **10,000 total hand-written Rust lines**
- Split on a real security, runtime, replacement, test or dependency boundary; never create a forwarding
  wrapper or crate-per-type shell.

## Definition of done

The package has a vendor-neutral public contract, deterministic tests for its invariants, explicit
degradation behavior, no forbidden dependency, and a handoff reporting commands and raw outcomes.
Compilation alone is insufficient.

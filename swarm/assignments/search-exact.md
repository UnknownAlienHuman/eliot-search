# `search-exact` implementation packet

**Path:** `crates/search-query/search-exact`  
**Capability:** C20  
**Delivery:** W6 / P12  
**Gate:** BLOCKED until source inventory/readback and access receipts are accepted  
**Trace:** S25, H15.2, P12  
**Direct public handoffs:** `search-contracts`, `search-domain`, `search-source-registry`, `search-revision-store`, `search-safe-reader`, `search-access`

Apply `../ASSIGNMENT_PROTOCOL.md`. Logical names below express required semantics, not mandatory Rust
spelling.

## Mission

Compile and execute frozen-denominator exact scans that can support narrowly stated complete-scope claims.

## Owns

- exact predicate profiles and serialization
- authoritative denominator freeze
- bounded compile/execute split
- exact execution report and complete-negative eligibility decision

## Must not own

- using indexed top-k as denominator
- semantic absence claims
- unbounded backtracking regex
- claiming completeness on unreadable, changed, cancelled or partially authorized scope

## Logical primitives

- ExactPredicate, ExactPredicateProfile, ExactDenominator, ExactScanPlan, ExactItemOutcome, ExactExecutionReport, CompleteNegativeDecision

## Logical operations

1. `compile_exact_scan(request, inventory, grant) -> Result<ExactScanPlan, ExactError>`
2. `execute_exact_scan(plan, reader, revision_store, budget, cancel) -> Result<ExactExecutionReport, ExactError>`
3. `evaluate_complete_negative(report) -> CompleteNegativeDecision`
4. `validate_plan_fence(plan, current_state) -> Result<(), ExactError>`

## Required invariants

- denominator is frozen from authoritative source inventory and exact revision IDs
- every denominator item is read or has explicit failure
- complete_scope requires no drift, unreadable item, timeout, cancellation or provider gap
- predicate proves only its declared semantics/input domain
- regex engine/profile is pinned non-backtracking and bounded

## Typed failure surface

- `SCOPE_CHANGED_OR_REVISION_UNAVAILABLE`
- `EXACT_DENOMINATOR_INCOMPLETE`
- `EXACT_SCAN_CANCELLED`
- `EXACT_SCAN_BUDGET_EXHAUSTED`
- `EXACT_PREDICATE_UNSUPPORTED`
- `INCOMPLETE_COVERAGE`

## Exit tests / evidence

- `complete_literal_negative`
- `safe_regex_profile`
- `raw_bytes_vs_decoded_text`
- `unreadable_item_blocks_complete_negative`
- `scope_drift_blocks_complete_negative`
- `cancelled_scan_truthful_partial`
- `semantic_overclaim_rejected`

## Suggested internal modules

```text
search-exact/src/
  predicate.rs
  compile.rs
  denominator.rs
  execute.rs
  report.rs
  complete.rs
  regex.rs
  error.rs
```

This is an internal file plan, not a request for more crates.

## Size / split

- Initial `src/` target: **≤ 7,500 hand-written lines**.
- Split review: **before 8,500 total hand-written Rust lines**.
- Hard stop: **10,000 including package-local tests**.
- Compile and execute remain one exactness contract while sharing predicate/denominator semantics; split only if executor runtime becomes independently replaceable.

The handoff must let a downstream agent consume the public contract without reading implementation
internals or the architecture master.

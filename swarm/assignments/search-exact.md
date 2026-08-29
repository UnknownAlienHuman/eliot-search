# `search-exact` implementation packet

**Path:** `crates/search-query/search-exact`  
**Capability:** C20  
**Delivery:** W6 / P12  
**Gate:** BLOCKED until authoritative inventory, readback and access-port handoffs are accepted  
**Trace:** S10.3, S25, H13, P12  
**Direct public handoffs:** `search-contracts`, `search-domain`, `search-access`

Apply `../ASSIGNMENT_PROTOCOL.md`. Logical names below express required semantics, not mandatory Rust spelling.

## Mission

Compile and execute bounded exact scans against a frozen authoritative denominator and produce truthful proof reports through inventory/readback ports.

## Owns

- `ExactScanPlan` compilation
- literal, safe-regex, qualified-symbol and structural predicate profiles
- frozen SourceRevision denominator
- execution completeness accounting
- negative-proof report semantics

## Must not own

- using indexed top-k as denominator
- semantic absence claims
- unbounded backtracking regex
- silently changing scope/revision during execution
- concrete registry, safe-reader, revision-store, redb or Qdrant dependencies

## Logical primitives

- `ExactPredicate`, `ExactPredicateProfile`, `ExactScanPlan`, `FrozenDenominator`, `ExactItemResult`, `ExactExecutionReport`, `ExactCoverage`, `SourceInventoryPort`, `SourceReadbackPort`

## Logical operations

1. `compile(request, authorized_scope, inventory) -> Result<ExactScanPlan, ExactError>`
2. `execute(plan, budget, cancel, readback) -> Result<ExactExecutionReport, ExactError>`
3. `evaluate_completeness(report) -> ExactCoverage`
4. `validate_predicate_profile(predicate) -> Result<(), ExactError>`

## Required invariants

- denominator comes from authoritative inventory, never indexed candidates
- every denominator item is read or has an explicit incompleteness reason
- unreadable, changed, timed-out or cancelled scope cannot produce complete negative proof
- raw-byte and decoded-text predicates remain distinct
- predicate engine/profile/version is bound into the plan
- all concrete storage access occurs behind ports

## Typed failure surface

- `SCOPE_CHANGED_OR_REVISION_UNAVAILABLE`
- `INCOMPLETE_COVERAGE`
- `EXACT_PREDICATE_UNSUPPORTED`
- `EXACT_BUDGET_EXHAUSTED`
- `CANCELLED`

## Exit tests / evidence

- `complete_literal_negative_requires_every_denominator_item`
- `unreadable_or_changed_item_blocks_complete_negative`
- `safe_regex_is_size_and_time_bounded`
- `raw_bytes_and_decoded_text_semantics_are_distinct`
- `cancellation_and_scope_drift_are_incomplete`
- `semantic_overclaim_rejected`
- `fake_inventory_and_readback_ports_prove_adapter_independence`

## Suggested internal modules

```text
search-exact/src/
  predicate.rs
  profile.rs
  compile.rs
  denominator.rs
  execute.rs
  coverage.rs
  report.rs
  error.rs
```

## Size / split

- Initial `src/` target: **≤ 7,500 hand-written lines**.
- Split review: **before 8,500 total hand-written Rust lines**.
- Hard stop: **10,000 including package-local tests**.
- Keep one proof owner; split only for an independently replaceable predicate engine/provider.

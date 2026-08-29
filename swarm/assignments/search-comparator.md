# `search-comparator` implementation packet

**Path:** `crates/search-query/search-comparator`  
**Capability:** C25  
**Delivery:** W6 / P11  
**Gate:** BLOCKED until subject resolver and validated structural candidate receipts are accepted  
**Trace:** S22, H15.2, P11  
**Direct public handoffs:** `search-contracts`, `search-domain`, `search-subject-resolver`

Apply `../ASSIGNMENT_PROTOCOL.md`. Logical names below express required semantics, not mandatory Rust
spelling.

## Mission

Build a descriptive cross-repository behavior matrix with lineage collapse, evidence roles, variants, conflicts and unknowns—never a correctness verdict.

## Owns

- analogue match-basis and behavior-signature comparison
- repository lineage/fork/mirror collapse
- evidence-role alignment across definition/test/caller/documentation
- comparison coverage and recommended-reading ordering

## Must not own

- claiming one implementation correct or normative
- counting forks as independent evidence
- hidden semantic synthesis
- source acquisition or result-card expansion

## Logical primitives

- ComparableImplementation, BehaviorSignature, EvidenceRoleSet, LineageGroup, ComparisonAxis, BehaviorObservation, CrossRepositoryBehaviorSet, ComparisonCoverage

## Logical operations

1. `compare(local, implementations, axes, policy) -> Result<CrossRepositoryBehaviorSet, CompareError>`
2. `collapse_lineages(implementations, relations) -> Vec<LineageGroup>`
3. `align_evidence_roles(group) -> BehaviorObservationSet`
4. `classify_shared_variant_outlier_conflict_unknown(observations) -> ComparisonMatrix`
5. `order_recommended_reading(matrix) -> Vec<SearchSourceHandle>`

## Required invariants

- same-name is only one match basis, not proof
- fork/mirror copies count once for independence
- tests/docs are evidence roles, not automatic truth
- configuration predicates remain attached
- output exposes unknowns and coverage, not a normative answer

## Typed failure surface

- `AMBIGUOUS_SUBJECT`
- `COMPARISON_SCOPE_EMPTY`
- `INSUFFICIENT_COMPARABLE_EVIDENCE`
- `LINEAGE_AMBIGUOUS`
- `INCOMPLETE_COVERAGE`

## Exit tests / evidence

- `renamed_true_analogue`
- `false_same_name`
- `fork_and_mirror_collapse`
- `decisive_test_role`
- `mutually_exclusive_cfg_variants`
- `no_correct_implementation_claim`

## Suggested internal modules

```text
search-comparator/src/
  model.rs
  match_basis.rs
  lineage.rs
  role.rs
  signature.rs
  matrix.rs
  coverage.rs
  reading.rs
  error.rs
```

This is an internal file plan, not a request for more crates.

## Size / split

- Initial `src/` target: **≤ 7,500 hand-written lines**.
- Split review: **before 8,500 total hand-written Rust lines**.
- Hard stop: **10,000 including package-local tests**.
- Keep comparison axes and lineage collapse together while they share one behavior-set contract.

The handoff must let a downstream agent consume the public contract without reading implementation
internals or the architecture master.

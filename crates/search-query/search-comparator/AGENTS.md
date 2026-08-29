# Agent contract — search-comparator

You own only `crates/search-query/search-comparator/`. Do not edit another package, the root workspace, or shared contracts.
When a missing contract blocks correct work, open a contract-change issue with the exact field,
invariant, producer, consumer and compatibility impact; do not patch around it.

The Architecture 8.4 master does not need to be loaded for ordinary work. This file contains the
package slice. Traceability only: S20.1, S22, P11.

## Mission

Align validated implementations by lineage, evidence role and behavior observations without declaring a normative winner.

## Ownership

- analogue ladder output interpretation
- fork/mirror/copy lineage collapse
- behavior signature and matrix
- shared/variant/outlier/conflict/unknown observations
- recommended reading handles

## Forbidden ownership

- claiming correct implementation
- counting forks as independent evidence
- using inaccessible lineages
- treating tests/docs as automatic truth

## Allowed dependencies

`search-contracts`, `search-domain`, `search-subject-resolver`. Additional internal or external dependencies require an explicit boundary review. Public
APIs may expose only `search-contracts` or package-owned opaque types; vendor types stay private.

## Required logical surface

These are behavior contracts, not mandated Rust syntax. Preserve the semantics even if the concrete API
is improved:

- `compare_implementations(local, candidates, portfolio) -> CrossRepositoryBehaviorSet`
- `collapse_lineages(candidates, policy) -> Vec<LineageGroup>`
- `align_evidence_roles(group) -> BehaviorSignature`
- `classify_observations(signatures) -> ComparisonMatrix`

## Failure surface

Use typed errors/reason codes. Relevant public reasons: `AMBIGUOUS_SUBJECT`, `INCOMPLETE_COVERAGE`, `REFERENCE_SCOPE_EMPTY`. Never turn a degraded or partial
state into an apparent success.

## Test seams and exit evidence

- `renamed true analogue is retained`
- `same-name false positive is rejected or marked weak`
- `fork/mirror copies count as one lineage`
- `decisive tests and cfg variants remain explicit`
- `unknowns and coverage gaps are preserved`

Property/fault tests belong beside the owning behavior. Shared control-corpus fixtures may be requested,
but the writer does not edit another package opportunistically.

## Size and split guard

- Delivery wave: **W6 / P11**
- Soft `src/` target: **8,000 lines**
- Hard review threshold: **10,000 total hand-written Rust lines**
- Split on a real security, runtime, replacement, test or dependency boundary; never create a forwarding
  wrapper or crate-per-type shell.

## Definition of done

The package has a vendor-neutral public contract, deterministic tests for its invariants, explicit
degradation behavior, no forbidden dependency, and a handoff reporting commands and raw outcomes.
Compilation alone is insufficient.

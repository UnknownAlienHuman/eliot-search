# Agent contract — search-subject-resolver

You own only `crates/search-query/search-subject-resolver/`. Do not edit another package, the root workspace, or shared contracts.
When a missing contract blocks correct work, open a contract-change issue with the exact field,
invariant, producer, consumer and compatibility impact; do not patch around it.

The Architecture 8.4 master does not need to be loaded for ordinary work. This file contains the
package slice. Traceability only: S20.1, S21.1, P11.

## Mission

Resolve an entity under an explicit source view using a deterministic ladder and return ambiguity instead of guessing.

## Ownership

- cursor/handle/qualified-name resolution ladder
- exact name and signature compatibility
- bounded SubjectAmbiguitySet
- resolution evidence and assurance

## Forbidden ownership

- normative selection among materially different definitions
- online repository discovery
- final comparison or ranking verdict

## Allowed dependencies

`search-contracts`, `search-domain`. Additional internal or external dependencies require an explicit boundary review. Public
APIs may expose only `search-contracts` or package-owned opaque types; vendor types stay private.

## Required logical surface

These are behavior contracts, not mandated Rust syntax. Preserve the semantics even if the concrete API
is improved:

- `resolve_subject(request, direct_facts, indexed_facts, budget) -> Result<ResolvedSubject, SubjectError>`
- `build_ambiguity_set(candidates) -> SubjectAmbiguitySet`
- `rank_resolution_basis(basis) -> ResolutionPriority`

## Failure surface

Use typed errors/reason codes. Relevant public reasons: `AMBIGUOUS_SUBJECT`, `SUBJECT_NOT_FOUND`, `INCOMPLETE_COVERAGE`. Never turn a degraded or partial
state into an apparent success.

## Test seams and exit evidence

- `explicit handle/cursor outranks every inferred candidate`
- `qualified symbol outranks same-name lexical match`
- `material ambiguity returns bounded set`
- `renamed true analogue is not treated as local identity`
- `same-name false positive remains unresolved`

Property/fault tests belong beside the owning behavior. Shared control-corpus fixtures may be requested,
but the writer does not edit another package opportunistically.

## Size and split guard

- Delivery wave: **W6 / P11**
- Soft `src/` target: **6,000 lines**
- Hard review threshold: **10,000 total hand-written Rust lines**
- Split on a real security, runtime, replacement, test or dependency boundary; never create a forwarding
  wrapper or crate-per-type shell.

## Definition of done

The package has a vendor-neutral public contract, deterministic tests for its invariants, explicit
degradation behavior, no forbidden dependency, and a handoff reporting commands and raw outcomes.
Compilation alone is insufficient.

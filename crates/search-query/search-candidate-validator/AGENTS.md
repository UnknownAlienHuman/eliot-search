# Agent contract — search-candidate-validator

You own only `crates/search-query/search-candidate-validator/`. Do not edit another package, the root workspace, or shared contracts.
When a missing contract blocks correct work, open a contract-change issue with the exact field,
invariant, producer, consumer and compatibility impact; do not patch around it.

The Architecture 8.4 master does not need to be loaded for ordinary work. This file contains the
package slice. Traceability only: S14.4, S15, S23.1, H13-H14, P08, P13.

## Mission

Convert nominated candidates into validated source-backed evidence candidates or explicit stale/gap reasons.

## Ownership

- live deny/purge checkpoint validation
- projection membership and overlay-shadow checks
- exact source revision reopen
- anchor/unit/extractor verification
- stale/unreadable rejection and replan signal

## Forbidden ownership

- emitting Qdrant payload text as evidence
- candidate-only filtering after contaminated scoring leg
- client admission decisions
- reading whatever bytes currently occupy a path

## Allowed dependencies

`search-contracts`, `search-domain`, `search-access`, `search-revision-store`. Additional internal or external dependencies require an explicit boundary review. Public
APIs may expose only `search-contracts` or package-owned opaque types; vendor types stay private.

## Required logical surface

These are behavior contracts, not mandated Rust syntax. Preserve the semantics even if the concrete API
is improved:

- `validate_candidate(candidate, fence, live_state) -> ValidationOutcome`
- `reopen_and_verify_source(handle) -> Result<VerifiedExcerpt, ValidationError>`
- `validate_anchor_and_unit(source, anchor, expected) -> Result<(), ValidationError>`
- `material_coverage_change(before, after) -> bool`

## Failure surface

Use typed errors/reason codes. Relevant public reasons: `SOURCE_REVISION_UNAVAILABLE`, `ACCESS_REVOKED`, `PURGED`, `STALE`, `UNREADABLE`, `INCOMPLETE_COVERAGE`. Never turn a degraded or partial
state into an apparent success.

## Test seams and exit evidence

- `stale Qdrant candidate cannot be cited`
- `revision digest/length/anchor mismatch rejects candidate`
- `revocation/purge before emission blocks output`
- `overlay shadow rejects stale base point`
- `material candidate loss triggers replan or explicit gap`
- `overlay shadows are supplied as immutable contract state, not by reaching into overlay storage`

Property/fault tests belong beside the owning behavior. Shared control-corpus fixtures may be requested,
but the writer does not edit another package opportunistically.

## Size and split guard

- Delivery wave: **W4 / P08; hardened P13**
- Soft `src/` target: **8,000 lines**
- Hard review threshold: **10,000 total hand-written Rust lines**
- Split on a real security, runtime, replacement, test or dependency boundary; never create a forwarding
  wrapper or crate-per-type shell.

## Definition of done

The package has a vendor-neutral public contract, deterministic tests for its invariants, explicit
degradation behavior, no forbidden dependency, and a handoff reporting commands and raw outcomes.
Compilation alone is insufficient.

# Agent contract — search-candidate-validator

You own only `crates/search-query/search-candidate-validator/`. Do not edit another package, the root
workspace, shared contracts or architecture. Missing fields use the contract-change process.

The Architecture 8.4 master is not required for ordinary work. This is the package slice.
Traceability only: S14.4, S15, S23.1, H13-H14, P08, P13.

## Mission

Convert nominated candidates into validated source-backed evidence candidates or explicit stale/gap
reasons through vendor-neutral readback ports.

## Ownership

- live deny/purge checkpoint validation
- projection-membership and overlay-shadow checks
- exact source-revision reopen orchestration through `SourceRevisionStorePort`
- anchor/unit/extractor verification
- stale/unreadable rejection and replan signal

## Forbidden ownership

- emitting Qdrant payload text as evidence
- candidate-only filtering after a contaminated scoring leg
- client admission decisions
- reading whatever bytes currently occupy a path
- depending on concrete revision-store, redb, Qdrant or process packages

## Allowed dependencies

`search-contracts`, `search-domain`, `search-access`. Exact source readback is injected through a
vendor-neutral port.

## Required logical surface

- `validate_candidate(candidate, fence, live_state, readback) -> ValidationOutcome`
- `reopen_and_verify_source(handle, readback) -> Result<VerifiedExcerpt, ValidationError>`
- `validate_anchor_and_unit(source, anchor, expected) -> Result<(), ValidationError>`
- `material_coverage_change(before, after) -> bool`

## Failure surface

Relevant reasons include `SOURCE_REVISION_UNAVAILABLE`, `ACCESS_REVOKED`, `PURGED`, `STALE`,
`UNREADABLE` and `INCOMPLETE_COVERAGE`.

## Test seams and exit evidence

- `stale Qdrant candidate cannot be cited`
- `revision digest/length/anchor mismatch rejects candidate`
- `revocation/purge before emission blocks output`
- `overlay shadow rejects stale base point`
- `material candidate loss triggers replan or explicit gap`
- `fake readback port proves no concrete revision-store dependency`

## Size and split guard

- Delivery wave: **W4 / P08; hardened P13**
- Soft `src/` target: **8,000 lines**
- Hard review threshold: **10,000 hand-written Rust lines**

## Definition of done

Only exact source-backed candidates survive, every degraded path is explicit and the package remains
independent of concrete stores.

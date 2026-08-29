# `search-candidate-validator` implementation packet

**Path:** `crates/search-query/search-candidate-validator`  
**Capability:** C24  
**Delivery:** W4 / P08; security hardening W7 / P13  
**Gate:** BLOCKED until access and source-readback port contracts are accepted  
**Trace:** S14.4, S15, S23.1, H13-H14, P08, P13  
**Direct public handoffs:** `search-contracts`, `search-domain`, `search-access`

Apply `../ASSIGNMENT_PROTOCOL.md`. Logical names below express required semantics, not mandatory Rust spelling.

## Mission

Convert nominated candidates into validated source-backed evidence candidates or explicit stale/gap reasons through a vendor-neutral readback port.

## Owns

- live deny/purge checkpoint validation
- projection-membership and overlay-shadow checks
- exact source-revision reopen orchestration
- anchor/unit/extractor verification
- stale/unreadable rejection and replan signal

## Must not own

- emitting Qdrant payload text as evidence
- candidate-only filtering after a contaminated scoring leg
- client admission decisions
- reading whatever bytes currently occupy a path
- concrete revision-store/redb/Qdrant/process dependencies

## Logical primitives

- `RawCandidate`, `ValidationFence`, `CandidateValidationContext`, `VerifiedSourceSlice`, `ValidatedCandidate`, `ValidationOutcome`, `ReplanSignal`, `SourceReadbackPort`

## Logical operations

1. `validate(candidate, fence, live_state, readback) -> ValidationOutcome`
2. `reopen_and_verify(handle, readback) -> Result<VerifiedSourceSlice, ValidationError>`
3. `validate_anchor_and_unit(source, anchor, expected) -> Result<(), ValidationError>`
4. `material_coverage_change(before, after) -> bool`

## Required invariants

- every emitted candidate is reauthorized and reopened from the exact revision
- digest, length, anchor, extractor/profile and overlay shadow all match
- revocation/purge is checked before readback and before emission
- stale vendor payload text is never evidence
- contaminated rank legs are discarded/replanned by the executor, not sanitized here
- concrete source storage remains behind a port

## Typed failure surface

- `SOURCE_REVISION_UNAVAILABLE`
- `ACCESS_REVOKED`
- `PURGED`
- `STALE`
- `UNREADABLE`
- `INCOMPLETE_COVERAGE`

## Exit tests / evidence

- `stale_qdrant_candidate_cannot_be_cited`
- `revision_digest_length_anchor_mismatch_rejected`
- `revocation_or_purge_before_emission_blocks_output`
- `overlay_shadow_rejects_stale_base`
- `material_candidate_loss_signals_replan_or_gap`
- `fake_readback_port_proves_store_independence`

## Suggested internal modules

```text
search-candidate-validator/src/
  context.rs
  security.rs
  membership.rs
  readback.rs
  anchor.rs
  overlay.rs
  outcome.rs
  error.rs
```

## Size / split

- Initial `src/` target: **≤ 7,500 hand-written lines**.
- Split review: **before 8,500 total hand-written Rust lines**.
- Hard stop: **10,000 including package-local tests**.

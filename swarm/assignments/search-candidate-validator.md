# `search-candidate-validator` implementation packet

**Path:** `crates/search-query/search-candidate-validator`  
**Capability:** C24  
**Delivery:** W4 / P08; hardening W7 / P13  
**Gate:** BLOCKED until revision readback, access and executor contracts are accepted  
**Trace:** S14.4-S15, S23.1, H13-H14, P08, P13  
**Direct public handoffs:** `search-contracts`, `search-domain`, `search-access`, `search-revision-store`

Apply `../ASSIGNMENT_PROTOCOL.md`. Logical names below express required semantics, not mandatory Rust
spelling.

## Mission

Turn nominated candidates into source-backed validated candidates by rechecking fences, exact revision bytes, anchors and extractor identity.

## Owns

- candidate validation pipeline and checkpoints
- exact source revision reopen/digest/length verification
- anchor/unit/extractor/profile verification
- stale rejection, shadow/reconcile request and validation receipts

## Must not own

- treating Qdrant payload/snippet as evidence
- admission or client belief
- post-filter-only revocation
- projecting unreadable/stale candidates as confirmed citations

## Logical primitives

- CandidateProposal, ValidationContext, ValidationCheckpoint, CandidateValidationState, ValidatedCandidate, CandidateRejection, ValidationReceipt, ReplanRequest

## Logical operations

1. `validate_candidate(candidate, context) -> Result<ValidatedCandidate, CandidateRejection>`
2. `recheck_fences(context, checkpoint) -> Result<(), CandidateRejection>`
3. `verify_revision_and_anchor(candidate, store) -> Result<SourceSlice, CandidateRejection>`
4. `classify_replan_need(rejections, coverage) -> ReplanDecision`

## Required invariants

- every emitted candidate reopens exact SourceRevision and verifies digest/anchor
- live deny/purge checked before readback and emission
- Qdrant text is never cited
- unreadable/stale candidates lower coverage or cause replan
- validation receipt binds plan/view/owner/security/profile generations

## Typed failure surface

- `SOURCE_REVISION_UNAVAILABLE`
- `ACCESS_REVOKED`
- `PURGED`
- `STALE_CANDIDATE`
- `ANCHOR_MAPPING_FAILED`
- `INCOMPLETE_COVERAGE`

## Exit tests / evidence

- `stale_qdrant_text_never_cited`
- `exact_revision_readback`
- `revocation_before_and_after_readback`
- `anchor_digest_mismatch_rejected`
- `overlay_shadow_rejected`
- `material_rejection_triggers_replan_or_gap`

## Suggested internal modules

```text
search-candidate-validator/src/
  pipeline.rs
  checkpoint.rs
  fence.rs
  revision.rs
  anchor.rs
  extractor.rs
  replan.rs
  receipt.rs
  error.rs
```

This is an internal file plan, not a request for more crates.

## Size / split

- Initial `src/` target: **≤ 7,500 hand-written lines**.
- Split review: **before 8,500 total hand-written Rust lines**.
- Hard stop: **10,000 including package-local tests**.
- Keep fence and source verification together because both are required before evidence emission.

The handoff must let a downstream agent consume the public contract without reading implementation
internals or the architecture master.

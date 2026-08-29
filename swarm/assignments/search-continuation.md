# `search-continuation` implementation packet

**Path:** `crates/search-query/search-continuation`  
**Capability:** C27  
**Delivery:** W4 / P08; hardening W7 / P13  
**Gate:** BLOCKED until query planner, access and epoch-pin receipts are accepted  
**Trace:** S14.3-S14.4, S26, H12, P08, P13  
**Direct public handoffs:** `search-contracts`, `search-domain`, `search-query-planner`, `search-access`, `search-epoch-pins`

Apply `../ASSIGNMENT_PROTOCOL.md`. Logical names below express required semantics, not mandatory Rust
spelling.

## Mission

Own bounded provider-local continuation records that either retain an in-memory window/pin or re-execute a versioned plan under its fence.

## Owns

- ephemeral continuation identity, TTL/count quotas and binding scope
- candidate-window/pin retention
- durable replan checkpoint schema for explicit durable jobs only
- authorization/fence revalidation and issued-ID suppression

## Must not own

- public raw Qdrant offsets, cursors or scores
- indefinite pins
- silent continuation against newer corpus
- durable record per ordinary query

## Logical primitives

- ContinuationDurability, ContinuationRecord, CandidateWindow, ReplanCheckpoint, IssuedCandidateSet, ContinuationPolicy, ContinuationExpansion

## Logical operations

1. `create_ephemeral(plan, window, pin, policy) -> Result<ContinuationHandle, ContinuationError>`
2. `create_durable_replan_checkpoint(job, plan) -> Result<ContinuationHandle, ContinuationError>`
3. `expand(handle, binding, current_state, budget) -> Result<ContinuationExpansion, ContinuationError>`
4. `expire(now) -> ExpiryReceipt`
5. `invalidate_by_security_generation(change) -> InvalidationReceipt`

## Required invariants

- handle is opaque, random, binding-scoped and auth-checked every expansion
- expired fence returns SNAPSHOT_EXPIRED with refresh option
- ephemeral handle is memory-only and restart-invalid
- durable checkpoint targets immutable admitted data, never unsaved bytes
- pin TTL/count are bounded

## Typed failure surface

- `SNAPSHOT_EXPIRED`
- `CONTINUATION_NOT_FOUND`
- `CONTINUATION_BINDING_MISMATCH`
- `ACCESS_REVOKED`
- `CONTINUATION_LIMIT_EXCEEDED`

## Exit tests / evidence

- `raw_qdrant_cursor_never_public`
- `binding_scope_enforced`
- `restart_invalidates_ephemeral`
- `expired_fence_never_silently_refreshes`
- `security_change_invalidates`
- `unsaved_buffer_not_durable`
- `bounded_pin_window`

## Suggested internal modules

```text
search-continuation/src/
  record.rs
  ephemeral.rs
  durable.rs
  window.rs
  expand.rs
  expiry.rs
  security.rs
  error.rs
```

This is an internal file plan, not a request for more crates.

## Size / split

- Initial `src/` target: **≤ 6,000 hand-written lines**.
- Split review: **before 8,500 total hand-written Rust lines**.
- Hard stop: **10,000 including package-local tests**.
- Ephemeral and durable checkpoint classes remain together while one public handle contract governs them; split if durable job runtime becomes separate.

The handoff must let a downstream agent consume the public contract without reading implementation
internals or the architecture master.

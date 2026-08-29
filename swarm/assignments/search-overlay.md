# `search-overlay` implementation packet

**Path:** `crates/search-query/search-overlay`  
**Capability:** C19  
**Delivery:** W5 / P09  
**Gate:** BLOCKED until W4 query pipeline and W2 unit/lexical contracts are accepted  
**Trace:** S18, S16.2, P09  
**Direct public handoffs:** `search-contracts`, `search-domain`, `search-unitizer`, `search-lexical`

Apply `../ASSIGNMENT_PROTOCOL.md`. Logical names below express required semantics, not mandatory Rust
spelling.

## Mission

Maintain bounded saved and authenticated unsaved overlays, shadow stale base memberships and produce direct transient candidates.

## Owns

- saved-overlay revision tracking awaiting publication
- memory-only unsaved buffer snapshots and TTLs
- overlay shadow set and revision ordering
- direct exact/token/structural transient candidate generation

## Must not own

- persisting unsaved bytes to redb/CAS/Qdrant/logs/backups/telemetry/eval/training
- durable handles to unsaved bytes
- a persistent second index
- inferring unsaved buffers from filesystem watchers

## Logical primitives

- SavedOverlayEntry, UnsavedBufferSnapshot, OverlayKey, OverlayRevision, OverlayShadowSet, OverlayBudget, OverlayCandidate, OverlayCoverage

## Logical operations

1. `admit_saved_overlay(revision) -> Result<SavedOverlayEntry, OverlayError>`
2. `attach_authenticated_buffer(binding, snapshot, ttl) -> Result<UnsavedBufferGuard, OverlayError>`
3. `replace_or_close_buffer(event) -> OverlayInvalidationReceipt`
4. `compute_shadow_set(base, overlays) -> OverlayShadowSet`
5. `retrieve_overlay(request, snapshot, budget) -> Result<OverlayCandidateSet, OverlayError>`
6. `merge_overlay_and_base(base, overlay, shadows) -> CandidateInputSet`

## Required invariants

- unsaved bytes are process-memory-only until explicit snapshot admission
- authenticated unsaved revision outranks saved worktree, which outranks published base
- closed/replaced/revoked/expired buffer invalidates immediately
- budget failure returns gap or invalidation-only, never stale base exposure
- durable metadata is non-reconstructive

## Typed failure surface

- `UNSAVED_BUFFER_UNOBSERVED`
- `UNSAVED_SNAPSHOT_NOT_ADMITTED`
- `OVERLAY_EXPIRED`
- `OVERLAY_BUDGET_EXHAUSTED`
- `OVERLAY_AUTHORIZATION_LOST`

## Exit tests / evidence

- `unsaved_content_never_persists_exhaustive_sinks`
- `precedence_unsaved_saved_published`
- `buffer_close_and_ttl_invalidate`
- `overlay_budget_never_unshadows_stale_base`
- `saved_overlay_is_durable_revision`
- `durable_handle_to_unsaved_rejected`

## Suggested internal modules

```text
search-overlay/src/
  saved.rs
  unsaved.rs
  auth.rs
  ttl.rs
  shadow.rs
  retrieve.rs
  merge.rs
  error.rs
```

This is an internal file plan, not a request for more crates.

## Size / split

- Initial `src/` target: **≤ 7,500 hand-written lines**.
- Split review: **before 8,500 total hand-written Rust lines**.
- Hard stop: **10,000 including package-local tests**.
- Saved and unsaved overlay remain one query-time precedence capability; persistence adapters are forbidden.

The handoff must let a downstream agent consume the public contract without reading implementation
internals or the architecture master.

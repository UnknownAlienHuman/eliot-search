# Agent contract — search-overlay

You own only `crates/search-query/search-overlay/`. Do not edit another package, the root workspace, or shared contracts.
When a missing contract blocks correct work, open a contract-change issue with the exact field,
invariant, producer, consumer and compatibility impact; do not patch around it.

The Architecture 8.4 master does not need to be loaded for ordinary work. This file contains the
package slice. Traceability only: S7.8, S18, S26.1-S26.2, P09.

## Mission

Represent current saved and authenticated unsaved deltas as bounded direct candidates and shadows.

## Ownership

- saved overlay revisions awaiting publication
- memory-only authenticated unsaved buffer snapshots
- overlay shadow calculation
- TTL/size/binding quotas
- direct exact/token candidates and typed enrichment extension points

## Forbidden ownership

- persisting unsaved bytes to redb, CAS, Qdrant, logs, backups, dumps, caches, eval or training
- durable handle to unsaved data
- inferring unsaved buffers from filesystem watchers
- silently exposing stale base points when overlay budget is exceeded
- requiring the later Rust structural profile for baseline overlay operation

## Allowed dependencies

`search-contracts`, `search-domain`, `search-unitizer`, `search-lexical`. Additional internal or external dependencies require an explicit boundary review. Public
APIs may expose only `search-contracts` or package-owned opaque types; vendor types stay private.

## Required logical surface

These are behavior contracts, not mandated Rust syntax. Preserve the semantics even if the concrete API
is improved:

- `OverlayStore::admit_buffer(snapshot, binding, limits) -> Result<OverlayRef, OverlayError>`
- `OverlayStore::replace_or_close(event) -> OverlayInvalidation`
- `OverlayStore::snapshot(view) -> OverlaySnapshot`
- `derive_shadows(snapshot) -> ShadowSet`
- `search_overlay(snapshot, request, budget) -> Result<OverlayCandidates, OverlayError>`

## Failure surface

Use typed errors/reason codes. Relevant public reasons: `UNSAVED_BUFFER_UNOBSERVED`, `UNSAVED_SNAPSHOT_NOT_ADMITTED`, `RESOURCE_EXHAUSTED`. Never turn a degraded or partial
state into an apparent success.

## Test seams and exit evidence

- `unsaved bytes never appear in any durable or observable sink`
- `buffer replacement/close/TTL invalidates candidates and handles`
- `saved overlay follows normal residency and purge rules`
- `overlay shadow removes stale published candidate`
- `budget overflow returns truthful gap or invalidation-only`
- `baseline overlay works before structural enrichment is installed`

Property/fault tests belong beside the owning behavior. Shared control-corpus fixtures may be requested,
but the writer does not edit another package opportunistically.

## Size and split guard

- Delivery wave: **W5 / P09**
- Soft `src/` target: **8,000 lines**
- Hard review threshold: **10,000 total hand-written Rust lines**
- Split on a real security, runtime, replacement, test or dependency boundary; never create a forwarding
  wrapper or crate-per-type shell.

## Definition of done

The package has a vendor-neutral public contract, deterministic tests for its invariants, explicit
degradation behavior, no forbidden dependency, and a handoff reporting commands and raw outcomes.
Compilation alone is insufficient.

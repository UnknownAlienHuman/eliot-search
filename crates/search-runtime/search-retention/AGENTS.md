# Agent contract — search-retention

You own only `crates/search-runtime/search-retention/`. Do not edit another package, the root workspace, or shared contracts.
When a missing contract blocks correct work, open a contract-change issue with the exact field,
invariant, producer, consumer and compatibility impact; do not patch around it.

The Architecture 8.4 master does not need to be loaded for ordinary work. This file contains the
package slice. Traceability only: S28, H6.2, P13.

## Mission

Execute crash-safe mark-and-sweep, monotonic purge and restore quarantine across Search-owned projections and CAS.

## Ownership

- mark root discovery and resumable sweep
- retention/legal-hold policy execution
- live purge fence, tombstone and receipts
- handle revocation and non-resurrection
- paired restore manifest revalidation

## Forbidden ownership

- claiming physical secure erase beyond evidence
- deleting client-owned canonical evidence
- refcount-only GC
- restore/reindex that bypasses purge tombstones

## Allowed dependencies

`search-contracts`, `search-domain`, `search-control-redb`, `search-revision-store`, `search-qdrant-bridge`, `search-epoch-pins`. Additional internal or external dependencies require an explicit boundary review. Public
APIs may expose only `search-contracts` or package-owned opaque types; vendor types stay private.

## Required logical surface

These are behavior contracts, not mandated Rust syntax. Preserve the semantics even if the concrete API
is improved:

- `RetentionEngine::mark(snapshot, active_pins) -> Result<MarkManifest, RetentionError>`
- `RetentionEngine::sweep(mark) -> Result<SweepReceipt, RetentionError>`
- `RetentionEngine::purge(command) -> Result<PurgeReceipt, RetentionError>`
- `RetentionEngine::validate_restore(manifest, live_sources) -> RestoreDecision`
- `RetentionEngine::apply_tombstones(candidate_set) -> FilteredSet`

## Failure surface

Use typed errors/reason codes. Relevant public reasons: `PURGED`, `RESTORE_PENDING_REVALIDATION`, `RESIDENCY_DOMAIN_MISMATCH`. Never turn a degraded or partial
state into an apparent success.

## Test seams and exit evidence

- `interrupted sweep resumes from root generation/manifest`
- `one membership removal preserves bytes reachable elsewhere`
- `purge fence precedes acknowledgement and deletion`
- `purged material cannot resurrect through restore/reindex`
- `receipt distinguishes logical/index/cache/backup/physical status`

Property/fault tests belong beside the owning behavior. Shared control-corpus fixtures may be requested,
but the writer does not edit another package opportunistically.

## Size and split guard

- Delivery wave: **W7 / P13**
- Soft `src/` target: **9,000 lines**
- Hard review threshold: **10,000 total hand-written Rust lines**
- Split on a real security, runtime, replacement, test or dependency boundary; never create a forwarding
  wrapper or crate-per-type shell.

## Definition of done

The package has a vendor-neutral public contract, deterministic tests for its invariants, explicit
degradation behavior, no forbidden dependency, and a handoff reporting commands and raw outcomes.
Compilation alone is insufficient.

# Agent contract — search-retention

You own only `crates/search-runtime/search-retention/`. Do not edit another package, the root workspace,
shared contracts or architecture. Missing fields use the contract-change process.

The Architecture 8.4 master is not required for ordinary work. This is the package slice.
Traceability only: S28, H6.2, P13.

## Mission

Execute crash-safe CAS mark-and-sweep, monotonic purge and restore quarantine through vendor-neutral
ports while keeping ordinary index reclamation and handle storage in their own owners.

## Ownership

- authoritative mark-root discovery and resumable CAS sweep orchestration
- retention/legal-hold policy execution
- live purge fence, tombstone and multi-layer receipts
- handle/index invalidation requests
- paired restore-manifest revalidation and quarantine
- non-resurrection proofs

## Forbidden ownership

- claiming physical secure erase beyond evidence
- deleting client-owned canonical evidence
- refcount-only GC
- restore/reindex that bypasses purge tombstones
- ordinary retired-point reclaim (owned by `search-index-reclaimer`)
- handle record storage/authorization (owned by `search-handles`)
- direct redb, Qdrant, process or revision-store adapter dependency

## Allowed dependencies

`search-contracts`, `search-domain`, `search-epoch-pins`, `search-index-reclaimer`,
`search-handles`. Control, object-store, index-admin and restore storage operations are injected through
vendor-neutral ports. Dependencies do not transfer state ownership.

## Required logical surface

- `RetentionEngine::mark(snapshot, active_pins, ports) -> Result<MarkManifest, RetentionError>`
- `RetentionEngine::sweep(mark, object_store) -> Result<SweepReceipt, RetentionError>`
- `RetentionEngine::purge(command, ports) -> Result<PurgeReceipt, RetentionError>`
- `RetentionEngine::validate_restore(manifest, live_sources) -> RestoreDecision`
- `RetentionEngine::apply_tombstones(candidate_set) -> FilteredSet`
- `RetentionEngine::emit_invalidation_set(change) -> LifecycleInvalidationSet`

## Failure surface

Relevant reasons include `PURGED`, `RESTORE_PENDING_REVALIDATION`, `RESIDENCY_DOMAIN_MISMATCH`,
`RETENTION_MARK_INCOMPLETE` and `PURGE_ACK_INCOMPLETE`.

## Test seams and exit evidence

- `interrupted sweep resumes from root generation/manifest`
- `one membership removal preserves bytes reachable elsewhere`
- `purge fence precedes acknowledgement and deletion`
- `purged material cannot resurrect through restore/reindex`
- `receipt distinguishes logical/index/cache/backup/physical status`
- `ordinary reclaimer cannot satisfy purge acknowledgement`
- `fake ports prove no direct redb/Qdrant/revision-store dependency`

## Size and split guard

- Delivery wave: **W7 / P13**
- Soft `src/` target: **8,500 lines**
- Hard review threshold: **10,000 hand-written Rust lines**
- CAS retention, purge and restore stay together as one monotonic lifecycle policy owner; independently
  replaceable backup/object providers require a new ADR.

## Definition of done

Lifecycle transitions are monotonic, resumable and receipt-backed; ordinary index reclamation and
handle state remain outside this package.

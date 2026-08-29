# search-overlay

**C19 — Saved and unsaved overlays.**

**Status:** package boundary and agent contract only; behavior is intentionally unimplemented.

Represent current saved and authenticated unsaved deltas as bounded direct candidates and shadows.

## Owns

- saved overlay revisions awaiting publication
- memory-only authenticated unsaved buffer snapshots
- overlay shadow calculation
- TTL/size/binding quotas
- direct exact/token candidates and typed enrichment extension points

## Must not own

- persisting unsaved bytes to redb, CAS, Qdrant, logs, backups, dumps, caches, eval or training
- durable handle to unsaved data
- inferring unsaved buffers from filesystem watchers
- silently exposing stale base points when overlay budget is exceeded
- requiring the later Rust structural profile for baseline overlay operation

- **Delivery wave:** W5 / P09
- **Soft source-line target:** 8,000
- **Agent instructions:** [AGENTS.md](AGENTS.md)

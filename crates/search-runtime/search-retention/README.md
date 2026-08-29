# search-retention

**C28 — Retention, purge and restore lifecycle.**

**Status:** package boundary and agent contract only; behavior is intentionally unimplemented.

Coordinate crash-safe CAS mark-and-sweep, monotonic purge and restore quarantine through vendor-neutral ports.

## Owns

- retention roots/leases and resumable CAS sweep
- purge fences, tombstones and truthful receipts
- restore revalidation/quarantine
- non-resurrection semantics

## Must not own

- ordinary retired-point reclamation
- handle storage/authorization
- concrete redb/Qdrant/revision-store access
- physical secure-erasure claims beyond evidence

- **Delivery wave:** W7 / P13
- **Soft source-line target:** 7,500
- **Agent instructions:** [AGENTS.md](AGENTS.md)

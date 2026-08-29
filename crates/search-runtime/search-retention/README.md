# search-retention

**C28 — Retention, purge and restore lifecycle.**

**Status:** package boundary and agent contract only; behavior is intentionally unimplemented.

Coordinate crash-safe CAS mark-and-sweep, monotonic purge and restore quarantine through
vendor-neutral ports.

## Owns

- mark roots and resumable CAS sweep policy
- retention/legal-hold execution
- live purge fences, tombstones and receipts
- restore revalidation/quarantine
- non-resurrection proof

## Must not own

- ordinary retired-point reclamation
- handle storage/authorization
- concrete redb/Qdrant/revision-store access
- physical secure-erase claims beyond evidence

- **Delivery wave:** W7 / P13
- **Soft source-line target:** 8,500
- **Agent instructions:** [AGENTS.md](AGENTS.md)

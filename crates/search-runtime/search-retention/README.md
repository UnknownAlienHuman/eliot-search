# search-retention

**C28 — Retention, purge and restore lifecycle.**

**Status:** `SOURCE_PRESENT`; retention, purge and sweep lifecycle source exists, while real owner-store integration, backup/restore cutover and installed fault qualification remain incomplete. See [central package status](../../../docs/product/PACKAGE_STATUS.toml). Current owners: #134 → #135 → #136.

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

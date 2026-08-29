# search-retention

**C28 — Retention, purge and restore.**

**Status:** package boundary and agent contract only; behavior is intentionally unimplemented.

Execute crash-safe mark-and-sweep, monotonic purge and restore quarantine across Search-owned projections and CAS.

## Owns

- mark root discovery and resumable sweep
- retention/legal-hold policy execution
- live purge fence, tombstone and receipts
- handle revocation and non-resurrection
- paired restore manifest revalidation

## Must not own

- claiming physical secure erase beyond evidence
- deleting client-owned canonical evidence
- refcount-only GC
- restore/reindex that bypasses purge tombstones

- **Delivery wave:** W7 / P13
- **Soft source-line target:** 9,000
- **Agent instructions:** [AGENTS.md](AGENTS.md)

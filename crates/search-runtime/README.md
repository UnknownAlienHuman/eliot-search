# search-runtime

**Cells C01 and C28** — Runtime Owner, Retention and Purge.

- **Owns:** one data-root owner epoch; process lease and lifecycle; supervised index process ownership;
  retirement, tombstones, rebuild and lifecycle receipts.
- **Must not own:** retrieval semantics, secure-erase guarantees.

Managed and standalone owners cannot simultaneously own one data root. Mode transition requires drain,
an owner-epoch fence and restart.

# search-index-reclaimer

**C17 — Retired-point reclamation executor.**

**Status:** `SOURCE_PRESENT`; exact retired-ID, pin-watermark and bounded resume source exists, while canonical publication/lifecycle composition and installed fault qualification remain incomplete. See [central package status](../../../docs/product/PACKAGE_STATUS.toml). Current owners: #126/#134.

Delete only exact committed retired point IDs older than every active route/epoch pin.

## Owns

- reclaim eligibility from committed retirement manifests
- pin-watermark checks
- bounded exact-ID delete batches and receipts
- crash-safe ordinary-reclaim resume

## Must not own

- publication visibility or point retirement decisions
- broad-filter correctness-path deletion
- security purge or CAS retention
- pin acquisition

- **Delivery wave:** W3 / P07; purge interaction hardened W7
- **Soft source-line target:** 4,500
- **Agent instructions:** [AGENTS.md](AGENTS.md)

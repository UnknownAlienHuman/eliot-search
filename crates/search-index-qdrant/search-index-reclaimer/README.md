# search-index-reclaimer

**C17 — Retired-point reclamation executor.**

**Status:** package boundary and agent contract only; behavior is intentionally unimplemented.

Delete only exact committed retired point IDs that are older than every active route/epoch pin.

## Owns

- reclaim eligibility from committed retirement manifests
- route/epoch watermark checks
- bounded exact-ID delete batches and receipts
- crash-safe resume of ordinary reclamation

## Must not own

- publication visibility commit
- broad-filter correctness-path deletion
- security purge or CAS retention
- pin acquisition

- **Delivery wave:** W3 / P07
- **Soft source-line target:** 4,500
- **Agent instructions:** [AGENTS.md](AGENTS.md)

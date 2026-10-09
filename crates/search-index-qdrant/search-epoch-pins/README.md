# search-epoch-pins

**C17 — Epoch and route pins.**

**Status:** `SOURCE_PRESENT`; bounded RAII pin, quota, watermark and drain-observation source exists, while canonical publication/reclamation composition and installed qualification remain incomplete. See [central package status](../../../docs/product/PACKAGE_STATUS.toml). Current owners: #126/#210.

Protect active query snapshots and old collection routes in memory without writing ordinary query leases.

## Owns

- RAII epoch/route pin registry
- pin quotas and cancellation release
- reclamation watermark
- bounded continuation pin integration
- route-drain observation

## Must not own

- durable `QueryFenceLease` for normal reads
- query result storage
- publication or deletion decisions
- indefinite pins

- **Delivery wave:** W3 / P07
- **Soft source-line target:** 4,500
- **Agent instructions:** [AGENTS.md](AGENTS.md)

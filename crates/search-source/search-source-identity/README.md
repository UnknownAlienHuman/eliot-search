# search-source-identity

**C04 — Source identity and namespace ownership.**

**Status:** `SOURCE_PRESENT`; stable identity, path-history, lineage and cutover source exists, while canonical digest ownership, durable ingestion and coherent currentness integration remain incomplete. See [central package status](../../../docs/product/PACKAGE_STATUS.toml). Current owners: #237/#110/#128.

Derive stable source identity, retain path history and enforce single-writer namespace ownership and cutover.

## Owns

- `SourceIdentity` derivation
- `PathBinding` history
- revision occurrence identity hooks
- `SourceNamespaceOwnership` state machine
- cutover receipt validation and fencing

## Must not own

- corpus/access policy inside `SourceIdentity`
- file content reads
- retrieval membership or ranking

- **Delivery wave:** W2 / P03
- **Soft source-line target:** 6,500
- **Agent instructions:** [AGENTS.md](AGENTS.md)

# search-research-export-adapter

**C30 optional Research profile — Optional Eliot Research export adapter.**

**Status:** `SOURCE_PRESENT` and optional; export/manifest/readback source exists, while canonical daemon-port integration and research-profile qualification remain incomplete and disabled. See [central package status](../../docs/product/PACKAGE_STATUS.toml). Current owners: #216/#228/#229/#230/#231.

Export qualified durable materializations through the exact eliotr.normalized.v1 wire bundle and validate ownership-cutover receipts.

## Owns

- manifest assembly
- native BLAKE3 readback and independent wire SHA-256
- ownership mode validation
- source.owner-cutover.v1 receipt validation
- unknown-field fail-closed behavior

## Must not own

- unsaved overlay export without durable admission
- relabeling internal digests as wire SHA-256
- transferring ownership by ordinary export
- cross-domain CAS/key reuse
- Research canonical writes
- opening CAS/redb/Qdrant directly; exact bytes are supplied through the daemon-owned export port

- **Delivery wave:** W8 / optional P14 profile
- **Soft source-line target:** 6,000
- **Agent instructions:** [AGENTS.md](AGENTS.md)

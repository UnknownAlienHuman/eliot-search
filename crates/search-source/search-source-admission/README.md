# search-source-admission

**Security support for C03/C06 — Source admission policy.**

**Status:** `SOURCE_PRESENT`; deny-by-default policy, decision and receipt source exists, while canonical digest/policy migration and durable source-ingestion composition remain incomplete. See [central package status](../../../docs/product/PACKAGE_STATUS.toml). Current owners: #241 → #110.

Evaluate a versioned deny-by-default source-admission policy without reading source bodies or mutating registry state.

## Owns

- canonical policy normalization and fingerprinting
- path/metadata/format/sensitivity observation evaluation
- deterministic decisions, reasons and receipts
- default exclusion fixtures

## Must not own

- filesystem/Git reads
- root registration, identity or membership state
- post-admission access authority
- silent allow-on-unknown behavior

- **Delivery wave:** W2 / P03
- **Soft source-line target:** 4,500
- **Agent instructions:** [AGENTS.md](AGENTS.md)

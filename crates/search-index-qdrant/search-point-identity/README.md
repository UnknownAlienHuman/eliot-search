# search-point-identity

**C14 — Collision-safe point identity.**

**Status:** package boundary and agent contract only; behavior is intentionally unimplemented.

Encode canonical point keys, derive namespace-separated IDs and make collisions detectable and non-destructive.

## Owns

- versioned `ProjectionPointKey` encoding
- canonical CBOR bytes
- BLAKE3-256 full digest
- 128-bit UUID projection
- existing-point identity comparison

## Must not own

- ad-hoc string or JSON identity derivation
- claiming collisions impossible
- source identity derivation
- performing upserts

- **Delivery wave:** W3 / P06
- **Soft source-line target:** 4,500
- **Agent instructions:** [AGENTS.md](AGENTS.md)

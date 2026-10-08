# search-point-identity

**C14 — Collision-safe point identity.**

**Status:** `SOURCE_PRESENT / LEGACY_PROFILE`; point-key/digest/collision source exists, but the selected sole canonical identity owner is #207. Do not extend or duplicate the legacy identity formula. See [central package status](../../../docs/product/PACKAGE_STATUS.toml).

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

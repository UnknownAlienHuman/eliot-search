# search-point-identity

**C14 — Collision-safe point identity.**

**Status:** the `s11` module supplies the stateless canonical identity API from #256. Existing root exports and current routing remain `LEGACY_PROFILE` until #259/#262 consumers migrate and #329 retires them. This source API delivery is not indexed-product qualification. See [S11 profile and consumer handoff](S11_PROFILE.md) and [central package status](../../../docs/product/PACKAGE_STATUS.toml).

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

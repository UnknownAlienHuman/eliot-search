# search-point-identity

**C14 — collision-safe S11 point identity.**

This package owns the exact versioned `ProjectionPointKey`, its canonical CBOR encoding, the full BLAKE3-256 identity digest and the namespace-separated 128-bit Qdrant UUID projection.

## Correctness boundary

The UUID is an address only. Before an existing UUID can be reused, callers must compare:

- the complete 32-byte identity digest;
- installation incarnation;
- collection generation;
- projection membership;
- representation and unit identities;
- projection profile set;
- point role.

Any mismatch is `POINT_ID_COLLISION`/`COLLISION_BLOCK` and must prevent overwrite.

## Owns

- versioned fixed-shape canonical CBOR;
- BLAKE3-256 full digest;
- namespace-separated UUID projection;
- deterministic identity comparison;
- bounded process-local collision registry.

## Must not own

- source identity or path semantics;
- policy/access decisions;
- vector or payload planning;
- Qdrant transport or upserts;
- claims that collisions are impossible.

- **Delivery wave:** W3 / P06
- **Soft source-line target:** 4,500
- **Agent instructions:** [AGENTS.md](AGENTS.md)

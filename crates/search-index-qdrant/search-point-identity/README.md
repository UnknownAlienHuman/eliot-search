# search-point-identity

**C14 — Collision-safe point identity.**

**Status:** exact S11.1/S11.2 pure identity implementation; downstream producer/publication migration is stacked separately.

This package owns the one normative `ProjectionPointKey` encoding and the distinction between a full
identity and its compact Qdrant address. It performs no Qdrant, filesystem, redb, source or access I/O.

## Canonical key

The exact schema-version-1 key contains only:

- installation incarnation;
- collection generation;
- projection membership;
- representation;
- unit;
- projection profile set;
- point role.

It is encoded as deterministic canonical CBOR. Contract UUIDs are 16-byte byte strings. The full
identity is `BLAKE3-256(canonical_key_bytes)`. A separate domain-separated hash projects that digest to
a UUID-compatible 128-bit Qdrant address.

The compact address is never accepted as proof of identity. Collision/recovery checks compare the full
256-bit digest and every independently represented S9.5 identity field. A mismatch returns
`POINT_ID_COLLISION` or `POINT_IDENTITY_MISMATCH`; overwrite is never permitted.

## Dependency boundary

S11.2 requires real BLAKE3-256. The package reuses the repository's exact `blake3 = 1.8.2` pin with
default features disabled and the pure-Rust implementation enabled. It exposes no third-party type,
starts no runtime, performs no I/O and adds no C/assembly hashing path.

## Owns

- versioned S11.1 point-key shape;
- deterministic canonical CBOR bytes;
- full BLAKE3-256 digest;
- domain-separated 128-bit Qdrant address;
- payload identity validation;
- bounded in-memory collision checking.

## Must not own

- source identity or access-policy derivation;
- projection planning or manifest persistence;
- Qdrant transport or upserts;
- ad-hoc string/JSON identity derivation;
- claims that collisions are impossible.

- **Delivery wave:** W3 / P06
- **Soft source-line target:** 4,500
- **Agent instructions:** [AGENTS.md](AGENTS.md)

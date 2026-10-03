# search-point-identity

**C14 — Collision-safe point identity.**

**Status:** exact S11.1/S11.2 package implementation; downstream producer/publication integration and
collection rebuild are pending.

The package encodes the versioned `ProjectionPointKey` as deterministic canonical CBOR, computes the
full BLAKE3-256 identity digest and derives a separately domain-separated 128-bit Qdrant UUID address.
A UUID match never permits overwrite without full-digest and canonical identity-field equality.

## Owns

- exact S11.1 key shape and schema-version admission;
- bounded deterministic CBOR encoding;
- BLAKE3-256 full identity digest;
- namespace-separated 128-bit Qdrant UUID projection;
- pure exact S9.5 identity-payload validation and collision decisions.

## Must not own

- source identity/path semantics;
- access, scoring or membership policy;
- Qdrant transport or upsert;
- mutable identity registries;
- ad-hoc string/JSON hashing;
- claims that 128-bit collisions are impossible.

## Canonical key

```text
schema_version
installation_incarnation_id
collection_generation_id
projection_membership_id
representation_id
unit_id
projection_profile_set_id
point_role = unit | relation | auxiliary
```

Source membership, source byte ranges, vector names, route names, wall time and insertion order are not
identity fields. The immutable `projection_profile_set_id` binds the complete required named-vector set
to one point identity.

Preparation owners enforce bounded uniqueness inside one point set. The Qdrant bridge performs durable
collision refusal by reading an existing UUID and comparing the full digest plus every independently
stored S9.5 identity field. C14 itself remains stateless.

## Compatibility

This profile replaces the previous length-prefixed/FNV-derived 128-bit implementation. Existing
experimental point IDs cannot be adopted; consumers must create a new collection generation and
rebuild projections.

- **Delivery wave:** W3 / P06
- **Soft source-line target:** 4,500
- **Dependency boundary review:** issue #206
- **Agent instructions:** [AGENTS.md](AGENTS.md)

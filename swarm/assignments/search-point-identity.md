# `search-point-identity` implementation packet

**Path:** `crates/search-index-qdrant/search-point-identity`  
**Capability:** C14  
**Delivery:** W3 / P06  
**Gate:** BLOCKED until W0 canonical contracts and W2 representation identities are accepted  
**Trace:** S11, H7, P06  
**Direct public handoffs:** `search-contracts`, `search-domain`

Apply `../ASSIGNMENT_PROTOCOL.md`. Logical names below express required semantics, not mandatory Rust
spelling.

## Mission

Derive collision-detectable Qdrant point identities from canonical projection keys without owning source identity.

## Owns

- versioned ProjectionPointKey canonical encoding
- BLAKE3-256 full identity digest
- namespace-separated UUID projection
- pre-upsert collision comparison logic and golden fixtures

## Must not own

- source identity/path semantics
- ad-hoc string or JSON hashing
- claiming collisions impossible
- overwriting an existing UUID on full-digest mismatch

## Logical primitives

- ProjectionPointKey, CanonicalPointKeyBytes, PointIdentityDigest, QdrantPointUuid, ExistingPointIdentity, CollisionDecision

## Logical operations

1. `encode_canonical_key(key) -> CanonicalPointKeyBytes`
2. `derive_point_identity(key) -> PointIdentity`
3. `compare_existing_identity(expected, observed) -> CollisionDecision`
4. `validate_identity_fields(expected, payload) -> Result<(), PointIdentityError>`

## Required invariants

- canonical encoding is versioned canonical CBOR or an architecture-equivalent accepted encoding
- full 256-bit digest is stored in manifest/payload
- UUID is only a namespace-separated projection of full digest
- mismatch blocks upsert before mutation
- one projection-unit identity carries its required named-vector set

## Typed failure surface

- `POINT_ID_COLLISION`
- `POINT_KEY_VERSION_UNSUPPORTED`
- `POINT_IDENTITY_MISMATCH`
- `CANONICAL_ENCODING_FAILED`

## Exit tests / evidence

- `canonical_bytes_and_digest_golden`
- `same_key_same_identity`
- `profile_change_changes_identity`
- `fake_truncated_uuid_collision_never_overwrites`
- `json_stringification_not_used`
- `full_digest_payload_validation`

## Suggested internal modules

```text
search-point-identity/src/
  key.rs
  canonical.rs
  digest.rs
  uuid.rs
  collision.rs
  fixture.rs
  error.rs
```

This is an internal file plan, not a request for more crates.

## Size / split

- Initial `src/` target: **≤ 4,500 hand-written lines**.
- Split review: **before 8,500 total hand-written Rust lines**.
- Hard stop: **10,000 including package-local tests**.
- Keep derivation and collision guard together; they are one correctness boundary.

The handoff must let a downstream agent consume the public contract without reading implementation
internals or the architecture master.

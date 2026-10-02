# Function contract — `search-point-identity`

**Status:** W3/P06 pure S11 identity owner.

## Operations

### `canonical_point_key_bytes(key, limits) -> Result<CanonicalPointKeyBytes, PointIdentityError>`

Encodes exactly the S11.1 `ProjectionPointKey` as deterministic canonical CBOR:

- schema version;
- installation incarnation;
- collection generation;
- projection membership;
- representation;
- unit;
- projection profile set;
- point role (`unit | relation | auxiliary`).

The map key order follows deterministic CBOR ordering. UUID-like contract IDs are 16-byte byte strings;
profile identity and point role are text. Ad-hoc concatenation, JSON, map iteration order and omitted
coordinates are forbidden.

### `point_identity_digest(bytes) -> PointIdentityDigest`

Computes the complete BLAKE3-256 digest of the canonical CBOR bytes. This is the identity digest stored
in the S9.5 payload and immutable projection manifest. A 128-bit value is never substituted for it.

### `project_qdrant_uuid(digest) -> PointId128`

Projects the full digest through the frozen `eliot-search/qdrant-point-uuid/v1` domain into a
UUID-compatible 128-bit Qdrant address. The address is not authority and is never assumed collision-free.

### `derive_point_identity(key, limits) -> Result<PointIdentity, PointIdentityError>`

Returns the exact key, full BLAKE3-256 digest and namespace-separated Qdrant address.

### `validate_identity_payload(expected, observed) -> Result<(), PointIdentityError>`

Checks the full digest and every independently represented S11.1 field carried by S9.5:
installation, collection generation, projection membership, representation, unit and profile set.

### `compare_existing_identity(expected, observed) -> Result<CollisionDecision, PointIdentityError>`

`VACANT` permits creation. `SAME_FULL_IDENTITY` permits idempotent replay. An occupied UUID with another
full digest or canonical identity field is `COLLISION_BLOCK`; a different observed UUID is an identity
mismatch. No caller may overwrite on either failure.

### `PointIdentityRegistry::register(identity) -> Result<CollisionDecision, PointIdentityError>`

Provides one bounded in-memory collision check while composing an exact point set. It persists nothing,
performs no I/O and cannot authorize a Qdrant mutation.

## Dependency boundary

The package uses the repository's exact `blake3 = 1.8.2` pure-Rust pin with default features disabled.
The dependency is required by S11.2, exposes no vendor type, starts no runtime, performs no I/O and adds
no C/assembly implementation. `search-contracts` remains the only public domain-type dependency.

## Semantics

All operations are pure, deterministic, bounded and retry-safe. Source identity, access policy,
projection planning, Qdrant transport and publication remain outside this package.

## Required fixtures

- canonical CBOR byte golden;
- BLAKE3-256 digest and UUID-projection goldens;
- every S11.1 coordinate changes both digest and address;
- unknown schema version rejection;
- simulated truncated-address collision blocks overwrite;
- mismatched identity payload blocks overwrite;
- bounded registry capacity.

# Function contract — `search-point-identity`

**Status:** W3/P06 pure S11 identity implementation; downstream integration and rebuild remain pending.

## Operations

### `encode_canonical_key(key, limits) -> Result<CanonicalPointKeyBytes, PointIdentityError>`

Encodes the exact S11.1 `ProjectionPointKey` as deterministic canonical CBOR under explicit bounds.
The fixed map contains schema version, installation incarnation, collection generation, projection
membership, representation, unit, projection profile set and point role. UUID values are 16-byte CBOR
byte strings. Ad-hoc strings, JSON, map iteration order and omitted fields are unrepresentable.

### `full_digest(bytes) -> PointIdentityDigest`

Computes BLAKE3-256 over the `eliot-search/point-identity/v1` domain prefix plus canonical key bytes.
The complete 256-bit digest is the identity guard stored in the projection manifest and S9.5 payload.

### `derive_qdrant_uuid(digest) -> QdrantPointUuid`

Hashes the full digest under the independent `eliot-search/point-uuid/v1` domain and projects the first
128 bits into the UUID address. The UUID is never treated as the complete identity.

### `derive_point_identity(key, limits) -> Result<PointIdentity, PointIdentityError>`

Returns the exact key, full BLAKE3-256 digest and namespace-separated Qdrant UUID projection.
Unknown key schema versions fail before derivation.

### `compare_existing_identity(expected, observed) -> CollisionDecision`

`VACANT` permits creation. `SAME_FULL_IDENTITY` requires the UUID, full digest and every independently
represented S9.5 identity field to match. Any mismatch is `COLLISION_BLOCK`, never overwrite.

### `validate_identity_payload(expected, payload) -> Result<(), PointIdentityError>`

Checks the full 256-bit digest plus installation, collection generation, projection membership,
representation, unit and profile-set fields before publication or recovery. A foreign UUID is
`POINT_IDENTITY_MISMATCH`; a matching UUID with any different full identity is `POINT_ID_COLLISION`.

## Semantics

All functions are pure, deterministic, bounded and retry-safe. No source identity, membership policy,
Qdrant transport, vector content or mutable registry is owned here. Named vectors required by one
immutable profile set share the same point identity; changing the profile-set ID creates another
identity. Preparation owners enforce bounded set uniqueness; the Qdrant bridge performs durable
collision refusal after exact point readback.

This profile intentionally replaces the legacy length-prefixed/FNV-derived identity. Existing
experimental point IDs are non-admissible and require a new collection generation and rebuild.

## Required fixtures

Canonical CBOR/digest/UUID goldens; same key/same identity; every S11 coordinate changes identity;
fake projected-UUID collision never overwrites; JSON/stringification guard; every independently stored
S9.5 identity field; foreign-UUID rejection; unknown schema version and finite-bound rejection.

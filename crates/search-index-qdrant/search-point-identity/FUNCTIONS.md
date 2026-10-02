# Function contract — `search-point-identity`

**Status:** W3/P06 canonical S11.1 identity owner; pure implementation.

## Canonical key

`ProjectionPointKey` is exactly:

```text
schema_version
installation_incarnation_id
collection_generation_id
projection_membership_id
representation_id
unit_id
projection_profile_set_id
point_role
```

The fixed version-1 canonical CBOR representation is a definite-length array in that order. UUIDs are 16-byte byte strings, the profile-set ID is text, and the role is a closed unsigned tag. Ad-hoc strings, JSON serialization, map iteration order and omitted fields are forbidden.

## Operations

### `canonical_point_key_bytes(key, limits) -> Result<CanonicalPointKeyBytes, PointIdentityError>`

Validates version and finite bounds, then returns the exact canonical CBOR bytes.

### `point_identity_digest(bytes) -> PointIdentityDigest`

Computes `BLAKE3-256(canonical_key_bytes)` exactly as specified by S11.2. The full 32-byte digest is stored in payload and manifest.

### `project_qdrant_uuid(digest) -> PointId128`

Hashes the full digest under the fixed `eliot-search/qdrant-point-uuid/v1` namespace and projects the first 128 bits. The UUID is only an address, never the complete identity.

### `derive_point_identity(key, limits) -> Result<PointIdentity, PointIdentityError>`

Returns the exact key, full digest and projected UUID.

### `compare_existing_identity(expected, observed) -> CollisionDecision`

`VACANT` permits creation. `SAME_FULL_IDENTITY` permits idempotent replay. Any UUID match with a different full digest or canonical field is `COLLISION_BLOCK` and must never overwrite.

### `validate_identity_fields(expected, observed) -> Result<(), PointIdentityError>`

Checks UUID, full 256-bit digest and every canonical identity field before publication or recovery.

### `PointIdentityRegistry::register(identity)`

Provides a finite process-local collision guard for a prepared point set. It is not durable authority and performs no transport or upsert.

## Semantics

All derivation and comparison operations are deterministic, bounded and side-effect free. The package owns no source identity/path semantics, membership policy, Qdrant transport, publication state or automatic collision recovery.

## Required fixtures

Canonical CBOR shape and digest; same key/same identity; profile/role/generation/membership changes alter identity; simulated truncated UUID collision never overwrites; full-digest readback validation; unsupported schema version rejection; finite registry capacity.

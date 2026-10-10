# Function contract — `search-point-identity`

**Status:** #256 exposes the stateless `s11` API. Its exact public names, domains,
limits and generation handoff are frozen in [S11_PROFILE.md](S11_PROFILE.md).
Existing root exports remain unchanged legacy APIs until #329 retirement.

The operation names below describe the logical S11 behavior; they are not
forwarding aliases. The old `eliot-search/point-identity/v1` domain in this
historical surface is superseded for the new profile by
`eliot/cbor/point-identity/v1`, with address domain `eliot/raw/point-address/v1`.

## Operations

### `encode_canonical_key(key) -> Result<CanonicalPointKeyBytes, PointIdentityError>`

Encodes `ProjectionPointKey` as versioned deterministic CBOR under explicit bounds. Ad-hoc strings,
JSON serialization, map iteration order and omitted load-bearing fields are forbidden.

### `full_digest(bytes) -> PointIdentityDigest`

Computes BLAKE3-256 through the shared canonical helper under the frozen S11 CBOR domain.

### `derive_qdrant_uuid(digest) -> QdrantPointUuid`

Projects the full digest into a namespace-separated UUID representation. The UUID is an address, not
the complete identity.

### `derive_point_identity(key) -> Result<PointIdentity, PointIdentityError>`

Returns canonical-key digest, projected UUID, schema version and the exact identity payload fields that
must be stored and read back.

### `compare_existing_identity(expected, observed) -> CollisionDecision`

`VACANT` permits creation. `SAME_FULL_IDENTITY` permits idempotent replay. Any UUID match with a
different full digest or canonical identity field is `COLLISION_BLOCK`, never overwrite.

### `validate_identity_payload(expected, payload) -> Result<(), PointIdentityError>`

Checks the full 256-bit digest and every load-bearing identity field before publication or recovery.

## Semantics

All functions are pure, deterministic, bounded and retry-safe. Cancellation is optional budget
checkpointing only. No source identity, membership policy, Qdrant transport or mutable registry state
is owned here.

## Required fixtures

Canonical byte/digest goldens; same key/same identity; profile/generation/membership changes alter
identity; simulated truncated UUID collision never overwrites; JSON/string hashing guard; full-digest
payload readback; unknown schema version rejection.

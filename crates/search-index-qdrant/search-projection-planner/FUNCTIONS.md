# Function contract — `search-projection-planner`

**Status:** canonical S9.5/S11 producer; pure planning only.

## Operations

### `validate_projection_input(input, profiles, budget) -> Result<ValidatedProjectionInput, ProjectionError>`

Requires one immutable `ProjectionMembership -> SourceMembership` scope, one collection generation and publication epoch, a profile-compatible representation, unique unit-role/scoring-document identities, and the exact named-vector set for every point.

### `build_minimal_payload(scope, unit, profiles, identity) -> Result<MinimalPointPayload, ProjectionError>`

Builds the exact closed S9.5 payload. It contains typed installation, collection, projection membership, access/scoring partition, source/revision/representation/unit, full point identity, scoring document, profile set, modality/format metadata, and epoch validity fields. It cannot represent source membership, ACLs, display names, paths, source/query text, payload digests, or vector digests.

### `build_point_spec(scope, unit, profiles, budget, identity_limits, digest_port) -> Result<PointSpec, ProjectionError>`

Derives the exact S11 point identity, builds the open-ended S9.5 payload, computes planner-owned canonical payload/vector digests through `ProjectionDigestPort`, and retains the exact readback expectation.

### `plan_projection(input, profiles, budget, identity_limits, digest_port) -> Result<ProjectionPlan, ProjectionError>`

Creates a deterministically point-ID-ordered exact point set for one projection membership, blocks compact-ID collisions, rejects duplicate unit roles/scoring documents and incompatible vectors, and emits the immutable manifest.

### `canonicalize_manifest(scope, profiles, points, budget, digest_port) -> Result<ProjectionManifest, ProjectionError>`

Produces versioned canonical manifest bytes containing the exact membership/profile scope, point UUIDs, full identity digests, unit IDs, payload digests, and expected vector names/digests.

### `diff_manifests(old, new) -> Result<ManifestDiff, ProjectionError>`

Returns exact create, retain, and retire entries. A scope/profile change replaces the complete point set. Broad source or payload-filter closure is structurally unavailable.

### `validate_schema_requirements(manifest, profiles, schema) -> Result<(), ProjectionError>`

Requires the exact 19-field S9.5 payload-index set with UUID/keyword/integer types and every named-vector shape required by the accepted profile set.

## Digest boundary

`ProjectionDigestPort::blake3_256` receives complete versioned canonical bytes. Implementations return ordinary BLAKE3-256 with no hidden prefix. The planner owns domain separation and canonical encoding; it does not accept caller-supplied payload/vector digests.

## Semantics

Planning is pure, deterministic, bounded, and retry-safe. Failure yields no usable partial plan or manifest. The package performs no Qdrant/redb/CAS/filesystem I/O and makes no source-admission or access decision.

## Deferred product gates

Downstream daemon composition must translate the package-owned S9.5 payload into the bridge contract, persist the manifest in CAS, verify exact readback, and publish only through the S13 coordinator. Live Qdrant qualification and full tests are not claimed by this package revision.

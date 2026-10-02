# Function contract — `search-projection-planner`

**Status:** W3/P06 pure S9.5/S11 producer implementation; publication/daemon integration remains stacked.

## Operations

### `validate_projection_input(input, profiles, budget) -> Result<ValidatedProjectionInput, ProjectionError>`

Requires one typed `SourceMembership -> ProjectionMembership` binding, one retained source revision,
one representation, one immutable access/scoring partition pair, a non-zero target epoch, a complete
unit set and the exact named-vector set required by the immutable projection profile set.

### `build_minimal_payload(input, unit, profiles, identity) -> Result<MinimalPointPayload, ProjectionError>`

Emits exactly the S9.5 opaque payload: typed installation, collection, projection membership,
access/scoring partitions, source/revision/representation/unit, full point digest, scoring document,
profile set, unit/modality/format metadata and epoch validity. Source membership, ACLs, paths, source or
query text, payload digests and vector digests are structurally absent.

### `build_point_spec(input, unit, profiles, budget, identity_limits) -> Result<PointSpec, ProjectionError>`

Derives the exact S11 identity through `search-point-identity`, derives membership-independent
`ScoringDocumentId`, computes the canonical payload digest and every exact named-vector digest, and
retains the control-only source-membership mapping outside Qdrant payload.

### `plan_projection(input, profiles, budget, identity_limits) -> Result<ProjectionPlan, ProjectionError>`

Creates one deterministically ordered exact point set for exactly one projection membership. Duplicate
unit roles, duplicate UUIDs, profile/vector drift, epoch zero and finite-budget overflow fail closed.

### `canonicalize_manifest(points, profiles, budget, identity_limits) -> Result<ProjectionManifest, ProjectionError>`

Produces immutable CAS-ready bytes and exact entries containing point UUID, canonical identity key,
full identity digest, source-membership mapping, unit ID, expected payload digest, expected vector
names/digests and unit/reference digests. No broad selector substitutes for exact IDs.

### `diff_manifests(old, new) -> Result<ManifestDiff, ProjectionError>`

Returns exact create, retain and retire entries. Reusing one physical UUID with changed immutable
content is `PROJECTION_MANIFEST_MISMATCH`; replacement requires a new S11 identity.

### `validate_schema_requirements(manifest, profiles, schema) -> Result<(), ProjectionError>`

Requires the exact 19-field S9.5 payload-index set and types plus every required named-vector shape.
Unknown additional payload indexes are incompatible with the qualified collection generation.

## Semantics

Planning is pure, deterministic and retry-safe. The package performs no Qdrant, redb, filesystem or
CAS I/O and owns no source/access/publication authority. BLAKE3 is a pinned implementation-private
dependency reviewed in #208; public APIs expose only `Blake3Digest32` and package-owned types.

`ScoringDocumentId` is derived from source revision, representation, unit and profile set with
membership excluded. Equal content projected into two memberships therefore has distinct point IDs but
the same scoring-document identity, enabling the route compiler to prevent IDF duplication.

## Deferred evidence

Deterministic golden/property tests and the full downstream publication process suite remain deferred
under the current code-first project priority. The minimum package gate is check plus strict Clippy.

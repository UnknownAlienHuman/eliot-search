# Function contract — `search-projection-planner`

**Status:** C13 canonical S9.5/S11.3 producer; pure implementation.

## Inputs

`ProjectionInput` contains already-admitted typed identities and metadata: installation and collection generation, one source/projection membership binding, immutable access/scoring partitions, source/revision/representation/unit/scoring-document IDs, profile set, point role, modality/format metadata, epoch validity, source range, residency proof and exact named vectors.

The planner does not decide access, discover sources, read files or call Qdrant.

## Operations

### `validate_projection_input(input, profiles, budget)`

Requires one exact profile-set/vector schema, finite vectors, canonical vector digests, a valid epoch interval and bounded source range.

### `build_point_spec(input, identity_limits)`

Builds the S11.1 canonical point key through `search-point-identity`, then emits:

- compact point UUID;
- full BLAKE3-256 identity digest;
- exact closed S9.5 payload;
- exact named vector values;
- canonical payload/vector digests retained outside Qdrant payload;
- source-membership/range/residency context for the immutable manifest.

### `plan_projection(inputs, profiles, budget, identity_limits)`

Requires one coherent projection membership/security/scoring/generation/profile scope, rejects duplicate unit roles and UUIDs, and creates a deterministic point-ID-ordered plan.

### `plan_scoped_projection(inputs, expected_units, scope, ...)`

Additionally proves exact admitted scope, residency and complete unit coverage.

### `canonicalize_manifest(points, budget)`

Produces immutable deterministic manifest bytes containing exact UUIDs, canonical identity keys/full digests, authoritative source-membership mapping, unit/source coordinates and expected payload/vector digests. No broad filter substitutes for the exact point set.

### `diff_manifests(old, new)`

Returns exact create/retain/retire point entries by UUID and full entry equality.

### `expected_payload_indexes()`

Returns the exact 19-field S9.5 payload-index set. `validate_schema_requirements` requires equality, not a permissive superset.

## Prohibited payload data

Source membership, corpus/repository display names, ACL subjects, paths, source/query text, payload digests and vector digests are not representable in `MinimalPointPayload`. Source membership and expected digests remain in manifest/control data.

## Required evidence

Exact S9.5 field set; one projection membership per point; canonical full identity digest; vector/payload digest verification; deterministic manifest reconstruction; exact old/new diff; incompatible profile or collection generation replans affected points; no broad-filter closure.

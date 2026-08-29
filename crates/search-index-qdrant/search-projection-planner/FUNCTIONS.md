# Function contract — `search-projection-planner`

**Status:** W3/P06 logical contract; pure planning only.

## Operations

### `validate_projection_input(input, profiles) -> Result<ValidatedProjectionInput, ProjectionError>`

Requires immutable admitted revision/representation/unit manifests, exactly one
`ProjectionMembership -> SourceMembership`, accepted profile IDs and complete vector encodings.

### `build_minimal_payload(unit, membership, profile_set, epoch) -> Result<MinimalPointPayload, ProjectionError>`

Emits only the opaque S9.5 fields. Corpus/repository display names, ACL subjects, membership arrays,
source text, paths and vendor metadata are rejected.

### `build_point_spec(input, identity_port) -> Result<PointSpec, ProjectionError>`

Combines the canonical point identity, exact named-vector set, payload digest, unit/reference digests
and expected readback shape. Point identity derivation remains owned by `search-point-identity`.

### `plan_projection(input, profiles, budget) -> Result<ProjectionPlan, ProjectionError>`

Creates a deterministically ordered exact point set and rejects duplicate UUIDs, duplicate unit roles,
missing vectors, profile mismatches and unbounded plans.

### `canonicalize_manifest(points) -> Result<ProjectionManifest, ProjectionError>`

Produces immutable CAS-ready manifest bytes containing exact UUIDs, full identity digests, unit IDs,
vector names and payload/vector digests. No broad selection predicate substitutes for the exact set.

### `diff_manifests(old, new) -> Result<ManifestDiff, ProjectionError>`

Returns exact create, retain and retire lists. Retained IDs require full identity and expected payload/
vector equality.

### `validate_schema_requirements(manifest, collection_schema) -> Result<(), ProjectionError>`

Proves every filterable payload field has the required index and every expected named vector exists
before the plan may enter publication.

## Semantics

Planning is pure, deterministic and retry-safe. Budget/cancellation yields no usable partial manifest.
The crate performs no Qdrant/redb/CAS I/O and makes no source/admission/access decision.

## Required fixtures

One membership per point; minimal payload disclosure guard; deterministic manifest bytes; exact
old/new diff; profile/generation change replaces affected points; duplicate/collision propagation;
schema/index completeness; broad-filter closure structurally unavailable.

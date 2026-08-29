# `search-projection-planner` implementation packet

**Path:** `crates/search-index-qdrant/search-projection-planner`  
**Capability:** C13  
**Delivery:** W3 / P06  
**Gate:** BLOCKED until point identity, lexical and unit contracts are accepted  
**Trace:** S7.5-S7.6, S8-S11, H10, P06  
**Direct public handoffs:** `search-contracts`, `search-domain`, `search-point-identity`

Apply `../ASSIGNMENT_PROTOCOL.md`. Logical names below express required semantics, not mandatory Rust
spelling.

## Mission

Compile immutable representations and memberships into exact point sets and projection manifests without performing Qdrant transport.

## Owns

- projection profile set descriptors
- point specifications and minimal opaque payload plans
- exact immutable projection manifests
- old/new manifest diff and closure/compensation point lists

## Must not own

- Qdrant network calls
- source acquisition or policy decisions
- broad payload-filter closure when exact manifests exist
- membership arrays or display metadata in point payload plans

## Logical primitives

- ProjectionProfileSet, ProjectionInput, PointSpec, NamedVectorSpec, MinimalPointPayload, ProjectionManifest, ManifestDiff, ProjectionPlan

## Logical operations

1. `plan_projection(input, profiles) -> Result<ProjectionPlan, ProjectionError>`
2. `build_point_spec(unit, membership, profiles) -> Result<PointSpec, ProjectionError>`
3. `canonicalize_manifest(points) -> ProjectionManifest`
4. `diff_manifests(old, new) -> ManifestDiff`
5. `validate_minimal_payload(payload) -> Result<(), ProjectionError>`

## Required invariants

- one ProjectionMembership maps to exactly one SourceMembership
- point payload includes only S9.5 opaque fields
- manifest contains exact point UUID/full digests/vector names/payload digest
- active filter fields have explicit schema/index requirements
- profile change creates new projection identity

## Typed failure surface

- `PROJECTION_PLAN_INVALID`
- `PROJECTION_MEMBERSHIP_VIOLATION`
- `PROJECTION_PAYLOAD_OVERDISCLOSURE`
- `PROJECTION_MANIFEST_MISMATCH`
- `PROFILE_SET_MISMATCH`

## Exit tests / evidence

- `one_membership_per_point_schema`
- `minimal_payload_no_names_acl_arrays`
- `manifest_determinism`
- `old_new_exact_diff`
- `profile_change_replans_all_affected_points`
- `collision_decision_propagates`

## Suggested internal modules

```text
search-projection-planner/src/
  profile.rs
  input.rs
  point.rs
  payload.rs
  manifest.rs
  diff.rs
  validation.rs
  error.rs
```

This is an internal file plan, not a request for more crates.

## Size / split

- Initial `src/` target: **≤ 7,000 hand-written lines**.
- Split review: **before 8,500 total hand-written Rust lines**.
- Hard stop: **10,000 including package-local tests**.
- Keep point-set planning and manifest production together; split only if modality planning gains a separate provider dependency.

The handoff must let a downstream agent consume the public contract without reading implementation
internals or the architecture master.

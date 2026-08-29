# Agent contract — search-projection-planner

You own only `crates/search-index-qdrant/search-projection-planner/`. Do not edit another package, the root workspace, or shared contracts.
When a missing contract blocks correct work, open a contract-change issue with the exact field,
invariant, producer, consumer and compatibility impact; do not patch around it.

The Architecture 8.4 master does not need to be loaded for ordinary work. This file contains the
package slice. Traceability only: S7.5, S8, S9.5, S11.3, S13, H10, P06.

## Mission

Plan the exact rebuildable point set and immutable manifests for one projection membership without performing vendor I/O.

## Ownership

- projection profile and input-descriptor validation
- one-membership-per-projection enforcement
- point/vector/payload plan construction from prepared contract artifacts
- old/new manifest diff
- expected payload/vector digests

## Forbidden ownership

- Qdrant transport
- source truth or access authority
- broad closure filters when exact IDs exist
- sharing retrieval points across memberships
- depending directly on parser, enricher or encoder implementations

## Allowed dependencies

`search-contracts`, `search-domain`, `search-point-identity`. Additional internal or external dependencies require an explicit boundary review. Public
APIs may expose only `search-contracts` or package-owned opaque types; vendor types stay private.

## Required logical surface

These are behavior contracts, not mandated Rust syntax. Preserve the semantics even if the concrete API
is improved:

- `plan_projection(input, profiles) -> Result<ProjectionPlan, ProjectionError>`
- `build_projection_manifest(plan) -> ProjectionManifest`
- `diff_manifests(old, new) -> ProjectionDelta`
- `validate_membership_isolation(plan) -> Result<(), ProjectionError>`

## Failure surface

Use typed errors/reason codes. Relevant public reasons: `MEMBERSHIP_ISOLATION_VIOLATION`, `PROJECTION_MANIFEST_MISMATCH`, `PROFILE_SET_MISMATCH`. Never turn a degraded or partial
state into an apparent success.

## Test seams and exit evidence

- `one SourceMembership maps to one ProjectionMembership`
- `manifest contains exact IDs, full digests and expected vector names`
- `no corpus names, ACL arrays or raw client identifiers in payload`
- `equivalent bytes across memberships create distinct point sets`
- `manifest diff never falls back to broad source filter`
- `prepared unit/vector inputs are consumed through contracts, not implementation dependencies`

Property/fault tests belong beside the owning behavior. Shared control-corpus fixtures may be requested,
but the writer does not edit another package opportunistically.

## Size and split guard

- Delivery wave: **W3 / P06**
- Soft `src/` target: **7,000 lines**
- Hard review threshold: **10,000 total hand-written Rust lines**
- Split on a real security, runtime, replacement, test or dependency boundary; never create a forwarding
  wrapper or crate-per-type shell.

## Definition of done

The package has a vendor-neutral public contract, deterministic tests for its invariants, explicit
degradation behavior, no forbidden dependency, and a handoff reporting commands and raw outcomes.
Compilation alone is insufficient.

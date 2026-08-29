# Agent contract — search-point-identity

You own only `crates/search-index-qdrant/search-point-identity/`. Do not edit another package, the root workspace, or shared contracts.
When a missing contract blocks correct work, open a contract-change issue with the exact field,
invariant, producer, consumer and compatibility impact; do not patch around it.

The Architecture 8.4 master does not need to be loaded for ordinary work. This file contains the
package slice. Traceability only: S11, H7, P06.

## Mission

Encode canonical point keys, derive namespace-separated IDs and make collisions detectable and non-destructive.

## Ownership

- versioned ProjectionPointKey encoding
- canonical CBOR bytes
- BLAKE3-256 full digest
- 128-bit UUID projection
- existing-point identity comparison

## Forbidden ownership

- ad-hoc string or JSON identity derivation
- claiming collisions impossible
- source identity derivation
- performing upserts

## Allowed dependencies

`search-contracts`, `search-domain`. Additional internal or external dependencies require an explicit boundary review. Public
APIs may expose only `search-contracts` or package-owned opaque types; vendor types stay private.

## Required logical surface

These are behavior contracts, not mandated Rust syntax. Preserve the semantics even if the concrete API
is improved:

- `canonical_point_key_bytes(key) -> Result<CanonicalBytes, PointIdError>`
- `point_identity_digest(bytes) -> PointIdentityDigest`
- `project_qdrant_uuid(digest) -> QdrantPointUuid`
- `compare_existing_identity(expected, observed) -> Result<(), PointIdError>`

## Failure surface

Use typed errors/reason codes. Relevant public reasons: `POINT_ID_COLLISION`, `CANONICAL_ENCODING_MISMATCH`. Never turn a degraded or partial
state into an apparent success.

## Test seams and exit evidence

- `golden canonical bytes and digest fixtures`
- `namespace separation changes UUID projection`
- `fake truncated-ID collision returns POINT_ID_COLLISION`
- `mismatched UUID is never approved for overwrite`
- `named vectors of one unit share one point identity`

Property/fault tests belong beside the owning behavior. Shared control-corpus fixtures may be requested,
but the writer does not edit another package opportunistically.

## Size and split guard

- Delivery wave: **W3 / P06**
- Soft `src/` target: **4,500 lines**
- Hard review threshold: **10,000 total hand-written Rust lines**
- Split on a real security, runtime, replacement, test or dependency boundary; never create a forwarding
  wrapper or crate-per-type shell.

## Definition of done

The package has a vendor-neutral public contract, deterministic tests for its invariants, explicit
degradation behavior, no forbidden dependency, and a handoff reporting commands and raw outcomes.
Compilation alone is insufficient.

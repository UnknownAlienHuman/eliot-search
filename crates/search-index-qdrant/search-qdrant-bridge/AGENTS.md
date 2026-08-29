# Agent contract — search-qdrant-bridge

You own only `crates/search-index-qdrant/search-qdrant-bridge/`. Do not edit another package, the root workspace, or shared contracts.
When a missing contract blocks correct work, open a contract-change issue with the exact field,
invariant, producer, consumer and compatibility impact; do not patch around it.

The Architecture 8.4 master does not need to be loaded for ordinary work. This file contains the
package slice. Traceability only: S9, S10, S27, H8, H10, P05.

## Mission

Own all qualified Qdrant process and transport details behind vendor-neutral Search ports.

## Ownership

- exact artifact qualification and process identity
- loopback/auth/ACL/Job Object lifecycle
- collection schema and capability probes
- strict-mode indexes
- upsert, close, query, count and exact readback transport

## Forbidden ownership

- recipe meaning, access authority or result interpretation
- vendor types in public ports
- automatic upgrades or unpinned latest
- CLI/worker/client direct Qdrant access

## Allowed dependencies

`search-contracts`, `search-domain`. Additional internal or external dependencies require an explicit boundary review. Public
APIs may expose only `search-contracts` or package-owned opaque types; vendor types stay private.

## Required logical surface

These are behavior contracts, not mandated Rust syntax. Preserve the semantics even if the concrete API
is improved:

- `QdrantSupervisor::start(qualified_artifact, secret) -> Result<ProcessGuard, QdrantError>`
- `QdrantBridge::probe_capabilities() -> Result<CapabilityReceipt, QdrantError>`
- `QdrantBridge::ensure_schema(schema) -> Result<SchemaReceipt, QdrantError>`
- `QdrantBridge::upsert_exact(batch, write_policy) -> Result<MutationReceipt, QdrantError>`
- `QdrantBridge::readback_exact(ids) -> Result<PointReadback, QdrantError>`
- `QdrantBridge::query(leg, budget) -> Result<CandidateStream, QdrantError>`

## Failure surface

Use typed errors/reason codes. Relevant public reasons: `QDRANT_UNAVAILABLE`, `QDRANT_CAPABILITY_MISMATCH`, `COLLECTION_SCHEMA_MISMATCH`. Never turn a degraded or partial
state into an apparent success.

## Test seams and exit evidence

- `loopback and API auth required`
- `signed i64 and missing-valid_until fixture`
- `filtered IDF and strict-mode unindexed-filter rejection`
- `wait=true strong write/readback fixture`
- `executable/hash/PID mismatch quarantines`
- `credentials absent from argv and logs`

Property/fault tests belong beside the owning behavior. Shared control-corpus fixtures may be requested,
but the writer does not edit another package opportunistically.

## Size and split guard

- Delivery wave: **W3 / P05**
- Soft `src/` target: **9,000 lines**
- Hard review threshold: **10,000 total hand-written Rust lines**
- Split on a real security, runtime, replacement, test or dependency boundary; never create a forwarding
  wrapper or crate-per-type shell.

## Definition of done

The package has a vendor-neutral public contract, deterministic tests for its invariants, explicit
degradation behavior, no forbidden dependency, and a handoff reporting commands and raw outcomes.
Compilation alone is insufficient.

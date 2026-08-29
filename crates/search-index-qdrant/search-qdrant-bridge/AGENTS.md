# Agent contract — search-qdrant-bridge

You own only `crates/search-index-qdrant/search-qdrant-bridge/`. Do not edit another package, the root
workspace, shared contracts or architecture. Missing fields use the contract-change process.

The Architecture 8.4 master is not required for ordinary work. This is the package slice.
Traceability only: S9, S10, H8.6-H8.9, H10, P05.

## Mission

Own the qualified Qdrant data plane and capability/schema probes behind vendor-neutral Search ports.

## Ownership

- collection schema and capability probes
- strict-mode payload indexes
- exact point upsert/close/delete/readback transport
- filtered query, count and scroll/administrative operations authorized by a port contract
- transport timeout, retry and response-shape validation
- private vendor-type translation

## Forbidden ownership

- executable qualification, process lifecycle, ACL, Job Object or secret storage
- recipe meaning, access authority, publication visibility or result interpretation
- vendor types in public ports
- automatic upgrades or unpinned latest
- CLI/worker/client direct Qdrant access

## Allowed dependencies

`search-contracts`, `search-domain`. The daemon supplies a ready endpoint/auth capability from
`search-qdrant-supervisor`; no direct package dependency is required.

## Required logical surface

- `QdrantBridge::connect(endpoint, auth_lease) -> Result<QdrantBridge, QdrantError>`
- `QdrantBridge::probe_capabilities() -> Result<CapabilityReceipt, QdrantError>`
- `QdrantBridge::ensure_schema(schema) -> Result<SchemaReceipt, QdrantError>`
- `QdrantBridge::upsert_exact(batch, write_policy) -> Result<MutationReceipt, QdrantError>`
- `QdrantBridge::readback_exact(ids) -> Result<PointReadback, QdrantError>`
- `QdrantBridge::query(leg, budget) -> Result<CandidateStream, QdrantError>`
- `QdrantBridge::delete_exact(ids, write_policy) -> Result<MutationReceipt, QdrantError>`

## Failure surface

Relevant reasons include `QDRANT_UNAVAILABLE`, `QDRANT_CAPABILITY_MISMATCH`,
`COLLECTION_SCHEMA_MISMATCH`, `QDRANT_RESPONSE_INVALID` and `QDRANT_AUTH_FAILED`.

## Test seams and exit evidence

- `signed i64 and missing-valid_until fixture`
- `filtered IDF and strict-mode unindexed-filter rejection`
- `wait=true strong write/readback fixture`
- `exact ID delete/readback receipt validation`
- `vendor response anomalies fail closed`
- `vendor types cannot cross public package boundary`
- `tests use a pre-qualified endpoint; process lifecycle is not duplicated`

## Size and split guard

- Delivery wave: **W3 / P05**
- Soft `src/` target: **7,000 lines**
- Hard review threshold: **10,000 hand-written Rust lines**

## Definition of done

The bridge is a bounded, qualified data-plane adapter only. Windows process ownership and credentials
remain in `search-qdrant-supervisor` / `search-os-secrets`.

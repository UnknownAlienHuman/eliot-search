# `search-qdrant-bridge` implementation packet

**Path:** `crates/search-index-qdrant/search-qdrant-bridge`  
**Capability:** C15  
**Delivery:** W3 / P05  
**Gate:** BLOCKED until P05 selects an exact qualified Qdrant artifact/client pair by ADR  
**Trace:** S9-S10, S27, H8, H10, P05-P06  
**Direct public handoffs:** `search-contracts`, `search-domain`

Apply `../ASSIGNMENT_PROTOCOL.md`. Logical names below express required semantics, not mandatory Rust
spelling.

## Mission

Own all Qdrant vendor transport, capability qualification, schema translation and low-level read/write/query operations behind vendor-neutral ports.

## Owns

- qualified artifact/client descriptor and capability probe suite
- collection schema creation, payload indexes and strict-mode admission
- translation between vendor-neutral point/filter/query contracts and qdrant-client types
- acknowledged mutations, exact readback/count and query cancellation

## Must not own

- recipe meaning, client authority or result interpretation
- exposing Qdrant types/clients/collections/point IDs to public consumers
- assuming capability from a version string
- unsafe indexed fallback when filtered IDF or strict filters fail

## Logical primitives

- QdrantArtifactDescriptor, CapabilityProbePlan, CapabilityReceipt, CollectionSchemaDescriptor, SearchIndexCommand, SearchIndexQuery, MutationAck, PointReadback, QueryStream, BridgeHealth

## Logical operations

1. `probe_capabilities(disposable_route) -> Result<CapabilityReceipt, BridgeError>`
2. `ensure_schema(descriptor) -> Result<SchemaReceipt, BridgeError>`
3. `upsert_wait_strong(batch, cancel) -> Result<MutationAck, BridgeError>`
4. `close_exact_points_wait_strong(ids, epoch, cancel) -> Result<MutationAck, BridgeError>`
5. `readback_exact(ids) -> Result<Vec<PointReadback>, BridgeError>`
6. `query_filtered(request, cancel) -> Result<QueryStream, BridgeError>`
7. `exact_count(filter) -> Result<u64, BridgeError>`

## Required invariants

- Qdrant is loopback/authenticated and exact artifact identity is verified
- strict mode starts only after mandatory payload indexes exist
- missing valid_until and filtered IDF fixtures pass before indexed admission
- publication mutations use wait=true and strong ordering
- vendor structs never cross public package ports

## Typed failure surface

- `QDRANT_UNAVAILABLE`
- `QDRANT_CAPABILITY_MISMATCH`
- `COLLECTION_SCHEMA_MISMATCH`
- `QDRANT_READBACK_MISMATCH`
- `QDRANT_OPERATION_CANCELLED`
- `QDRANT_STRICT_FILTER_REJECTED`

## Exit tests / evidence

- `disposable_capability_suite`
- `signed_i64_and_missing_upper_bound_fixture`
- `filtered_idf_noninterference_fixture`
- `strict_mode_unindexed_filter_rejection`
- `wait_true_readback_fixture`
- `vendor_type_public_api_guard`

## Suggested internal modules

```text
search-qdrant-bridge/src/
  artifact.rs
  capability.rs
  schema.rs
  filter.rs
  mutation.rs
  readback.rs
  query.rs
  health.rs
  vendor.rs
  error.rs
```

This is an internal file plan, not a request for more crates.

## Size / split

- Initial `src/` target: **≤ 7,500 hand-written lines**.
- Split review: **before 8,500 total hand-written Rust lines**.
- Hard stop: **10,000 including package-local tests**.
- If process supervision or artifact installation grows, keep it in daemon/runtime. Split bridge transport only on a real protocol/client replacement boundary.

The handoff must let a downstream agent consume the public contract without reading implementation
internals or the architecture master.

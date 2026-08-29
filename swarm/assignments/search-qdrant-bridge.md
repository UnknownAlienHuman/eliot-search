# `search-qdrant-bridge` implementation packet

**Path:** `crates/search-index-qdrant/search-qdrant-bridge`  
**Capability:** C15 data plane  
**Delivery:** W3 / P05  
**Gate:** BLOCKED until P05 selects an exact qualified Qdrant server/client pair and supervisor receipt exists  
**Trace:** S9-S10, H8.6-H8.9, H10, P05-P06  
**Direct public handoffs:** `search-contracts`, `search-domain`

Apply `../ASSIGNMENT_PROTOCOL.md`. Logical names below express required semantics, not mandatory Rust spelling.

## Mission

Own the qualified Qdrant collection/point/query data plane and capability/schema probes behind vendor-neutral Search ports.

## Owns

- capability probe suite and collection schema admission
- payload indexes and strict-mode translation
- exact point mutation/readback/delete and filtered query/count transport
- private translation between Search contracts and qdrant-client types
- cancellation, timeout and response-shape validation

## Must not own

- executable/process/ACL/Job Object lifecycle or secret storage
- recipe meaning, client authority, publication visibility or result interpretation
- vendor types/clients/collections/point IDs in public ports
- capability assumptions from a version string
- automatic upgrade or unsafe indexed fallback

## Logical primitives

- `CapabilityProbePlan`, `CapabilityReceipt`, `CollectionSchemaDescriptor`, `SearchIndexCommand`, `SearchIndexQuery`, `MutationAck`, `PointReadback`, `QueryStream`, `BridgeHealth`

## Logical operations

1. `connect(endpoint, auth_lease) -> Result<QdrantBridge, BridgeError>`
2. `probe_capabilities(disposable_route) -> Result<CapabilityReceipt, BridgeError>`
3. `ensure_schema(descriptor) -> Result<SchemaReceipt, BridgeError>`
4. `upsert_wait_strong(batch, cancel) -> Result<MutationAck, BridgeError>`
5. `close_exact_wait_strong(ids, epoch, cancel) -> Result<MutationAck, BridgeError>`
6. `delete_exact_wait_strong(ids, cancel) -> Result<MutationAck, BridgeError>`
7. `readback_exact(ids) -> Result<Vec<PointReadback>, BridgeError>`
8. `query_filtered(request, cancel) -> Result<QueryStream, BridgeError>`
9. `exact_count(filter) -> Result<u64, BridgeError>`

## Required invariants

- endpoint/process identity is supplied by an accepted supervisor receipt
- strict mode starts only after mandatory payload indexes exist
- missing `valid_until` and filtered-IDF fixtures pass before indexed admission
- correctness mutations use `wait=true`, strong ordering and exact readback
- vendor structs never cross public package ports

## Typed failure surface

- `QDRANT_UNAVAILABLE`
- `QDRANT_CAPABILITY_MISMATCH`
- `COLLECTION_SCHEMA_MISMATCH`
- `QDRANT_READBACK_MISMATCH`
- `QDRANT_OPERATION_CANCELLED`
- `QDRANT_RESPONSE_INVALID`

## Exit tests / evidence

- `disposable_capability_suite`
- `signed_i64_and_missing_upper_bound_fixture`
- `filtered_idf_noninterference_fixture`
- `strict_mode_unindexed_filter_rejection`
- `wait_true_mutation_readback_fixture`
- `exact_delete_receipt_fixture`
- `vendor_type_public_api_guard`
- `process_lifecycle_not_duplicated`

## Suggested internal modules

```text
search-qdrant-bridge/src/
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

## Size / split

- Initial `src/` target: **≤ 7,000 hand-written lines**.
- Split review: **before 8,500 total hand-written Rust lines**.
- Hard stop: **10,000 including package-local tests**.
- Process lifecycle remains in `search-qdrant-supervisor`.

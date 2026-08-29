# `search-eliot-adapter` implementation packet

**Path:** `crates/search-eliot-adapter`  
**Capability:** C30 optional ELIOT profile  
**Delivery:** W8 / optional P14  
**Gate:** OPTIONAL and disabled by default; start only when the generic provider edge is accepted and this profile is explicitly requested  
**Trace:** S1.3, S32.3, H16.3-H16.5, P14  
**Direct public handoffs:** `search-contracts`, `search-domain`, `search-provider-protocol`

Apply `../ASSIGNMENT_PROTOCOL.md`. Logical names below express required semantics, not mandatory Rust
spelling.

## Mission

Map ELIOT-owned scope/view/fence contracts to generic Search requests and map Search candidates back without transferring authority.

## Owns

- ELIOT compatibility mapping types at the leaf boundary
- WorkScope/disclosure ceiling to Search grant/request mapping
- SourceView/StateFence/capability pulse mapping
- candidate/coverage/reason mapping back to ELIOT provider results

## Must not own

- ELIOT storage/canonical-writer imports
- canonical database credentials
- task, admission, verification or finish authority
- returning an ELIOT memory disposition
- creating a second mutable source catalogue

## Logical primitives

- EliotBindingInput, EliotScopeMapping, EliotViewMapping, EliotProviderRequest, EliotProviderResult, EliotCompatibilityReceipt

## Logical operations

1. `map_scope_and_disclosure(input, capability) -> Result<SearchRequestContext, AdapterError>`
2. `map_view_and_state_fence(input) -> Result<SourceViewBinding, AdapterError>`
3. `map_search_result(result) -> Result<EliotProviderResult, AdapterError>`
4. `validate_no_reverse_authority(mapping) -> Result<(), AdapterError>`

## Required invariants

- Search owns mutable admitted source namespaces; ELIOT keeps immutable refs
- adapter receives no canonical credentials or reverse write channel
- provider failure narrows coverage and does not block unrelated ELIOT work
- no new eliот.search authority surface is invented
- generic Search contracts remain canonical

## Typed failure surface

- `CLIENT_ADAPTER_AUTHORITY_VIOLATION`
- `ELIOT_SCOPE_MAPPING_FAILED`
- `ELIOT_VIEW_FENCE_MISMATCH`
- `PROVIDER_COVERAGE_DEGRADED`
- `PROFILE_DISABLED`

## Exit tests / evidence

- `disabled_by_default`
- `no_canonical_db_dependency_guard`
- `exact_scope_view_fence_mapping_fixture`
- `search_result_has_no_memory_disposition`
- `provider_failure_narrows_coverage`
- `no_reverse_write_channel`

## Suggested internal modules

```text
search-eliot-adapter/src/
  scope.rs
  view.rs
  fence.rs
  capability.rs
  request.rs
  result.rs
  authority_guard.rs
  error.rs
```

This is an internal file plan, not a request for more crates.

## Size / split

- Initial `src/` target: **≤ 5,500 hand-written lines**.
- Split review: **before 8,500 total hand-written Rust lines**.
- Hard stop: **10,000 including package-local tests**.
- Keep as a leaf adapter; do not push ELIOT concepts into contracts/domain. Split only on an external ELIOT protocol version boundary.

The handoff must let a downstream agent consume the public contract without reading implementation
internals or the architecture master.

# `search-epoch-pins` implementation packet

**Path:** `crates/search-index-qdrant/search-epoch-pins`  
**Capability:** C17  
**Delivery:** W3 / P07  
**Gate:** BLOCKED until collection route and Epoch contracts are accepted  
**Trace:** S13.7, S14, H12, P07  
**Direct public handoffs:** `search-contracts`, `search-domain`

Apply `../ASSIGNMENT_PROTOCOL.md`. Logical names below express required semantics, not mandatory Rust
spelling.

## Mission

Protect in-flight collection routes and visible epochs with bounded in-memory RAII pins and compute safe reclamation watermarks.

## Owns

- EpochPinRegistry and RoutePinRegistry
- RAII query/continuation pin guards
- pin snapshots and reclamation watermark computation
- connection/cancellation cleanup semantics

## Must not own

- durable ordinary query leases
- query history
- retention policy or physical deletion
- keeping expired continuations pinned indefinitely

## Logical primitives

- PinKey, PinOwner, PinGuard, RoutePinGuard, PinRegistrySnapshot, ReclamationWatermark, PinLimitPolicy

## Logical operations

1. `acquire_epoch_pin(route, epoch, owner) -> Result<PinGuard, PinError>`
2. `acquire_route_pin(route, owner) -> Result<RoutePinGuard, PinError>`
3. `snapshot_pins() -> PinRegistrySnapshot`
4. `compute_reclamation_watermark(retired, snapshot) -> ReclamationWatermark`
5. `release_owner_pins(owner)`
6. `expire_bounded_continuation_pins(now) -> ExpiryReceipt`

## Required invariants

- a point/route observable by any active pin is never reclaimable
- ordinary query pin is memory-only and ends on completion/cancel/disconnect/crash
- pin counts and TTLs are bounded
- route migration retains old route until every admitted pin drains
- epoch identity includes collection generation

## Typed failure surface

- `PIN_LIMIT_EXCEEDED`
- `PIN_OWNER_UNKNOWN`
- `SNAPSHOT_EXPIRED`
- `ROUTE_STILL_PINNED`
- `PIN_REGISTRY_INCONSISTENT`

## Exit tests / evidence

- `active_pin_blocks_reclamation`
- `guard_drop_releases_pin`
- `disconnect_releases_owner_pins`
- `continuation_ttl_is_bounded`
- `old_route_drains_after_last_pin`
- `crash_requires_no_durable_query_lease_recovery`

## Suggested internal modules

```text
search-epoch-pins/src/
  key.rs
  registry.rs
  guard.rs
  watermark.rs
  expiry.rs
  error.rs
```

This is an internal file plan, not a request for more crates.

## Size / split

- Initial `src/` target: **≤ 4,500 hand-written lines**.
- Split review: **before 8,500 total hand-written Rust lines**.
- Hard stop: **10,000 including package-local tests**.
- Epoch and route pins remain one process-local registry while they share lifecycle and cleanup.

The handoff must let a downstream agent consume the public contract without reading implementation
internals or the architecture master.

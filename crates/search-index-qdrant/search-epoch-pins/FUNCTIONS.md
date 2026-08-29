# Function contract — `search-epoch-pins`

**Status:** W3/P07 process-local lifecycle contract.

No ordinary query or ephemeral continuation pin is written to durable storage.

## Operations

### `acquire_epoch_pin(route, epoch, owner, limits) -> Result<EpochPinGuard, PinError>`

Atomically validates the active route/epoch and increments one bounded owner/key reference. The guard is
non-serializable and releases exactly once on drop.

### `acquire_route_pin(route, owner, limits) -> Result<RoutePinGuard, PinError>`

Protects a collection generation during migration/drain independently of a particular epoch.

### `renew_continuation_pin(guard, new_expiry, policy) -> Result<PinRenewalReceipt, PinError>`

May extend only within the original route/epoch, binding and configured maximum TTL. Ordinary query
pins are not renewable.

### `release_owner_pins(owner) -> PinReleaseReceipt`

Idempotently releases all pins for request cancellation, disconnect or explicit continuation expiry.

### `snapshot() -> PinRegistrySnapshot`

Returns a consistent bounded snapshot of active route/epoch counts and earliest expiries without
exposing client identities.

### `compute_reclamation_watermark(retired, snapshot) -> ReclamationWatermark`

Marks an exact retired manifest/route reclaimable only when no active epoch or route pin can observe it.

### `expire_continuation_pins(now, policy) -> ExpiryReceipt`

Expires only bounded continuation pins. Time is supplied by the caller; the registry does not own a
clock.

## Concurrency and failure semantics

Acquire/release is linearizable within one daemon process. Guard drop is idempotent. A daemon crash
ends process-local pins; durable jobs resume/replan from durable source checkpoints and do not pretend
the old pin survived. Registry inconsistency fails closed and blocks reclaim.

## Required fixtures

Concurrent acquire/release; guard drop; cancellation/disconnect cleanup; bounded TTL/count; old-route
drain after final pin; stale/foreign owner rejection; crash requiring no durable query-lease recovery;
watermark never advances past an active pin.

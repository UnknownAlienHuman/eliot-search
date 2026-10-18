# Pin source and implementation

## Current API

| Source API | Actual work |
| --- | --- |
| [`PinRegistry::new`](src/lib.rs#L207) | Creates process-local state for supplied active route, visible epoch and limits. |
| [`publish_active_route`](src/lib.rs#L226) | Updates those fields under the lock; rejects epoch regression only on the same route. It does not observe a control commit. |
| [`acquire_epoch_pin`](src/lib.rs#L242) | Queries require the active route/current epoch. The raw continuation case accepts any epoch at or below current visibility with an admissible expiry. |
| [`acquire_route_pin`](src/lib.rs#L292) | Adds a guard for any supplied route, without a retired-route admission check. |
| [Guard release/drop and `release_owner_pins`](src/lib.rs#L314) | Remove process-local records and counts. Cancellation/disconnect must invoke the real owning cleanup path. |
| [`snapshot`](src/lib.rs#L368), [`compute_reclamation_watermark`](src/lib.rs#L592) | Copy counts under the lock, then count matching route pins and epochs below the supplied retirement epoch. No lock covers a later deletion. |
| [`expire_continuation_pins`](src/lib.rs#L332) | Removes at most the requested number of expired continuation pins using caller time. |
| [`ContinuationPins::with_query_pin`](src/continuation/creation.rs#L25) | Checks the still-live query guard and inserts its continuation guard under the same registry lock; failed creation releases the new guard. |
| [Continuation `validate`, `release`, `retire_released`](src/continuation.rs#L135) | Validate the original binding/lifetime, release its guard, and separately remove its retained bookkeeping record. |

## Close the real lifetime gaps

**Use continuous protection.** [Daemon reclaim admission](../../../bins/eliot-searchd/src/rebuild_composition/kernel/reclaim.rs#L32)
takes a snapshot and returns a plan; [execution](../search-index-reclaimer/src/execute.rs#L122)
does not consult the registry. The raw continuation and route acquisition methods
above can add a protecting pin after that observation. A copied `reclaimable` bool
cannot close this race.

Connect admission and retirement in the existing lifecycle owner: new queries
acquire the published view, continuations transfer a live guard, and old-route
admission ends before final drain permits deletion. Keep the existing
[`create_pinned_ephemeral`](../../search-query/search-continuation/src/security/creation.rs#L42)
consumer; remove alternative old-view acquisition APIs when no caller needs them.
This is an integration gap in source, not an observed production deletion.

**Bound a continuation's actual lifetime.** The low-level
[`EpochPinGuard::renew_continuation_pin`](src/lib.rs#L432) checks expiry relative
to the supplied current time, not the original creation time. It can revive an
expired but unswept record or extend repeatedly. Its only caller is a test.
Prefer the fixed lifetime already enforced by `ContinuationPins::validate`;
do not preserve this unused renewal API as a product requirement.

**Free completed bindings.** [`ContinuationPins::new`](src/continuation.rs#L83)
caps records at `MAX_LIST_ITEMS` (4,096), including released records.
`release` retains those records. The only current `retire_released` caller is
[failed-creation cleanup](src/continuation/creation.rs#L164); successful
[native invalidation](../../../bins/eliot-searchd/src/access_composition/native_security/invalidation.rs#L240)
calls only `release`. Repeated completed lifecycles can therefore fill a
long-lived store. Remove the binding when its owning continuation cleanup is
complete; retain only an actual unfinished cleanup obligation.

Keep live guards through query I/O. Registry snapshots, copied epochs and receipt
labels do not themselves keep the indexed data alive.

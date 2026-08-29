# P18 advanced-scale supplement — `search-epoch-pins`

**Status:** blocked. Existing process-local route/epoch pin semantics remain normative.

## Required operations

```text
acquire_migration_route_pin(route, owner, limits) -> RoutePinGuard
fence_old_route_for_new_pins(route, revision) -> RoutePinFence
snapshot_route_drain(route, generation, clock) -> RouteDrainSnapshot
evaluate_old_route_reclaimability(retired_route, snapshot) -> RouteReclaimEligibility
invalidate_scale_owner_epoch(previous, next) -> PinInvalidationReceipt
```

## Invariants

- route switch does not move existing pins to the candidate route;
- old route accepts no new pins after the committed fence while existing pins drain;
- watermarks are per route/generation and unknown/stale state fails closed;
- worker/model/document request pins are bounded and released on cancel/disconnect/crash;
- process restart never assumes old in-memory pins survived;
- reclaimer receives exact current drain evidence, not a scalar global watermark.

## Required evidence

Concurrent old/new route queries across switch; continuation TTL/renewal; disconnect/crash cleanup;
route fence race; final-pin release; stale owner/generation rejection; no old-route reclaim while any pin
can observe it.

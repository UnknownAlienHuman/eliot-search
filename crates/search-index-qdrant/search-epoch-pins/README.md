# search-epoch-pins → one published-view manager

The useful responsibility is small: keep the exact immutable view used by a request alive until the request
and any derived `QueryWindow` finish. The current raw pin registry is a removal target.

Read the [implementation guide](../../../docs/implementation/README.md), [source map](FUNCTIONS.md) and
[publication map](../search-publication/FUNCTIONS.md).

## Current source

About 36 KiB of production Rust exposes caller-selected route/epoch publication, arbitrary route pins, raw
route/epoch acquisition, owner IDs/counts, caller-time renewal/expiry, sampled watermarks and a second
continuation binding store. These APIs permit new old pins after a deletion decision and retain completed
binding history.

## Replacement

```text
ViewManagerState {
  current: Arc<PublishedViewState>
  retired: ViewId → Weak<PublishedViewState>
}

ViewGuard(Arc<PublishedViewState>)
```

Use a standard mutex. `pin_current()` briefly locks and clones only current. A live request may clone its
guard into a window. There is no method accepting route, generation, sequence, owner ID, caller time or TTL.

Publication commits durable current/cleanup state first, then installs the target under the short manager
lock. Do not hold this mutex across redb fsync. Requests which pin old before install remain valid and are
visible through the old weak ref after swap. A crash in the gap restarts from durable current.

The manager stores only weak retired refs and prunes dead entries. Last strong drop sends a best-effort wake;
periodic cleanup scanning is fallback. Never delete in `Drop`.

## Cleanup conditions

```text
point IDs closed at N:
  no live view on same route/generation has sequence < N

old collection:
  no live view uses that route/generation
```

The immediate predecessor is insufficient: a live v1 can still need a point after v2 is gone and v3 closed
it. Snapshot every upgradable weak view for the cleanup decision. If a weak ref cannot upgrade, no new old
strong ref can appear because old views are never acquired by ID.

After restart only durable current is installed; old interactive guards are gone, while publication's finite
cleanup rows remain.

## Remove

- `PinRegistry`, raw active-route/epoch acquisition and route pins;
- pin/owner IDs, counts, release-by-owner and caller-time renewal;
- snapshots, watermarks, fences and reclaimable Booleans;
- continuation pin/binding catalogue and released-history retirement;
- P18 route-drain/pin state and API-shape fixtures;
- this package if the final manager fits naturally in publication/query composition.

Keep only ordinary `Arc` ownership and current→window guard cloning.

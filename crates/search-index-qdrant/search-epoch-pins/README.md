# search-epoch-pins → one published-view manager

The useful responsibility is small: keep the exact immutable `PublishedView` used by a request alive until
that request and any derived `QueryWindow` finish. The current raw route/epoch pin registry is not the target
owner and should disappear after its useful RAII idea moves.

Read the [architecture](../../../docs/architecture/README.md),
[implementation guide](../../../docs/implementation/README.md), [source map](FUNCTIONS.md) and
[publication map](../search-publication/FUNCTIONS.md).

## Current source

The package contains about 36 KiB of production Rust and exposes:

- caller-selected active route/visible epoch publication;
- arbitrary route pins and raw route/epoch acquisition;
- caller owner IDs, release-by-owner and renewal from caller-supplied time;
- sampled pin snapshots/reclamation watermarks;
- a second continuation binding store which can retain released records.

These APIs permit the wrong race: a deletion decision can be made from a sampled count while another old
pin is still acquirable. Repairing snapshots and permits would preserve the wrong authority.

## Replacement

One manager owns the current view:

```text
ViewManagerState {
  current: Arc<PublishedViewState>
  retired: ViewId → Weak<PublishedViewState>
}

ViewGuard(Arc<PublishedViewState>)
```

A standard mutex around this state is sufficient. `pin_current()` briefly locks and clones only `current`.
Publication commits and swaps under the same short lock after every external mutation has verified. There
is no method accepting a route, generation, sequence, owner ID, clock or requested TTL.

A live request may clone its guard into a `QueryWindow`. This is ordinary Arc ownership, not a renewable
lease. The manager retains only weak refs to old states and therefore cannot keep them alive. A drop wake
nudges cleanup; a periodic eligibility scan is the fallback.

## Correct cleanup condition

Do not equate “last guard of the immediately previous view” with safe deletion.

```text
v1 sees point P
v2 sees point P
v3 closes P at sequence 3
```

If no v2 guard exists but a v1 guard is live, P must remain. Exact point cleanup is safe only when no live
view on the same route/generation has a sequence below the point's exclusive `valid_until`. A replaced
collection is safe only when no live view uses that route/generation.

The manager evaluates every weak live view. If an old weak reference cannot upgrade, no old strong guard can
be created later because old views are never acquired by ID. Existing strong guards may clone, and that
existing strong ownership keeps the weak reference upgradeable.

After process restart only the durable current view is installed. Old interactive guards/windows are gone,
but publication's durable `RetiredCleanup` rows still remember unfinished physical deletion.

## Remove after cutover

- `PinRegistry`, `RouteIdentity`, raw `publish_active_route` and arbitrary route/epoch acquisition;
- pin IDs, owner IDs/counts, release-by-owner and caller-clock renewal;
- `PinRegistrySnapshot`, reclamation watermark/fence and public reclaim Booleans;
- separate continuation pin/binding catalogue and retained released history;
- P18 scale/pin state machine and old API-shape fixtures.

Move only private RAII guard ownership and the useful live-query-to-window transfer. If the resulting manager
is small, place it in publication/query composition and remove this crate.

## Completion

A real query pins exactly the committed current view, publication switches to a new view while old evidence
continues to work, no new old guard can appear after the swap, a window inherits only its request's guard,
and cleanup waits for all relevant older view sequences/generations. Restart drops ephemeral guards without
forgetting durable cleanup work.

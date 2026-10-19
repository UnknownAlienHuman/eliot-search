# Published-view guards — source and replacement map

The current package contains useful RAII release mechanics, but its public route/epoch/snapshot model is
larger and less safe than the application needs. Replace it with one current-view manager used by query,
query windows, publication and cleanup.

## Current source findings

Production source is about 36 KiB (`lib.rs` 21,015 bytes, continuation 15,323 bytes before tests).

| Current API | Problem |
|---|---|
| `PinRegistry::new(active_route, visible_epoch, limits)` | Creates a second process-local route/epoch authority from caller values rather than the committed `PublishedView`. |
| `publish_active_route(route, epoch)` | Mutates that authority independently of redb current-view commit. |
| `acquire_route_pin` | Pins an arbitrary route and can protect every point in it. It does not require current-view ownership. |
| `acquire_epoch_pin` | Accepts route/epoch/owner/purpose/caller time; continuation mode may pin older epochs and renew them. |
| `release_owner_pins` | Releases by caller-known owner ID rather than lexical ownership of a guard/window. |
| `expire_continuation_pins(now, max)` | Trusts caller time and scans registry history. |
| `snapshot` + watermark | Samples counts, releases the lock and returns public data. It cannot prevent another old acquisition before deletion. |
| continuation bindings | A second store transfers/renews/releases pins and can retain released bindings until a separate retirement call. |
| `with_query_pin` shape | Useful idea only: transfer protection from a live query to an unpublished continuation under one owner. |

A process restart already drops every in-memory pin. Persisting more pin records would not make query
recovery useful; interactive requests can repeat.

## Target state

```text
struct ViewManager {
    inner: Mutex<ViewManagerState>,
    cleanup_wake: CleanupWake,
}

struct ViewManagerState {
    current: Arc<PublishedViewState>,
    retired: BTreeMap<ViewId, Weak<PublishedViewState>>,
}

struct PublishedViewState {
    view: PublishedView,
}

struct ViewGuard {
    state: Arc<PublishedViewState>,
}
```

A standard mutex is intentional: acquisition and swap are short and infrequent relative to query work. No
`arc-swap`/lock-free dependency or generic lease registry is needed.

### `pin_current() -> ViewGuard`

Under the manager lock, clone `current`. Return no public pin ID or owner record. The guard exposes immutable
view data to its query but no method to change route/sequence or renew lifetime.

### `clone_for_window(&ViewGuard) -> ViewGuard`

A live request may clone its existing Arc when creating a `QueryWindow`. Window insertion occurs only after
the first output frame succeeds. There is no API which looks up an old view by ID or accepts caller expiry.
Window expiry/drop releases the Arc normally.

### `commit_and_swap(expected, committed_target)`

Publication holds the manager lock while performing the short redb current-view transaction and swapping the
in-memory Arc:

```text
lock manager / block pin_current
→ verify manager.current.id == expected
→ redb compare/set current + insert RetiredCleanup + clear PendingPublication
→ commit
→ retired.insert(old.id, Arc::downgrade(&old))
→ current = Arc::new(target)
→ unlock
```

No Qdrant/object I/O occurs under the lock. If redb commit acknowledgement is uncertain, publication reads
the typed rows before releasing the lock and installs exactly the durable result. A crash reconstructs the
manager from durable current state.

### `live_views_snapshot() -> Vec<Arc<PublishedViewState>>`

Under the manager lock:

1. clone current;
2. upgrade every retired weak ref;
3. remove dead weak entries;
4. return strong snapshots for the bounded cleanup/GC decision.

Returning strong snapshots keeps their manifests/artifacts alive for that pass. The manager itself never
retains a strong retired reference.

### Cleanup eligibility

For a same-generation point cleanup with exclusive threshold `N`:

```text
safe = live_views.none(|view|
    view.route == cleanup.route
    && view.generation == cleanup.generation
    && view.sequence < N)
```

For whole-generation cleanup:

```text
safe = live_views.none(|view|
    view.route == cleanup.route
    && view.generation == cleanup.generation)
```

The first rule is stronger than waiting for the immediate predecessor. Example: v1 and v2 include P; v3
closes P. No v2 guard does not permit deletion while v1 remains. `valid_until = 3` means every live same-
generation sequence below 3 matters.

If a retired weak ref cannot upgrade, there is no strong old guard left. No future old strong ref can appear:
the only acquisition path is current, and cloning an old guard requires an existing old strong guard.

### Cleanup wake

Last strong drop may send a best-effort wake. The cleanup worker also scans periodically and at publication/
startup, so a lost wake causes delay, not leakage of correctness. Do not perform Qdrant deletion in `Drop`.

## Durable interaction

View guards are deliberately process-local. Publication persists only:

- one `CurrentPublishedView`;
- one optional `PendingPublication`;
- finite `RetiredCleanup` records for actual unfinished external deletion.

After restart old guards do not exist; cleanup records can be re-evaluated against current only. This is the
small crash-safe complement to Lucene's in-memory snapshot rule. Do not persist every query/window as a lease.

## Capacity

Bound live query windows and retained result metadata at the query-window owner. The view manager needs only
current plus weak retired entries; dead entries are pruned. A finite durable cleanup backlog backpressures
new publication before external work, not query reads. Successful queries do not consume lifetime slots
forever.

## Remove after caller cutover

- `PinRegistry`, `RouteIdentity`, `PinLimits`, `EpochPinPurpose` and all public pin/receipt types;
- route/epoch acquisition and active-route publication;
- owner-count maps, release-by-owner, renewal and caller-time expiry;
- snapshot/watermark/fence/reclaimability APIs;
- continuation binding catalogue and released-binding retirement;
- scale/hardening/source-shape fixtures preserving these APIs.

Move the small manager/guard code into publication/query composition if no independent package boundary
remains.

## Donor correction

Lucene `SnapshotDeletionPolicy` holds in-memory refcounts per commit generation and suppresses deletion while
a snapshot exists. Its own documentation explicitly says this state is not persisted. Eliot reuses only the
in-process lifetime rule; crash-safe cleanup obligations live in finite `RetiredCleanup` records.

Tantivy's segment updater passes the set of currently useful files to directory GC after metadata commit.
Eliot similarly snapshots current/pending/cleanup/live view roots; it does not import Tantivy's backend.

## Required behavior checks

- query pins exactly the current committed view;
- commit/swap blocks new old-view acquisition at one linearizable boundary;
- window cloning requires a live guard;
- v1/v2/v3 point example never deletes early;
- generation cleanup waits for all views using it;
- last old drop wakes but does not execute deletion;
- dead weak entries disappear and do not consume permanent capacity;
- restart installs current only and resumes durable cleanup rows;
- uncertain redb commit cannot split durable current from in-memory current;
- no caller-created snapshot/watermark/time value authorizes deletion.

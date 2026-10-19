# Published-view manager — source and replacement map

Replace the current route/epoch pin registry with one private current-view owner used by query, windows,
publication and cleanup.

## Current source findings

Production source is about 36 KiB.

| Current API | Problem |
|---|---|
| `PinRegistry::new(active_route, visible_epoch, limits)` | Creates a second route/epoch authority from caller values instead of committed current view. |
| `publish_active_route` | Can diverge from durable current view. |
| `acquire_route_pin` / `acquire_epoch_pin` | Acquires arbitrary routes/epochs and accepts caller owner/time/purpose. |
| `release_owner_pins` / renewal / expiry | Maintains an ownership/lease database rather than lexical guard lifetime. |
| snapshot/watermark | Samples counts and cannot prevent a later old acquisition. |
| continuation store | Adds another binding/renew/release history which can retain completed records. |

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

struct ViewGuard(Arc<PublishedViewState>);
```

A normal mutex is sufficient. No lock-free pointer/new dependency or generic lease registry.

### `pin_current()`

Lock, clone `current`, unlock. Return immutable view data only. No public pin ID, route selector, owner record
or renewal.

### Window clone

A live request may clone its existing guard while preparing a `QueryWindow`. The window is inserted only
after first output success. There is no old-view lookup by ID. Expiry/drop releases ordinary Arc ownership.

### `install_after_commit(target)`

Publication first commits redb current/cleanup state. Then:

```text
lock manager
→ if target is newer than current:
     retired.insert(current.id, Arc::downgrade(&current))
     current = Arc::new(target)
→ prune dead weak refs
→ unlock
```

Do not hold this lock across redb fsync or Qdrant/object I/O. A query which pins old before install remains
correct and is tracked after the weak ref is inserted. If the process crashes before install, startup reads
durable current. If manager state is poisoned, stop new serving and restart; do not roll back durable state.

Serving and status read manager current. Publication success is emitted only after install.

### `live_views_snapshot()`

Under lock, clone current and upgrade every retired weak ref; prune dead entries. Return strong snapshots for
one bounded cleanup/GC decision, then release lock before manifest traversal or deletion.

### Cleanup eligibility

Same-generation point cleanup with exclusive threshold `N`:

```text
safe = live_views.none(|v|
  v.route == cleanup.route
  && v.generation == cleanup.generation
  && v.sequence < N)
```

Whole-generation cleanup:

```text
safe = live_views.none(|v|
  v.route == cleanup.route
  && v.generation == cleanup.generation)
```

Do not use “last guard of immediate predecessor”. Example: v1 and v2 see P; v3 closes P. A live v1 still
blocks deletion after v2 is gone.

If a weak ref cannot upgrade, no future old strong ref can appear: old views are never acquired by ID and
cloning requires an existing strong guard.

### Drop/wake

`PublishedViewState::drop` may send a best-effort cleanup wake. It performs no Qdrant/object/redb operation.
Periodic scanning and publication/startup wakes guarantee eventual retry even if a wake is lost.

### Restart

Install the durable current view. Old request/window guards do not survive. Durable `RetiredCleanup` rows are
reevaluated against current only. No durable query lease database.

## Lock ordering

- root OS lock exists for daemon lifetime;
- object-GC gate may enclose object finalization + first redb reference;
- redb transactions never run while holding view-manager mutex;
- manager snapshots are cloned briefly, then long object/Qdrant work runs unlocked.

This avoids disk I/O in the query hot lock and prevents GC/publication deadlocks.

## Remove after cutover

- all raw pin/route/epoch/owner/receipt types;
- active-route publication and arbitrary old acquisition;
- owner count maps, release-by-owner, caller-time renewal/expiry;
- snapshot/watermark/fence/reclaimability APIs;
- continuation pin catalogue and retained released bindings;
- P18 scale and old shape tests.

Move the small manager to publication/query composition if no independent package boundary remains.

## Focused checks

- query pins exactly current;
- request pinned in durable-commit→install gap remains valid;
- install makes old acquisition impossible afterward;
- window clone requires live request guard;
- v1/v2/v3 and generation cleanup thresholds;
- dead weak pruning and no permanent capacity leak;
- restart current install and cleanup resumption;
- no redb/Qdrant/object I/O under manager lock.

# Published-view guards — source and replacement

The current package contains useful RAII release mechanics, but its public route/epoch/snapshot model is
larger and less safe than the application needs. Replace it with one current-view owner used by query,
continuation and publication cleanup.

## Current source findings

| Current API | Problem |
|---|---|
| `publish_active_route(route, epoch)` | Accepts raw caller values instead of the committed publication view. |
| `acquire_route_pin` | Can pin an arbitrary route; route pins make every point in that route unreclaimable. |
| `acquire_epoch_pin` | Exposes route/epoch acquisition independently of the current view. |
| `snapshot` + `compute_reclamation_watermark` | Samples counts, releases the lock and returns public data. A later old-route/epoch pin can appear before deletion. |
| `release_owner_pins` | Releases by caller-known owner ID rather than a held guard/window. |
| continuation `renew` / raw acquisition | Caller time can extend protection and legacy acquisition can create old pins without a live query. |
| continuation `release` | Releases the guard but normally retains the binding; repeated windows can fill capacity. |
| [`with_query_pin`](src/continuation/creation.rs) | Useful mechanism: under the same registry lock, transfers protection from a live query guard to an unpublished continuation. |

The package is process-local. Restart already drops ordinary guards; no durable lease database is required
for a short-lived query page.

## Replacement model

Publication commits one immutable `PublishedView`:

```text
view_id / route / physical generation / visible epoch
manifest root/pages
scope and retrieval profile
```

The view owner holds one atomic current pointer. `pin_current()` clones a non-public `ViewGuard` containing
an `Arc`/equivalent reference to that immutable view. Query code cannot choose a route or epoch.

A continuation can obtain a second guard only from its still-live query guard, using the existing
`with_query_pin` transfer pattern. The operation publishes the window only after its record/output is
accepted. No raw old-view acquisition and no renewable continuation lease.

When publication swaps the current pointer:

1. new queries can acquire only the new view;
2. existing query/windows continue using their exact old view;
3. no API can create another old-view guard;
4. the old guard count decreases monotonically;
5. the last drop makes that view eligible for cleanup.

This is the reclamation permit. It is not a sampled public Boolean, registry digest or durable receipt.
The view owner queues one cleanup callback/item after the last guard. Registry poison/disappearance is
reported and cleanup remains blocked; it never becomes apparent success.

## Continuation/window lifetime

One query-window owner keeps:

```text
view guard
ordered result identities and already selected excerpts
cursor
optional opaque locators into the same window
created/expires time and bounded bytes/items
```

Expiry/terminal completion removes the record and drops its guard. Restart expires process-local windows;
a client can repeat/replan the query. Do not persist guards, renewable leases or released tombstone records.

Remove normal use of:

- route pins and arbitrary old route/epoch acquisition;
- `PinRegistrySnapshot`, `ReclamationWatermark` and caller-provided fences;
- release-by-owner string and renewal based on caller time;
- separate continuation-pin records after the window owner adopts the guard;
- package tests that enforce these old API shapes rather than view behavior.

## Donor mechanism

Lucene's
[`SnapshotDeletionPolicy`](https://github.com/apache/lucene/blob/8a8ef488218405ee18ba9803c3ac1779ef74093a/lucene/core/src/java/org/apache/lucene/index/SnapshotDeletionPolicy.java)
keeps a refcount per commit generation, suppresses deletion while a snapshot exists and releases it
explicitly. Reuse that small lifetime rule. Eliot does not adopt Lucene or its index format.

## Required behavior checks

- query pins exactly the current committed view;
- publication switches current while an old query remains valid;
- continuation transfer cannot occur after the query guard is gone;
- new query cannot acquire the old view after switch;
- last old guard schedules cleanup exactly once;
- window expiry/finish removes its record and releases protection;
- restart leaves only the current durable view and pending publication state;
- no forged snapshot/watermark or caller time can authorize deletion.

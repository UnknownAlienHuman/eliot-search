# search-publication

Own one real recoverable transition from prepared projection pages to one immutable `PublishedView`.
Publication is the only owner of pending Qdrant effects, visible-view commit and derived-index cleanup.

Read the [architecture](../../../docs/architecture/README.md),
[implementation](../../../docs/implementation/README.md) and [source map](FUNCTIONS.md).

## Current source and missing connection

The package `PublicationCoordinator` is an in-memory phase machine. Its methods accept prepared values,
receipt labels and external observations but do not write redb, call Qdrant or publish a query view. Search
finds package/tests callers, not the daemon product.

The daemon contains a second `publication_composition::Publisher` with a `PublicationFloor`, file journal,
CAS reservation, compensation and retirement types. `entry.rs` declares it under `#[allow(dead_code)]`; its
consumers are process/module-shape tests. It is not the live publisher.

redb has typed publication intent/visibility code, but searches for `persist_publication_intent` find its
own tests. The current intent retains an opaque prepared-manifest receipt/reference while the complete
manifests/diff remain in process memory. There is no implemented application `PublishedView` owner/pointer.

These are three partial state machines, not three layers to synchronize.

## One publication operation

```text
bounded projection pages
→ write immutable operation/manifest pages
→ short redb pending record
→ Qdrant mutation page + exact readback
→ short redb visible-view commit
→ atomic current PublishedView swap
→ old-view drain
→ bounded point/generation/artifact cleanup
```

Local preparation is disposable until the pending record exists. After an RPC may have committed, the same
pending operation remains; recovery reads actual Qdrant/control state and advances its cursor. No new
operation identity, inferred rollback or compensation theatre.

## Pending artifact

Write the actual bounded operation artifact before redb references it. It contains:

- target physical route/generation, retrieval profile and target epoch;
- ordered new `ViewManifestEntry` pages and current/old manifest roots;
- complete/narrow Qdrant mutation input pages;
- exact expected readback for each page;
- retired point IDs or retired physical generation;
- next mutation/readback/visibility/cleanup cursor;
- finite operation limits and version.

Manifest pages are immutable objects through the shared current-format object store. redb retains only the
small current/pending records and object references. It never stores source text, vectors, whole point
batches or an eternal completed-operation history.

## Effects and visibility

1. Validate one prepared page stream and write its immutable pages.
2. Commit pending metadata in a short redb transaction.
3. Execute one Qdrant page through the single bridge/runtime under the original absolute deadline.
4. Read back actual relevant point/vector/payload/schema state.
5. Advance the pending cursor in a short transaction.
6. After every required page is verified, atomically commit route/generation/epoch/manifest roots as the
   visible view.
7. Swap one in-memory current-view pointer built from that committed record.

Never hold a redb writer across object or Qdrant I/O. A failed local transaction before commit is known
failure. Timeout after possible Qdrant dispatch is unresolved. A committed view with failed in-memory swap
is recovered by rebuilding the pointer from committed control state; do not rerun mutations.

## View lifetime and cleanup

A query obtains only a private guard to the atomic current `PublishedView`. A `QueryWindow` can clone that
guard from its live request. Once publication swaps the pointer, no new old-view guard can appear; existing
old guard count decreases monotonically.

Publication cleanup starts after the relevant guards drain:

- same generation: delete exact retired IDs in bounded pages and read back absence;
- replaced generation: delete the whole old collection after its final guard;
- manifest/source/prepared objects: remove exact unreferenced objects only after current, pending and live
  view roots no longer reference them;
- uncertain delete: retain the cursor/root, inspect state and retry; leaking is safer than deleting live data.

Move the useful exact delete/readback helper from `search-index-reclaimer`; delete that package's public
plan/proof/permit/checkpoint platform. No generic leases, legal purge, tombstones or second GC journal.

## Removal boundary

After the real operation is connected, remove:

- the current receipt/observation phase API and whole-preparation digest/reconstruction loops;
- daemon `publication_composition`, `FileJournal`, `PublicationFloor` and process-only fault machine;
- redb intent phases that merely mirror supplied receipts instead of a real effect cursor;
- completed publication receipt/history archives;
- separate index reclaimer journal and sampled pin-watermark proof;
- source-shape/module-ownership tests that require the deleted decomposition.

Keep useful validation of page ordering, stable point identity, exact readback, visible-view atomicity and
unknown-outcome recovery. Change producer/consumer APIs together; do not add a facade preserving the three
old machines.

## Completion

Close when a daemon restart can resume before/after each object/redb/Qdrant/visibility boundary, one real
query pins the committed view, an old query survives a switch, no new old guard can appear, and retired
points/generations/artifacts are cleaned only after the final guard. Package phase tests alone do not
establish publication.

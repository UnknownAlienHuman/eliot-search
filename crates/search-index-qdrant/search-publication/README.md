# search-publication

Own the only transition from prepared retrieval pages to one queryable immutable `PublishedView`.
Publication owns the Qdrant effect cursor, visible-view commit, current-view swap and cleanup after old
readers drain.

Read the [architecture](../../../docs/architecture/README.md),
[implementation guide](../../../docs/implementation/README.md) and [source map](FUNCTIONS.md).

## Current source is not a publisher

Public main contains about 102 KiB of non-test Rust in this package. It has no Qdrant or redb dependency.
`PublicationCoordinator` stores full manifests/diffs and several copies of exact point-ID lists while
accepting caller-produced receipts and observations. `recover` only classifies those supplied values.
`restore_inflight` requires the caller to reconstruct the original preparation, intent, receipt prefix,
control observation and Qdrant observation before it can rebuild the in-memory machine.

Repository search finds package/tests/documentation users, not a connected daemon caller. The daemon has a
second dead-code `publication_composition::Publisher`; redb has a third intent/visible-epoch machine. None
publishes a query view or owns a real Qdrant cursor.

Do not connect or synchronize these three machines. Replace them.

## One operation

```text
ordered prepared pages
→ immutable operation/manifest artifact
→ small PendingPublication row
→ Qdrant page dispatch + exact readback
→ CurrentPublishedView compare/set
→ atomic current-view swap
→ old-view drain
→ exact point/generation/artifact cleanup
```

A publication receives one target view sequence. New points are staged with `valid_from = target`; retired
points are closed with `valid_until = target`. Until redb commits the target `CurrentPublishedView`, queries
continue to pin the previous sequence and cannot see staged points. After commit, old pinned queries still
see their previous sequence.

This visibility interval is the useful mechanism. It does not require ten durable phases, seven global
guards, success receipts, snapshot acknowledgements or an epoch reservation archive.

## Durable state

The immutable artifact contains bounded ordered pages:

- target route/physical generation/profile/view sequence;
- point upserts and exact old-point closures;
- `ViewManifestEntry` pages binding point IDs to retained evidence;
- exact expected Qdrant readback for each page;
- retired IDs or replaced physical generation;
- old/new manifest roots and cleanup work.

redb stores only small current records:

```text
CurrentPublishedView {
  view_id / sequence
  route / physical generation / profile
  manifest roots and retained-object roots
}

PendingPublication {
  artifact_ref
  expected_current_view
  target_view
  next_effect/readback/cleanup cursor
  compact state
}
```

Write and verify the artifact before redb references it. Never hold a redb writer across object or Qdrant
I/O. redb contains no source text, vectors, point batches or completed-operation history.

## Forward recovery, not compensation theatre

Before any possible Qdrant dispatch, cancellation or validation failure discards the local preparation.
After dispatch may have occurred, keep the same pending operation and inspect actual Qdrant state. If the
expected page is absent, dispatch it; if present, advance; if conflicting, block the target and keep serving
the committed current view.

Baseline recovery finishes the same operation. It does not mint a replacement identity, rewind a view
sequence, restore old payloads from receipts, abandon through membership fences or publish an
invalidation-only epoch. Later source changes queue behind the pending publication and become the next
ordinary publication.

## View lifetime and cleanup

Queries can only `pin_current()`. A `QueryWindow` may inherit a guard from its live request. After the
pointer swaps, no new old-view guard can appear; the old guard count only falls.

After the last relevant guard:

- same physical generation: delete exact retired IDs in bounded pages and read back absence;
- replaced generation: delete the whole old collection;
- local source/prepared/manifest objects: delete only after current, pending and live-view roots disappear;
- uncertain deletion: retain the cursor and retry after readback. Leaking is safer than deleting live data.

## Remove after cutover

- `PublicationCoordinator`, `PublicationPhase`, durable-intent/receipt/observation/restore/abort APIs;
- repeated whole-manifest reconstruction and canonical digest preimages between trusted adjacent owners;
- daemon `publication_composition`, `FileJournal`, `PublicationFloor`, compensation and retirement models;
- redb VisibleEpoch/intent/snapshot-receipt machines;
- separate reclaimer journal and raw epoch-pin registry;
- package/process/module-shape fixtures whose only purpose is preserving those boundaries.

Keep ordered diff/page validation, stable point identity checks, real Qdrant readback, typed current/pending
rows, view guards and exact cleanup.

## Completion

A real retained file must publish, become queryable through the committed view and reopen the correct
retained excerpt. Restart must resume around artifact persistence, pending commit, each RPC/readback,
visible-view commit and pointer swap. An old query must survive the switch and cleanup must wait for its
last guard. Package phase tests do not establish publication.

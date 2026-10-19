# search-publication

Own the only transition from prepared retrieval pages to one queryable immutable `PublishedView`. Publication
owns Qdrant mutation recovery, the visible-view commit and the finite backlog of external cleanup which is
still genuinely unfinished.

Read the [architecture](../../../docs/architecture/README.md),
[implementation guide](../../../docs/implementation/README.md) and [source map](FUNCTIONS.md).

## Current source is not a publisher

Public main contains about 102 KiB of non-test Rust in this package but no Qdrant, redb or daemon dependency.
`PublicationCoordinator` keeps complete manifests/diffs and several copies of point IDs while accepting
caller-produced receipts and observations. Recovery classifies those supplied values; it does not reopen the
durable operation or inspect Qdrant. Daemon publication composition and redb publication codecs form two
more disconnected machines.

Do not connect or synchronize the three machines. Replace them with one operation.

## One publication operation

```text
ordered prepared pages
→ immutable operation/manifest artifact
→ one PendingPublication row
→ Qdrant page mutation + exact readback
→ CurrentPublishedView compare/set
→ current-view pointer swap
→ one finite RetiredCleanup obligation when physical deletion remains
```

A publication receives one monotonically increasing target sequence. New points use
`valid_from = target`; retired points are closed with `valid_until = target`. Until the current-view commit,
queries pin the previous sequence. After commit, new queries use the target while existing guards keep their
exact older sequence, route, manifest and evidence.

This visibility interval is the useful mechanism. It does not require ten phases, seven global guards,
terminal receipts, snapshot acknowledgements or an epoch-reservation archive.

## Durable state

The immutable operation artifact owns bounded ordered pages:

- target route/physical generation/profile/view sequence;
- full or narrow Qdrant mutations and exact expected readback;
- `ViewManifestEntry` pages binding stable point IDs to retained evidence;
- exact point closures or a replaced physical generation;
- cleanup pages needed after visibility.

redb stores only current unfinished state:

```text
CurrentPublishedView {
  view_id / sequence
  route / physical generation / retrieval profile
  manifest and retained-object roots
}

PendingPublication {
  operation_artifact_ref
  expected_current_view
  target_view
  next effect/readback cursor
}

RetiredCleanup {
  cleanup_id
  cleanup_artifact_ref
  kind: exact point IDs | whole physical generation
  route / generation
  retire_before_sequence
  next effect/readback cursor
}
```

`RetiredCleanup` is not publication history. A row exists only while a real Qdrant delete or its readback is
unfinished. It is removed after exact completion. The table is finite; publication backpressures before a
new external operation when the configured outstanding-cleanup bound is full.

Write and verify every artifact before redb references it. Never hold a redb writer across object or Qdrant
I/O. redb contains no source text, vectors, point batches or terminal-operation archive.

## Commit and current-view boundary

The durable view switch and creation of its cleanup obligation happen in one short transaction:

```text
block new pin_current() calls
→ compare expected current view and pending operation
→ write CurrentPublishedView
→ insert RetiredCleanup when needed
→ clear PendingPublication
→ commit redb
→ swap the in-memory current Arc
→ unblock new pins
```

No Qdrant or object I/O occurs while the pin/swap lock is held. If the redb commit acknowledgement is
uncertain, reopen the current/pending/cleanup rows while new pins remain blocked, then either install the
committed target or keep the old pointer. A crash reconstructs the current pointer and cleanup backlog from
redb.

Clearing `PendingPublication` without inserting `RetiredCleanup` would lose the exact old IDs/collection on
a crash after the view switch. Keeping the old operation forever would create a history ledger. The finite
cleanup row is the small correct boundary.

## Live view ownership

The baseline needs no raw pin registry or new dependency. One manager can hold:

```text
current: Arc<PublishedViewState>
retired: ViewId → Weak<PublishedViewState>
```

`pin_current()` briefly locks the manager and clones only `current`. A live request may clone its existing
guard into a `QueryWindow`; no API reacquires an arbitrary route, generation or sequence. The manager stores
only weak references to retired views, so it does not keep them alive. Last-drop notification wakes cleanup;
a periodic scan is the fallback.

Cleanup eligibility is evaluated against every live view, not only the immediate predecessor:

- exact point IDs closed at sequence `N` are safe only when no live view on the same route/generation has
  `sequence < N`;
- an old collection is safe only when no live view uses that route/generation.

Example: v1 and v2 both reference a point and v3 closes it. Absence of a v2 guard does not permit deletion
while a v1 guard is live. A sampled count or “last guard of v2” would be wrong.

After restart no old process-local guards survive. Durable cleanup rows remain, so previously blocked deletes
become eligible and are not forgotten.

## Forward recovery

Before any possible Qdrant dispatch, cancellation/validation failure discards local preparation. After a
request may have reached Qdrant, keep the same pending page and inspect actual state. If absent, dispatch;
if expected state is present, advance; if conflicting, block the target and keep serving the committed view.

Baseline recovery finishes the same operation. It does not mint another sequence, rewind a view, compensate
from receipts, abandon through membership fences or publish invalidation-only epochs. Later source changes
become the next ordinary publication.

Cleanup uses the same rule. Unknown delete keeps the same `RetiredCleanup` cursor and resolves by exact
same-route/generation readback. The safe failure is retained data.

## Physical-generation replacement

Schema/profile/vector incompatibility creates a complete new physical generation. Build and verify it with
the same pending operation, commit the new view, then keep one generation-cleanup row until no live view uses
the old collection. Delete the collection whole; do not create per-point retirement records for an obsolete
generation.

## Remove after cutover

- `PublicationCoordinator`, phase/receipt/observation/recovery/restore/abort APIs;
- daemon publication composition, file journal, floor, compensation and retirement models;
- old redb intent/VisibleEpoch/successor/checkpoint/snapshot-publisher codecs;
- whole-manifest canonical digest admission and duplicated million-ID vectors;
- raw epoch-pin and separate reclaimer proof/checkpoint platforms;
- W7/P18/package-shape/process fixtures which preserve those APIs.

Keep ordered page validation, stable point/evidence checks, real Qdrant readback, typed current/pending/cleanup
rows, current-view guards and exact cleanup.

## Completion

A retained file publishes and becomes queryable from the committed view. Restart resumes around artifact
persistence, pending commit, every RPC/readback, visible-view commit and cleanup. An old query survives a
switch; no new old view can be acquired; point deletion waits for every older visible sequence in that
generation; generation deletion waits for every guard using it; and crash after view commit cannot lose the
cleanup obligation.

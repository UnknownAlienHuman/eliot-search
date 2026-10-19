# search-publication

Own the only transition from prepared retrieval pages to one immutable query view. Publication owns Qdrant
mutation recovery, durable current-view commit, installation of the in-process current pointer and the finite
backlog of real external cleanup still unfinished.

Read the [architecture](../../../docs/architecture/README.md),
[implementation guide](../../../docs/implementation/README.md) and [source map](FUNCTIONS.md).

## Replace the three disconnected machines

Public main has roughly 102 KiB of non-test Rust in this package but no Qdrant, redb or daemon dependency.
It stores complete manifests/diffs and caller receipts in a ten-phase coordinator. Daemon publication and
redb visibility codecs form two more partial machines. None owns one real apply/readback/view path.

Replace them with:

```text
ordered prepared pages
→ immutable operation/manifest artifact
→ PendingPublication
→ Qdrant page mutation + exact readback
→ one redb transaction:
     CurrentPublishedView = target
     RetiredCleanup += unfinished physical deletion
     PendingPublication = none
→ short in-process current-pointer swap
→ eligibility-gated cleanup
```

## Visibility rule

Every publication receives one monotonically increasing target sequence:

- new points use `valid_from = target`;
- retired points use exclusive `valid_until = target`;
- a query filters with the sequence in its pinned `PublishedView`.

Until the in-process pointer swap, new requests may still pin the old immutable view. This is safe: the old
sequence, manifest and validity interval remain queryable and merely delay cleanup. Publication is reported
visible only after the pointer is installed. If the process crashes after redb commit and before swap,
restart installs the durable target and no old request survives.

Do not hold the view-manager mutex across redb fsync or Qdrant/object I/O.

## Durable state

```text
CurrentPublishedView {
  view_id / sequence
  route / generation / retrieval profile
  manifest/evidence roots
}

PendingPublication {
  operation_artifact_ref
  expected_current_view
  target_view
  next apply/readback cursor
}

RetiredCleanup {
  cleanup_artifact_ref
  kind: exact point IDs | whole generation
  route / generation
  retire_before_sequence
  next delete/readback cursor
}
```

`RetiredCleanup` is not history. It exists only while actual delete/readback is unfinished and is removed
after exact success. The finite table backpressures new publication before new external work when full.

Write/verify immutable artifacts before redb references them. Never store source text, vectors or whole point
batches in redb. Never hold a redb writer across object/Qdrant I/O.

## Recovery

Before possible Qdrant dispatch, failure discards local preparation. After dispatch may have happened, keep
the same pending page and inspect actual state. Dispatch only absent work, read back exact expected state and
advance one small cursor. Conflicting state blocks the target while the committed view remains serviceable.

At view commit, transfer unfinished delete pages to `RetiredCleanup` in the same redb transaction which sets
current and clears pending. Clearing pending without this transfer loses cleanup after crash; retaining the
whole completed operation forever creates an archive.

An uncertain redb commit is resolved from typed current/pending/cleanup rows before pointer swap. Do not
compensate from receipts, rewind sequences or mint a replacement operation.

## View lifetime and cleanup

The view manager stores one strong current `Arc` and weak retired refs. `pin_current()` clones only current.
A live request can clone its guard into a `QueryWindow`; callers cannot reacquire an old route/sequence by ID.

Correct cleanup conditions:

```text
point IDs closed at N:
  no live view on the same route/generation has sequence < N

old generation:
  no live view uses that route/generation
```

Waiting only for the immediate predecessor is wrong: v1 can remain live after v2 is gone while v3 closes a
point visible in both. Last-drop wakes cleanup; deletion never runs in `Drop`.

Unknown delete retains the same cleanup cursor and retries exact same-route/generation readback. A replaced
physical generation deletes as one collection after all its guards drain.

## Remove after cutover

- phase/receipt/observation/recovery/restore/abort coordinator APIs;
- daemon file journal, floor, compensation and retirement models;
- old redb intent/VisibleEpoch/successor/checkpoint/snapshot-publisher codecs;
- repeated whole-manifest digest preimages and duplicate ID vectors;
- raw pin registry and separate reclaimer proof/checkpoint platform;
- W7/P18/package/process/shape fixtures preserving those boundaries.

Keep ordered page validation, stable point/evidence checks, real Qdrant readback, typed current/pending/cleanup
rows, current-view guards and exact cleanup.

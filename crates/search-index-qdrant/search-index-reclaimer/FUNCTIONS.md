# Index cleanup — source audit and consolidation

Ordinary Qdrant cleanup is the terminal external effect of committed publication, not a separate proof and
journal platform. Move the useful exact delete/readback helper into publication/bridge and remove this
package when callers switch.

## Current source findings

Production source is about 25.3 KiB:

| Current source | Problem |
|---|---|
| `manifest.rs` | `validate_retired_manifest` compares two caller-supplied structures. It neither derives IDs from committed publication nor binds its digest to `point_ids`. |
| `plan.rs` | Copies the manifest into batches and creates custom FNV-like plan/operation identities. |
| `execute.rs` | Useful synchronous exact delete/readback skeleton, but the port lacks authoritative route/generation/deadline ownership and is detached from live views. |
| `receipt.rs` | Adds public batch/checkpoint/receipt state which would require another durable journal. |
| callers | Package/live fixtures dominate; no connected production `execute_batch` caller or checkpoint owner was found. |

A sampled `ReclamationWatermark` cannot close acquisition races. The current raw pin registry also checks
only route/epoch counts, not the exact set of older view sequences which may still reference a retired point.

## Durable cleanup record

Publication's visible-view transaction transfers only unfinished physical work from pending state:

```text
RetiredCleanup {
  cleanup_id
  cleanup_artifact_ref
  kind: PointIds | Generation
  route / generation
  retire_before_sequence
  next effect/readback cursor
}
```

The immutable artifact owns exact sorted ID pages or the exact old collection identity. The record is
removed after exact completion. There is no separate operation identity, proof, checkpoint history or
terminal receipt.

The finite cleanup table is allowed to contain multiple outstanding view transitions. A single pending row
cannot remain occupied by cleanup while later views publish, and clearing pending without this row would
lose cleanup on crash. When the finite bound is full, publication backpressures before new external work.

## Eligibility from live views

Publication/view manager supplies a strong snapshot of every currently live view.

### Same generation

Points closed with exclusive `valid_until = N` are physically deletable iff:

```text
no live view has
  route == cleanup.route
  generation == cleanup.generation
  sequence < N
```

This is not equivalent to “the immediate old view has no guard.” Example:

```text
v1: P visible
v2: P visible
v3: P closed at N=3
```

A live v1 still needs P even when no v2 guard exists. Deleting after only v2 drains breaks v1.

### Replaced generation

The old collection is deletable iff no live view uses its route/generation. Delete the collection whole;
do not create per-point retire pages for a generation no future view will use.

### Restart

Old interactive guards/windows are process-local and do not survive. Durable cleanup rows do. At startup the
manager installs current only, then cleanup reevaluates all rows; work blocked solely by old process guards
becomes eligible.

## Cleanup execution

For an eligible exact-ID page:

1. load the next immutable page;
2. inspect exact same-route/generation state first if a prior dispatch may have happened;
3. issue exact ID delete with the original cleanup deadline/cancellation policy;
4. read back exact absence of every ID;
5. advance cursor in one short redb transaction;
6. remove completed page data and final row after success.

For a generation row, inspect/delete/read back exact collection absence.

Timeout, cancellation or transport failure after possible dispatch is unresolved. Keep the same cursor and
read actual state. Partial/foreign/unexpected results block advancement. Broad-filter deletion is absent.

The bridge owns vendor translation only. Publication owns row/cursor/recovery. This package owns nothing
after helpers move.

## Local artifact GC

Qdrant cleanup and immutable-object GC are separate effects but use the same lifetime roots:

- current source/prepared references;
- `CurrentPublishedView` manifest/evidence roots;
- one `PendingPublication` artifact;
- finite `RetiredCleanup` artifacts/pages;
- live view snapshots/query windows;
- explicitly selected retained history, if any.

Object producers hold a shared GC gate from finalization through first durable reference. Bounded GC holds
the exclusive gate while snapshotting roots and deleting a small exact unreferenced page. This prevents a
newly finalized or deduplicated object from being deleted before its source/view/cleanup reference commits.

Unknown root traversal or delete keeps bytes. No generic protection graph, lease catalogue or sweep journal.

## Donor interpretation

Lucene's snapshot policy refcounts in-memory commit generations and suppresses deletion while a snapshot is
held; its documentation explicitly says that state is not crash-persistent. Eliot reuses the process-local
view lifetime only and persists physical cleanup obligations separately.

Tantivy's segment updater asks its directory GC to remove files outside the committed/in-flight living set.
Eliot similarly derives living objects from current/pending/cleanup/live roots. It does not import Tantivy's
segment writer or JSON registry.

## Removal boundary

Delete after direct publication/bridge callers move:

- caller `PublicationCommitProof` and retired-manifest field-equality admission;
- public reclaim plan/batch/checkpoint/receipt constructors and custom digests;
- route/epoch watermark and reclaimable Boolean inputs;
- separate operation port/journal;
- purge/restore/rollback interaction state;
- P18/W7/source-shape/process fixtures;
- package and workspace dependencies.

Move only private exact delete/absence-readback code which remains useful.

## Required behavior checks

- v1/v2/v3 point example never deletes early;
- old query continues after new view commit;
- new old-view acquisition is impossible after swap;
- exact point cleanup starts at the correct sequence threshold;
- generation cleanup waits for every view using it;
- crash after current-view commit preserves the cleanup row;
- restart with no old guards resumes eligible work;
- timeout after delete resolves by exact same-route/generation readback;
- partial/unexpected IDs block cursor advancement;
- cleanup backlog bound causes safe publication backpressure;
- current/pending/cleanup/live object roots survive GC;
- no ordinary cleanup path claims purge or secure erase.

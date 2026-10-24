# Index cleanup — temporary extraction map

This package has no independent durable responsibility in the target product. It currently contains a useful exact Qdrant delete/readback skeleton surrounded by caller proofs, plans, digests, receipts and checkpoint state. Move the useful private helper into publication/bridge, then delete this package and its workspace dependencies.

Read the [publication owner](../search-publication/FUNCTIONS.md), [Qdrant bridge](../search-qdrant-bridge/FUNCTIONS.md) and [stopped-root maintenance](../../search-runtime/search-retention/FUNCTIONS.md).

## Current source findings

- `manifest.rs` validates caller-supplied structures instead of deriving exact retired IDs from committed publication.
- `plan.rs` copies manifests into another plan and adds custom identities.
- `execute.rs` contains the reusable exact delete/absence-readback shape but lacks the current route, view, deadline and recovery owner.
- `receipt.rs` creates a second cleanup journal.
- R028 at `37dbb198` traces daemon `rebuild_composition::authorize_reclaim`: it returns a plan after
  caller-proof/pin checks, performs no deletion and has no production caller. A feature gate does not
  connect `execute_batch` to the real bridge; its `IndexAdmin` is still a supplied abstraction.

## Current owner

Publication commits one finite unfinished cleanup obligation with the new current view:

```text
RetiredCleanup {
  artifact containing exact point IDs or exact retired collection identity
  route / generation
  retire_before_sequence when point cleanup is used
  next effect/readback cursor
  possible-dispatch state needed for recovery
}
```

There is no separate reclaim plan, proof, receipt history or terminal record. When the finite cleanup table is full, publication backpressures before another external mutation.

### Reader eligibility

A same-generation point closed with exclusive `valid_until = N` is eligible only when no live view of the same route/generation has `sequence < N`. A retired collection is eligible only when no live view uses that route/generation.

Checking the immediate predecessor or one watermark is insufficient. New requests can acquire only current; existing queries/windows retain their exact old view guard. Restart removes old process-local guards but not durable cleanup rows.

### Writer eligibility

Reader drain is not writer quiescence. Do not delete or declare completion while an earlier Qdrant writer may still recreate or replace the state. One matching readback, elapsed client deadline or reconnect does not prove terminality. Unresolved abandoned-route obligations remain visible and bounded as specified by publication.

## Exact cleanup effect

For a settled exact-ID page:

1. load the immutable page named by the cleanup row;
2. inspect exact same-route/generation state when a prior delete may have happened;
3. issue exact-ID deletion under the current attempt deadline/cancellation;
4. read back exact absence;
5. advance the durable cursor in one short redb transaction;
6. remove the row/artifact only after exact completion.

For a retired generation, inspect/delete/read back exact collection absence. Partial or foreign state blocks advancement. Broad-filter deletion is not used. Qdrant vendor conversion stays private to the bridge; publication owns the row, cursor, recovery and visibility.

## Local immutable-object GC is not this operation

Local object collection is explicit **stopped-root maintenance**. The daemon and all producers/windows are stopped, one exclusive root lock is held, all current/prepared/published/pending/cleanup references are validated and marked once, then only proven-unreferenced objects are swept. Unknown traversal or delete keeps bytes.

Do not implement the previous online shared/exclusive `ObjectGcGate`, per-publication root rescans, generic lease catalogue or sweep journal. Qdrant cleanup remains online because it is coordinated with live view/writer state; local object-file GC does not.

## Removal boundary

Delete after direct publication/bridge callers move:

- `PublicationCommitProof` and retired-manifest field-equality admission;
- reclaim plan/batch/checkpoint/receipt constructors and custom digests;
- route/epoch watermark and caller `reclaimable` booleans;
- separate operation port/journal;
- purge/restore/rollback interactions;
- W7/P18/source-shape/process fixtures;
- package and workspace dependencies.

Move only the private exact delete/readback code that remains useful.

## Required behavior checks after integration

- a live v1 prevents deletion even when v2 is gone and v3 closed the point;
- generation cleanup waits for every view using it;
- possible delayed writers prevent unsafe cleanup;
- current-view commit crash preserves cleanup work;
- restart resumes work blocked only by dead process guards;
- unknown/partial delete resolves from exact state without broad retry;
- backlog bound causes safe publication backpressure;
- stopped-root GC preserves every current/pending/cleanup object root;
- ordinary cleanup never claims purge or secure erase.

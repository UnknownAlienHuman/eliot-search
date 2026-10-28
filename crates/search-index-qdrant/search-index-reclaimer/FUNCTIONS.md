# Index cleanup — extract whole-collection helper, delete the platform

This package has no independent durable responsibility in the target product. It contains some exact Qdrant
delete/readback mechanics surrounded by plans, digests, receipts, watermarks and checkpoints. Move the useful
private whole-collection helper into publication/bridge, then delete this package and its workspace edges.

Read the [publication owner](../search-publication/FUNCTIONS.md),
[Qdrant bridge](../search-qdrant-bridge/FUNCTIONS.md) and
[stopped-root maintenance](../../search-runtime/search-retention/FUNCTIONS.md).

## Current source findings

- `manifest.rs` validates caller-supplied retired structures instead of deriving cleanup from committed current/view state.
- `plan.rs` copies manifests and creates another identity/plan layer.
- `execute.rs` has a reusable exact delete/absence-readback shape but lacks the actual view/deadline/recovery owner.
- `receipt.rs` creates a second cleanup journal.
- daemon `authorize_reclaim` returns plans after proof/pin checks but performs no connected deletion.

## Stage 3A cleanup owner

Publication commits one compact row with the new current view:

```text
RetiredCleanup {
  exact never-reused physical collection name
  expected predecessor/current relationship
  unresolved-attempt state needed to prevent retargeting
}
```

No point-ID pages, retire-before sequence, reclaim plan, checkpoint cursor, receipt history or terminal record is
required in Stage 3A. Visible collections are immutable and cleanup is whole-generation only.

A row is eligible when:

- the collection is not durable current;
- no pending/current/future build can target it;
- no live `PublishedView` or `QueryWindow` guard uses it;
- no owned unresolved request/build remains targetable;
- no redb writer or view-manager lock is held across backend I/O.

Last guard drop only wakes cleanup.

## Exact effect

```text
eligible physical collection
→ bridge terminal delete under attempt deadline/cancellation
→ exact exists/get observation
→ logical absence
→ short redb removal of cleanup/obsolete metadata
```

Never reuse the collection name. Do not infer completion from list omission. Unknown/partial/foreign state or
uncertain response keeps the row and bounded publication backpressure. Logical absence is not physical erasure.

Execute delayed request/build versus collection-delete behavior against selected Qdrant 1.20 before claiming
safety.

## Stage 3B

Incremental point closure does not restore per-point physical deletion. Individual points remain until the same
whole-generation cleanup. There is still no point reclaimer, possible-writer ledger or per-point cleanup journal.

## Local immutable-object GC is separate

Local object collection is explicit stopped-root maintenance: all producers/windows stopped, exclusive root held,
all current/prepared/published/pending/cleanup references validated and marked, then only proven-unreferenced
objects swept. Unknown traversal/deletion keeps bytes.

Do not restore online `ObjectGcGate`, per-publication root rescans, generic lease catalogue or sweep journal.

## Removal boundary

Delete after publication/bridge callers move:

- `PublicationCommitProof`, retired manifests and field-equality admission;
- reclaim plans/batches/checkpoints/receipts/custom digests;
- route/epoch watermarks and caller `reclaimable` booleans;
- exact point-delete pages and per-point writer eligibility machinery;
- separate operation port/journal;
- purge/restore/rollback integrations and old fixtures;
- package/workspace dependencies.

Move only the private whole-collection delete/absence code which remains useful.

## Checks

Old/current/new guards, no dispatch after retirement, restart with cleanup rows, commit/install crash, exact
logical absence, delayed request/delete race, name non-reuse, uncertainty/backpressure, measured retained-generation
disk and stopped-root object preservation. Ordinary cleanup never claims purge or secure erase.

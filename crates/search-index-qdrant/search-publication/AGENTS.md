# Working on search-publication

Read the root [architecture](../../../docs/architecture/README.md),
[implementation](../../../docs/implementation/README.md) and local [source map](FUNCTIONS.md).
Change planner, redb, bridge, current-view owner and query lookup together.

Publication owns one transition:

```text
bounded local plan
→ durable pending artifact/reference
→ Qdrant effects and readback
→ committed PublishedView
→ old-view drain and cleanup
```

Before pending persistence, preparation is disposable and must not occupy the coordinator permanently.
After an external request may have committed, the same operation remains recoverable. redb stores small
current/pending metadata; ordered pages and effect/cleanup cursor live in the artifact owner. Do not hold a
redb writer across Qdrant I/O.

Publish one immutable current-view pointer after durable visibility commit. New queries pin only that view;
old guards keep old manifests/evidence. Cleanup is part of the same operation after old guards drain. Remove
the daemon's duplicate publisher, caller-created proof receipts, whole-manifest digest preimages and the
separate reclaimer journal.

Verify every durable/external boundary, old-query survival, no old-view reacquisition after switch,
full/narrow Qdrant updates and exact cleanup. Package-only phase fixtures do not establish publication.

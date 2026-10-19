# Working on search-publication

Read the [architecture](../../../docs/architecture/README.md),
[implementation guide](../../../docs/implementation/README.md) and local [source map](FUNCTIONS.md).
Change planner, typed redb state, Qdrant bridge, view manager and query lookup together.

Implement one owner:

```text
operation artifact
→ PendingPublication
→ Qdrant apply/readback
→ redb: CurrentPublishedView + RetiredCleanup - PendingPublication
→ short view-manager install
→ eligibility-gated cleanup
```

Do not hold the view-manager mutex across redb fsync or any object/Qdrant I/O. Durable commit comes first,
then a short `Arc` pointer install. A request which pins old in that gap remains valid and simply delays
cleanup. Report publication success only after install; restart reconstructs from redb.

Clearing pending without inserting unfinished cleanup loses old IDs/collection after crash. Retaining the
whole completed publication creates history. Persist only finite `RetiredCleanup` rows and remove them after
exact delete/readback.

Use private `Arc/Weak` guards. No route/sequence acquisition, owner ID or renewal API. Cleanup checks every
live view: point IDs closed at N wait until no same-generation live view has sequence below N; a collection
waits until no live view uses it. Immediate-predecessor drain is insufficient.

Keep ordered pages, view-sequence validity, exact readback and forward recovery. Delete phase/receipt/
observation/restore/abort APIs, old redb visibility machines, bridge receipt ledger, raw pins and separate
reclaimer state.

Coordinate object lifetime through the shared GC gate: final object publication/reuse through first durable
reference under shared protection; bounded root snapshot/deletion under exclusive protection. Never acquire
view manager while holding long redb/object/Qdrant work.

After code, run minimal Rust 1.98 compiler/Clippy for changed production owners and focused crash, commit-gap,
v1/v2/v3, cleanup-backlog and GC-race behavior. Broad tests follow the connected product.

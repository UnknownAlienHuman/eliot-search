# search-epoch-pins

Keep an active query's published route/epoch readable for the lifetime of that
query. A continuation retains that protection while it can still be resumed.
Retired points or an old collection can be removed after their final guard ends.

The [registry](src/lib.rs) uses an `Arc<Mutex<_>>` and RAII guards in one process.
Ordinary queries need no durable lease records. The continuation owner already has
a [transfer from a live query guard](src/continuation/creation.rs), used by
[`create_pinned_ephemeral`](../../search-query/search-continuation/src/security/creation.rs).

A registry snapshot is an observation of current pins, not permission for a later
delete. Current raw acquisition APIs can introduce an old pin after that snapshot.
Publication, admission and reclamation still need one connected lifetime rule;
`publish_active_route` is currently called only in tests.

The useful donor is the per-query snapshot lifetime in
[Tantivy 0.22.0](https://github.com/quickwit-oss/tantivy/blob/17d5869ad61ea9f1072ef40546ddcada3a14b067/examples/basic_search.rs):
obtain one searcher, reuse it throughout related query work, then release it.
This is an ownership example. Eliot still implements its Qdrant route/epoch view.

Read the [architecture](../../../docs/architecture/README.md),
[implementation](../../../docs/implementation/README.md), [API map](FUNCTIONS.md)
and [agent notes](AGENTS.md).

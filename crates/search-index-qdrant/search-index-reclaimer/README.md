# search-index-reclaimer

Remove rebuildable Qdrant points that a committed publication retired and no
active query or continuation can still read. Use exact point IDs in bounded
batches. Publication determines retirement; this work reclaims its storage.

The package currently contains [pure manifest/plan builders](src/plan.rs) and a
[synchronous executor](src/execute.rs) over `IndexAdmin`. Implementations and
execution callers are fixtures, including a
[live Qdrant adapter](tests/live_rebuild.rs#L191). Daemon
[`authorize_reclaim`](../../../bins/eliot-searchd/src/rebuild_composition/kernel/reclaim.rs)
builds a plan, but there is no production `execute_batch` caller.

[`checkpoint` and `resume`](src/receipt.rs) manipulate in-memory values; no
checkpoint persistence or maintenance loop is supplied here. The current API also
does not carry cancellation, a deadline or a live pin guard into deletion.

Connect one committed retirement list, the real query-view lifetime and the
existing Qdrant client. Retain only progress needed to finish or retry work.
Ordinary index reclamation does not delete retained source bytes or acknowledge
security purge.

Read the [architecture](../../../docs/architecture/README.md),
[implementation](../../../docs/implementation/README.md), [API map](FUNCTIONS.md)
and [agent notes](AGENTS.md).

# search-publication

One owner applies prepared Qdrant work, commits current evidence metadata and view, then completes eligible
physical cleanup. It replaces the disconnected package/daemon/control state machines.

Read [implementation](../../../docs/implementation/README.md), then the single
[operation recipe](FUNCTIONS.md). Evidence lookup uses redb snapshots described in the
[control map](../../search-control-redb/FUNCTIONS.md), not a custom immutable manifest index.

This README does not duplicate the algorithm. Historical phase, receipt, raw-pin and whole-manifest APIs
are removal targets, not compatibility promises. Local object GC is stopped-root maintenance; online Qdrant
cleanup retains its actual unfinished rows and waits for old views.

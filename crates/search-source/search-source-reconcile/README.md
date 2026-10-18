# search-source-reconcile

Compare a registered source with an observation so ingestion can update content, rebind a path or
remove a confirmed missing source from current search.

**Current source:** [`reconcile_source`](src/lib.rs) is a pure comparison. The package also contains
batch validation and an in-memory operation ledger. It has no filesystem walker, watcher/USN producer,
durable inventory or connected daemon scheduler. Old logical function lists were design proposals.

**Target:** one incremental daemon path consumes coalesced hints and inventory entries, reads changed
sources, retains their exact bytes and applies current source state. Startup, resume, explicit sync and
periodic recovery use the same scanner. Missing observations remain visible; incomplete scans do not
confirm deletions. The reconciler does not own source bytes or index publication.

See [implementation](../../../docs/implementation/README.md) for the application path and
[FUNCTIONS.md](FUNCTIONS.md) for actual callers, donor choices and removal targets.

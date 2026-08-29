# Index and publication family

**Organizational capability family — not a Cargo package.**

## Child packages

- [`search-projection-planner/`](search-projection-planner/) — C13: Plan the exact rebuildable point set and immutable manifests for one projection membership without performing vendor I/O.
- [`search-point-identity/`](search-point-identity/) — C14: Encode canonical point keys, derive namespace-separated IDs and make collisions detectable and non-destructive.
- [`search-qdrant-bridge/`](search-qdrant-bridge/) — C15: Own all qualified Qdrant process and transport details behind vendor-neutral Search ports.
- [`search-publication/`](search-publication/) — C16: Serialize projection commits, verify exact readback and linearize visibility only through guarded control-journal commit.
- [`search-epoch-pins/`](search-epoch-pins/) — C17: Protect active query snapshots and old collection routes in memory without writing ordinary query leases.

## Family invariants

- Qdrant stores rebuildable projections only.
- Vendor transport is isolated in search-qdrant-bridge.
- VisibleEpoch changes only in the guarded control-journal commit.
- Exact point manifests replace broad correctness-path updates.

Each writer agent owns exactly one child package and follows that package's `AGENTS.md`.

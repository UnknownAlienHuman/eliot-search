# search-qdrant-bridge

Private translation between Eliot Search and the pinned Qdrant SDK. This package owns actual collection,
point, query and readback RPCs. Publication owns effect ordering, recovery and visible-view commit;
retained sources and published evidence metadata establish citations.

Read the [architecture](../../../docs/architecture/README.md),
[implementation guide](../../../docs/implementation/README.md) and [single implementation recipe](FUNCTIONS.md).
Do not treat this README as a second mutation or query specification.

## Current code versus the target

At `7d92fb8b`, `RealDataPlane::connect` no longer accepts `QualifiedGate`. The live-probe report validator
returns `()` for diagnostics/tests; it cannot issue a constructor credential.

The rest of the direct-runtime cutover is not complete. The current adapter still has supervisor/key-lease
binding, an exact server-build check, fixed nested timeouts, a process-local mutation map, explicit sparse
inputs and the read-only blocking facade. Removing the gate did not connect publication/query to native BM25.

The source inventory, selected operations, immutable published-point rule, empty-scope behavior, native
inference readback and late-write boundary live only in [FUNCTIONS.md](FUNCTIONS.md). In particular, SDK
support for narrow vector/payload updates is not an instruction to implement them or mutate published points.

[Analysis registry](../../../docs/audit/ANALYSIS_REGISTRY.md) records inspected revisions and exact check
coverage. A successful library check/Clippy run is not a daemon, live-server or recovery result.

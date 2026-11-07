# Indexing and publication

The selected path uses one operator-started same-host Qdrant server. Existing crate names are source locations,
not a requirement to preserve seven services. The [delivery guide](../../docs/implementation/README.md#delivery-sequence)
owns stage order; the recipes below own algorithms.

## Baseline profile

| Change | Read only this owner and affected callers |
|---|---|
| SDK, connection, schema, requests, readback | [Bridge](search-qdrant-bridge/FUNCTIONS.md) |
| Retrieval-text meaning and scoring profiles | [Lexical](../search-lexical/FUNCTIONS.md) |
| Fixed generation input, commit, restart, cleanup | [Publication](search-publication/FUNCTIONS.md) and [control](../search-control-redb/FUNCTIONS.md) |
| Page construction / point identity | [Projection](search-projection-planner/FUNCTIONS.md) / [identity](search-point-identity/FUNCTIONS.md), only for that stage |
| Current/retired view guards | [View lifetime](search-publication/FUNCTIONS.md#view-lifetime-and-whole-generation-cleanup); fold old pins/reclaimer helpers into their real consumers |
| Optional process-supervisor policy | [Removal boundary](search-qdrant-supervisor/FUNCTIONS.md); not a connection prerequisite |

The active exact client/server profile is 1.20.0. Only the bridge consumes the SDK; vectorless InsertOnly uses
the required empty NamedVectors container with zero vector entries. The bridge owns schema/request mapping.
Publication and read-only search are connected; exact native scopes belong in the work register. Dormant
supervisor/identity/pins/reclaimer Rust packages are removed; their retained recipes are policy, not services.

## Effect and lifetime boundary

Connect before the first generation exists. Connection success is not an installed PublishedView; use the
[readiness cases](search-qdrant-bridge/FUNCTIONS.md#connection-versus-index-readiness). Publication owns visibility
and retirement, not a backend alias or process receipt. Query pins its exact physical collection and metadata.
No second index, supervisor requirement, online local-object GC or duplicate recovery ledger is introduced.

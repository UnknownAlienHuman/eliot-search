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
| Existing process-supervisor code | [Removal boundary](search-qdrant-supervisor/FUNCTIONS.md); not a connection prerequisite |

The main client pin is still 1.19.0; the selected 1.20.x path is implementation work. The bridge owns the exact
upgrade and schema mapping. Do not reconstruct those settings from an old issue, example or another README.

## Effect and lifetime boundary

Connect before the first generation exists. Connection success is not an installed PublishedView; use the
[readiness cases](search-qdrant-bridge/FUNCTIONS.md#connection-versus-index-readiness). Publication owns visibility
and retirement, not a backend alias or process receipt. Query pins its exact physical collection and metadata.
No second index, supervisor requirement, online local-object GC or duplicate recovery ledger is introduced.

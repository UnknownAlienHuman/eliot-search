# Indexing and publication

The [implementation guide](../../docs/implementation/README.md) defines one coherent publication and
query path. This directory groups existing code; the package list is not the target architecture.

| Existing code | Role / concrete direction |
|---|---|
| [projection planner](search-projection-planner/) | Build point/manifest data from prepared units. Produce bounded batches; a full canonical manifest must not create an accidental per-membership manifest limit. |
| [point identity](search-point-identity/) | Identify real projections consistently across planning, backend and lookup. Remove copied bridge identities rather than cast unrelated IDs. |
| [Qdrant supervisor](search-qdrant-supervisor/) | Launch/stop and identify the actual local Qdrant process when managed by this application. |
| [Qdrant bridge](search-qdrant-bridge/) | Translate shared payloads and perform SDK writes/queries/readback. It owns no second publication/replay journal. |
| [publication](search-publication/) | One transition owner integrated with real pending control writes, indexed effects and publication commit. Fold useful daemon Publisher behavior into this path. |
| [epoch pins](search-epoch-pins/) | Protect admitted query views/routes in bounded in-memory state; release must eventually remove records. |
| [index reclaimer](search-index-reclaimer/) | Delete exact committed retired IDs after the actual pin check, with one deadline and useful readback. Supplied proof labels do not establish that IDs are retired. |

A timeout after a possible Qdrant write leaves an unknown outcome for the publication owner to resolve.
Only committed state makes a new view visible. Queries pin that view; reclamation cannot race new
admission of a retired one. Use the existing guarded mechanisms together instead of preserving
parallel machines in the daemon, coordinator and bridge.

# Indexing and publication

The [implementation guide](../../docs/implementation/README.md) defines one coherent publication and query
path. This directory groups existing code; the package list is not the target architecture.

| Existing code | Role / concrete direction |
|---|---|
| [projection planner](search-projection-planner/) | Build stable point and versioned manifest data from prepared units in bounded ordered pages. |
| [point identity](search-point-identity/) | Identify real retrieval projections consistently across planning, backend and lookup. Remove revision/offset-bound and duplicate bridge identities. |
| [Qdrant supervisor](search-qdrant-supervisor/) | **Optional only.** Baseline connects to a configured endpoint. Current ~90.7 KiB package has no product caller and models qualification/receipts it cannot establish; delete it unless a small concrete managed-child convenience is selected. |
| [Qdrant bridge](search-qdrant-bridge/) | One private SDK client/runtime for live schema, exact mutations/readback and query. It owns no process qualification, publication or replay journal. |
| [publication](search-publication/) | One pending artifact/cursor, exact Qdrant effects/readback, durable current view and finite unfinished cleanup rows. |
| [epoch pins](search-epoch-pins/) | Replace with one small current-view manager. Queries pin only current; windows clone a live guard. |
| [index reclaimer](search-index-reclaimer/) | Fold exact point/generation deletion into publication cleanup, then delete the package. |

The first product may use an externally started local Qdrant service. Managed process ownership, automatic
restart and release qualification cannot block source → publication → query integration.

A timeout after a possible Qdrant write leaves an unknown outcome for publication to resolve from actual
state. Only committed `CurrentPublishedView` makes a new view visible. Old evidence remains while its guards
exist; cleanup uses durable exact obligations rather than a second lifecycle platform.

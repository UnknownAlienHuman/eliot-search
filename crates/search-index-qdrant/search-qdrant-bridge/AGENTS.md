# Working on search-qdrant-bridge

Read the root [architecture](../../../docs/architecture/README.md),
[implementation](../../../docs/implementation/README.md) and local [source map](FUNCTIONS.md).

Use the existing pinned Qdrant client, private vendor conversion and reusable connection/runtime. The bridge
owns concrete schema/mutation/query/readback I/O only. Source bytes, published-view selection, evidence
binding and recovery state remain with their existing owners.

Adopt the shared stable retrieval-point model with planner, point identity, payload and query consumers.
Qdrant payload contains only stable identity/filter/group/validity data. Source revision, representation and
range live in the versioned view manifest; never mutate an old view's citation through payload updates.

Use local `qdrant/bm25` `Document` inputs with matching explicit document/query profile options. Resolve one
selected scope into retrieval filter and `idf.corpus`. Use native fusion/grouping for Qdrant legs.

Select the real mutation:

- full upsert for a new/complete authoritative point;
- vector add/delete for selected named vectors;
- payload merge/delete/overwrite with explicit semantics;
- exact ID or whole-generation deletion for cleanup.

Await `wait=true`, use one absolute deadline and read back affected state. Possible external effect remains
pending under publication; no bridge operation ledger, qualification credential or strong-ordering floor for
the one-node/one-writer baseline.

Remove duplicate bridge point/payload/filter types, numeric-ID padding, payload vector-digest extensions and
current fixed field/index counts with their callers. Verify the real retained-file → Qdrant → pinned-manifest
→ source-excerpt path; historical codec bytes and source-shape fixtures do not constrain the cutover.

# search-qdrant-bridge

Private translation between Eliot's selected published view and the existing Qdrant Rust client.
Keep one connection/runtime, bounded I/O and vendor types inside this package.

## Target boundary

- Planner supplies stable retrieval points, actual BM25/vector input and expected stored state.
- Publication owns pending/visible/recovery and cleanup cursors.
- Query supplies a resolved scope and uses the pinned view manifest for source evidence.
- Bridge performs schema, full/narrow mutations, exact readback and scoped query.

Qdrant payload stores stable identity/filter/group/validity fields. Exact source revision, representation and
range are versioned view-manifest data, not mutable point citation fields.

## Native search

Use local `qdrant/bm25` with explicit matching document/query options. Send the selected scope both as the
retrieval filter and sparse `params.idf.corpus`; filtering alone does not scope IDF. Native prefetch/RRF/
grouping handles Qdrant branches. Keep one shard for the local baseline and avoid duplicate portfolio points.

Top-k is not absence proof and requires no unconditional exact count. Candidate points become evidence only
after pinned-manifest lookup and retained-source validation.

## Mutations

Full upsert overwrites the entire point and is used only for a complete authoritative replacement.
Use vector add/delete and payload merge/delete/overwrite operations for narrow changes. Adding a named-vector
schema does not populate existing points; incompatible profiles normally use a fully populated new physical
generation before view switch.

Every operation uses `wait=true`, one absolute deadline and exact readback. A timeout after possible dispatch
stays unresolved under publication. For the one-node/one-replica/one-writer baseline, strong ordering and an
executed qualification report are not runtime credentials.

The current S9/S11 and bridge field counts describe old code, not compatibility requirements. Change shared
identity/payload/schema and all direct consumers together; index only fields real filters/groups use.

[FUNCTIONS.md](FUNCTIONS.md) · [Implementation](../../../docs/implementation/README.md) ·
[Agent notes](AGENTS.md)

# Qdrant bridge

The private boundary for actual Qdrant schema, point, query and readback calls. One client/runtime serves
publication and query. Publication owns effect ordering/recovery/view commit; retained objects and pinned metadata
establish citations.

[FUNCTIONS.md](FUNCTIONS.md) owns SDK operations and readback.
[Lexical](../../search-lexical/FUNCTIONS.md) owns physical/query profile semantics;
[publication](../search-publication/FUNCTIONS.md) owns uncertainty and cleanup.
The selected scoring-text/TextQuery path replaces baseline sparse inference, not an additional backend.

The lock still contains client 1.19.0; selected 1.20.x integration is not implemented. Earlier gate-removal library
checks do not establish daemon/server behavior. [Registry](../../../docs/audit/ANALYSIS_REGISTRY.md#evidence-status)
owns evidence; [historical source inventory](https://github.com/UnknownAlienHuman/eliot-search/blob/45ddf887c9e84861b1302334a1f5ecabb263f7f3/crates/search-index-qdrant/search-qdrant-bridge/README.md)
records remaining legacy wrappers at that revision.

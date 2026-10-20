# Working on search-lexical

Read the [architecture](../../docs/architecture/README.md),
[implementation guide](../../docs/implementation/README.md), [source/caller audit](FUNCTIONS.md) and
[Qdrant bridge map](../search-index-qdrant/search-qdrant-bridge/FUNCTIONS.md).

Do not repair, version or connect the current general analyzer, frozen profiles, local sparse encoders,
scoring-corpus builder, qualification records or legacy daemon BM25 index. They are replacement/deletion
targets and have no current Qdrant product caller.

Implement the selected path:

```text
bounded unit/query text
→ optional small deterministic code-identifier expansion
→ Qdrant native qdrant/bm25 Document input
→ selected base idf.corpus
```

Preserve only useful Unicode identifier behavior: whole identifier, unique bounded snake/camel/acronym
components, XID boundaries, token-local NFC, deterministic ordering, finite expansion and cancellation. Do
not retain source revision/ordinal/offsets, token-position arrays, term-statistics maps, receipts, fixture
identities or local vector bytes in that helper. Source/evidence owners already retain exact coordinates.

Change preparation, planner, bridge/publication and query consumers together. Do not compute BM25 twice to
satisfy an old expected vector digest. Do not add a lexical-provider trait, encoder registry, second backend,
compatibility adapter or profile qualification service.

After code, run minimal Rust 1.98 check and strict Clippy for the helper and actual preparation/query/bridge
consumers, plus focused Unicode/expansion/limit cases and one connected native-Qdrant retrieval. Delete sparse
byte goldens, collision/qualification corpora, denominator digests, manual BM25 and legacy lexical fixtures
with their APIs.

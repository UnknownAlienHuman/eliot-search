# search-lexical

The selected baseline uses Qdrant scoring full-text BM25 and TextQuery. [FUNCTIONS.md](FUNCTIONS.md) owns text
composition, the physical/query profile split, token semantics and scope/statistics requirements.

Preparation supplies source text and selected context once. Do not append local identifier aliases or retain
a local BM25 encoder. Punctuation-sensitive Exact remains a separate retained-source operation.

The [bridge](../search-index-qdrant/search-qdrant-bridge/FUNCTIONS.md) owns SDK translation. Keep a local text
composer only where real preparation consumers need it; otherwise inline it. The selected integration remains
unimplemented; [the registry](../../docs/audit/ANALYSIS_REGISTRY.md#evidence-status) owns evidence status.

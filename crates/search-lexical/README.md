# search-lexical — native BM25 plus one bounded code helper

This package currently contains about **200.8 KiB** of three incompatible lexical systems. The selected
product uses Qdrant native BM25 for ranking. Only a small deterministic identifier-expansion helper may
remain.

Read the [architecture](../../docs/architecture/README.md),
[implementation guide](../../docs/implementation/README.md), [exact source/caller audit](FUNCTIONS.md) and
[Qdrant bridge map](../search-index-qdrant/search-qdrant-bridge/FUNCTIONS.md).

## Verified disposition

The only production Rust imports of `search-lexical` are the legacy daemon snapshot lexical modules. That
application builds its own postings and BM25 in memory and serves separate TCP/token commands. The frozen
sparse profiles, scoring-corpus builder and code-identifier encoder have no external production caller.

Delete the local analyzer/sparse/scoring/qualification systems and the legacy daemon in-memory index when the
native Qdrant path connects. Do not preserve their byte formats, profile digests or numerical behavior.

## Selected behavior

Preparation sends bounded text to Qdrant `Document(model="qdrant/bm25")`; query sends bounded query text
with the same published profile. The profile explicitly binds tokenizer/case/stemmer/stopwords/folding and
BM25 parameters. The selected base source population supplies `idf.corpus`.

For code, a small pure helper may add the whole identifier and unique bounded snake/camel/acronym components
using `unicode-ident` and token-local NFC. It returns prepared text/terms, not local sparse vectors, source
coordinates, receipts or fixture identities. Source ranges and exact matching remain with unit/retained-source
owners.

Keep this crate only if that shared algorithm remains substantial and has real preparation/query consumers.
Otherwise move it to the retrieval-input owner and delete the package. Never add a lexical provider registry,
second index or qualification platform.

## Completion

One real prepared unit is indexed and queried through native Qdrant BM25 under the selected scope, then
resolved through the pinned manifest to retained evidence. No local postings/BM25, frozen sparse profile,
vector digest prediction or legacy lexical command remains.

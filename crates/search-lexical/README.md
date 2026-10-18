# search-lexical

Keep the small amount of text preparation that improves source search: useful code identifier
expansion, explicit normalization and original source spans. The current Rust implementation also
contains general sparse encoders, corpus statistics and qualification machinery; these are removal
targets, not the intended product surface.

The [architecture](../../docs/architecture/README.md) and
[implementation](../../docs/implementation/README.md) supersede historical immutable-profile,
compatibility, qualification and size requirements. [FUNCTIONS.md](FUNCTIONS.md) maps the actual
source to the replacement.

## Native text ranking

Use Qdrant's local `qdrant/bm25` through the existing Rust client for ordinary text. Send bounded
`Document { text, model, options }` inputs at ingestion and query time; the bridge owns the SDK
translation. Qdrant supplies token hashing, TF/length weighting and sparse retrieval. Its explicit
`params.idf.corpus` supplies IDF over the selected published scope.

This is a new integration of an existing dependency, not an already connected Eliot path. The pinned
[server implementation](https://github.com/qdrant/qdrant/blob/74f3e85b9473c62560006c043e13737ce6b48412/src/common/inference/service.rs)
allows absent inference-provider configuration and evaluates BM25 locally. No model download,
FastEmbed or second search backend is needed.

## What remains here

Retain identifier behavior that helps find code: whole identifiers and useful snake/camel/acronym
components, bounded expansion, token-local normalization and original ranges. Apply the same
preparation to documents and queries. Qdrant's word tokenizer does not split camel case and breaks
underscores; already expanded terms can use its whitespace tokenizer.

Choose text processing explicitly. Native defaults include English stopwords and stemming; disable
them for code and neutral text. Keep exact matching and evidence reads in the existing source owners:
ranked tokens and sparse scores do not establish literal identity or absence.

Replace the old sparse encoders and their callers together. Keep useful token/search behavior checks;
old feature indexes, numeric scores, profile digests and fixture bytes impose no compatibility promise.

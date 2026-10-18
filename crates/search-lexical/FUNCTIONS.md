# Source map and implementation — search-lexical

The current crate implements more machinery than the product needs. Use native Qdrant BM25 for text
ranking and retain a small code-preparation kernel where it improves retrieval. The
[current implementation](../../docs/implementation/README.md) governs this change; the table below
describes existing source, not APIs that future callers must preserve.

## Existing source and disposition

| Source/API | Keep or change |
|---|---|
| [`analyze`](src/lib.rs) | Reuse bounded token handling and original ranges only where an actual consumer needs them. Do not keep a second general text analyzer merely to reproduce old sparse bytes. |
| [`CodeIdentifiersProfile::analyze`](src/code_identifiers.rs) | Extract the useful XID, whole-identifier, snake/camel/acronym, token-local NFC and original-span behavior. Keep input/expansion limits and cancellation; simplify the identity-bound profile wrapper. |
| [`encode_code_document`, `encode_code_query`](src/code_identifiers/encoding.rs) | Replace numerical encoding with prepared text sent to native BM25 when code enrichment is selected. Remove accepted-profile tokens and runtime fixture execution from the normal path. |
| [Frozen code/text profiles](src/frozen.rs) | Retire their canonical descriptors, term-index seeds and frozen-vector compatibility. They currently select logarithmic document/query TF; native BM25 is a ranking change, not a byte-compatible substitution. |
| [Sparse profiles/encoding](src/sparse/profile.rs), [mapping](src/sparse/mapping.rs), [weighting](src/sparse/numerical.rs) | Remove replaced local TF/IDF modes, hashing/collision machinery, qualification records and encoding receipts with their consumers. Use concrete BM25 options rather than another provider framework. |
| [`ScoringCorpusBuilder`](src/scoring.rs) | Remove the separate logical-document DF/statistics builder from the native path. Native IDF counts indexed points carrying the selected sparse vector, not distinct Eliot scoring-document identities. |
| [Code fixtures](src/code_identifiers/fixtures.rs) | Keep useful identifier/Unicode/range and bounded-work cases as ordinary checks. Drop golden sparse bytes, qualification digests and compatibility certificates for retired encoders. |

## Native ingestion and query

The existing `qdrant-client = 1.19.0` supports
[`DocumentBuilder`](https://github.com/qdrant/rust-client/blob/7c838035ae7b9455636dcaca918a55b7d7ca638f/src/builders/document_builder.rs).
The bridge sends a named `Document` in `PointStruct::new` through `upsert_points`; it sends
`Query::new_nearest(Document { .. })` with the same vector name/options through `query`.
See the [bridge operation map](../search-index-qdrant/search-qdrant-bridge/FUNCTIONS.md#native-query-and-scope).

The pinned server initializes its inference service at startup. Its
[`is_address_valid`](https://github.com/qdrant/qdrant/blob/74f3e85b9473c62560006c043e13737ce6b48412/src/common/inference/service.rs#L368)
accepts an absent address; `infer` returns local BM25 results before remote HTTP.
The [model dispatcher](https://github.com/qdrant/qdrant/blob/74f3e85b9473c62560006c043e13737ce6b48412/src/common/inference/local_model.rs)
selects document or query encoding. No dummy endpoint/token or external model service is required.

Prepare bounded unit text once; retain original source bytes and coordinates with their existing owner.
The current projection planner requires precomputed vectors and expected vector digests. Change that
input with the bridge to accept native text/options and validate the actual generated named-vector
readback; do not compute BM25 a second time to fill an obsolete expected digest.

## Text and code semantics

Use explicit, matching ingestion/query options. For neutral text and code, set
`stemmer: {"type":"none"}`, `stopwords: {"languages":[],"custom":[]}`, `ascii_folding:false`
and an explicit case policy. The server otherwise defaults to English stemming/stopwords.
Do not use the deprecated `language:"none"` shortcut. These options and the fixed ingestion
`k`, `b`, `avg_len` are defined by the
[pinned BM25 implementation](https://github.com/qdrant/qdrant/blob/74f3e85b9473c62560006c043e13737ce6b48412/src/common/inference/bm25_inference.rs).
`avg_len` is a supplied estimate, not a continuously recomputed corpus mean.

For ordinary text use the native word tokenizer; select multilingual processing when the supported
corpus needs it. For enriched code, preserve complete identifiers plus useful components and submit
the resulting terms with `tokenizer:"whitespace"`; native
[tokenizers](https://github.com/qdrant/qdrant/blob/74f3e85b9473c62560006c043e13737ce6b48412/lib/segment/src/index/field_index/full_text_index/tokenizers/mod.rs)
split word input on non-alphanumeric characters and do not implement Eliot's identifier expansion.
Keep normalization separate from original byte coordinates. Search relevance and original-span behavior
matter; reproducing a retired vector layout does not.

A change to tokenization or document weighting requires re-encoding affected indexed content. Use the
existing rebuild/published-view lifecycle to avoid mixing incompatible vectors, without a migration
platform or runtime qualification protocol. A published view supplies the query's actual options.

Group hits by source when useful, but do not claim grouping changes native IDF into file-level DF.
Keep one indexed occurrence for the intended unit/revision population and avoid duplicate ingestion.
Regex, exact identifiers and complete negative checks remain with exact/source matching; BM25 only ranks.

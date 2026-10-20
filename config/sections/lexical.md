# Lexical retrieval settings disposition

The current registry exposes only `lexical.profile_id = "baseline-v1"`; the generic snapshot does not connect
it to the Qdrant collection/query path. The old code/neutral/collision profile fields are not implemented.

Target:

```toml
[lexical]
profile = "qdrant-bm25-v1"
```

The exact profile binds stored/query `Document` role, tokenizer/case/stemming/stopwords/folding and BM25/IDF
semantics actually used by publication and query. A profile/schema change builds and verifies a new generation
then publishes it; old points are never reinterpreted in place.

No accepted-profile receipt, collision-fixture ID, section digest or automatic provider switch.

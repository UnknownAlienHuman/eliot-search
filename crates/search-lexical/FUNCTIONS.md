# Lexical retrieval — Qdrant 1.20 scoring full-text, not a local BM25 engine

This owner supplies the concrete lexical profile used by publication and query. It does not own source identity,
publication state, evidence, scope or Exact. R051 replaces the old `qdrant/bm25` sparse-vector design with direct
BM25 over a scoring full-text payload index in Qdrant 1.20.

## Selected baseline profile

The implementation cut upgrades server and Rust client together to one exact 1.20 patch. One vectorless physical
collection stores points with bounded payload fields:

```text
point identity/evidence key
source membership/filter keys
retrieval_text
```

Create one scoring text index before loading points:

```text
field             = retrieval_text
tokenizer         = code
lowercase         = true
ascii_folding     = false
stemmer           = none
stopwords         = none
min/max token len = unset unless measurement selects explicit bounds
scoring           = BM25
nested            = false
```

The exact chosen schema, field name and query-time BM25 `k`/`b` values belong to the retrieval profile. A semantic
change creates a fresh physical generation; old collections are never reinterpreted.

The Rust 1.20 API surface inspected at tag `v1.20.0` is concrete:

- `TextIndexParamsBuilder::new(TokenizerType::Code).lowercase(true).bm25_scoring()`;
- `CreateFieldIndexCollectionBuilder` for the payload index;
- `Query::new_text` / `TextQueryBuilder` for BM25 query text and optional `k`/`b`;
- `QueryPointsBuilder::using`, `filter`, `params`, `limit`;
- `SearchParamsBuilder::idf` for a selected statistics corpus;
- `UpsertPointsBuilder::update_mode(UpdateMode::InsertOnly)`;
- `PointStruct.vectors` is optional, so the lexical collection needs no dummy vector.

Recheck and compile the exact selected patch with the real bridge; source inspection is not compiler evidence.

## Retrieval text: no alias inflation

Preparation/unitization provides one bounded `retrieval_text` for each unit. It may include the actual unit text
and a finite structural heading/signature/path context **once** when that context is a selected product behavior.
It must not append snake/camel/acronym aliases generated from the same identifier.

Qdrant's `code` tokenizer already splits camelCase, snake_case and acronym transitions and also indexes the joined
identifier. Local alias expansion would duplicate terms, alter document length/term frequency and make ranking
profile-dependent for little benefit. Remove it from the baseline. If measured relevance later justifies another
text composition, define a new retrieval profile and rebuild; do not silently mutate existing text.

Exact spelling, punctuation and byte/location requests remain retained-source Exact. `C++`, `C#`, operators,
string literals and other punctuation-sensitive predicates are not made exact by the code tokenizer.

## Query semantics

A `TextQuery` scores a point when it contains any query term. Free text is not a hidden Boolean/phrase language.
Required or excluded terms are added only from an explicit typed request as Qdrant filter conditions. Do not infer
quotes, `-term`, `AND`, language or stemming rules from arbitrary user text.

For the first one-file smoke the physical collection equals the selected population, so issue one `TextQuery`
without a candidate filter or `idf.corpus`.

For later scoped requests distinguish:

```text
base population = selected source memberships intersected with real access/session restrictions
candidate filter = base population + explicit request-only path/kind/topical narrowing
idf.corpus       = base population
```

The base source/access filter appears in both candidate eligibility and IDF. Request-only narrowing does not
silently redefine statistics. If the base population is the whole physical collection, omit `idf` rather than
sending a redundant filter. Explicit empty returns empty without RPC; unknown/incomplete scope never widens.

Create keyword/payload indexes for every field used by base or candidate filters. Qdrant BM25 statistics are
physical-shard-local; the baseline uses one ordinary shard and no per-source custom-shard portfolio.

## Visibility and cost

Create the text and filter indexes before point ingestion. Build points with explicit `insert_only` and `wait`,
then verify collection/index schema, exact point payloads/count and at least one deterministic `TextQuery` canary
before publication may commit the matching view. Point readback alone does not prove the scoring index works.

Qdrant reports that full-text statistics can temporarily include stale data after updates/deletes until segment
optimization. Stage 2/3A avoids this class by building a new collection and never mutating it after visibility.
Stage 3B, if selected, must measure score drift/optimizer behavior rather than assuming immediate statistics.

Scoped IDF and text scoring can perform corpus-dependent work. Measure cold/warm narrow/broad requests, actual
server CPU/memory, cancellation and result quality. Top-k bounds output, not necessarily backend work.

## Remove with callers

After the direct path is connected, remove:

- local postings/BM25 engines and scoring-corpus builder;
- `Document(text, "qdrant/bm25", ...)` sparse document/query inference;
- fixed local `avg_len` and sparse-vector byte/profile goldens;
- frozen analyzer/qualification/acceptance receipts;
- alias-expansion/profile identities driven by tests or resource ceilings;
- duplicate numerical encoders and unused dependencies.

Keep only a small retrieval-text composer if it has real preparation/publication callers. Inline it if no package
boundary remains justified.

## Focused verification

After code and minimal Rust 1.98 check/Clippy, execute: vectorless collection creation; explicit code-tokenizer
schema; camel/snake/acronym matches without alias duplication; joined identifier match; OR term behavior; typed
required/excluded filters; whole-collection and selected-source IDF; explicit empty/unknown scope; punctuated Exact
separation; insert-only conflict/readback; deterministic query order; cancellation; and cold/warm cost. No such
runtime result is claimed by R051.

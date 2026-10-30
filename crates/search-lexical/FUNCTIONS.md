# Lexical retrieval — explicit Qdrant 1.20 physical and query profiles

This owner defines the concrete lexical meaning consumed by preparation, publication and query. It does not own
source identity, publication state, scope, evidence or Exact. R052 removes hidden Qdrant defaults and separates
physical index schema from query-time BM25 scoring.

## Physical text-index profile V1

The first product uses one vectorless physical collection and one payload field:

```text
retrieval_text
```

Build the text index with every behavior/resource field explicit:

```rust
TextIndexParamsBuilder::new(TokenizerType::Code)
    .lowercase(true)
    .ascii_folding(false)
    .phrase_matching(false)
    .disabled_stemmer()
    .enable_hnsw(false)
    .memory(Memory::Pinned) // Stage-2 one-file profile
    .bm25_scoring()
```

Leave `stopwords`, `min_token_len`, `max_token_len` and `nested` absent and verify they read back absent. Do not use
the deprecated `on_disk` field.

Why every field is explicit:

- text and keyword payload indexes default `enable_hnsw` to true; the vectorless lexical baseline needs no
  payload-aware vector HNSW links;
- index memory defaults to `Pinned`; Stage 2 selects that deliberately for one file instead of inheriting it;
- phrase matching would add a positional capability the baseline does not expose;
- stemming, stopwords, token-length clipping and ASCII folding can silently change identifier meaning;
- one-character identifiers and Unicode distinctions must not disappear through an implicit limit/normalization.

The physical profile includes field names, tokenizer/normalization/index parameters, text composition, point
payload/filter schema, ordinary shard count and index residency. Any physical-profile change creates a fresh
never-reused generation; an old collection is never reinterpreted.

### Stage 3A resource profile

For declared corpus classes, choose text/filter index memory explicitly from `Pinned | Cached | Cold` before
measurement. Do not let the server default choose. A different residency choice creates a new physical generation
under the selected profile. The release matrix records build/query latency, RAM/cache pressure and disk.

Stage 2 has one file and no candidate filter, so it creates only the text index. Stage 3A adds exact keyword/filter
indexes only for fields used by real source/path-kind filters. Those indexes also set
`enable_hnsw(false)` and explicit memory. Prefix optimization is added only for an actual query shape; no tenant/
access subsystem is selected.

## Retrieval text: no alias inflation

Each unit supplies one bounded `retrieval_text`: actual unit text plus finite selected structural context once.
Do not append snake/camel/acronym aliases.

Qdrant's 1.20 Code tokenizer:

- splits snake_case, SCREAMING_CASE, camelCase, PascalCase and acronym transitions;
- keeps letter+digit identifiers such as `utf8`, `sha256` and `x86` together where specified;
- indexes document subwords plus detached joined camel/snake identifier tokens;
- gives ranking queries both subwords and joined tokens when lowercase is enabled;
- gives full-text filter queries subwords only.

Therefore local aliases duplicate index terms, alter TF/document length and can degrade ranking. A measured
alternative text composition is a new physical retrieval profile and fresh generation.

Punctuation-sensitive identifiers/operators/strings remain retained-source Exact. Code tokenization does not make
`C++`, `C#`, `::`, quoted text or exact byte/location predicates exact.

## Query-scoring profile V1

BM25 `k` and `b` are request-time parameters, not index schema. The first query profile fixes:

```text
k = 1.2
b = 0.75
```

Validate client-side before dispatch:

```text
k is finite and k >= 0
b is finite and 0 <= b <= 1
```

Changing only `k`/`b` does **not** rebuild the collection. Capture the exact scoring profile in the admitted request
and any resulting live window/diagnostics. Do not expose tuning fields until a real consumer/measurement requires
them. No baseline `score_threshold`: use bounded top-k because absolute BM25 scores vary by corpus/profile.

## Ranking and filter semantics

`TextQuery` is any-token BM25 nomination. Free query text is not parsed as Boolean, phrase or negation syntax.

If the product later exposes typed required/excluded text clauses, Qdrant full-text filters use **all query
subwords in any order**. With Code tokenizer they do not use joined identifier tokens and are not exact identifier
or phrase predicates. Phrase filters require a separately selected positional profile; exact identifiers remain
Exact or an explicit exact keyword producer.

## Scope and IDF

The [source-selection owner](../search-query/search-access/FUNCTIONS.md#candidate-and-statistics-scope) defines the
local base population, candidate narrowing, explicit-empty behavior and IDF scope. There is no session-access or
tenant predicate to implement. An entire-collection base may omit the IDF override, not request-only filters.
All actual filter fields are payload-indexed. BM25 statistics remain shard-local; the baseline uses one ordinary
shard and no per-source custom-shard portfolio.

## Visibility and response economy

Create/verify the text index before point ingestion. Build with explicit `InsertOnly` and `wait`, then verify schema,
point payload/count and an actual deterministic `TextQuery` canary before publication commits the view.

Query requests explicitly return no vectors and only the routing payload required for evidence lookup (normally
`source_id`; point ID comes from the result). Do not return `retrieval_text` or duplicate excerpts over the Qdrant
response; the retained object supplies evidence.

Stage 2/3A visible collections are immutable. Qdrant notes that text statistics can remain stale after point
updates/deletes until segment optimization; Stage 3B must measure that behavior if activated.

## Remove with callers

Remove local postings/numerical BM25, sparse `qdrant/bm25` inference, fixed local `avg_len`, alias expansion,
frozen analyzer/qualification receipts, score-normalization profiles and duplicate encoders. Keep only a small
retrieval-text composer if real callers justify it; inline it otherwise.

## Focused verification

After code and minimal Rust 1.98 check/Clippy, verify exact schema readback including disabled stemmer/phrase/HNSW,
absent stopwords/length/nested, explicit memory, vectorless points, Code tokenizer subword/joined behavior,
all-token filter behavior, k/b validation, no score threshold, minimal payload/no vectors, whole/scoped IDF,
punctuation Exact separation, deterministic readiness query and cold/warm resource cost. R052 changes docs only.

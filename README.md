# ELIOT Search

ELIOT Search is a standalone local search application for large codebases and document collections. It returns
ranked source-backed locations with verified excerpts, exact retained revision identity and explicit gaps. Qdrant
is derived retrieval state; immutable objects remain the evidence source.

## Read first

| Need | Document |
|---|---|
| Product purpose and invariants | [Architecture](docs/architecture/README.md) |
| Shortest implementation path | [Implementation spine](docs/implementation/README.md) |
| Current checkpoint/evidence | [R051 registry](docs/audit/ANALYSIS_REGISTRY.md#resume) |
| One concrete cut | Nearest `FUNCTIONS.md`, actual callers and owning issue |

[Documentation authority](docs/README.md) explains which layer wins. Historical waves, receipts and old issue text
are evidence only.

## Current truth

The product is **not implemented end to end**. R051 verifies the exact Qdrant Rust 1.20 query/schema surface and
synchronizes lexical/query/preparation/control instructions; it does not claim compiler or runtime success.

```text
1 Foundation
  root/plain settings → handle-safe final-manifest-last root → DataRootGuard

2 Walking skeleton
  one file → ObjectRef → preparation/retrieval_text
  → vectorless Qdrant 1.20 collection
  → scoring Text index (Code tokenizer)
  → insert_only points + TextQuery canary
  → evidence/current commit + PublishedView
  → TextQuery → retained excerpt → CLI

3A correctness/reference
  restart + authoritative inventory + complete fresh generations
  + predeclared release measurements

3B only where budgets fail
  source-local diff + stable occurrence identity + point additions/closures
```

The current Cargo lock still contains `qdrant-client 1.19.0`; server/client/schema/query upgrade together in the
real #263/#264 code cut.

## Important boundaries

- Plain storage is baseline; #598 encryption is optional.
- Windows root/object finalization is owner-private and handle-bound; path-based `persist_noclobber` is not trusted
  as final authority.
- Baseline lexical collection is vectorless. Retrieval text is stored once in payload.
- Selected index profile: Code tokenizer, lowercase, no stemmer/stopwords/ascii folding, BM25 scoring.
- No local sparse BM25 inference, fixed `avg_len` or automatic snake/camel/acronym alias expansion.
- Free search text is any-term nomination; typed required/excluded clauses use filters; punctuation-sensitive
  predicates use Exact.
- Base source/access selection affects candidates and `idf.corpus`; request-only narrowing affects candidates only.
- `PublishedView` pins exact physical collection + redb snapshot. Stage 2/3A has no sequence.
- Stage 2/3A uses complete immutable collections and generation-bound point IDs; no occurrence matching.
- Baseline cleanup deletes whole retired collections only.
- No aliases, custom-shard portfolio, Edge fallback, query scheduler/tickets or second backend.
- No redb writer/view/window lock spans backend/object/process/model I/O.

## Workspace map

| Responsibility | Location |
|---|---|
| CLI/daemon composition | [bins/eliot-search](bins/eliot-search/), [bins/eliot-searchd](bins/eliot-searchd/) |
| Root/settings | [search-runtime-owner](crates/search-runtime/search-runtime-owner/), [search-config](crates/search-config/) |
| Source/objects | [crates/search-source](crates/search-source/) |
| Preparation/lexical text | [crates/search-prep](crates/search-prep/), [search-lexical](crates/search-lexical/) |
| Qdrant/publication | [crates/search-index-qdrant](crates/search-index-qdrant/) |
| Query/evidence | [crates/search-query](crates/search-query/) |
| Compact metadata | [search-control-redb](crates/search-control-redb/) |

This is inventory, not final crate topology. Move useful behavior with real callers and remove displaced routes and
exclusive fixtures.

## Verification status

Only R031 has current compiler evidence: locked Rust 1.98 check/minimal Clippy for materializer + unitizer. Current
binaries, Foundation, Windows finalizer, Qdrant 1.20, vectorless text scorer, walking skeleton, restart, relevance,
scale and end-to-end behavior remain unrun. See [evidence status](docs/audit/ANALYSIS_REGISTRY.md#evidence-status).

[QUICKSTART.md](QUICKSTART.md) documents only an unverified diagnostic one-shot route until #264 works.

## License

See [LICENSE](LICENSE).

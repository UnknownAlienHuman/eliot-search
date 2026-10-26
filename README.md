# ELIOT Search

ELIOT Search is a standalone local search application for large codebases and document collections. It returns
ranked source-backed locations with verified excerpts, exact retained revision identity and explicit gaps. Qdrant
is derived retrieval state; immutable objects remain the evidence source.

## Read first

| Need | Document |
|---|---|
| Product purpose and invariants | [Architecture](docs/architecture/README.md) |
| Shortest implementation path | [Implementation spine](docs/implementation/README.md) |
| Current checkpoint/evidence | [R052 registry](docs/audit/ANALYSIS_REGISTRY.md#resume) |
| One concrete cut | Nearest `FUNCTIONS.md`, actual callers and owning issue |

[Documentation authority](docs/README.md) explains which layer wins. Historical waves, receipts and old issue text
are evidence only.

## Current truth

The product is **not implemented end to end**. R052 removes hidden Qdrant schema/resource defaults and separates
physical index profile from query-time scoring; it does not claim compiler or runtime success.

```text
1 Foundation
  root/plain settings → handle-safe final-manifest-last root → DataRootGuard

2 Walking skeleton
  one file → ObjectRef → retrieval_text
  → vectorless Qdrant 1.20 collection
  → explicit scoring Text index V1
  → InsertOnly points + deterministic TextQuery canary
  → PublishedView
  → TextQuery(k=1.2,b=0.75) → retained excerpt → CLI

3A correctness/reference
  restart + inventory + explicit filter/residency profile
  + complete fresh generations + release budgets

3B only where budgets fail
  source-local diff + stable occurrence identity + point additions/closures
```

The Cargo lock still contains `qdrant-client 1.19.0`; server/client/schema/query upgrade together in #263/#264.

## Exact lexical baseline

Physical Text index V1:

```text
field=retrieval_text
Code tokenizer
lowercase=true
ascii_folding=false
phrase_matching=false
stemmer=Disabled
stopwords/min/max/nested absent
BM25 scoring
enable_hnsw=false
memory=Pinned for Stage 2
```

Stage 2 creates no source/access filter indexes because one file is the whole population. Stage 3A adds only real
filter indexes and explicitly selects `Pinned|Cached|Cold` residency per measured corpus profile.

Query scoring is separate: baseline k=1.2, b=0.75, finite/range validated, no score threshold. Changing only k/b
does not rebuild the collection.

Code tokenizer already emits subwords and joined camel/snake tokens for ranking; do not append local aliases. Free
text is any-token nomination. Typed full-text filters, if exposed, require all subwords in any order and are not
exact identifiers/phrases. Punctuation-sensitive predicates use retained-source Exact.

Qdrant responses contain no vectors and only routing payload required for pinned evidence lookup; excerpts always
come from immutable objects.

## Other boundaries

- Plain storage baseline; #598 encryption optional.
- Windows root/object finalization handle-bound; path-based `persist_noclobber` not trusted final authority.
- `PublishedView` pins exact collection + physical profile + redb snapshot; Stage 2/3A has no sequence.
- Complete immutable collections and generation-bound point IDs before point optimization.
- Whole-collection cleanup only in baseline.
- No aliases, custom-shard portfolio, Edge fallback, query scheduler/tickets or second backend.
- No redb writer/view/window lock across backend/object/process/model I/O.

## Verification status

Only R031 has current compiler evidence: locked Rust 1.98 check/minimal Clippy for materializer + unitizer. Current
binaries, Foundation, Windows finalizer, Qdrant 1.20, explicit text-index profile, walking skeleton, restart,
relevance, scale and end-to-end behavior remain unrun. See [registry](docs/audit/ANALYSIS_REGISTRY.md#evidence-status).

[QUICKSTART.md](QUICKSTART.md) remains an unverified diagnostic route until #264 works.

## License

See [LICENSE](LICENSE).

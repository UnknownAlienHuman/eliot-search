# Donor and analogue register — installed state, candidates and selected patterns

This live index records exact installed dependencies, implementation-time candidates and upstream behavior which
materially constrains a concrete ELIOT owner. It does not approve dependencies, reserve pins or turn another system
into architecture.

**Current checkpoint:** R052 over inspected ELIOT head `87c84c24`, 2026-10-26.  
**Previous detailed register:** [`da382b66`](https://github.com/UnknownAlienHuman/eliot-search/blob/da382b66e59d0b44e758bad9b22e2f5703a4e8c2/docs/audit/DONOR_VERIFICATION_REGISTER_2026-10-09.md).

Keep four states separate:

1. upstream behavior inspected;
2. ELIOT use selected;
3. exact dependency/service installed and connected;
4. behavior executed through the product.

Recheck exact patch, features, MSRV, advisories, native artifacts and API with the real consumer.

<a id="installed-selected-and-optional"></a>
## Installed dependencies / current service state

| Donor | Current state | Boundary |
|---|---|---|
| `qdrant-client` / Qdrant | repository lock/historical server evidence still 1.19; R049–R052 select one exact 1.20.x patch for implementation | One private derived-index adapter; default product unconnected. |
| `redb` | locked 2.6.3 | Compact metadata and real read snapshots; no file/Qdrant effects inside transactions. |
| Tokio | locked 1.53.1 in async consumers | I/O/deadline/process primitives; dropped future does not prove external termination. |
| `pulldown-cmark` | exact 0.13.5, defaults disabled | Markdown `OffsetIter`; version/checksum are transform identity. |
| `quick-xml` | locked 0.42.0 | Bounded XML/JATS parsing, not a document platform. |
| `toml_parser` | locked 1.1.4+spec-1.1.0 | Legacy decoder, not selected typed replacement. |
| `aho-corasick` | locked 1.1.5 | Literal Exact primitive, not connected request owner. |
| Unicode/glob helpers | `unicode-ident 1.0.26`, `unicode-normalization 0.1.25`, `globset 0.4.20` | Narrow helpers; no compiler identity/inventory authority. |

R031 Actions run 40084893130 passed locked Rust 1.98 check/minimal Clippy for materializer + unitizer only.

## Implementation-time candidates not installed

| Candidate | Intended use | Adoption rule |
|---|---|---|
| high-level `toml` + Serde; `serde_json` | Typed settings/framed JSON | Select with actual consumers; preserve sparse absence/native paths/redacted diagnostics. |
| owner-private finalizer; optional `tempfile` random staging | Same-parent root/object publication | Windows path-based `persist_noclobber` is not trusted final authority. |
| `ignore::WalkBuilder` | Authoritative bounded source inventory | Select with #271; inspect builder/traversal/entry errors. |
| `notify::RecommendedWatcher` | Optional dirty hints | Add only after inventory works without it. |
| `regex-automata` | Optional Exact regex | Select only with concrete #248 consumer. |
| PDFium / `pdfium-render` | Optional PDF native-text profile | Pin crate and packaged native artifact together. |
| `rmcp` | Optional stdio MCP leaf | Absent from lock; recheck post-baseline. |
| tree-sitter / SCIP | Optional outline/semantic producer | Add only for a real orientation consumer. |
| model runtime | Optional rerank/embedding | Require measured lexical-baseline benefit and exact artifact/profile. |

<a id="qdrant-120-direct-bm25"></a>
## Qdrant 1.20 direct BM25 — exact inspected surface

Official Rust client tag `v1.20.0` resolves to tag object `3e792085...` and commit
`3b5a75aa371169486a9af8fc2748a76b379f0d02`, released 2026-10-22. R051/R052 inspected generated proto/builders,
conversions, official snippet tests and matching Qdrant server v1.20 source.

Concrete Rust surface:

```rust
CreateCollectionBuilder::new(name) // vectors_config optional
TextIndexParamsBuilder::new(TokenizerType::Code)
CreateFieldIndexCollectionBuilder
KeywordIndexParamsBuilder
TextQueryBuilder::new(text).k(k).b(b)
QueryPointsBuilder::{using,filter,params,limit,with_vectors,with_payload}
SearchParamsBuilder::idf
UpsertPointsBuilder::update_mode(UpdateMode::InsertOnly)
```

`PointStruct.vectors` is optional, so the baseline collection is vectorless.

### Physical text-index profile V1

Exact Stage-2 builder shape:

```rust
TextIndexParamsBuilder::new(TokenizerType::Code)
    .lowercase(true)
    .ascii_folding(false)
    .phrase_matching(false)
    .disabled_stemmer()
    .enable_hnsw(false)
    .memory(Memory::Pinned)
    .bm25_scoring()
```

Keep `stopwords`, min/max token length and nested path absent and verify absence on readback. Do not use deprecated
`on_disk`.

Important upstream defaults which must not remain hidden:

- payload text/keyword indexes default `enable_hnsw=true`;
- index memory defaults to `Pinned`;
- phrase matching defaults false;
- stemming defaults disabled;
- scoring defaults disabled;
- lowercase defaults true and ASCII folding false.

ELIOT sets every selected field explicitly. Stage 2 deliberately uses Pinned for one file. Stage 3A explicitly
selects `Pinned | Cached | Cold` per corpus/release profile and measures RAM/cache/disk. Exact keyword/filter indexes
also set `enable_hnsw(false)` and explicit memory. Prefix/tenant optimization appears only for a real request shape.

A physical profile change creates a fresh generation. Query-time k/b does not.

### Query-scoring profile V1

```text
k=1.2
b=0.75
```

The builder itself stores floats without client validation; Qdrant validates `k >= 0` and `0 <= b <= 1`. ELIOT
also rejects non-finite/out-of-range values before dispatch. There is no baseline score threshold. A future k/b
change is captured in the request/window profile and does not rebuild the physical index.

### Code tokenizer semantics

Official server source confirms:

- snake/camel/Pascal/acronym splitting;
- documented digit+letter behavior;
- documents index subwords plus detached joined camel/snake identifier tokens;
- ranking queries receive subwords plus joined identifiers when lowercase=true;
- full-text filter queries receive subwords only.

Therefore baseline retrieval text contains source/context once and no local snake/camel/acronym aliases. Duplicates
would change term frequency/document length.

`TextQuery` scores documents containing any query term. Full-text `MatchText` filters use all query subwords in any
order. With phrase matching disabled, such filters are neither phrase nor exact identifier predicates. Exact
punctuation/location remains retained-source Exact or a separately selected exact keyword producer.

### Scope and IDF

```text
base population = selected sources ∩ actual access/session restriction
candidate filter = base population + request-only narrowing
idf.corpus       = base population
```

If base is whole collection, omit `idf`. Explicit empty avoids RPC; unknown/incomplete never widens. All real filter
fields require payload indexes. BM25 statistics are physical-shard-local, so baseline uses one ordinary shard and no
per-source custom-shard portfolio.

### Visibility/readiness and response economy

Create/verify text/filter indexes before ingestion. Build with explicit InsertOnly + wait, then verify exact schema,
point/payload/count and execute a deterministic TextQuery canary. Readback alone does not establish scorer wiring.

Query explicitly requests no vectors and only evidence-routing payload, e.g.
`with_vectors(false)` plus `with_payload(vec!["source_id"])`. It does not return retrieval text or excerpts.

Qdrant text statistics can remain stale after point updates/deletes until optimization. Stage 2/3A avoids
post-visibility mutation. Stage 3B must measure score/optimizer convergence.

Primary sources:

- [Rust client v1.20.0](https://github.com/qdrant/rust-client/releases/tag/v1.20.0)
- [Text index builder](https://github.com/qdrant/rust-client/blob/v1.20.0/src/builders/text_index_params_builder.rs)
- [Keyword index builder](https://github.com/qdrant/rust-client/blob/v1.20.0/src/builders/keyword_index_params_builder.rs)
- [Text query builder](https://github.com/qdrant/rust-client/blob/v1.20.0/src/builders/text_query_builder.rs)
- [QueryPoints builder](https://github.com/qdrant/rust-client/blob/v1.20.0/src/builders/query_points_builder.rs)
- [Payload/vector selector conversions](https://github.com/qdrant/rust-client/blob/v1.20.0/src/grpc_conversions/primitives.rs)
- [Code tokenizer](https://github.com/qdrant/qdrant/blob/v1.20.0/lib/segment/src/index/field_index/full_text_index/tokenizers/code.rs)
- [Text scoring/filter semantics](https://github.com/qdrant/qdrant/blob/v1.20.0/lib/segment/src/index/field_index/full_text_index/full_text_index_read.rs)
- [Server text-index defaults](https://github.com/qdrant/qdrant/blob/v1.20.0/lib/segment/src/data_types/index.rs)
- [BM25 validation](https://github.com/qdrant/qdrant/blob/v1.20.0/lib/api/src/rest/schema.rs)

<a id="plain-object-publication-donor"></a>
## Plain root/object publication — platform correction

Git's useful pattern remains same-parent temporary write, streaming identity, file sync, no-clobber finalization and
exact existing/final verification.

`tempfile 3.27.0` is suitable for random staging but Windows `persist_noclobber` calls path-based
`SetFileAttributesW`/`MoveFileExW`; it is not bound to the opened stage and retained parent. The trusted Windows
boundary is one owner-private handle-based rename with replacement disabled, reopened/read back through retained
handles. Run native replacement/crash/durability canaries. Unix uses private directory-relative no-replace or
verified link/unlink fallback.

<a id="analog-systems"></a>
## Analogues — patterns only

| System | Use | Reject |
|---|---|---|
| Git loose objects | Same-parent content-addressed finalize | No source-membership inference or unchecked existing final. |
| Tantivy | Commit/visibility and GC separation | No fallback/backend trait or second scoped-statistics owner. |
| Zoekt | Immutable temporary build/finalize | No second exact index or mandatory delta/tombstone stack. |
| Watchman / `ignore` | Events are observations; uncertainty recrawls | No service/cursor/event ledger; inventory is correctness. |
| qntx/qmd | Release DB mutex around long work | No SQLite replacement/default model runtime. |
| Veles | Simple manifest | Reject manifest-first, missing artifact as empty, size+mtime authority and old-seen deletion. |
| codesearch | Optional product catalogue | Reject all-at-once grammars/models/two indexes/watcher/HTTP/TUI/MCP closure. |
| Qdrant Edge | Possible later embedded replacement | Beta/recent WAL concerns; never fallback/dual backend. |

<a id="qdrant-update-mode-and-generation-cleanup"></a>
## Qdrant update and cleanup

Stage 2/3A uses new immutable collections, InsertOnly point creation, exact readiness, explicit view install and
whole-collection retirement. No per-point delete, closure, sequence or stable cross-generation ID. Stage 3B adds
point mutation only where failed budgets require it; default upsert remains forbidden.

Whole-generation cleanup waits for no current/pending/future target, live view/window or unresolved attempt, then
performs terminal deletion and exact logical-absence observation. Never reuse name; uncertainty retains cleanup
state/backpressure.

<a id="native-bm25-and-separate-statistics-scope"></a>
## Native BM25 and statistics scope

Direct scoring full-text BM25 replaces sparse document/query inference and local avg_len. `idf.corpus` is the base
source/access population, independent from request-only narrowing. Top-k bounds output, not backend work. Measure
cold/warm narrow/broad cost.

<a id="markdown-offset-iterator-released-fix-not-yet-adopted"></a>
## Markdown OffsetIter correction

R031 moved `pulldown-cmark` 0.13.1 → 0.13.5 with exact lock and transform identity. Connected parser/materialize/
replay evidence remains unrun under #215/#331.

<a id="typed-configuration-donor-boundaries"></a>
## Typed configuration limits

The current lock lacks high-level `toml`, Serde and `serde_json`. Preserve sparse absence, closed structs, native OS
paths, bounded input/redacted diagnostics and reject invalid effective settings before effects. Baseline physical
text profile and k/b are closed code profiles, not a large user-configurable schema. Add tuning fields only with a
real measured consumer.

<a id="a-chunk-api-is-not-an-incremental-reader"></a>
## Exact matcher seam

`search-exact::literal::scan_chunks` preserves overlapping matches over supplied chunks but is not the connected
retained reader and lacks cancellation inside its byte loop. Compile once per request, reset at source boundaries
and feed bounded retained blocks. Raw bytes, decoded text and derived representations remain distinct.

## Currentness rule

Caller searches, package sizes and analogue behavior are revision-specific. Before a code cut, recheck only exact
callers/API involved, then record inspected SHA, selected use, installed state and unrun work in the registry.

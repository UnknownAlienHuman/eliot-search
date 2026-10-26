# Donor and analogue register — installed state, candidates and selected patterns

This live index records exact installed dependencies, implementation-time candidates and upstream behavior which
materially constrains a concrete ELIOT owner. It does not approve a dependency, reserve a future pin or turn an
analogue into architecture.

**Current checkpoint:** R051 over inspected ELIOT head `a4c23110`, 2026-10-26.  
**Previous detailed register:** [`da382b66`](https://github.com/UnknownAlienHuman/eliot-search/blob/da382b66e59d0b44e758bad9b22e2f5703a4e8c2/docs/audit/DONOR_VERIFICATION_REGISTER_2026-10-09.md).

Keep four states separate:

1. upstream behavior inspected;
2. ELIOT use selected;
3. exact dependency/service installed and connected;
4. behavior executed through the product.

Recheck exact patch, features, MSRV, advisories, native artifacts and API with the real consumer.

## Installed dependencies / current service state

| Donor | Current state | Boundary |
|---|---|---|
| `qdrant-client` / Qdrant | repository lock/server evidence still 1.19; R049–R051 select one exact 1.20.x patch for implementation | One private derived-index adapter. No connected default product yet. |
| `redb` | locked 2.6.3 | Compact current/published metadata and real read snapshots. No files/Qdrant inside transactions; old readers retain pages. |
| Tokio | locked 1.53.1 in async consumers | Process/I/O/deadline primitives. Dropping a future does not prove external work stopped. |
| `pulldown-cmark` | exact 0.13.5, defaults disabled | Markdown `OffsetIter`; version/checksum are transform identity. |
| `quick-xml` | locked 0.42.0 | Bounded XML/JATS parsing, not a document platform. |
| `toml_parser` | locked 1.1.4+spec-1.1.0 | Legacy decoder, not the selected typed replacement. |
| `aho-corasick` | locked 1.1.5 | Literal Exact primitive, not the connected request owner. |
| Unicode/glob helpers | `unicode-ident 1.0.26`, `unicode-normalization 0.1.25`, `globset 0.4.20` | Narrow helpers; no compiler identity or inventory authority. |

R031 Actions run 40084893130 passed locked Rust 1.98 check/minimal Clippy for materializer + unitizer only.

## Implementation-time candidates not installed

| Candidate | Intended use | Adoption rule |
|---|---|---|
| high-level `toml` + Serde; `serde_json` | Typed settings and framed JSON | Select with actual Foundation/protocol consumers; preserve sparse absence/native paths/redacted diagnostics. |
| owner-private Windows/Unix finalizer; optionally `tempfile` for random staging | Same-parent staging and no-clobber root/object publication | Windows path-based `persist_noclobber` is not trusted final authority. Pin helper dependencies with #330/#360 code and canaries. |
| `ignore::WalkBuilder` | Authoritative bounded source inventory | Select with #271 after walking skeleton; inspect builder/traversal/entry errors. |
| `notify::RecommendedWatcher` | Optional dirty hints | No correctness authority; add only after inventory works without it. |
| `regex-automata` | Optional Exact regex | Select only with concrete #248 consumer. |
| PDFium / `pdfium-render` | Optional PDF native-text profile | Pin crate and matching packaged native artifact together. |
| `rmcp` | Optional stdio MCP leaf | Absent from lock; R039 inspected stable 3.7.0. Recheck post-baseline. |
| tree-sitter / SCIP | Optional outline/semantic producers | Add one producer only for a real orientation consumer. |
| model runtime | Optional rerank/embedding | Require measured lexical benefit and one exact artifact/profile. |

## Qdrant 1.20 direct BM25 — exact selected API

Official Rust client tag `v1.20.0` resolves to verified tag object `3e792085...` and commit
`3b5a75aa371169486a9af8fc2748a76b379f0d02`, dated 2026-10-22. R051 inspected the generated proto/builders and
snippet tests, not only prose docs.

Concrete Rust surface:

```rust
CreateCollectionBuilder::new(name)              // vectors_config optional
TextIndexParamsBuilder::new(TokenizerType::Code)
    .lowercase(true)
    .bm25_scoring()
CreateFieldIndexCollectionBuilder::new(collection, field, FieldType::Text)
TextQueryBuilder::new(text).k(k).b(b)
QueryPointsBuilder::new(collection)
    .query(Query::new_text(...))
    .using(field)
    .filter(...)
    .params(SearchParamsBuilder::default().idf(IdfParams { corpus: ... }))
    .limit(limit)
UpsertPointsBuilder::new(collection, points)
    .wait(true)
    .update_mode(UpdateMode::InsertOnly)
```

`PointStruct.vectors` and collection `vectors_config` are optional, so the baseline lexical collection is
vectorless; no dummy vector/sparse BM25 inference is needed.

Selected text index/profile:

```text
field=retrieval_text
tokenizer=Code
lowercase=true
ascii_folding=false
stemmer=none
stopwords=none
scoring=BM25
nested=false
k/b explicit at query time
```

`TextQuery` matches any query term. Required/excluded terms are explicit filter clauses; arbitrary free text is not
implicitly parsed as Boolean/phrase syntax. Exact punctuation/location remains retained-source Exact.

The code tokenizer splits camelCase/snake_case/acronym boundaries and also indexes the joined identifier. Therefore
ELIOT does not append local snake/camel/acronym aliases in the baseline: duplicated terms would change term
frequency/document length and complicate profile identity. A measured alternative text composition requires a new
retrieval profile/fresh generation.

### Scope and IDF

For one text query:

```text
base population = selected sources ∩ real access/session restriction
candidate filter = base population + request-only narrowing
idf.corpus       = base population
```

If base population equals the whole physical collection, omit `idf`; global statistics are exactly correct.
Explicit empty returns empty without RPC. Unknown/incomplete never widens. Source/access/path-kind fields used by
filters require payload indexes.

BM25 statistics are physical-shard-local. No generic per-source/root custom-shard portfolio: cross-shard scores use
different populations and require another fusion owner. Baseline uses one ordinary shard.

### Visibility/readiness

Create scoring/filter indexes before point ingestion. Build with `insert_only` + `wait`, then verify schema,
point payload/count **and an actual deterministic TextQuery canary**. Readback alone does not establish that the
selected field/scorer/index is usable.

Qdrant documents that text statistics may temporarily include stale updated/deleted data until segment
optimization. Stage 2/3A avoids post-visibility point updates/deletes by using immutable fresh collections. Stage
3B must measure scoring/optimizer behavior if activated.

Primary sources:

- [Rust client v1.20.0 tag](https://github.com/qdrant/rust-client/releases/tag/v1.20.0)
- [Text index builder](https://github.com/qdrant/rust-client/blob/v1.20.0/src/builders/text_index_params_builder.rs)
- [Text query builder](https://github.com/qdrant/rust-client/blob/v1.20.0/src/builders/text_query_builder.rs)
- [QueryPoints builder](https://github.com/qdrant/rust-client/blob/v1.20.0/src/builders/query_points_builder.rs)
- [SearchParams IDF builder](https://github.com/qdrant/rust-client/blob/v1.20.0/src/builders/search_params_builder.rs)
- [Insert-only builder](https://github.com/qdrant/rust-client/blob/v1.20.0/src/builders/upsert_points_builder.rs)
- [BM25 text-index snippet](https://github.com/qdrant/rust-client/blob/v1.20.0/tests/snippet_tests/test_create_text_index_with_bm25.rs)
- [TextQuery snippet](https://github.com/qdrant/rust-client/blob/v1.20.0/tests/snippet_tests/test_query_points_with_text.rs)

## Plain root/object publication — platform correction

Git's useful pattern remains same-parent temporary write, streaming identity, file sync, no-clobber finalization
and exact existing/final verification.

`tempfile 3.27.0` is suitable for random staging and has MSRV 1.63. Its Windows `persist_noclobber`, however,
changes attributes and calls path-based `MoveFileExW`; it is not tied to the already-open stage handle and retained
destination-parent handle. It is not the trusted Windows finalizer.

Windows selected boundary:

```text
opened stage handle + retained verified parent handle + relative final leaf
→ owner-private SetFileInformationByHandle rename, replacement disabled
→ reopen through retained parent/root
→ exact final verification
```

Run a native canary for exact info class/struct, access/share flags, destination-exists refusal, path/parent
replacement, crash debris and final readback. Refuse unsupported profiles rather than silently falling back.

Unix uses a private directory-relative no-replace primitive; hard-link + unlink fallback requires exact readback and
visible stage debris handling. File-data flush, namespace atomicity, readback and sudden-power-loss durability are
distinct claims.

## Analogues — patterns only

| System | Use | Reject |
|---|---|---|
| Git loose objects | Same-parent content-addressed write/finalize | No source-membership inference or unchecked existing final. |
| Tantivy | Commit and reader visibility are separate; GC separate | No fallback/backend trait or ELIOT-owned scoped-statistics subsystem. |
| Zoekt | Immutable temporary build/finalize; state classification | No second exact index or mandatory delta-shard/tombstone stack. |
| Watchman / `ignore` | Events are observations; uncertainty recrawls | No service/cursor/event ledger; inventory is correctness. |
| qntx/qmd | Release DB mutex around long inference work | No SQLite replacement/default model runtime. |
| Veles | Simple manifest easy to inspect | Reject manifest-first, missing artifact as empty, size+mtime authority and old-seen deletion. |
| codesearch | Useful optional product catalogue | Reject all-at-once grammars/models/two indexes/watcher/HTTP/TUI/MCP closure. |
| Qdrant Edge | Promising embedded replacement candidate | Beta/recent WAL concerns; no fallback/dual backend. Test newer release only after server path works. |

## Markdown correction

R031 moved `pulldown-cmark` 0.13.1 → 0.13.5 with exact lock and transform identity. Connected parser/materialize/
replay evidence remains unrun under #215/#331.

## Typed configuration limits

The current lock lacks high-level `toml`, Serde and `serde_json`. Preserve sparse absence, closed structs, native OS
paths where needed, bounded input/redacted diagnostics and reject invalid effective settings before effects. Add
fields only with actual consumers.

## Exact matcher seam

`search-exact::literal::scan_chunks` preserves overlapping matches over supplied UTF-8 chunks, but is not the
connected retained reader and lacks cancellation inside its byte loop. Compile once per request, reset at source
boundaries and feed bounded retained blocks. Raw bytes, decoded text and derived representations remain distinct.

## Currentness rule

Caller searches, package sizes and analogue behavior are revision-specific. Before a code cut, recheck only the
exact current callers and donor API involved, then record inspected SHA, selected use, installed state and unrun
work in the analysis registry.

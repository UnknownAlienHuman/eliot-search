# Donor and analogue register — installed state, candidates and selected patterns

This live index records exact installed dependencies, implementation-time candidates and upstream behavior which
materially constrains a concrete ELIOT owner. It does not approve dependencies, reserve pins or turn another system
into architecture.

**Current checkpoint:** R054 over inspected ELIOT head `9910a15b`, 2026-10-26.  
**Previous detailed register:**
[`da382b66`](https://github.com/UnknownAlienHuman/eliot-search/blob/da382b66e59d0b44e758bad9b22e2f5703a4e8c2/docs/audit/DONOR_VERIFICATION_REGISTER_2026-10-09.md).

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
| `qdrant-client` / Qdrant | repository lock/historical server evidence still 1.19; R049–R054 select one exact 1.20.x patch for implementation | One private derived-index adapter; default product unconnected. |
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
| documented Win32 absolute-target finalizer | Same-parent root/object publication | Must pass R054 canary; no private NT, relative-root-wrapper or path-only fallback. |
| optional random-stage helper | Stage creation only | A helper such as `tempfile` never becomes final publication authority. |
| `ignore::WalkBuilder` | Authoritative bounded source inventory | Select with #271; inspect builder/traversal/entry errors. |
| `notify::RecommendedWatcher` | Optional dirty hints | Add only after inventory works without it. |
| `regex-automata` | Optional Exact regex | Select only with concrete #248 consumer. |
| PDFium / `pdfium-render` | Optional PDF native-text profile | Pin crate and packaged native artifact together. |
| `rmcp` | Optional stdio MCP leaf | Absent from lock; recheck post-baseline. |
| tree-sitter / SCIP | Optional outline/semantic producer | Add only for a real orientation consumer. |
| model runtime | Optional rerank/embedding | Require measured lexical-baseline benefit and exact artifact/profile. |

<a id="qdrant-120-direct-bm25"></a>
## Qdrant 1.20 direct BM25 — exact inspected surface

Official Rust client `v1.20.0` and Qdrant Server 1.20 expose the required vectorless scoring text path:

```rust
CreateCollectionBuilder::new(name)
TextIndexParamsBuilder::new(TokenizerType::Code)
CreateFieldIndexCollectionBuilder
TextQueryBuilder::new(text).k(k).b(b)
QueryPointsBuilder::{using,filter,params,limit,with_vectors,with_payload}
SearchParamsBuilder::idf
UpsertPointsBuilder::update_mode(UpdateMode::InsertOnly)
```

### Physical text-index profile V1

```text
retrieval_text Text index
Code tokenizer; lowercase=true
ascii_folding=false; phrase_matching=false; stemmer disabled
stopwords/min/max/nested absent
BM25; enable_hnsw=false; memory=Pinned
```

Read back every physical field. Stage 3A selects `Pinned|Cached|Cold` per measured corpus profile and creates only
filter indexes used by real scope clauses.

### Query-scoring profile V1

```text
k=1.2
b=0.75
no score threshold
```

ELIOT rejects non-finite/out-of-range values before dispatch. A k/b change is request/window meaning and does not
rebuild the physical collection.

Code tokenizer ranking receives subwords plus joined camel/snake identifiers. Full-text filters receive subwords
only. Therefore baseline retrieval text contains source/context once and no local identifier aliases. Exact
punctuation/location remains retained-source Exact.

```text
base population = selected sources ∩ actual access/session restriction
candidate filter = base population + request-only narrowing
idf.corpus       = base population
```

If base is whole collection, omit `idf`. Statistics are physical-shard-local, so baseline uses one ordinary shard
and no per-source custom-shard portfolio.

Create/verify indexes before ingestion. Build with InsertOnly + wait, verify exact schema/point/payload/count and
execute a deterministic TextQuery canary. Query requests no vectors and only evidence-routing payload.

Primary sources:

- [Rust client v1.20.0](https://github.com/qdrant/rust-client/releases/tag/v1.20.0)
- [Text index builder](https://github.com/qdrant/rust-client/blob/v1.20.0/src/builders/text_index_params_builder.rs)
- [Text query builder](https://github.com/qdrant/rust-client/blob/v1.20.0/src/builders/text_query_builder.rs)
- [Code tokenizer](https://github.com/qdrant/qdrant/blob/v1.20.0/lib/segment/src/index/field_index/full_text_index/tokenizers/code.rs)
- [Server text-index defaults](https://github.com/qdrant/qdrant/blob/v1.20.0/lib/segment/src/data_types/index.rs)

<a id="plain-object-publication-donor"></a>
## Plain root/object publication — R054 Windows correction

Git's useful high-level pattern remains same-parent stage, streaming identity, file-data flush, no-clobber
finalization and exact existing/final verification.

### Rejected Windows paths

- `tempfile::persist_noclobber` uses path-based `SetFileAttributesW` + `MoveFileExW`.
- Legacy `registration_native::rename` passes relative leaf + non-null parent through the Win32 wrapper and is
  reachable from source-root commands.
- R053 then selected a private direct `NtSetInformationFile`-style user-mode call. That is not a documented Win32
  application contract and is removed from the baseline.

Official `FILE_RENAME_INFO` documentation says a relative `RootDirectory` may be supplied, but a current supported-
build report shows the wrapper may convert the destination to an absolute path while leaving `RootDirectory`
non-null, producing `ERROR_INVALID_PARAMETER`. Documentation alone therefore does not qualify that relative form.

### Selected documented Windows candidate

```text
retain verified handles for every directory from data root to final parent
  with share read/write and without FILE_SHARE_DELETE
retain opened stage handle with DELETE access
derive canonical absolute final path from retained final-parent handle
→ SetFileInformationByHandle(FileRenameInfo)
   RootDirectory=NULL
   ReplaceIfExists=FALSE
→ reopen final relative to retained parent
→ verify stable identity and exact bytes
```

Omitting delete sharing prevents rename/delete replacement of every retained component while the absolute path is
used. The canary must prove accepted path form, access/share flags, destination-present refusal, ancestor/parent
replacement resistance, same-volume/local-filesystem support, failure debris and exact readback. Unsupported
profiles refuse.

Do not use `MoveFileExW`, relative-root wrapper, private `NtSetInformationFile`, undocumented ntdll FFI or a generic
filesystem layer. The old source-registration publication phases are removed with callers.

Primary references:

- [Microsoft FILE_RENAME_INFO](https://learn.microsoft.com/windows/win32/api/winbase/ns-winbase-file_rename_info)
- [Microsoft SetFileInformationByHandle](https://learn.microsoft.com/windows/win32/api/fileapi/nf-fileapi-setfileinformationbyhandle)
- [Microsoft CreateFile share modes](https://learn.microsoft.com/windows/win32/api/fileapi/nf-fileapi-createfilew)
- current Microsoft Q&A reproduction of relative `RootDirectory` wrapper failure
- current ELIOT `registration_native.rs` at inspected branch head

### Unix candidate

Use a private directory-relative no-replace rename when available. A same-filesystem hard-link + unlink fallback is
allowed with exact final verification and visible stage debris. `std::fs::rename` may overwrite and is not selected.

File-data flush, namespace publication, exact readback and sudden-power-loss durability are separate claims.

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
performs terminal deletion and exact logical-absence observation. Never reuse a name; uncertainty retains cleanup
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
text profile and k/b are closed code profiles, not a large user-configurable schema.

<a id="a-chunk-api-is-not-an-incremental-reader"></a>
## Exact matcher seam

`search-exact::literal::scan_chunks` preserves overlapping matches over supplied chunks but is not the connected
retained reader and lacks cancellation inside its byte loop. Compile once per request, reset at source boundaries
and feed bounded retained blocks. Raw bytes, decoded text and derived representations remain distinct.

## Currentness rule

Caller searches, package sizes and analogue behavior are revision-specific. Before a code cut, recheck only exact
callers/API involved, then record inspected SHA, selected use, installed state and unrun work in the registry.

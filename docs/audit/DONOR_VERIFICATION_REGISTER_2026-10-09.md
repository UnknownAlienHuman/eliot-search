# Donor and analogue register — installed state, candidates and selected patterns

This live index records exact installed dependencies, implementation-time candidates and upstream behavior which
materially constrains a concrete ELIOT owner. It does not approve dependencies, reserve pins or turn another system
into architecture.

**Current checkpoint:** R053 over inspected ELIOT head `8942d823`, 2026-10-26.  
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
| `qdrant-client` / Qdrant | repository lock/historical server evidence still 1.19; R049–R053 select one exact 1.20.x patch for implementation | One private derived-index adapter; default product unconnected. |
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
| direct owner-private Windows native finalizer | Same-parent root/object publication | Must pass R053 canary; no Win32 wrapper/path fallback. |
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
CreateCollectionBuilder::new(name) // vectors optional
TextIndexParamsBuilder::new(TokenizerType::Code)
CreateFieldIndexCollectionBuilder
TextQueryBuilder::new(text).k(k).b(b)
QueryPointsBuilder::{using,filter,params,limit,with_vectors,with_payload}
SearchParamsBuilder::idf
UpsertPointsBuilder::update_mode(UpdateMode::InsertOnly)
```

### Physical text-index profile V1

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

Keep stopwords, min/max token length and nested path absent and verify that exact absence on readback. Do not use
deprecated `on_disk`.

Upstream defaults which must not remain hidden:

- payload text/keyword indexes default `enable_hnsw=true`;
- index memory defaults to `Pinned`;
- phrase matching defaults false;
- stemming defaults disabled;
- scoring defaults disabled;
- lowercase defaults true and ASCII folding false.

Stage 2 explicitly uses Pinned for one file. Stage 3A selects `Pinned|Cached|Cold` per corpus/release profile and
measures RAM/cache/disk. Real keyword/filter indexes also disable HNSW and set memory explicitly.

### Query-scoring profile V1

```text
k=1.2
b=0.75
no score threshold
```

ELIOT rejects non-finite/out-of-range values before dispatch. A k/b change is request/window meaning and does not
rebuild the physical collection.

### Code tokenizer semantics

Official server source confirms snake/camel/Pascal/acronym splitting. Documents and ranking queries receive
subwords plus detached joined camel/snake identifiers. Full-text filter queries receive subwords only.

Therefore baseline retrieval text contains source/context once and no local identifier aliases. `TextQuery` scores
any matching term. Full-text `MatchText` filters require all query subwords in any order; with phrase matching off,
they are neither phrase nor exact-identifier predicates. Exact punctuation/location remains retained-source Exact.

### Scope, IDF and sharding

```text
base population = selected sources ∩ actual access/session restriction
candidate filter = base population + request-only narrowing
idf.corpus       = base population
```

If base is whole collection, omit `idf`. Explicit empty avoids RPC; unknown/incomplete never widens. Statistics are
physical-shard-local, so baseline uses one ordinary shard and no per-source custom-shard portfolio.

### Visibility/readiness and response economy

Create/verify indexes before ingestion. Build with explicit InsertOnly + wait, verify exact schema/point/payload/
count and execute a deterministic TextQuery canary. Query requests no vectors and only evidence-routing payload,
not retrieval text or excerpts.

Text statistics can remain stale after point mutation until optimization. Stage 2/3A avoids post-visibility
mutation. Stage 3B must measure score/optimizer convergence.

Primary sources:

- [Rust client v1.20.0](https://github.com/qdrant/rust-client/releases/tag/v1.20.0)
- [Text index builder](https://github.com/qdrant/rust-client/blob/v1.20.0/src/builders/text_index_params_builder.rs)
- [Text query builder](https://github.com/qdrant/rust-client/blob/v1.20.0/src/builders/text_query_builder.rs)
- [IDF corpus snippet](https://github.com/qdrant/rust-client/blob/v1.20.0/tests/snippet_tests/test_query_points_with_idf_corpus.rs)
- [Code tokenizer](https://github.com/qdrant/qdrant/blob/v1.20.0/lib/segment/src/index/field_index/full_text_index/tokenizers/code.rs)
- [Text scoring/filter semantics](https://github.com/qdrant/qdrant/blob/v1.20.0/lib/segment/src/index/field_index/full_text_index/full_text_index_read.rs)
- [Server text-index defaults](https://github.com/qdrant/qdrant/blob/v1.20.0/lib/segment/src/data_types/index.rs)

<a id="plain-object-publication-donor"></a>
## Plain root/object publication — R053 platform correction

Git's useful high-level pattern remains same-parent stage, streaming identity, file-data flush, no-clobber
finalization and exact existing/final verification.

### Rejected Windows paths

`tempfile 3.27.0::persist_noclobber` calls path-based `SetFileAttributesW` and `MoveFileExW` on Windows. It may help
with random stage creation but is not final authority.

The current ELIOT legacy source-registration owner is also a negative donor:

```text
--register-source-root / --unregister-source-root
→ registration_mutation::publish
→ registration_native::rename
→ SetFileInformationByHandle(FileRenameInfo)
```

It passes a relative leaf and non-null parent through the Win32 wrapper. The route is reachable, so agents must not
mistake its existence for an already-qualified handle-relative finalizer.

Current Windows behavior/reporting indicates the Win32 wrapper can absolutize the name while retaining
`RootDirectory`, producing an invalid parameter combination. Formal `FILE_RENAME_INFO` structure documentation is
therefore insufficient evidence that the wrapper executes the selected relative operation.

### Selected Windows candidate

Use one owner-private direct native file-information call, building an exact native rename buffer with replacement
disabled, retained parent handle and bounded relative leaf. Validate both returned native status and completed
`IO_STATUS_BLOCK`, then reopen through the retained parent with the ordinary native reader and verify identity and
bytes.

The exact information class/layout is selected only after a canary on every supported architecture/profile. The
canary covers destination-exists refusal, access/share flags, path/parent replacement, same-volume behavior,
failure debris, exact readback and error mapping. Unsupported native/filesystem/deployment profiles refuse. No
Win32-wrapper or path-only fallback, generic filesystem layer, source-registration phases or publication receipts.

The existing direct `NtCreateFile` relative-open seam is useful evidence, not qualification of a new rename.

Primary references:

- [Microsoft FILE_RENAME_INFO](https://learn.microsoft.com/windows/win32/api/winbase/ns-winbase-file_rename_info)
- [Microsoft SetFileInformationByHandle](https://learn.microsoft.com/windows/win32/api/fileapi/nf-fileapi-setfileinformationbyhandle)
- [Microsoft NtSetInformationFile](https://learn.microsoft.com/windows-hardware/drivers/ddi/ntifs/nf-ntifs-ntsetinformationfile)
- current ELIOT `registration_native.rs` at the inspected branch head

### Unix candidate

Use a private directory-relative no-replace rename when available. A hard-link + unlink fallback is allowed only
on the same filesystem, with exact final verification and visible stage debris. `std::fs::rename` may overwrite and
is not selected.

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

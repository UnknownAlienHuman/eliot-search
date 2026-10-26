# Donor and analogue register — installed state, candidates and selected patterns

This is the live upstream/analogue index. It records exact installed dependencies, implementation-time
candidates and patterns which materially simplify a concrete ELIOT owner. It does not approve a dependency,
reserve a future pin or turn another system into ELIOT architecture.

**Current checkpoint:** R048 over inspected head
`858d1e34a9053b6c3f51ba5d81d3b92ceb14b9f3`, 2026-10-25.  
**Previous detailed register:**
[`da382b66`](https://github.com/UnknownAlienHuman/eliot-search/blob/da382b66e59d0b44e758bad9b22e2f5703a4e8c2/docs/audit/DONOR_VERIFICATION_REGISTER_2026-10-09.md).

Keep four states separate:

1. upstream behavior inspected;
2. ELIOT use selected;
3. exact dependency/service installed and connected to a caller;
4. behavior executed through the product path.

Recheck the exact current version, features, MSRV, advisories, native artifacts and API at implementation time.

<a id="installed-selected-and-optional"></a>

## Installed dependencies and selected service baseline

| Donor | Installed state | ELIOT boundary and important limit |
|---|---|---|
| `qdrant-client` / Qdrant | client locked `1.19.0`; selected server baseline `1.19.0` | One private derived-index adapter. The default product path is still unconnected. Physical collection identity belongs to `PublishedView`; aliases are not serving authority. |
| `redb` | locked `2.6.3` | Compact current/published rows and real read snapshots. Transactions do not include files/Qdrant and long readers retain pages. |
| Tokio | locked `1.53.1` in current async consumers | Process/I/O/deadline primitives. Dropping a future neither cancels arbitrary blocking work nor settles an external mutation. |
| `pulldown-cmark` | exact `0.13.5`; defaults disabled | Markdown materializer uses empty options plus `OffsetIter`; version/checksum are transform identity. |
| `quick-xml` | locked `0.42.0` | Bounded JATS/XML parsing/coordinates, not a universal document platform. |
| `toml_parser` | locked `1.1.4+spec-1.1.0` | Legacy low-level decoder, not the selected typed replacement. |
| `aho-corasick` | locked `1.1.5` | Useful overlapping literal primitive; not yet the connected retained-source Exact owner. |
| `unicode-ident` / `unicode-normalization` | `1.0.26` / `0.1.25` | Narrow lexical inputs; normalization is not compiler symbol identity. |
| `globset` | locked `0.4.20` | Admission matching only; not an inventory owner. |

R031 Actions run 40084893130 passed Rust 1.98 locked check/minimal Clippy for
`search-materializer` plus `search-unitizer` only.

## Implementation-time candidates not installed

| Candidate | Intended use | Adoption rule |
|---|---|---|
| high-level `toml` + Serde; `serde_json` | Incremental typed settings and bounded framed JSON | Select with Foundation/protocol consumers. Preserve sparse-layer absence, closed nested structs, native paths and redacted diagnostics. |
| `tempfile` | Same-directory unique staging and no-clobber publication for plain objects/root manifest | Current inspected release is `3.27.0`, MSRV 1.63. Prefer `NamedTempFile::new_in` + `persist_noclobber`; pin only with #330/#360 code and selected Windows checks. |
| `ignore::WalkBuilder` | Authoritative ignore-aware bounded source inventory | Select with #271 after the one-file skeleton. Inspect builder, traversal and `DirEntry::error()` channels. |
| `notify::RecommendedWatcher` | Optional dirty hints after inventory convergence works | No correctness authority; absent from lock. Add only after #271 works without notifications. |
| `regex-automata` | Optional bounded Exact regex | Literal Exact is baseline. Select only with a concrete #248 request/consumer. |
| PDFium via `pdfium-render` | First optional retained-PDF native text profile | Pin crate and matching packaged native artifact together with #227. |
| `rmcp` | Optional stdio MCP tools leaf | Absent from lock. R039 inspected stable `3.7.0`; recheck only with post-baseline #219. |
| tree-sitter / SCIP | Optional source-backed outline/semantic facts | Add one producer only for a concrete orientation consumer. |
| model/tokenizer/runtime | Optional reranking/embedding | Require measured lexical-baseline benefit and one exact artifact/profile. |

These are alternatives and optional leaves, not a dependency shopping list.

<a id="plain-object-publication-donor"></a>

## Plain object publication: selected Git/tempfile pattern

Git's loose-object writer is the strongest small donor for the plain object store:

- create a temporary file in the final object's directory;
- stream bytes while computing the object identity;
- sync and close the file before finalization;
- verify that the calculated identity matches the intended object;
- finalize into the content-addressed path.

Primary source:
[Git loose-object writer at `7e34d817`](https://github.com/git/git/blob/7e34d8174dcda3435d1404c9a30bef46afe6c13e/odb/source-loose.c).

ELIOT keeps the pattern but not Git's `FOF_SKIP_COLLISION_CHECK` assumption. When a final path already exists,
ELIOT opens it through the object reader and verifies exact format/profile/kind/address/length/content before
converging.

`tempfile 3.27.0` exposes `NamedTempFile::persist_noclobber`. On supported platforms it provides a small stable
Rust surface for final-directory staging without `std::fs::rename`'s overwrite behavior. A crash may leave a
second hard link/stage; that is safe maintenance debris, not an operation ledger.

Selected implementation shape:

```text
NamedTempFile::new_in(final_parent)
→ bounded write + streaming content address
→ sync_all according to advertised profile
→ persist_noclobber(final_path)
→ existing/final exact readback
→ ObjectRef
```

If the path API cannot preserve the selected Windows retained-handle/no-reparse boundary, keep one tiny
Windows finalizer inside the object owner. Do not create a generic filesystem abstraction.

Advertise namespace atomicity and the exact executed file-sync behavior. Do not claim sudden-power-loss
durability from final readback or a no-op directory sync.

<a id="analog-systems"></a>

## Analog systems — adopt the pattern, reject the subsystem

| System/readset | Useful pattern | Explicit non-adoption |
|---|---|---|
| [Tantivy `b9c4ed1c`](https://github.com/quickwit-oss/tantivy/blob/b9c4ed1c62729001c3c1ce0cc6cdf7dd4a5eb82a/src/indexer/index_writer.rs) | Durable commit and serving visibility are separate; explicit reader reload makes visibility deterministic. Segment GC is separate from commit. | No Tantivy fallback/backend trait. Its default BM25 statistics span all segments; scoped statistics would become ELIOT-owned complexity. |
| [Zoekt `572f5e4a`](https://github.com/sourcegraph/zoekt/blob/572f5e4a3a37532300298e2ba38513c15b2e0319/index/builder.go) | Build immutable temporary shards, rename completed artifacts, classify missing/corrupt/version/options/content and remove stale artifacts after successful publication. | No delta-shard/tombstone stack and no second exact-search index. Borrow full-generation build first; add incremental mutation only after the working product measures a need. |
| Watchman official model | Filesystem events/clocks are observations; overflow/uncertainty requires recrawl and conservative fresh-instance handling. | No Watchman service, durable event cursor or watcher authority. `ignore::WalkBuilder` inventory remains correctness; `notify` only sets a dirty latch. |
| [qntx/qmd `8747cafa`](https://github.com/qntx/qmd/commit/8747cafab6b3af79d93f92b2c27ff5cfaf935c8f) | Release a shared database mutex before embedding/reranking/generation; lock per statement. Small core/CLI/MCP leaves are a useful topology. | No replacement of retained evidence/Qdrant with SQLite merely for packaging simplicity; no default model runtime. |
| [Veles `f49adb4f`](https://github.com/julymetodiev/Veles/blob/f49adb4f828543d2b9bb5e3469582abcf03adaaa/crates/veles-core/src/persist.rs) | Simple manifest and explicit incremental classification are easy to understand. | Reject manifest-first publication, missing-artifact-as-empty, `size+mtime == unchanged`, silent walk/stat omission and direct `old - seen == removed`. These are counterexamples for ELIOT's view-last/stable-read/incomplete-scan rules. |
| [codesearch `765e3daa`](https://github.com/flupkede/codesearch/blob/765e3daad6f4614b1d91789d682c6e94d5be3a08/Cargo.toml) | Broad product exploration shows useful optional capabilities. | Do not copy the all-at-once closure: many grammars, embedding runtime/downloads, two indexes, watcher/debouncer, HTTP/TUI, SCIP and vendored MCP patch before a minimal retained-evidence path. |
| [Git loose objects](https://github.com/git/git/blob/7e34d8174dcda3435d1404c9a30bef46afe6c13e/odb/source-loose.c) | Content-addressed same-directory stage/write/sync/finalize. | Do not infer ELIOT source membership or trust an existing final without exact verification. |

## Selected delivery consequence

Immutable-segment systems support a simpler implementation order than immediate per-point incrementality:

```text
first Search
→ restart/reconcile with a complete fresh generation
→ only then incremental point create/fixed closure if measurement justifies it
```

The fresh-generation profile is final architecture, not a throwaway backend. It uses the same retained objects,
prepared outcomes, evidence rows, physical collection identity and `PublishedView`. The later incremental profile
adds intra-generation validity and stable occurrence reuse in a newly built compatible generation.

This split prevents #329 point continuity, point closures and compaction thresholds from blocking the first
restart-convergent application.

## Qdrant collection aliases are not serving authority

Qdrant can atomically change collection aliases, but an alias switch cannot atomically install ELIOT's matching
redb evidence snapshot. Qdrant snapshots also do not preserve aliases as collection data. Therefore:

- `PublishedView` carries the exact physical collection generation;
- query addresses that physical collection directly;
- old pinned views keep their old physical generation;
- aliases may be an operator convenience only and are not recovery/visibility truth.

Do not add an alias manager to the baseline.

<a id="qdrant-update-mode-and-generation-cleanup"></a>

## Qdrant update modes and generation cleanup

Official Qdrant point documentation distinguishes default overwrite/upsert, `insert_only` and `update_only`;
explicit update modes are available from server 1.17, so selected 1.19 includes them.

ELIOT uses two profiles:

### Fresh-generation profile — stages 2 and 3A

```text
new never-reused physical collection
→ explicit insert_only complete build
→ exact readback + complete evidence snapshot
→ atomic redb current commit
→ explicit PublishedView install
```

Once visible, the collection is immutable. Any source change builds another generation. No point closure,
per-point deletion, stable cross-generation point ID or alias switch is required.

### Incremental profile — stage 3B

Only after the fresh-generation path works and rebuild cost is measured:

```text
new point → explicit insert_only
replaced point → one fixed non-creating valid_until
compaction/profile change → fresh never-reused generation
```

Default upsert is forbidden for retries. A missing-point closure must not create a point. Individual point IDs
remain physically present until whole-generation retirement.

Whole-generation cleanup requires no current/pending target and no live view/window guard, then terminal
collection deletion plus exact logical-absence observation. Never reuse the name. Uncertainty retains cleanup
state/backpressure. Logical absence is not physical-erasure evidence. Delayed point/delete behavior remains a
live Qdrant 1.19 case.

Primary Qdrant references:

- [points / update mode](https://qdrant.tech/documentation/concepts/points/#update-mode)
- [collections and aliases](https://qdrant.tech/documentation/concepts/collections/)
- [delete collection](https://api.qdrant.tech/api-reference/collections/delete-collection)

<a id="native-bm25-and-separate-statistics-scope"></a>

## Qdrant native BM25, scope and IDF

Use Qdrant native BM25 rather than a second numerical engine. For every sparse leaf:

```text
base population = compatible selected sources + pinned validity/access partition
retrieval filter = base population + request-only narrowing
idf.corpus       = base population
```

`idf.corpus` does not filter hits. In 1.19 it is shard-local. Empty statistics remove rarity/length signals;
missing/incomplete statistics are not known-empty Search. Scoped IDF may enumerate its selected population
before ranking, so top-k does not bound backend work.

Tantivy confirms the alternative cost: its default BM25 provider aggregates token/doc statistics across every
segment, while a custom provider must supply the exact scoped counts. Keeping Qdrant avoids building and
maintaining that second statistics owner before measurement.

Primary references:

- [native BM25](https://qdrant.tech/documentation/inference/inference-bm25/)
- [scoped statistics](https://qdrant.tech/documentation/manage-data/multitenancy/#per-tenant-scoring-statistics)
- [Qdrant 1.19 IDF source](https://github.com/qdrant/qdrant/blob/v1.19.0/lib/segment/src/index/sparse_index/sparse_vector_index/read_view/idf.rs)
- [Tantivy BM25 provider](https://github.com/quickwit-oss/tantivy/blob/b9c4ed1c62729001c3c1ce0cc6cdf7dd4a5eb82a/src/query/bm25.rs)

## Qdrant word tokenization is not exact identifier identity

The 1.19 word tokenizer splits on non-alphanumeric characters. Exact literal/name/location operations remain
independent of BM25 candidates.

<a id="markdown-offset-iterator-released-fix-not-yet-adopted"></a>

## Markdown OffsetIter correction

R031 moved the materializer from `pulldown-cmark 0.13.1` to `0.13.5`, preserving empty options and
`OffsetIter` while updating lock and transform identity. Old output is not relabelled. Actual connected
materialize/replay evidence remains #215/#331.

<a id="typed-configuration-donor-boundaries"></a>

## Typed configuration limits

The current lock lacks high-level `toml`, Serde and `serde_json`. The
[config owner](../../crates/search-config/FUNCTIONS.md#layer-semantics) owns incremental fields and precedence.
Preserve sparse absence, close nested structs, keep native OS paths outside TOML text where applicable, bound
input before decoding, redact diagnostics and reject invalid effective settings before effects.

<a id="a-chunk-api-is-not-an-incremental-reader"></a>

## Exact matcher seam

`search-exact::literal::scan_chunks` preserves overlapping matches across supplied chunks, but builds the NFA
per call, requires pre-supplied chunks and lacks cancellation inside the loop. Compile once per request and feed
bounded blocks from the retained reader. Raw bytes, decoded text and derived representations are distinct.

## Currentness rule

Caller searches, package sizes and analogue behavior are revision-specific evidence. Before a code cut, recheck
only the exact current callers and exact donor API involved. Record inspected SHA, selected use and unrun work in
the analysis registry.
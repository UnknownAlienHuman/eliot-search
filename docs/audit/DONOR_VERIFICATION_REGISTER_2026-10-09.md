# Donor and analogue register — installed state, candidates and selected patterns

This is the live upstream/analogue index. It records exact installed dependencies, implementation-time
candidates and patterns which materially simplify a concrete ELIOT owner. It does not approve a dependency,
reserve a future pin or turn another system into ELIOT architecture.

**Current checkpoint:** R049 over inspected head
`fed10c1e17e07f5bd309200dd25245fcbf85a0be`, 2026-10-26.

Keep four states separate:

1. upstream behavior inspected;
2. ELIOT use selected;
3. exact dependency/service installed and connected to a caller;
4. behavior executed through the product path.

Recheck exact version, features, MSRV, advisories, native artifacts and API with the actual code cut.

<a id="installed-selected-and-optional"></a>
## Installed dependencies and selected service baseline

| Donor | Current state | ELIOT boundary and important limit |
|---|---|---|
| `qdrant-client` / Qdrant | lock currently has client `1.19.0`; R049 selects server/client `1.20.x` for the walking-skeleton code cut | Qdrant 1.20 adds direct BM25 scoring on full-text payload indexes, `TextQuery` and the `code` tokenizer. Upgrade Cargo/lock/schema/query together; no runtime claim exists yet. |
| `redb` | locked `2.6.3` | Compact current/published rows and real read snapshots. Transactions do not include files/Qdrant; long readers retain pages. |
| Tokio | locked `1.53.1` in current async consumers | Process/I/O/deadline primitives. Dropping a future neither cancels blocking work nor settles external effects. |
| `pulldown-cmark` | exact `0.13.5`, defaults disabled | Materializer uses empty options plus `OffsetIter`; version/checksum are transform identity. |
| `quick-xml` | locked `0.42.0` | Bounded JATS/XML parsing and coordinates, not a universal document platform. |
| `toml_parser` | locked `1.1.4+spec-1.1.0` | Legacy low-level decoder, not the selected typed replacement. |
| `aho-corasick` | locked `1.1.5` | Useful overlapping literal primitive; not yet the connected retained-source Exact owner. |
| `globset` | locked `0.4.20` | Admission matching only; not an inventory owner. |

R031 Actions run 40084893130 passed Rust 1.98 locked check/minimal Clippy for `search-materializer` plus
`search-unitizer` only.

## Implementation-time candidates not installed

| Candidate | Intended use | Adoption rule |
|---|---|---|
| high-level `toml` + Serde; `serde_json` | Incremental typed settings and bounded framed JSON | Select with Foundation/protocol consumers. Preserve sparse-layer absence, closed nested structs, native paths and redacted diagnostics. |
| owner-private native no-clobber finalizer | Root manifest and plain immutable objects | Required on Windows unless an executed alternative preserves opened-stage and retained-parent identity. Keep it inside root/object owners. |
| `tempfile` | Optional same-directory random stage helper, mainly Unix | Current inspected `3.27.0`, MSRV 1.63. Do **not** use its current Windows `persist_noclobber` as the trusted finalizer; it is path-based. Pin only if the concrete owner still benefits after native finalization is selected. |
| `ignore::WalkBuilder` | Authoritative ignore-aware bounded inventory | Select with #271 after the one-file skeleton. Inspect builder, traversal and `DirEntry::error()` channels. |
| `notify::RecommendedWatcher` | Optional dirty hints after reconciliation | No correctness authority; add only after #271 works without notifications. |
| `regex-automata` | Optional bounded Exact regex | Literal Exact is baseline; select only with a concrete #248 consumer. |
| PDFium via `pdfium-render` | Optional retained-PDF native text | Pin crate and matching packaged native artifact together with #227. |
| `rmcp` | Optional stdio MCP leaf | Absent from lock; recheck only with post-baseline #219. |
| tree-sitter / SCIP | Optional source-backed outline/semantic facts | Add one producer only for a concrete orientation consumer. |
| model/tokenizer/runtime | Optional reranking/embedding | Require measured benefit over the lexical baseline and one exact artifact/profile. |

These are alternatives and optional leaves, not a dependency shopping list.

<a id="plain-object-publication-donor"></a>
## Plain object publication — corrected platform boundary

Git's loose-object writer supplies the right high-level pattern:

```text
same-directory unique stage
→ stream bytes while computing identity
→ flush/sync and close
→ no-clobber finalization
→ exact existing/final verification
```

Primary source: [Git loose-object writer at `7e34d817`](https://github.com/git/git/blob/7e34d8174dcda3435d1404c9a30bef46afe6c13e/odb/source-loose.c).

ELIOT does not copy Git's skipped collision check. Existing final bytes and metadata are verified exactly.

### `tempfile 3.27.0` finding

The public API offers `persist_noclobber`, but the platform implementations matter:

- Linux/Apple/Redox attempt `renameat`/`renameat2` with `NOREPLACE`, then fall back to `hard_link` + `unlink`; the
  original stage link may remain after a crash/failure.
- Windows changes attributes by path and calls `MoveFileExW` without `MOVEFILE_REPLACE_EXISTING`. This is
  no-clobber at the destination name, but it is **path-based** and does not bind finalization to the already-open
  stage handle or retained destination-directory handle.

Sources:

- [tempfile Unix implementation](https://github.com/Stebalien/tempfile/blob/1c294cc29d2a48746637252a0e30b1bebc05b3e6/src/file/imp/unix.rs)
- [tempfile Windows implementation](https://github.com/Stebalien/tempfile/blob/1c294cc29d2a48746637252a0e30b1bebc05b3e6/src/file/imp/windows.rs)

**Decision:** retract R048's cross-platform preference for `persist_noclobber` as the finalizer. It may remain a
stage helper where safe, but Windows final publication uses one owner-private handle-based operation. Microsoft
`FILE_RENAME_INFO`/`SetFileInformationByHandle` supports failure when the destination exists and can resolve a
relative name from a directory handle; the exact target Windows profile requires an executed canary before
claiming support. No generic filesystem abstraction is introduced.

Advertise only the achieved profile: namespace no-clobber/atomicity, exact file-data flush and final readback.
Do not claim sudden-power-loss directory durability without executed platform evidence.

<a id="qdrant-120-direct-bm25"></a>
## Qdrant 1.20 direct BM25 and code tokenizer

Qdrant 1.20 adds BM25 scoring directly on a scoring full-text payload index:

- text is stored once in payload;
- no sparse-vector inference step is required for documents or queries;
- average document length and term statistics derive from indexed data;
- `k` and `b` are query-time values;
- the new `code` tokenizer splits identifiers on underscores and case changes while retaining the full
  identifier.

Primary sources:

- [Qdrant 1.20 release](https://github.com/qdrant/qdrant/releases/tag/v1.20.0)
- [Qdrant 1.20 full-text announcement](https://qdrant.tech/blog/qdrant-1.20.x/)
- [full-text BM25 documentation](https://qdrant.tech/documentation/search/text-search/full-text-search/)
- [Rust client 1.20 release](https://github.com/qdrant/qdrant-client/releases/tag/v1.20.0)

**Selected baseline:** upgrade server/client/schema/query together during #263/#264, store retrieval text in one
explicit payload field, create a scoring full-text index with explicit tokenizer/lowercase/stemmer/stopword
settings, and use `TextQuery`. Keep source bytes in immutable objects; Qdrant payload is derived retrieval state.
Exact punctuated identifier search remains independent.

The direct full-text index uses more disk/memory than a non-scoring text index and its statistics may include
outdated segment data until optimization after updates. Fresh-generation publication avoids serving partially
mutated statistics in Stage 3A.

<a id="qdrant-partition-and-shard-boundary"></a>
## Qdrant partitions/custom shards — not a generic rebuild shortcut

Qdrant supports payload partitions and user-defined shard keys. Custom shards are useful for a modest number of
large partitions that are always queried independently. They are not selected as a generic per-root/per-source
replacement strategy for ELIOT:

- BM25/full-text statistics are computed per physical shard;
- when several shards are queried, each shard scores with its own statistics population;
- an `idf.corpus` filter cannot reach across shards and can silently see empty/partial local overlap;
- comparing or fusing independently scored shard result lists would create another ranking owner;
- each shard consumes storage/index resources and high-cardinality shard keys are explicitly discouraged.

Primary sources:

- [Qdrant multitenancy and per-tenant scoring](https://qdrant.tech/documentation/manage-data/multitenancy/)
- [distributed/custom sharding](https://qdrant.tech/documentation/distributed_deployment/)
- [time-based sharding](https://qdrant.tech/documentation/tutorials-operations/time-based-sharding/)

Custom sharding may be reconsidered only for a product scope that is always searched as one partition and has
measured resource benefit. It is not part of the baseline portfolio query.

<a id="qdrant-edge-boundary"></a>
## Qdrant Edge — promising, not selected baseline

Qdrant Edge 0.8.0 is an official Rust crate with in-process sparse/BM25 search, filters, on-disk shards and
read-only follower views. It could eventually remove the external service/network boundary and maps naturally to
immutable generation directories.

It is not selected now because:

- official documentation still labels Edge beta;
- Edge is single-shard/single-device with manual `optimize()` and explicit flush/drop obligations;
- the released 0.8.0 line recently exposed WAL acknowledgement/replay/persistence defects; fixes appear in newer
  Qdrant source/release work but are not yet established as a separately qualified stable Edge release for ELIOT;
- the crate is a large embedded storage dependency and would materially change build, licensing review, Windows
  behavior and failure ownership.

Primary sources:

- [Edge overview](https://qdrant.tech/documentation/edge/)
- [Edge vs Server](https://qdrant.tech/documentation/edge/edge-vs-qdrant-cluster/)
- [Edge BM25](https://qdrant.tech/documentation/edge/edge-bm25/)
- [qdrant-edge 0.8.0 docs](https://docs.rs/qdrant-edge/0.8.0/qdrant_edge/)
- [Qdrant 1.20 Edge fixes](https://github.com/qdrant/qdrant/releases/tag/v1.20.0)

After the server walking skeleton works, an isolated Edge spike may compare build size, startup, persistence,
Windows behavior, BM25 relevance and generation lifecycle. It must replace the bridge cleanly, not create a dual
backend.

<a id="analog-systems"></a>
## Analogue systems — adopt the pattern, reject the subsystem

| System | Useful pattern | Explicit non-adoption |
|---|---|---|
| Tantivy | Durable commit and reader visibility are separate; segment GC is separate. | No fallback/backend trait; scoped statistics would become ELIOT-owned complexity. |
| Zoekt | Immutable temporary shards, completed-artifact publication and explicit state classification. | No second exact-search index or delta-shard/tombstone stack. |
| Watchman / `ignore` | Events are observations; uncertainty causes recrawl; inventory errors are explicit. | No Watchman service, durable event cursor or watcher authority. |
| qntx/qmd | Release database mutex before embedding/reranking/generation; lock per statement. | No SQLite replacement or default model runtime. |
| Veles | Simple manifest/update topology is easy to read. | Reject manifest-first publication, missing-artifact-as-empty, size+mtime authority and `old-seen == removed`. |
| codesearch | Broad product catalogue. | Reject all-at-once grammars/models/two indexes/watcher/HTTP/TUI/SCIP/MCP closure. |

<a id="qdrant-update-mode-and-generation-cleanup"></a>
## Qdrant generation profiles and cleanup

### Fresh-generation profile — Stages 2 and 3A

```text
new never-reused physical collection
→ scoring full-text schema
→ explicit insert_only complete build
→ exact readback + complete evidence snapshot
→ atomic redb current commit
→ explicit PublishedView install
```

Once visible, the collection is immutable. Any selected source change builds another generation. No point
closure, per-point deletion, stable cross-generation ID or alias switch is required.

### Incremental profile — Stage 3B

Only if Stage 3A fails declared budgets:

```text
new point → explicit insert_only
replaced point → one fixed non-creating valid_until
compaction/profile change → fresh never-reused generation
```

Whole-generation cleanup requires no current/pending target and no live view/window guard, then terminal deletion
plus exact logical-absence observation. Never reuse the name. Uncertainty retains cleanup state/backpressure.
Logical absence is not physical-erasure evidence.

<a id="native-bm25-and-separate-statistics-scope"></a>
## BM25 scope and IDF

For every query leaf:

```text
base population = compatible selected sources + pinned validity/access partition
retrieval filter = base population + request-only narrowing
idf.corpus       = base population
```

Qdrant statistics are shard-local. Empty corpus overlap removes rarity/length signals rather than falling back.
Scoped IDF may enumerate the selected population before ranking, so top-k does not bound backend work. Measure
cold/warm broad/narrow scopes through the connected 1.20 `TextQuery` path.

<a id="typed-configuration-donor-boundaries"></a>
## Typed configuration limits

The current lock lacks high-level `toml`, Serde and `serde_json`. Preserve sparse absence, close nested structs,
keep native paths outside TOML text where applicable, bound input before decoding, redact diagnostics and reject
invalid effective settings before effects.

<a id="a-chunk-api-is-not-an-incremental-reader"></a>
## Exact matcher seam

`search-exact::literal::scan_chunks` preserves overlapping matches across supplied chunks, but builds the NFA per
call, requires pre-supplied chunks and lacks cancellation inside the loop. Compile once per request and feed
bounded blocks from the retained reader. Raw bytes, decoded text and derived representations remain distinct.

## Currentness rule

Caller searches, package sizes and analogue behavior are revision-specific. Before a code cut, recheck only the
exact current callers and donor API involved. Record inspected SHA, selected use and unrun work in the registry.

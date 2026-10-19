# Donor and reuse index

**Refreshed:** 2026-10-18 against Eliot source
`c8e38b6fc9829f1cd38070773b1b3e5b21de18ad` and the pinned upstream revisions below.
This is static source research, not a dependency-upgrade or runtime qualification claim.

Use a donor to remove a mechanism. Do not import a donor application, preserve Eliot wrappers around it or
build a provider framework for every alternative.

## Selected mechanisms

| Eliot seam | Inspected donor | Use | Do not import |
|---|---|---|---|
| CLI dispatch | [ripgrep `4649aa9`](https://github.com/BurntSushi/ripgrep/blob/4649aa9700619f94cf9c66876e9549d83420e16c/crates/core/main.rs) | Parse one invocation, dispatch one concrete mode. | Repeated argv capture, command fingerprinting or a new CLI framework. |
| Filesystem inventory | [ignore walk `4649aa9`](https://github.com/BurntSushi/ripgrep/blob/4649aa9700619f94cf9c66876e9549d83420e16c/crates/ignore/src/walk.rs) | Nested ignore rules and bounded traversal. | Another glob language or whole-portfolio collect/sort for receipts. |
| Change hints | [notify `a1d7c2d`](https://github.com/notify-rs/notify/blob/a1d7c2d8f80786679d58ec6d5986a1d4278bc8cf/notify/src/windows.rs) | One `RecommendedWatcher` feeding the shared dirty queue. | Event continuity claims, duplicate `PollWatcher`, or a watcher supervisor per root. |
| Debouncing | [notify-debouncer-full `76f48c7`](https://github.com/notify-rs/notify/blob/76f48c70bbf7fed75440d580e498e90cbc6ce140/notify-debouncer-full/src/lib.rs) | Rename/burst coalescing ideas only. | Default `FileIdMap`: it recursively follows links and retains IDs for the whole watched tree. |
| Git commit views | [Git `c44beea`](https://github.com/git/git/blob/c44beea485f0f2feaf460e2ac87fdd5608d63cf0/builtin/cat-file.c) | Resolve exact OIDs; `ls-tree`, raw `diff-tree`, reused bounded `cat-file --batch-command`. | Checkout, textconv, filters, lazy fetch, process-per-file or mandatory dual backend. |
| Native Git alternative | [gitoxide `ad0202e`](https://github.com/GitoxideLabs/gitoxide/blob/ad0202e3ab396d752a56664001c134f48d834177/gix-diff/src/tree/function.rs) | Optional no-Git tree/ODB path. | Shipping both backends before a packaging requirement exists. |
| Control state | [redb 2.6.3 transactions](https://github.com/cberner/redb/blob/v2.6.3/src/transactions.rs) | Short typed write transactions and MVCC read snapshots; load current state once and update the changed pointer/cache. | Eliot's eternal operation ledger, complete record copy after every write, or pretending redb transacts with Qdrant/files. |
| Immutable object finalization | [Git object finalization `c44beea`](https://github.com/git/git/blob/c44beea485f0f2feaf460e2ac87fdd5608d63cf0/object-file.c) | Stage locally, publish to the content address with link/rename, then remove the temporary name after success/reuse. | Git's ODB, pack format or an operation receipt per immutable write. |
| Orphan file GC | [Tantivy `ManagedDirectory` `6788926`](https://github.com/quickwit-oss/tantivy/blob/67889269cae67063c372bc236b102d632a021568/src/directory/managed_directory.rs) | Track files created by the owner; compute living files under the metadata lock; failed deletes remain tracked for retry. | Tantivy as a backend or its persisted JSON registry as Eliot's source-of-truth graph. Eliot derives roots from current/pending/pinned records. |
| JSON / config | [Serde 1.0.229](https://github.com/serde-rs/serde/blob/v1.0.229/README.md), [serde_json 1.0.151](https://github.com/serde-rs/json/blob/v1.0.151/src/de.rs), [toml 1.1.7](https://github.com/toml-rs/toml/blob/f72038c2508112dfaaedb227a511f91bf9b2015c/crates/toml/src/de/mod.rs) | Bounded typed frames/settings. | Handwritten JSON/TOML grammars, canonical hashes for ordinary records or parsing an unbounded input. |
| Deadlines | [Tokio 1.53.1](https://github.com/tokio-rs/tokio/blob/tokio-1.53.1/tokio/src/time/timeout.rs) | One absolute `timeout_at` deadline across phases. | Renewing a full duration for every RPC or another generic budget framework. |
| Indexed storage/query | [Qdrant client 1.19.0 `7c83803`](https://github.com/qdrant/rust-client/blob/7c838035ae7b9455636dcaca918a55b7d7ca638f/src/qdrant_client/points.rs) | Existing client for schema, full/narrow mutations, readback, query and grouping. | Vendor types outside the bridge, another backend or another operation ledger. |
| Lexical ranking | [Qdrant BM25 `74f3e85`](https://github.com/qdrant/qdrant/blob/74f3e85b9473c62560006c043e13737ce6b48412/src/common/inference/bm25_inference.rs) | Local document/query BM25 with explicit profile options. | Custom generic sparse engine, cloud/model worker or English defaults for neutral/code text. |
| Filtered IDF / fusion | [Qdrant IDF `74f3e85`](https://github.com/qdrant/qdrant/blob/74f3e85b9473c62560006c043e13737ce6b48412/lib/segment/src/index/sparse_index/sparse_vector_index/read_view/idf.rs) | Same resolved scope for retrieval and `idf.corpus`; native prefetch/RRF/grouping. | Duplicate portfolio points, global IDF service or local fusion graph for Qdrant-only legs. |
| Structure-aware document chunks | [Docling Core `f5f01f9`](https://github.com/docling-project/docling-core/blob/f5f01f97e9e1c7a50c1f378e114ca063085985ce/docling_core/transforms/chunker/hybrid_chunker.py) | Hierarchical boundaries, token-aware split, merge adjacent peers with matching headings. | Python runtime/storage stack or tokenizer-specific source identity. |
| Section chunks | [Unstructured `1b02b4d`](https://github.com/Unstructured-IO/unstructured/blob/1b02b4d189c260ed9b1aca29583203e233d853cb/unstructured/chunking/title.py) | Preserve title/section and optional page boundaries. | Its complete partition/chunk framework. |
| Unstructured oversized fallback | [FastCDC paper](https://www.usenix.org/conference/atc16/technical-sessions/presentation/xia) | Measure content-defined boundaries inside an oversized plain section. | Global semantic identity or an assumption that CDC preserves meaning. |
| Code outline | [ast-grep outline](https://ast-grep.github.io/guide/tools/outline.html) | Bounded syntax facts with real ranges. | Eliot's line/brace parser, a compiler platform or unsupported semantic relationships. |
| Semantic locations | [SCIP](https://github.com/sourcegraph/scip) / language producer output | Import actual definitions/references tied to the indexed snapshot. | Assuming every producer emits implementation/call relationships; inspected rust-analyzer output does not. |
| PDF | [pdfium-render](https://github.com/ajrcarey/pdfium-render) | Native page text and page/text coordinates in the optional worker. | Generic document runtime, OCR on pages with usable text or invented raw-PDF byte offsets. |
| Published-view lifetime | [Lucene snapshot policy `8a8ef48`](https://github.com/apache/lucene/blob/8a8ef488218405ee18ba9803c3ac1779ef74093a/lucene/core/src/java/org/apache/lucene/index/SnapshotDeletionPolicy.java) | Generation refcount; deletion suppressed while a snapshot exists. | Lucene backend, durable query leases or public reclaim permits. |
| Artifact GC | [Tantivy segment updater `6788926`](https://github.com/quickwit-oss/tantivy/blob/67889269cae67063c372bc236b102d632a021568/src/indexer/segment_updater.rs) | Committed/in-flight metadata lists useful files; remove untracked files after commit. | A second index or Tantivy's whole writer/segment system. |

### Storage donor conclusions

The donor mechanisms agree on a small design:

```text
immutable object:
  unique stage → bounded write/sync → no-clobber finalization
  → exact existing/final verification → remove stage

control state:
  short typed redb transaction → committed current value
  → optional narrow in-memory pointer/cache update

cleanup:
  living references from current/pending/pinned owners
  → bounded exact deletion; failed deletion stays discoverable
```

Git removes the temporary name after successful link/rename rather than preserving it as permanent
success evidence. Tantivy registers owned files before creation so crash debris remains discoverable, then
removes files absent from the live set while retaining failed deletions for retry. Eliot should copy those
mechanics, not either donor's storage format or metadata database.

## Existing workspace owners to reuse

- BLAKE3/SHA-256 execution belongs to `search-contracts`; remove unnecessary hashes before replacing the
  remaining copied algorithm implementations.
- Unicode XID/NFC/case-fold decisions already have selected libraries/profiles. Do not create another
  Unicode engine.
- `pulldown-cmark`, `quick-xml`, `globset`, `aho-corasick`, regex and Qdrant client already have real callers.
- The typed provider client, Windows transport and credential adapter already exist; connect the local
  commands instead of extracting another client framework.
- Point identity, verified UnitSet and publication/planner code exist, but their mixed revision/retrieval
  identity must change with actual consumers rather than be wrapped for compatibility.

## Context and research donors

| Donor | Useful idea | Limitation relevant to Eliot |
|---|---|---|
| EdgeQuake | Return retrieval context separately from final generation; batch selected chunks. | Context-only can still invoke keyword/model/embedding work. Extraction reuse does not eliminate later embedding, and its inspected reuse key omitted supplied heading context. |
| LightRAG | Select/deduplicate IDs before loading source text. | A selected hub can load all incident edges before truncation; small output does not bound traversal. |
| Microsoft GraphRAG | Order related sources and deduplicate text units. | Local context repeats relationship scans; inspected incremental detection compared titles, not complete content. |
| NotebookLM public material | Explicit source selection, source-grounded retrieval and navigation back to evidence. | Public material does not reveal a complete chunker/index/cache/reranker implementation; agentic web/code research is outside Search. |
| Cloudflare AI Search | Retrieval result can be returned before generation. | Hosted service/runtime is not a local dependency; embeddings/reranking still cost work. |
| Anthropic Contextual Retrieval | A bounded heading/path/signature prefix can make an isolated chunk intelligible. | Generated context is per-chunk model work and depends on the document; it is optional derived input, not quoted source. |

Eliot's small implementation is therefore: existing path/title/heading/syntax records form the source
guide; select bounded relations/ranges before I/O; share verified excerpts through the same query window;
add a selected model only for exact embedding or finite rerank input. No notebook store, GraphRAG database,
query-rewriting LLM or generated research-answer pipeline is required.

## Rejected baseline mechanisms

- Tantivy/Lucene as a second search backend.
- General provider/model registry before one real model caller exists.
- Generic migration, compatibility, legal-hold, purge, retention-lease or restore platforms.
- Whole-tree watcher identity caches and duplicate periodic walkers.
- Per-site digest ledgers, committed package maps and partial Rust import/type analysis.
- Canonical hashes, signatures or qualification reports for normal local requests and transitions.
- Multiple physical point copies solely for portfolio membership.
- Permanent success-stage aliases or operation receipts for immutable object writes.
- Full mirrored control snapshots rebuilt after each typed current-state update.

## Verification rule

A donor is accepted only when the connected Eliot path uses it and the replaced code is deleted. Record the
pinned source/API, actual caller, removed mechanism, focused behavior checks and remaining limitations.
Compilation or a donor tutorial does not establish Windows, live-Qdrant, scale or installed behavior.

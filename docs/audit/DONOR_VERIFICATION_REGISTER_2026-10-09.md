# Donor evidence and limits

Rechecked 2026-10-21: Eliot source `c8e38b6`, replacement documentation `32f26536`, Qdrant server
`v1.19.0`, and the workspace's Aho-Corasick `1.1.5` selection. This is source inspection, not a benchmark,
completed integration or dependency upgrade. The [previous index](https://github.com/UnknownAlienHuman/eliot-search/blob/32f265368c45627f1e93cd539361884b07e4d41a/docs/audit/DONOR_VERIFICATION_REGISTER_2026-10-09.md)
retains earlier donor research; its blanket implementation recommendations are not current instructions.

A donor removes code only when its actual semantics fit the consumer. A feature name, native implementation
or small response does not establish cheap execution, exact identity or complete evidence. Current operation
rules live in implementation and owner maps, not in this research index.

## Rechecked retrieval assumptions

### Filtered IDF performs population work

In Qdrant 1.19.0, [`read_view/idf.rs`](https://github.com/qdrant/qdrant/blob/v1.19.0/lib/segment/src/index/sparse_index/sparse_vector_index/read_view/idf.rs)
(blob `7e0d2ed0b7b433b3cdb55e1e66ca07e9289daef0`):

- `fill_global_idf_statistics` reads posting-list lengths and the indexed-vector count.
- `fill_corpus_idf_statistics` first calls `collect_corpus_points`; this enumerates the filtered population
  with a vector into a sorted ID list or a dense bit mask.
- The mask is sized by total segment points. The switch threshold is `max(total_points / 32, 128)`.
- Term frequencies then require posting-list walks against the mask or skips through the selected IDs.

Thus a request for ten results can still enumerate a large selected population before ranking. Payload
indexes help select that population; they do not turn this implementation into cached constant-time IDF.
For a broad corpus, mask allocation scales with the physical segment and traversal with selected points
and queried postings, not returned cards. This is a source-derived cost, not a measured latency result.

Eliot needs scope/validity-specific statistics to keep staged/retired/unselected points out of ranking. Keep
that meaning; do not silently switch to global IDF to make a benchmark fast. But do not claim large-corpus
interactive performance from `top_k`, page size or native delegation. Measure cold/warm requests, different
scope sizes and retained/staged points through the real query. A timeout remains incomplete, not empty.
No local IDF engine, per-request collection or statistics cache is selected by this finding. A different
layout or deliberately weaker scoring contract needs an explicit measured decision.

### Lexical tokens do not preserve identifier identity

Qdrant 1.19.0 [`WordTokenizer::tokenize`](https://github.com/qdrant/qdrant/blob/v1.19.0/lib/segment/src/index/field_index/full_text_index/tokenizers/mod.rs)
(blob `5d10e3ca3e4b99c4062d1132bb86fa0e9c3ab224`) splits on every non-alphanumeric character. Consequently
`parse_manifest`, `parse-manifest` and `parse::manifest` have the same word sequence; `C++` and `C#` both
lose their punctuation. Adding the original spelling to a text string does not keep it as an indivisible
term. Snake/camel aliases improve recall; they do not implement exact-name resolution.

The old architecture already distinguished this in S12.3 and S21. Exact literal/name/location requests
must not depend on a BM25 top-k candidate set. The [lexical map](../../crates/search-lexical/FUNCTIONS.md)
and [query behavior](../../crates/search-query/README.md) retain that boundary without resurrecting the
recipe or resolver platform.

[`bm25_inference.rs`](https://github.com/qdrant/qdrant/blob/v1.19.0/src/common/inference/bm25_inference.rs)
(blob `06be18127c0b283fd699dcc1be9dc4a060979842`) also defaults to English stemming and stopwords when they
are omitted. The document/query roles differ. Its defaulting behavior is not a neutral code/text profile.
Use explicit selected options; independent native-vector verification is limited as stated in the bridge.

### A chunk API is not an incremental reader

Eliot [`literal.rs`](../../crates/search-query/search-exact/src/literal.rs) at `c8e38b6` correctly carries
Aho state across UTF-8 chunks and preserves overlapping matches. However, `scan_chunks(&[&str], ...)` builds
the NFA inside every invocation, requires the chunks to be supplied already, and has no cancellation check
inside its byte loop. Calling it once per file is not compile-once streaming across a corpus.

The selected [Aho-Corasick automaton API](https://docs.rs/aho-corasick/1.1.5/aho_corasick/automaton/index.html)
already exposes state transitions. Reuse the NFA once per request, reset state per source and feed bounded
blocks with cancellation checks. Do not casually substitute a non-overlapping convenience iterator for the
existing overlapping contract. The [exact map](../../crates/search-query/search-exact/FUNCTIONS.md) also
restores the old S25 distinction between raw bytes and decoded/derived text.

## Selected mechanisms, not wholesale donor applications

| Need | Source retained from research | Applicability boundary |
|---|---|---|
| CLI | [ripgrep `4649aa9` main](https://github.com/BurntSushi/ripgrep/blob/4649aa9700619f94cf9c66876e9549d83420e16c/crates/core/main.rs) | Parse once and dispatch. Do not copy its whole application or build command fingerprints. |
| Inventory | [ignore walk `4649aa9`](https://github.com/BurntSushi/ripgrep/blob/4649aa9700619f94cf9c66876e9549d83420e16c/crates/ignore/src/walk.rs), [notify `a1d7c2d`](https://github.com/notify-rs/notify/blob/a1d7c2d8f80786679d58ec6d5986a1d4278bc8cf/notify/src/windows.rs) | Ignore traversal and event hints; neither proves a complete observation history. |
| Debounce | [notify-debouncer-full `76f48c7`](https://github.com/notify-rs/notify/blob/76f48c70bbf7fed75440d580e498e90cbc6ce140/notify-debouncer-full/src/lib.rs) | Burst/rename ideas only; its whole-tree FileIdMap is not the selected inventory. |
| Git | [Git `c44beea` cat-file](https://github.com/git/git/blob/c44beea485f0f2feaf460e2ac87fdd5608d63cf0/builtin/cat-file.c) | Exact OIDs and bounded raw plumbing. [gitoxide `ad0202e`](https://github.com/GitoxideLabs/gitoxide/blob/ad0202e3ab396d752a56664001c134f48d834177/gix-diff/src/tree/function.rs) remains an alternative, not a second mandatory backend. |
| Metadata snapshots | [redb 2.6.3](https://github.com/cberner/redb/blob/v2.6.3/src/transactions.rs) | Typed point/range reads and real read snapshots; no transaction across Qdrant/files, and old readers retain database pages. |
| Immutable files | [Eliot cutover publisher](https://github.com/UnknownAlienHuman/eliot-search/blob/c8e38b6fc9829f1cd38070773b1b3e5b21de18ad/crates/search-control-redb/src/migration/cutover_artifact.rs), [Git object-file `c44beea`](https://github.com/git/git/blob/c44beea485f0f2feaf460e2ac87fdd5608d63cf0/object-file.c) | Extract temp/no-clobber/final verification/cleanup. Neither imports migration, Git ODB or another writer. |
| Windows durability candidate | [MoveFileEx](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-movefileexa) | A candidate, not proof of required no-clobber/power-loss behavior. No-op directory sync plus readback is not power-loss evidence. |
| JSON/settings | [Serde](https://github.com/serde-rs/serde), [serde_json](https://github.com/serde-rs/json), [toml](https://github.com/toml-rs/toml) | Typed syntax and capped buffers. Earlier researched releases do not prove these dependencies are already wired into Eliot. |
| Async timeouts | [Tokio 1.53.1 timeout](https://github.com/tokio-rs/tokio/blob/tokio-1.53.1/tokio/src/time/timeout.rs) | One attempt deadline; dropping a future does not cancel arbitrary blocking work or prove an external mutation stopped. |
| Indexed retrieval | [Qdrant client `7c83803`](https://github.com/qdrant/rust-client/blob/7c838035ae7b9455636dcaca918a55b7d7ca638f/src/qdrant_client/points.rs), Qdrant 1.19.0 sources above | One SDK boundary; native BM25, selected filtering and real effects. No numerical-correctness, latency or old-write-terminality claim from a feature name. |
| Structure | [Docling `f5f01f9`](https://github.com/docling-project/docling-core/blob/f5f01f97e9e1c7a50c1f378e114ca063085985ce/docling_core/transforms/chunker/hybrid_chunker.py), [Unstructured `1b02b4d`](https://github.com/Unstructured-IO/unstructured/blob/1b02b4d189c260ed9b1aca29583203e233d853cb/unstructured/chunking/title.py) | Section/split/merge behavior, not Python runtimes or proof of edit-stable occurrence identity. FastCDC remains only a measured oversized-plain-section candidate. |
| Source coordinates | Existing pulldown-cmark and quick-xml consumers | Retain actual parser ranges/maps; syntax observations do not prove call targets or behavior. |
| Old-view lifetime | [Lucene SnapshotDeletionPolicy `8a8ef48`](https://github.com/apache/lucene/blob/8a8ef488218405ee18ba9803c3ac1779ef74093a/lucene/core/src/java/org/apache/lucene/index/SnapshotDeletionPolicy.java) | Refcount idea only. Lucene protects immutable commits/files; this does not prove Qdrant mutable retrieval state is snapshotted. |
| Living-file ideas | [Tantivy ManagedDirectory `6788926`](https://github.com/quickwit-oss/tantivy/blob/67889269cae67063c372bc236b102d632a021568/src/directory/managed_directory.rs) | Reference selection and retryable failed deletes. Current Eliot local GC is stopped-root maintenance, not this donor's locking/registry design. |

## Optional research is not a selected implementation

Syntax outline tools, SCIP producers and pdfium-render remain candidates with concrete consumers required.
The unversioned ast-grep outline page was not retrievable in this recheck; do not claim that link as a newly
verified API. SCIP symbols/occurrences must be tied to the indexed source; missing call/implementation facts
cannot be fabricated. PDF page text/regions are derived representations, not binary-PDF byte ranges.

Prior EdgeQuake/LightRAG/GraphRAG research contributes select-before-load and context-reuse lessons, not a
framework. NotebookLM public material does not reveal a complete reusable index/chunker. Cloudflare AI Search
is a hosted service, not a local dependency. Generated contextual text costs work and is not quoted source.
These references are not evidence that Eliot's navigation/comparison path exists or works.

## Corrections to this register itself

The previous index said to connect the existing typed provider client/Windows transport while current
owner maps retire their pairing/grant stack. Reuse useful owned I/O only through the selected current
protocol; do not wire the old stack back in. Its cleanup synopsis also omitted the later stopped-root
choice. Research notes now point to owner recipes instead of publishing a competing algorithm.

There is no single 'accepted donor' boolean. Distinguish inspected upstream behavior, proposed Eliot use,
actual connected caller and measured runtime outcome in ordinary prose when making a claim. Do not turn
that distinction into a registry, review receipt, per-file stamp or launch requirement.

Next implementation evidence is concrete: scoped-IDF work at realistic corpus size; code spelling versus
lexical recall; raw/decoded exact semantics; cancellation through real reads; and source-backed navigation
and comparison examples. No elapsed-time, relevance, live-Qdrant or exhaustive-audit result is claimed here.

# Donor evidence and limits

Version/consumer status rechecked 2026-10-23 against Eliot `5af8cde5416fa33e3f1a8f3c19367ed520eac9a4`,
its manifests and lock; R028 at `37dbb198` additionally checks the Markdown caller and upstream 0.13.5 fix.
R029 at `afc3bfca` checks typed configuration API/default/path boundaries; no dependency changes follow.
Dependencies are unchanged between those checkpoints. Current official Qdrant/Tokio/TOML documentation was also consulted. Pinned Qdrant
source-cost findings below come from the 2026-10-21 inspection and remain version-specific. This is not a
benchmark, completed integration or dependency upgrade. The [previous index](https://github.com/UnknownAlienHuman/eliot-search/blob/32f265368c45627f1e93cd539361884b07e4d41a/docs/audit/DONOR_VERIFICATION_REGISTER_2026-10-09.md)
retains earlier donor research; its blanket implementation recommendations are not current instructions.

A donor removes code only when its actual semantics fit the consumer. A feature name, native implementation
or small response does not establish cheap execution, exact identity or complete evidence. Current operation
rules live in implementation and owner maps, not in this research index.

## Installed, selected and optional

| Donor | Inspected dependency state | Actual use and next boundary |
|---|---|---|
| `qdrant-client` | Locked `1.19.0`; selected server baseline `1.19.0` is a separate deployment input. | Real SDK bridge exists, but normal publication/query is unconnected. `QualifiedGate` was removed; exact server-build `74f3e85b`, supervisor-binding and key-lease requirements still remain in the constructor. Follow the [bridge cutover](../../crates/search-index-qdrant/search-qdrant-bridge/FUNCTIONS.md), not the retired probe gate. |
| `aho-corasick` | Locked `1.1.5`. | Existing literal helper; connect its state machine to actual retained reads as described below. |
| `redb` | Locked `2.6.3`. | Real control-store dependency. Keep typed point/range/snapshot behavior; long readers retain pages and transactions do not include Qdrant/files. |
| Tokio | Locked `1.53.1` in daemon/bridge/reclaimer consumers. | Reuse async I/O/runtime/deadline primitives. Runtime presence does not connect the selected session or cancel blocking/native/server work. |
| `pulldown-cmark` | Locked `0.13.1`; maintained `0.13.5` fix selected for the next verified Markdown dependency cut, not installed. | Existing offset/coordinate consumer reaches the printable panic case under sufficient limits. See [released fix](#markdown-offset-iterator-released-fix-not-yet-adopted) and the materializer owner; no local parser fork or blacklist. |
| `quick-xml` | Locked `0.42.0`. | Existing parsing/coordinate consumer, not a working PDF/document worker. |
| `toml_parser` | Locked `1.1.4+spec-1.1.0`. | Current custom config decoder uses this low-level parser. It is not the proposed high-level typed decoder. |
| High-level `toml` + `serde`; `serde_json` | Absent as packages from this lock. Typed TOML/JSON is the selected replacement direction; versions/features are not yet pinned. | Choose and lock with actual settings/framing consumers. [TOML docs](https://docs.rs/toml/latest/toml/) inspected at `1.1.8+spec-1.1.0` expose typed `from_slice` with `parse`/`serde`; that inspected release is not an automatic dependency upgrade. |
| `ignore::WalkBuilder` | Selected walker donor, absent from the lock; [0.4.33 API](https://docs.rs/ignore/0.4.33/ignore/struct.WalkBuilder.html) is a reference. | [Source reconcile](../../crates/search-source/search-source-reconcile/FUNCTIONS.md) must connect observation/acquisition; the present sync entrypoint still refuses with `SOURCE_ROOT_SYNC_EFFECTS_UNAVAILABLE`. |
| PDFium, tree-sitter, MCP SDK | `pdfium-render`, `tree-sitter` and `rmcp` are absent from the lock. | Optional candidates, not installed handlers. Select only against an actual document/outline/adapter operation; see [query seams](../../crates/search-query/README.md#current-owners-and-cutover). |

## Typed configuration donor boundaries

Official documentation checked 2026-10-23: high-level `toml` **1.1.8+spec-1.1.0**, TOML **1.1.0** and
Serde's current derive documentation. The repository still uses low-level `toml_parser 1.1.4+spec-1.1.0`;
`toml`, `serde` and `serde_json` are absent from the inspected lock. These checks select implementation
constraints, not installed versions or a completed #333 cutover.

| Verified donor behavior | Consequence for the concrete settings owner |
|---|---|
| [`toml::from_str`](https://docs.rs/toml/latest/toml/fn.from_str.html) is available with `parse` and `serde`; the [crate source](https://docs.rs/toml/latest/src/toml/lib.rs.html) exposes both string and byte decoding. | Pin compatible dependencies/features with consuming code; decode the bounded UTF-8 file directly into typed layers. |
| [Serde `deny_unknown_fields`](https://serde.rs/container-attrs.html) is a struct attribute and is incompatible with `flatten`; [defaults](https://serde.rs/field-attrs.html) fill missing fields during deserialization. | Close every nested struct. Layer defaults must preserve absence; populating application defaults in each layer would overwrite earlier values. |
| [TOML 1.1.0](https://toml.io/en/v1.1.0) uses Unicode strings/UTF-8 documents and rejects duplicate definitions. | Separate OS-valued path inputs from file text; no promise that a TOML string encodes arbitrary native path bytes. Keep syntax rejection distinct from effective-value validation. |
| [`toml::de::Error`](https://docs.rs/toml/latest/toml/de/struct.Error.html) exposes an optional source span; its display can include input context. | Handle absent spans and bounded safe diagnostics; do not dump config text or credentials through automatic error formatting. |

The [config owner](../../crates/search-config/FUNCTIONS.md#sparse-layers-and-validation) defines precedence,
whole-list replacement, omission/default behavior, validation, path bases and focused examples. These are
Eliot policy choices; the libraries do not select them. No parser fork, generic dotted-key database or second
settings registry is required. Adoption and execution remain #333 work.

## Rechecked retrieval assumptions

### Native BM25 and separate statistics scope

Qdrant's [native BM25 API](https://qdrant.tech/documentation/inference/inference-bm25/) accepts document/query
text. Reuse it instead of recreating numerical BM25 in Eliot. The bridge owns the differing document/query
options and the limits of generated-vector readback.

The official [scoped-statistics documentation](https://qdrant.tech/documentation/manage-data/multitenancy/#per-tenant-scoring-statistics)
states that `idf.corpus` is independent of retrieval filtering, available from 1.19.0 and shard-local.
It cannot by itself exclude a result. Index corpus-filter fields; strict mode can reject unindexed filters.
An empty statistics corpus does not fall back to global IDF: term rarity disappears; BM25 text queries also
lose length normalization. Therefore missing/incorrect corpus state must not be interpreted as known-empty
Search. The [bridge recipe](../../crates/search-index-qdrant/search-qdrant-bridge/FUNCTIONS.md#schema-scope-and-idf)
owns the two filters and explicit-empty handling.

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

The current [language-neutral API documentation](https://qdrant.tech/documentation/search/text-search/sparse-vector-search/)
uses `stemmer: {"type": "none"}` and an empty `stopwords: {}`; `language: "none"` is deprecated from 1.19.
Keep Eliot's selected tokenizer/normalization options consistent across ingestion and query. The documentation's
multilingual example is not an instruction to silently replace the published word-tokenizer profile.

### A chunk API is not an incremental reader

Eliot [`literal.rs`](../../crates/search-query/search-exact/src/literal.rs), rechecked at `5af8cde5`, carries
Aho state across UTF-8 chunks and preserves overlapping matches. However, `scan_chunks(&[&str], ...)` builds
the NFA inside every invocation, requires the chunks to be supplied already, and has no cancellation check
inside its byte loop. Calling it once per file is not compile-once streaming across a corpus.

The selected [Aho-Corasick automaton API](https://docs.rs/aho-corasick/1.1.5/aho_corasick/automaton/index.html)
already exposes state transitions. Reuse the NFA once per request, reset state per source and feed bounded
blocks with cancellation checks. Do not casually substitute a non-overlapping convenience iterator for the
existing overlapping contract. The [exact map](../../crates/search-query/search-exact/FUNCTIONS.md) also
restores the old S25 distinction between raw bytes and decoded/derived text.

## Markdown offset iterator: released fix, not yet adopted

Eliot's `markdown/parse.rs` uses `Parser::new_ext(..., Options::empty()).into_offset_iter()` and calls
`next()` synchronously. The printable input `"- [o]:u\n    \t"` from upstream
[#1129](https://github.com/pulldown-cmark/pulldown-cmark/issues/1129#issuecomment-5893660830)
passes the current raw path's control-character check; the original NUL-bearing reproducer does not.
There is no panic boundary around construction/iteration. This is source-level applicability to the locked
0.13.1 path, not a locally executed panic or an observed live-ingestion incident.

The upstream issue closed on 2026-10-19; [0.13.5](https://github.com/pulldown-cmark/pulldown-cmark/releases/tag/v0.13.5)
shipped on 2026-10-21 at `48a2767b5b8ab0a4893e80d942e510065889fc6b`.
The [release PR](https://github.com/pulldown-cmark/pulldown-cmark/pull/1181) includes the #1021 backport.
In its [firstpass.rs](https://github.com/pulldown-cmark/pulldown-cmark/blob/48a2767b5b8ab0a4893e80d942e510065889fc6b/pulldown-cmark/src/firstpass.rs#L423-L443),
`scan_next_line_or_lazy_continuation` now ends the paragraph when the remainder after container scanning is
blank. The corresponding [0.13.1 function](https://github.com/pulldown-cmark/pulldown-cmark/blob/v0.13.1/pulldown-cmark/src/firstpass.rs#L417-L437)
lacks that condition. This addresses the empty `TightParagraph` path described in #1129.

The old 0.13.4 plain-iterator fix could hide the panic by ending the event stream before closing tags; plain
iteration is not a coordinate-preserving workaround. The [0.13.4 → 0.13.5 diff](https://github.com/pulldown-cmark/pulldown-cmark/compare/v0.13.4...v0.13.5)
also changes other parsing/range behavior. A patch-release title or upstream test result does not establish
Eliot's offset/structure compatibility or CPU bound.

Use the maintained release through the [materializer correction](../../crates/search-prep/search-materializer/FUNCTIONS.md#markdown-donor-correction).
[#441](https://github.com/UnknownAlienHuman/eliot-search/issues/441) owns that implementation. Current Cargo pin,
lock and donor descriptor still say 0.13.1; no upgrade, reproducer or materializer/replay run occurred in R028.

## Selected mechanisms, not wholesale donor applications

| Need | Source retained from research | Applicability boundary |
|---|---|---|
| CLI | [ripgrep `4649aa9` main](https://github.com/BurntSushi/ripgrep/blob/4649aa9700619f94cf9c66876e9549d83420e16c/crates/core/main.rs) | Parse once and dispatch. Do not copy its whole application or build command fingerprints. |
| Inventory | [ignore walk `4649aa9`](https://github.com/BurntSushi/ripgrep/blob/4649aa9700619f94cf9c66876e9549d83420e16c/crates/ignore/src/walk.rs), [notify `a1d7c2d`](https://github.com/notify-rs/notify/blob/a1d7c2d8f80786679d58ec6d5986a1d4278bc8cf/notify/src/windows.rs) | Selected traversal donor and event-hint research, not an installed sync path. Neither proves a complete observation history. |
| Debounce | [notify-debouncer-full `76f48c7`](https://github.com/notify-rs/notify/blob/76f48c70bbf7fed75440d580e498e90cbc6ce140/notify-debouncer-full/src/lib.rs) | Burst/rename ideas only; its whole-tree FileIdMap is not the selected inventory. |
| Git | [Git `c44beea` cat-file](https://github.com/git/git/blob/c44beea485f0f2feaf460e2ac87fdd5608d63cf0/builtin/cat-file.c) | Exact OIDs and bounded raw plumbing. [gitoxide `ad0202e`](https://github.com/GitoxideLabs/gitoxide/blob/ad0202e3ab396d752a56664001c134f48d834177/gix-diff/src/tree/function.rs) remains an alternative, not a second mandatory backend. |
| Metadata snapshots | [redb 2.6.3](https://github.com/cberner/redb/blob/v2.6.3/src/transactions.rs) | Typed point/range reads and real read snapshots; no transaction across Qdrant/files, and old readers retain database pages. |
| Immutable files | [Eliot cutover publisher](https://github.com/UnknownAlienHuman/eliot-search/blob/c8e38b6fc9829f1cd38070773b1b3e5b21de18ad/crates/search-control-redb/src/migration/cutover_artifact.rs), [Git object-file `c44beea`](https://github.com/git/git/blob/c44beea485f0f2feaf460e2ac87fdd5608d63cf0/object-file.c) | Extract temp/no-clobber/final verification/cleanup. Neither imports migration, Git ODB or another writer. |
| Windows durability candidate | [MoveFileEx](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-movefileexa) | A candidate, not proof of required no-clobber/power-loss behavior. No-op directory sync plus readback is not power-loss evidence. |
| JSON/settings | [Serde](https://github.com/serde-rs/serde), [serde_json](https://github.com/serde-rs/json), [toml](https://github.com/toml-rs/toml) | Typed syntax and capped buffers are targets; these packages are absent from the inspected lock. Current custom TOML uses `toml_parser`. |
| Async timeouts | [Tokio 1.53.1 timeout](https://github.com/tokio-rs/tokio/blob/tokio-1.53.1/tokio/src/time/timeout.rs) | One attempt deadline; dropping a future does not cancel arbitrary blocking work or prove an external mutation stopped. |
| Indexed retrieval | [Qdrant client `7c83803`](https://github.com/qdrant/rust-client/blob/7c838035ae7b9455636dcaca918a55b7d7ca638f/src/qdrant_client/points.rs), Qdrant 1.19.0 sources above | One SDK boundary; native BM25, selected filtering and real effects. No numerical-correctness, latency or old-write-terminality claim from a feature name. |
| Structure | [Docling `f5f01f9`](https://github.com/docling-project/docling-core/blob/f5f01f97e9e1c7a50c1f378e114ca063085985ce/docling_core/transforms/chunker/hybrid_chunker.py), [Unstructured `1b02b4d`](https://github.com/Unstructured-IO/unstructured/blob/1b02b4d189c260ed9b1aca29583203e233d853cb/unstructured/chunking/title.py) | Section/split/merge behavior, not Python runtimes or proof of edit-stable occurrence identity. FastCDC remains only a measured oversized-plain-section candidate. |
| Source coordinates | Existing pulldown-cmark and quick-xml consumers | Retain actual parser ranges/maps; syntax observations do not prove call targets or behavior. |
| Old-view lifetime | [Lucene SnapshotDeletionPolicy `8a8ef48`](https://github.com/apache/lucene/blob/8a8ef488218405ee18ba9803c3ac1779ef74093a/lucene/core/src/java/org/apache/lucene/index/SnapshotDeletionPolicy.java) | Refcount idea only. Lucene protects immutable commits/files; this does not prove Qdrant mutable retrieval state is snapshotted. |
| Living-file ideas | [Tantivy ManagedDirectory `6788926`](https://github.com/quickwit-oss/tantivy/blob/67889269cae67063c372bc236b102d632a021568/src/directory/managed_directory.rs) | Reference selection and retryable failed deletes. Current Eliot local GC is stopped-root maintenance, not this donor's locking/registry design. |

Tokio details previously established in [R024](https://github.com/UnknownAlienHuman/eliot-search/blob/2d33dbdeefce0eb0c564f013a8e66032e73f6693/docs/audit/ANALYSIS_REGISTRY.md)
remain relevant: [`timeout`](https://docs.rs/tokio/1.53.1/tokio/time/fn.timeout.html) polls the future first,
so non-yielding work can exceed the deadline. Cancellation of `read_exact`/`write_all` can lose their internal
partial progress; a dropped wrapper does not make blocking stdin interruptible or settle a dispatched server
write. The [client](../../crates/search-provider-client/FUNCTIONS.md) and
[protocol](../../crates/search-provider-protocol/FUNCTIONS.md) own the concrete framing/cleanup decisions.

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

## Keep donor knowledge current

When a selected dependency, service version or consuming operation changes, check its official versioned API,
release notes and the source behavior on which that operation relies. Record the actual pin, changed behavior
and affected owner here, then correct that owner's recipe and code together. Keep earlier source links when
their limits still matter. A newer upstream release is an input to an upgrade decision, not permission to
rewrite every integration, add a runtime qualification gate or repeatedly research unrelated candidates.

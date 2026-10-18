# Donor and reuse index

**Refreshed:** 2026-10-18 against source `c8e38b6fc9829f1cd38070773b1b3e5b21de18ad`.
This index distinguishes existing mechanisms, actual replacement opportunities and unselected candidates.
It is static source research, not Rust/native/live-Qdrant qualification or a dependency upgrade proposal.
Use [root rules](../../AGENTS.md) and the [implementation guide](../implementation/README.md).

Use donor code to remove a mechanism, not to justify preserving every old Eliot abstraction. The
[current architecture](../architecture/README.md) retires baseline obligations that have no local product
need. Reuse existing owners before adding wrappers; keep exact source truth and actual I/O boundaries.

## 1. Existing mechanisms and actual reductions

| Mechanism / inspected source | Current use and remaining work |
|---|---|
| [BLAKE3 1.8.2](https://github.com/BLAKE3-team/BLAKE3/blob/df610ddc3b93841ffc59a87e3da659a15910eb46/src/lib.rs) | Already used by [shared digest owner](../../crates/search-contracts/src/digest.rs). First remove unnecessary plan/receipt hashes. Retained identity/integrity uses the real bounded owner, including #316's exact-stream consumer work; do not buffer whole large objects or reinterpret an existing digest as a different algorithm/profile. |
| [RustCrypto sha2 0.10.9](https://github.com/RustCrypto/hashes/blob/82c36a428f8d6f05f3bfccdedb243e9d1f85359d/sha2/src/lib.rs) | Already accepted by the same owner. Three copied `sha256.rs` files total 685 physical lines. #315 and separate document-worker/supervisor slices can replace algorithm execution, preserving SHA-256 RAW versus PARTS_V1 framing. The 685 lines are gross source inventory, not achieved net deletion. |
| [Qdrant Rust client 1.19.0](https://github.com/qdrant/rust-client/blob/7c838035ae7b9455636dcaca918a55b7d7ca638f/Cargo.toml) | Existing vendor client, not a new donor adoption. #262 removes copied public PointId/payload/schema/eligibility definitions in the bridge in favor of shared Eliot contracts; private vendor conversion and strict validation remain. #263 supplies actual effects. `qdrant/qdrant-client` is the Python repository; Rust source is `qdrant/rust-client`. |
| `unicode-ident =1.0.26`, `unicode-normalization =0.1.25` | Already used by [search-lexical](../../crates/search-lexical/Cargo.toml). XID18 and NFC17 profiles/range maps remain explicit. Do not plan deletion of a nonexistent custom XID/NFC engine. Normalization is not full case folding. |
| `pulldown-cmark =0.13.1`, `quick-xml =0.42.0` | Already used by [Markdown](../../crates/search-prep/search-materializer/src/markdown/parse.rs) and [JATS](../../crates/search-prep/search-materializer/src/jats/parse.rs). They supply parsing events; local raw-span, bounds, cancellation and DTD/PI rejection remain product duties. No duplicate CommonMark/XML engine was established. |
| `redb =2.6.3` | Already used by [search-control-redb](../../crates/search-control-redb/Cargo.toml). [Database API](https://github.com/cberner/redb/blob/v2.6.3/src/db.rs) supplies a single writer and read snapshots; [transactions](https://github.com/cberner/redb/blob/v2.6.3/src/transactions.rs) default to Immediate durability. Use typed current-record methods, not a second general journal/receipt platform. redb does not make files and Qdrant one transaction; retain actual pending publication, not every completed update. |
| Reusable provider client and native Windows owners | [Typed client](../../crates/search-provider-client/), [Windows connection](../../crates/search-runtime/search-provider-transport-windows/) and [purpose-bound credential adapter](../../bins/eliot-search/src/native_credentials.rs) exist. #332 connects actual commands over owned child stdio or an OS-owned pipe. The unconnected trusted-registration/listener path belongs to the optional restricted-client profile. [Current linkage/deletion map](../../bins/eliot-search/FUNCTIONS.md); reuse the engine and the smallest concrete transport that serves the local result. |
| S11 / indexed owner output | Existing planner/publication use S11. #262 consumes the planner's shared payload CBOR and separate vector evidence; old bridge revision/ordinal/digest fields are not rename-compatible S9.5 inputs. [Exact bridge disposition](../../crates/search-index-qdrant/search-qdrant-bridge/FUNCTIONS.md#shared-payload-and-identity-cutover). Preserve private vendor conversion. The [indexed donor map](INDEXED_SPINE_PORT_MAP_2026-10-09.md) prevents reimporting already delivered stacks. |

Exact workspace pins/features live in [Cargo.toml](../../Cargo.toml) and checksums in
[Cargo.lock](../../Cargo.lock). Existing crypto acceptance/provenance is retained in
[CRYPTO_DONOR_ACCEPTANCE](CRYPTO_DONOR_ACCEPTANCE_2026-10-09.md). The accepted BLAKE3 profile remains
`default-features=false`, `std,pure,zeroize`; upstream places `pure` among unstable/testing features, so
keep the exact pin. `sha2` remains default-features off; no second SHA implementation or compression API.
Do not add mmap/rayon/runtime file helpers to the shared authority package.

## 2. Architecture donors read in source

| Primary source inspected | Mechanism to use / Eliot consequence |
|---|---|
| [ripgrep 14.1.1 main](https://github.com/BurntSushi/ripgrep/blob/4649aa9700619f94cf9c66876e9549d83420e16c/crates/core/main.rs) | `flags::parse` produces one `HiArgs`, then `run` selects a concrete mode. In Eliot's [entry](../../bins/eliot-searchd/src/entry.rs), replace recapturing `maybe_run` chains and repeated config/argv fingerprints with one parsed invocation. Preserve `OsString` paths. This organisation needs no new CLI framework or dependency. |
| [Tantivy 0.22.0 basic search](https://github.com/quickwit-oss/tantivy/blob/17d5869ad61ea9f1072ef40546ddcada3a14b067/examples/basic_search.rs) | One writer/commit and a cheap per-query immutable searcher snapshot, followed by TopDocs and stored-document read. Transfer the concrete snapshot/readback shape; keep Qdrant as Eliot's backend. A tutorial does not solve Eliot's publication/recovery itself. |
| [Qdrant Rust client 1.19.0 points](https://github.com/qdrant/rust-client/blob/7c838035ae7b9455636dcaca918a55b7d7ca638f/src/qdrant_client/points.rs), [query](https://github.com/qdrant/rust-client/blob/7c838035ae7b9455636dcaca918a55b7d7ca638f/src/qdrant_client/query.rs) | Existing `upsert_points`, `get_points` and `query` plus Eliot's [reused blocking runtime](../../crates/search-index-qdrant/search-qdrant-bridge/src/real/blocking.rs). Produce bounded batches before calling the SDK: `upsert_points_chunked` receives the entire points vector and copies while splitting; it is not streaming ingestion. Remove duplicate bridge contracts/receipt ownership. The SDK does not replace source validation or publication; do not copy a tutorial's destructive collection reset. |
| [Serde 1.0.229](https://github.com/serde-rs/serde/blob/v1.0.229/README.md#serde-in-action), [serde_json 1.0.151 decoder](https://github.com/serde-rs/json/blob/v1.0.151/src/de.rs), [encoder](https://github.com/serde-rs/json/blob/v1.0.151/src/ser.rs) | Derives, `from_slice` and `to_writer` replace Eliot's [manual schema codec](../../crates/search-provider-protocol/src/frame/client/wire.rs), fixed-order cursors/escaping and [substring event parsing](../../bins/eliot-search/src/public_client.rs). Versions exist in Cargo.lock; these production codecs still need direct dependency integration. Bound the frame before parsing and bound output. `from_reader` waits for EOF, so it is unsuitable as-is on a persistent connection. No separate JSON grammar or frozen field order is required for ordinary records. |
| [toml 1.1.7 API](https://github.com/toml-rs/toml/blob/f72038c2508112dfaaedb227a511f91bf9b2015c/crates/toml/src/de/mod.rs), [parser](https://github.com/toml-rs/toml/blob/f72038c2508112dfaaedb227a511f91bf9b2015c/crates/toml/src/de/parser/mod.rs) | `from_slice` / `from_str` into typed Serde settings replace Search's config event receiver, repeated string decode and generic document machinery. `xtask` already locks this version with `std,serde,parse`; production currently uses only `toml_parser`. The donor allocates token/event vectors and a `DeTable` before typed deserialization; default recursion limit 80 remains enabled. Bound file bytes before parsing and validate real fields. No streaming or measured memory claim; [decision](../../crates/search-config/TOML_PROFILE_2.md). |
| [Rust environment](https://doc.rust-lang.org/std/env/fn.vars.html), [bounded reader](https://doc.rust-lang.org/std/io/trait.Read.html#method.take) | `vars_os` and a bounded read over one opened file are sufficient for daemon config capture. Current `vars()` can panic on unrelated non-Unicode data before prefix filtering; metadata followed by `fs::read` does not bound a file that grows meanwhile. OS-string paths and `take(limit+1)` need no new custody framework; byte limits do not establish I/O latency. |
| [Tokio 1.53.1 timeout source](https://github.com/tokio-rs/tokio/blob/tokio-1.53.1/tokio/src/time/timeout.rs) | `timeout_at` consumes an absolute instant. Convert one deadline at admission and propagate it across query/readback/pages. Current bridge `OpContext` stores a duration and restarts a timeout per RPC; more per-ABI counters do not fix that. |
| [Cloudflare Durable Objects](https://developers.cloudflare.com/durable-objects/best-practices/rules-of-durable-objects/), [Queues delivery](https://developers.cloudflare.com/queues/reference/delivery-guarantees/) | Design donor: one state owner, short local transactions and repeatable bounded background work. External I/O is not part of the local storage transaction; repeated deliveries need operation-specific handling. In Eliot this means redb current/pending records, a shared work queue and Qdrant publication, not a hosted Workers/DO/Queues deployment, SQLite backend or generic exactly-once engine. |

These are inspected mechanisms, not benchmark or runtime claims. Direct Serde/TOML production integration
is proposed; redb/Qdrant/Tokio already have production adapters. Existing [globset](../../crates/search-source/search-source-admission/src/policy_v2.rs)
and [aho-corasick](../../crates/search-query/search-exact/src/literal.rs) have real callers. Reuse them;
do not import donor applications, another backend or another layer around each API.

## 3. Git: replace a small parser; complete the missing backend

Use existing #335 for one real Git source path. The current [Git reader](../../crates/search-source/search-safe-reader/src/git.rs)
validates already-decompressed loose-object bytes from an abstract backend; no production ODB/pack/ref
reader calls it. A donor completes missing functionality, not deletion of a nonexistent pack engine.

**First implementation: controlled installed Git plumbing.** Resolve refs to exact commit/tree OIDs,
use `ls-tree -r -z --full-tree` initially and raw `diff-tree --no-renames` for changes, then one reused
`cat-file --batch-command --buffer -Z` child for bounded `info`/`contents` batches. This replaces the
old blanket external-Git prohibition. Direct argument arrays, raw builtins and disabled replacement/lazy
fetch modes preserve the selected view without checkout, textconv, filters or indexed-code execution.

[Git 2.51.0 cat-file source](https://github.com/git/git/blob/c44beea485f0f2feaf460e2ac87fdd5608d63cf0/builtin/cat-file.c)
streams raw blobs; its command queue until `flush` is not inherently bounded. Its
[tree diff](https://github.com/git/git/blob/c44beea485f0f2feaf460e2ac87fdd5608d63cf0/tree-diff.c) skips equal
OID/mode subtrees. Check type/size before content, bound batches/output/deadline and retain exact blobs
through the existing source writer. Git GC, missing objects, LFS pointers and conflict index stages do
not turn into current worktree evidence. Occurrence/path identity is separate from a blob OID.

**Alternative only if packaging requires no installed Git:**
[gix-odb](https://github.com/GitoxideLabs/gitoxide/blob/ad0202e3ab396d752a56664001c134f48d834177/gix-odb/src/lib.rs)
and direct `gix_diff::tree(TreeRefIter, TreeRefIter, State, objects, Visit)` at inspected revision
`ad0202e3ab396d752a56664001c134f48d834177`. Avoid high-level attribute/similarity processing for an OID
change list. `find_object` loads a body into a buffer; it is not a streaming/allocation bound. Select one
backend, not a permanent native/CLI dual implementation. The
[reader map](../../crates/search-source/search-safe-reader/FUNCTIONS.md) owns exact commands, source
binding and remaining limitations. No legacy decoder, format migration or extra Git authority is required.

## 4. Canonical bytes and Unicode decisions

The existing [closed canonical codec](../../crates/search-contracts/src/canonical.rs) owns bounded
JSON/CBOR and strict parse/re-encode equality. Its CBOR ordering is RFC 8949 **§4.2.3 length-first**, not
§4.2.1 core deterministic ordering. These are facts about the old bytes, not universal requirements for
new records. Ordinary typed JSON needs no canonical digest; actual content identity still hashes bytes.

Change new internal formats and their real ends together; rebuild derived generations. Reject unsupported
old roots before writes and leave their user data intact. An old reader, compatibility shim or export
implementation is not a prerequisite for the new application. Add one only for a separately selected
retained-data use; never silently reinterpret old identities or delete sole copies.

Ciborium 0.2.2 has not been selected as a production dependency. Its
[prior assessment](https://github.com/UnknownAlienHuman/eliot-search/blob/927265d1dd71f3d355131f38ac73365e6558a77e/docs/audit/DONOR_VERIFICATION_REGISTER_2026-10-09.md)
established that it was not a byte-compatible replacement for the old profile. That does not justify
handwritten codecs or block a simpler new format. No new canonical-codec package is needed.

[#253 / PR #322](UNICODE_CASEFOLD_DECISION_2026-10-09.md) already selected exact Unicode 18 C+F full
folding and goldens. Default folding excludes T; Turkic behavior is a distinct profile. No implicit extra
normalization; expanded scalars retain original source ranges. #254 remains the production consumer.
Do not reopen the completed decision, use floating UCD data, or substitute simple regex folding.

## 5. Further donor records: read the relevant section only

The [implementation requirement map](../implementation/README.md#preserve-the-useful-requirements)
connects old product requirements to these concrete choices. New donor integrations below are planned;
Qdrant capabilities are present in the pinned server/SDK but not connected to Eliot's normal query.

### Native lexical search and scope

| Requirement / source | Use and remove |
|---|---|
| Ordinary lexical search; [Qdrant 1.19.0 local model](https://github.com/qdrant/qdrant/blob/74f3e85b9473c62560006c043e13737ce6b48412/src/common/inference/local_model.rs), [service](https://github.com/qdrant/qdrant/blob/74f3e85b9473c62560006c043e13737ce6b48412/src/common/inference/service.rs) | `qdrant/bm25` computes locally; absent remote inference address is permitted. SDK `DocumentBuilder` supplies bounded text/options for named upsert and nearest query. Remove the generic local sparse hashing/BM25/frozen-corpus/profile-proof path; no cloud, ONNX or model worker is needed for this mode. |
| Text/code tokenization; [BM25 adapter](https://github.com/qdrant/qdrant/blob/74f3e85b9473c62560006c043e13737ce6b48412/src/common/inference/bm25_inference.rs) | Configure ingest/query identically. Defaults include English language processing. Neutral/code uses explicit no stemmer and empty stopwords; word tokenizer does not split camel case. A small identifier-term expansion plus whitespace tokenization can retain code names/components. Keep exact source matching separately. |
| Corpus-restricted IDF; [Qdrant documentation](https://qdrant.tech/documentation/manage-data/multitenancy/#per-tenant-idf-statistics) | Supply `params.idf.corpus` explicitly; retrieval filter alone is insufficient. Existing Eliot `ScopedToRetrieval` already sends it. Statistics are shard-local physical sparse-vector points, not distinct files. One compatible population becomes one query/filter; remove per-membership legs and `OverlapFreeRouteProof`, not filtered IDF. |
| Bounded ranking/groups; [Query API](https://qdrant.tech/documentation/concepts/hybrid-queries/) | Reuse prefetch, RRF and native groups for indexed legs. Retain a small local merge only for real direct/overlay inputs. Native RRF formula/tie behavior differs from Eliot's old arithmetic; accept the new ranking, preserve useful result ordering and verify relevance. Grouping does not undo duplicate point contributions to IDF. |

The planner currently requires prepared vector values/digests. Native `Document` input therefore needs a
coherent preparation/planner/bridge change, not another uncalled SDK helper. Do not calculate a second
BM25 vector merely to satisfy the old digest contract. Read actual generated vector/point state where
publication needs it. Corpus filtering scans selected points and query-term postings; it is not a free
statistics lookup. Keep one shard initially, and do not create a collection per repository or an extra
physical projection per overlapping portfolio. See [lexical](../../crates/search-lexical/FUNCTIONS.md)
and [bridge](../../crates/search-index-qdrant/search-qdrant-bridge/FUNCTIONS.md) for exact APIs and limits.

### Working-tree discovery

Use [ripgrep/ignore WalkBuilder](https://github.com/BurntSushi/ripgrep/blob/4649aa9700619f94cf9c66876e9549d83420e16c/crates/ignore/src/walk.rs)
for explicit nested ignore rules and streaming entries/errors, plus one
[notify 8.2.0 watcher](https://github.com/notify-rs/notify/blob/a1d7c2d8f80786679d58ec6d5986a1d4278bc8cf/notify/src/windows.rs)
for hints. One queue handles changed paths and startup/resume/periodic scans. `PollWatcher` already
rescans, so adding it beside that scanner doubles work. Watcher silence is not content continuity;
incomplete scans cannot retire unseen files or clear changes arriving during the scan. Current root
refresh only probes availability. [Reconciliation implementation](../../crates/search-source/search-source-reconcile/FUNCTIONS.md)
replaces the old imaginary watcher/inventory APIs and lifetime ledger.

### Code navigation and comparison

| Useful depth | Donor and boundary |
|---|---|
| Names, signatures and file outline | [ast-grep outline 0.44.0](https://ast-grep.github.io/reference/cli/outline), an [alpha feature](https://ast-grep.github.io/guide/outline-code). One bounded explicit immutable-file batch produces JSON file records and byte ranges. Its channel is unbounded, file size is checked after reading and per-file failures may be skipped; Eliot bounds inputs/output and accounts for missing files. Replace the line/brace Rust scanner, not build another parser framework. |
| In-process syntax when required | [Tree-sitter parser/tags](https://tree-sitter.github.io/tree-sitter/4-code-navigation.html) can replace the outline process with selected grammars and byte-range captures. Choose one syntax owner. No compiler/name resolution or second graph database follows from a parse tree. |
| Actual definitions/references | Import [SCIP](https://github.com/scip-code/scip/blob/fcb3cb3c679fe2da18141550f864b9e67afab297/scip.proto) or a selected LSP session's locations, tied to exact input snapshot/buffer version and position encoding. The inspected [rust-analyzer SCIP writer](https://github.com/rust-lang/rust-analyzer/blob/91a3d3b547342c525a1a7f4d0983e6c4f1872d3a/crates/rust-analyzer/src/cli/scip.rs) emits occurrences/signatures but sets relationships empty and omits document text. The schema's implementation relation does not mean that this producer supplies it. |
| Compare relevant implementations | Reuse lexical/name/structural candidates and validated source excerpts. The local comparator consumes supplied observations/digests; it does not derive behavior. Different digests do not establish a behavioral conflict. Show source-backed differences and unknowns, and group only known repository lineage. |
| Exact absence/completeness | Reuse existing Aho-Corasick/regex source matching through one bounded retained-inventory scan. `proof.rs` duplicates literal matching and accepts a prebuilt readback vector; replace those mechanisms with the actual matcher/reader path. A rank result, syntax outline or missing SCIP occurrence is not an exhaustive semantic proof. |

[Code enrichment](../../crates/search-prep/search-code-enricher/FUNCTIONS.md),
[subject resolution](../../crates/search-query/search-subject-resolver/README.md),
[comparison](../../crates/search-query/search-comparator/README.md) and
[exact scanning](../../crates/search-query/search-exact/FUNCTIONS.md) name real code/consumers.
Indexing with rust-analyzer may execute Cargo build scripts/proc macros; it is an explicitly selected
preparation action, never an invisible ordinary query. Reuse available revision-bound artifacts first.

### Documents

First implement native PDF text using
[pdfium-render 0.9.4](https://github.com/ajrcarey/pdfium-render/blob/ea2961cb8ac777bb1a06dddb8efd272d34d74f18/README.md)
in the existing doc worker, with one selected matching PDFium library. Disable unused rendering/image
features; return page text/ranges and gaps. PDFium does not guarantee human reading order in every layout
and serializes calls, so more Rust threads do not make that parser parallel. This adds a native library;
no runtime/Windows result is claimed by the source inspection.

[Xberg](https://github.com/xberg-io/xberg/blob/28c5311862e6f3466b4e762bf73a2ffe4b318dfa/crates/xberg/Cargo.toml)
is an alternative for selected multi-format support. Its current `pdf` selects its native engine, while
`pdf-pdfium` is separate; do not assume Kreuzberg v4 examples describe current Xberg. Import the selected
extractor, not its retrieval/crawler/agent stack. [Docling native pipeline](https://docling-project.github.io/docling/usage/advanced_options/#extract-the-native-content-of-a-pdf)
also runs without OCR/layout/table models, but keeps its Python/parser runtime and parser ordering.
GROBID supplies selected scientific TEI/coordinates; OCR adds recognition for pages lacking usable text.
OCRmyPDF sidecar omits already-text and skipped pages. None of these outputs supplies raw binary-PDF
byte offsets automatically. The [document implementation](../../bins/eliot-search-doc-worker/FUNCTIONS.md)
contains concrete APIs and sources. A real searchable PDF replaces #227's generic-runtime-only ladder.

### Other existing records

TOML/glob/matcher pins remain in [WAVE2_DONOR_ACCEPTANCE](WAVE2_DONOR_ACCEPTANCE_2026-10-09.md);
reuse actual Cargo metadata instead of the old partial Rust analyser. Historical indexed source remains
in the [indexed donor map](INDEXED_SPINE_PORT_MAP_2026-10-09.md). Old “next”/`DECISION_REQUIRED`/coding-base
statements are dated evidence, not a mandatory queue. Keep new implementation decisions here or in the
existing owner/issue; do not create another requirement registry.

## 6. Adoption without another process catalogue

Use the existing issue/PR to name the selected version/API/features, what code it replaces and any real
format or runtime limitation. Review new dependency/license/target changes when they occur; reuse the
existing accepted dependency evidence otherwise. Do not require another ecosystem audit, donor gate or
transitive-symbol dossier merely to use a library already present. The result is a connected caller and
a removed old mechanism, verified for its actual behavior.
